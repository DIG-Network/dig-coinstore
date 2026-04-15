//! RocksDB storage backend implementation.
//!
//! Implements [`StorageBackend`] using RocksDB with **one column family per logical store** from
//! [`super::schema::ALL_COLUMN_FAMILIES`]. This matches **STO-002** ([`STO-002.md`](../../docs/requirements/domains/storage/specs/STO-002.md)):
//! twelve CFs, per-CF write-buffer sizing, bloom / prefix tuning (coordinated with [`STO-004`](../../docs/requirements/domains/storage/specs/STO-004.md)),
//! FIFO compaction for append-only snapshots vs Level for the rest ([`STO-006`](../../docs/requirements/domains/storage/specs/STO-006.md)),
//! and database-wide durability / parallelism knobs from the same spec.
//!
//! # Design notes
//!
//! - **API-003 `bloom_filter`:** When `false`, block-based blooms are disabled on every CF (cheap CI / constrained hosts).
//!   Prefix extractors for puzzle-hash style CFs stay enabled so [`StorageBackend::prefix_scan`] keeps correct
//!   iterator semantics even without blooms.
//! - **Per-CF `Options`:** Each [`ColumnFamilyDescriptor`] carries its own memtable + table factory settings; the DB
//!   `Options` carry global flags (`create_if_missing`, WAL limits, etc.).
//!
//! # Requirements: STR-003, STO-002, STO-005
//! # Spec: docs/requirements/domains/storage/specs/STO-002.md
//! # SPEC.md: Section 7.2 (RocksDB Column Families)

use std::path::Path;
use std::sync::Arc;

use rocksdb::{
    BlockBasedOptions, ColumnFamilyDescriptor, DBCompactionStyle, FifoCompactOptions, IteratorMode,
    Options, SliceTransform, WriteBatch as RocksWriteBatch, DB,
};

use crate::config::{CoinStoreConfig, BLOOM_FILTER_BITS_PER_KEY};

use super::schema::ALL_COLUMN_FAMILIES;
use super::schema::{
    CF_ARCHIVE_COIN_RECORDS, CF_COIN_BY_CONFIRMED_HEIGHT, CF_COIN_BY_PARENT,
    CF_COIN_BY_PUZZLE_HASH, CF_COIN_BY_SPENT_HEIGHT, CF_COIN_RECORDS, CF_HINTS, CF_HINTS_BY_VALUE,
    CF_MERKLE_NODES, CF_METADATA, CF_STATE_SNAPSHOTS, CF_UNSPENT_BY_PUZZLE_HASH,
};
use super::{StorageBackend, StorageError, WriteBatch, WriteOp};

/// RocksDB-backed storage for dig-coinstore.
///
/// Wraps a `rocksdb::DB` instance with pre-created column families matching
/// the schema defined in [`super::schema`]. Thread-safe via `Arc<DB>` —
/// RocksDB handles internal locking for concurrent reads and writes.
///
/// # Column families
///
/// All 12 column families from SPEC Section 7.2 are created on open:
/// `coin_records`, `coin_by_puzzle_hash`, `unspent_by_puzzle_hash`,
/// `coin_by_parent`, `coin_by_confirmed_height`, `coin_by_spent_height`,
/// `hints`, `hints_by_value`, `merkle_nodes`, `archive_coin_records`,
/// `state_snapshots`, `metadata`.
pub struct RocksDbBackend {
    db: Arc<DB>,
}

/// Database-wide [`Options`] shared by [`RocksDbBackend::open`] and [`RocksDbBackend::list_column_families`].
///
/// **STO-002** § Initialization + § Global RocksDB Options: create flags, `keep_log_file_num(10)`,
/// `increase_parallelism(4)` (maps to background jobs), periodic `bytes_per_sync` / `wal_bytes_per_sync`,
/// and `max_total_wal_size` cap. Write-buffer sizing comes from [`CoinStoreConfig`].
///
/// # `max_open_files` vs FIFO (`state_snapshots`)
///
/// [`column_family_descriptor`] sets **FIFO** compaction on [`CF_STATE_SNAPSHOTS`](super::schema::CF_STATE_SNAPSHOTS). RocksDB rejects
/// open unless **`max_open_files == -1`** (“FIFO compaction only supported with max_open_files = -1”).
/// Therefore this function **always** sets `max_open_files(-1)` and does **not** forward
/// [`CoinStoreConfig::rocksdb_max_open_files`] (that knob still exists on the config struct for
/// API-003 parity and for any future non-FIFO layout; see field docs in [`crate::config::CoinStoreConfig`]).
///
/// # Requirement: STO-002
fn db_options_for_open(config: &CoinStoreConfig) -> Options {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_write_buffer_size(config.rocksdb_write_buffer_size);
    opts.set_max_open_files(-1);
    opts.set_keep_log_file_num(10);
    opts.increase_parallelism(4);
    opts.set_bytes_per_sync(1 << 20);
    opts.set_wal_bytes_per_sync(1 << 20);
    opts.set_max_total_wal_size(256 * 1024 * 1024);

    let mut block_opts = BlockBasedOptions::default();
    if config.bloom_filter {
        block_opts.set_bloom_filter(f64::from(BLOOM_FILTER_BITS_PER_KEY), false);
    }
    opts.set_block_based_table_factory(&block_opts);
    opts
}

/// Per-CF memtable budget from **STO-002** § Per-CF Configuration Summary (MiB → bytes).
fn cf_write_buffer_bytes(cf: &str) -> usize {
    const MIB: usize = 1024 * 1024;
    match cf {
        CF_COIN_RECORDS | CF_MERKLE_NODES => 64 * MIB,
        CF_COIN_BY_PUZZLE_HASH | CF_UNSPENT_BY_PUZZLE_HASH => 32 * MIB,
        CF_STATE_SNAPSHOTS => 8 * MIB,
        CF_METADATA => 4 * MIB,
        CF_COIN_BY_PARENT
        | CF_COIN_BY_CONFIRMED_HEIGHT
        | CF_COIN_BY_SPENT_HEIGHT
        | CF_HINTS
        | CF_HINTS_BY_VALUE
        | CF_ARCHIVE_COIN_RECORDS => 16 * MIB,
        // Defensive: callers outside `ALL_COLUMN_FAMILIES` should not reach here.
        _ => 16 * MIB,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BloomProfile {
    /// Block-based bloom (~10 bits/key when enabled via [`CoinStoreConfig::bloom_filter`]).
    Full,
    /// Same bloom bits + fixed 32-byte prefix extractor (puzzle hash / hint leading component).
    Prefix32,
    /// No block bloom (sequential / append-heavy CFs per STO-002 table).
    None,
}

fn cf_bloom_profile(cf: &str) -> BloomProfile {
    match cf {
        CF_COIN_BY_PUZZLE_HASH | CF_UNSPENT_BY_PUZZLE_HASH | CF_HINTS_BY_VALUE => {
            BloomProfile::Prefix32
        }
        CF_COIN_BY_CONFIRMED_HEIGHT | CF_COIN_BY_SPENT_HEIGHT | CF_STATE_SNAPSHOTS => {
            BloomProfile::None
        }
        CF_COIN_RECORDS
        | CF_COIN_BY_PARENT
        | CF_HINTS
        | CF_MERKLE_NODES
        | CF_ARCHIVE_COIN_RECORDS
        | CF_METADATA => BloomProfile::Full,
        _ => BloomProfile::Full,
    }
}

fn cf_uses_fixed_prefix_32(cf: &str) -> bool {
    matches!(
        cf,
        CF_COIN_BY_PUZZLE_HASH | CF_UNSPENT_BY_PUZZLE_HASH | CF_HINTS_BY_VALUE
    )
}

/// Build a [`ColumnFamilyDescriptor`] for one logical store.
///
/// Applies write-buffer sizing, bloom / prefix table options, and compaction style per **STO-002** tables.
/// `state_snapshots` uses **FIFO** compaction (checkpoint append pattern); all other CFs use **Level**.
fn column_family_descriptor(cf: &str, config: &CoinStoreConfig) -> ColumnFamilyDescriptor {
    let mut o = Options::default();
    o.set_write_buffer_size(cf_write_buffer_bytes(cf));

    if cf_uses_fixed_prefix_32(cf) {
        o.set_prefix_extractor(SliceTransform::create_fixed_prefix(32));
    }

    let mut block = BlockBasedOptions::default();
    if config.bloom_filter {
        match cf_bloom_profile(cf) {
            BloomProfile::None => {}
            BloomProfile::Full | BloomProfile::Prefix32 => {
                block.set_bloom_filter(f64::from(BLOOM_FILTER_BITS_PER_KEY), false);
            }
        }
    }
    o.set_block_based_table_factory(&block);

    if cf == CF_STATE_SNAPSHOTS {
        let mut fifo = FifoCompactOptions::default();
        // 1 GiB total file size before dropping oldest SSTs — generous for checkpoints until PRF-008 tuning lands.
        fifo.set_max_table_files_size(1024 * 1024 * 1024);
        o.set_fifo_compaction_options(&fifo);
        o.set_compaction_style(DBCompactionStyle::Fifo);
    } else {
        o.set_compaction_style(DBCompactionStyle::Level);
    }

    ColumnFamilyDescriptor::new(cf, o)
}

impl RocksDbBackend {
    /// Open (or create) a RocksDB database using [`CoinStoreConfig`] tuning.
    ///
    /// **STO-002:** Creates every CF in [`ALL_COLUMN_FAMILIES`] with `create_missing_column_families(true)` so
    /// schema evolution can add new named stores on reopen without a manual migration tool.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::BackendError` if the database cannot be opened
    /// (e.g., path doesn't exist and can't be created, lock file conflict).
    pub fn open(config: &CoinStoreConfig) -> Result<Self, StorageError> {
        let path: &Path = config.storage_path.as_path();
        let opts = db_options_for_open(config);
        let cf_descriptors: Vec<ColumnFamilyDescriptor> = ALL_COLUMN_FAMILIES
            .iter()
            .map(|name| column_family_descriptor(name, config))
            .collect();

        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)
            .map_err(|e| StorageError::BackendError(format!("Failed to open RocksDB: {}", e)))?;

        Ok(Self { db: Arc::new(db) })
    }

    /// List column family names on disk at [`CoinStoreConfig::storage_path`].
    ///
    /// Uses the **same** [`db_options_for_open`] profile as [`Self::open`] so `rocksdb::DB::list_cf` agrees with
    /// open descriptors. Exposed for **STO-002** integration tests (`tests/sto_002_tests.rs`) and diagnostics.
    ///
    /// **Note:** RocksDB may include a `default` CF; callers typically filter it when comparing to [`ALL_COLUMN_FAMILIES`].
    pub fn list_column_families(config: &CoinStoreConfig) -> Result<Vec<String>, StorageError> {
        let opts = db_options_for_open(config);
        DB::list_cf(&opts, config.storage_path.as_path()).map_err(|e| {
            StorageError::BackendError(format!("Failed to list RocksDB column families: {}", e))
        })
    }

    /// Get a reference to the column family handle, or return an error if unknown.
    fn cf_handle(&self, cf: &str) -> Result<&rocksdb::ColumnFamily, StorageError> {
        self.db
            .cf_handle(cf)
            .ok_or_else(|| StorageError::UnknownColumnFamily(cf.to_string()))
    }
}

impl StorageBackend for RocksDbBackend {
    fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let cf_handle = self.cf_handle(cf)?;
        self.db
            .get_cf(cf_handle, key)
            .map_err(|e| StorageError::BackendError(format!("RocksDB get error: {}", e)))
    }

    fn put(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let cf_handle = self.cf_handle(cf)?;
        self.db
            .put_cf(cf_handle, key, value)
            .map_err(|e| StorageError::BackendError(format!("RocksDB put error: {}", e)))
    }

    fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StorageError> {
        let cf_handle = self.cf_handle(cf)?;
        self.db
            .delete_cf(cf_handle, key)
            .map_err(|e| StorageError::BackendError(format!("RocksDB delete error: {}", e)))
    }

    /// Atomically apply all operations in the batch.
    ///
    /// Uses RocksDB's native `WriteBatch` for a single WAL fsync.
    /// This is the primary performance optimization for block application
    /// (STO-005): a block with 1000 coins generates ~5000 write ops, all
    /// committed in one atomic batch instead of individual puts.
    fn batch_write(&self, batch: WriteBatch) -> Result<(), StorageError> {
        if batch.is_empty() {
            return Ok(());
        }

        let mut rocks_batch = RocksWriteBatch::default();

        for op in &batch.ops {
            match op {
                WriteOp::Put { cf, key, value } => {
                    let cf_handle = self.cf_handle(cf)?;
                    rocks_batch.put_cf(cf_handle, key, value);
                }
                WriteOp::Delete { cf, key } => {
                    let cf_handle = self.cf_handle(cf)?;
                    rocks_batch.delete_cf(cf_handle, key);
                }
            }
        }

        self.db
            .write(rocks_batch)
            .map_err(|e| StorageError::BackendError(format!("RocksDB batch write error: {}", e)))
    }

    /// Prefix scan: return all KV pairs where the key starts with `prefix`.
    ///
    /// Uses RocksDB's prefix iterator. Results are in key order.
    /// The scan terminates when keys no longer match the prefix.
    fn prefix_scan(&self, cf: &str, prefix: &[u8]) -> Result<Vec<super::KvPair>, StorageError> {
        let cf_handle = self.cf_handle(cf)?;
        let iter = self.db.iterator_cf(
            cf_handle,
            IteratorMode::From(prefix, rocksdb::Direction::Forward),
        );

        let mut results = Vec::new();
        for item in iter {
            let (key, value) =
                item.map_err(|e| StorageError::BackendError(format!("Iterator error: {}", e)))?;

            // Stop when keys no longer start with the prefix.
            if !key.starts_with(prefix) {
                break;
            }

            results.push((key.to_vec(), value.to_vec()));
        }

        Ok(results)
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.db
            .flush()
            .map_err(|e| StorageError::BackendError(format!("RocksDB flush error: {}", e)))
    }

    fn compact(&self, cf: &str) -> Result<(), StorageError> {
        let cf_handle = self.cf_handle(cf)?;
        self.db
            .compact_range_cf(cf_handle, None::<&[u8]>, None::<&[u8]>);
        Ok(())
    }
}
