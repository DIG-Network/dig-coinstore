//! RocksDB storage backend implementation.
//!
//! **Verification:** [`tests/sto_002_tests.rs`](../../tests/sto_002_tests.rs) (twelve CFs, isolation, reopen,
//! `dyn` [`StorageBackend`], schema-evolution reopen, write-buffer table alignment).
//! **STO-004 (bloom / prefix / L0 pin):** [`tests/sto_004_tests.rs`](../../tests/sto_004_tests.rs) exercises
//! [`sto004_bloom_plan_for_column_family`] (matrix) plus an open smoke check.
//! **STO-005 (WriteBatch + WAL durability):** [`tests/sto_005_tests.rs`](../../tests/sto_005_tests.rs) — atomic
//! cross-CF batches, failure rollback, `write_opt` with `sync=true` (see [`sto005_batch_write_options`]).
//! **STO-006 (compaction + memtable depth):** [`tests/sto_006_tests.rs`](../../tests/sto_006_tests.rs) — Level vs FIFO
//! matrix, normative Level tuning constants, FIFO byte cap, `max_write_buffer_number`, manual `compact`.
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
//! # Requirements: STR-003, STO-002, STO-004, STO-005, STO-006
//! # Spec: docs/requirements/domains/storage/specs/STO-002.md
//! # SPEC.md: Section 7.2 (RocksDB Column Families)

use std::path::Path;
use std::sync::Arc;

use rocksdb::{
    BlockBasedOptions, ColumnFamilyDescriptor, DBCompactionStyle, FifoCompactOptions, IteratorMode,
    Options, SliceTransform, WriteBatch as RocksWriteBatch, WriteOptions, DB,
};

use crate::config::{CoinStoreConfig, BLOOM_FILTER_BITS_PER_KEY};

/// Memtable prefix-bloom size ratio for puzzle-hash style column families (**STO-004** SHOULD).
///
/// Passed to [`Options::set_memtable_prefix_bloom_ratio`] together with a fixed 32-byte
/// [`rocksdb::SliceTransform::create_fixed_prefix`] so prefix seeks benefit before SST flush.
/// RocksDB caps the effective bloom bytes at 0.25 × write-buffer internally.
///
/// # Spec
/// [`STO-004.md`](../../docs/requirements/domains/storage/specs/STO-004.md) § Prefix bloom filters.
pub const STO004_MEMTABLE_PREFIX_BLOOM_RATIO: f64 = 0.1;

use super::schema::ALL_COLUMN_FAMILIES;
use super::schema::{
    CF_ARCHIVE_COIN_RECORDS, CF_COIN_BY_CONFIRMED_HEIGHT, CF_COIN_BY_PARENT,
    CF_COIN_BY_PUZZLE_HASH, CF_COIN_BY_SPENT_HEIGHT, CF_COIN_RECORDS, CF_HINTS, CF_HINTS_BY_VALUE,
    CF_MERKLE_NODES, CF_METADATA, CF_STATE_SNAPSHOTS, CF_UNSPENT_BY_PUZZLE_HASH,
    STO002_ROCKS_WRITE_BUFFER_BYTES, STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER,
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
/// # `max_open_files` vs FIFO (`archive_coin_records`, `state_snapshots`)
///
/// [`column_family_descriptor`] sets **FIFO** compaction on [`CF_ARCHIVE_COIN_RECORDS`](super::schema::CF_ARCHIVE_COIN_RECORDS)
/// and [`CF_STATE_SNAPSHOTS`](super::schema::CF_STATE_SNAPSHOTS) (**STO-006**). RocksDB rejects open unless
/// **`max_open_files == -1`** (“FIFO compaction only supported with max_open_files = -1”).
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

    // **STO-004 / diagnostics:** ticker-based statistics (e.g. bloom usefulness) are optional signals for
    // integration tests and operators. Cost is a small amount of CPU; acceptable for coinstore workloads.
    opts.enable_statistics();

    // SST blooms and table factories are configured per column family in [`column_family_descriptor`];
    // the implicit `default` CF keeps RocksDB defaults.
    opts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BloomProfile {
    /// Point-lookup CFs: classic SST bloom via [`BlockBasedOptions::set_bloom_filter`] with
    /// `block_based = false` → `rocksdb_filterpolicy_create_bloom_full` (**STO-004** full bloom, 10 bits/key).
    Full,
    /// Puzzle-hash / hint-prefix CFs: **same** classic full-key bloom (`block_based = false`) as point lookups,
    /// plus fixed 32-byte prefix extractor + [`STO004_MEMTABLE_PREFIX_BLOOM_RATIO`] on the `Options` object.
    /// Prefix iteration uses the extractor; the SST filter still uses the full-key policy bit layout RocksDB
    /// documents for prefix-enabled tables ([`STO-004.md`](../../docs/requirements/domains/storage/specs/STO-004.md)).
    Prefix32,
    /// Sequential / range-heavy CFs: **no** SST bloom (STO-004 “None” row).
    None,
}

/// RocksDB bloom-related knobs for one logical column family (**STO-004**).
///
/// This is the **authoritative plan** applied by [`column_family_descriptor`] and asserted by
/// [`tests/sto_004_tests.rs`](../../tests/sto_004_tests.rs). Keeping it in one place avoids drift between
/// production `Options`/`BlockBasedOptions` wiring and verification tables.
///
/// # Field semantics (rust-rocksdb)
///
/// - [`BlockBasedOptions::set_bloom_filter`]: `sst_bloom_uses_block_based_builder == false` selects the
///   **full-key** bloom implementation (`rocksdb_filterpolicy_create_bloom_full`), matching the normative
///   STO-004 snippets (both point-lookup and prefix rows use `10, false`).
/// - [`BlockBasedOptions::set_pin_l0_filter_and_index_blocks_in_cache`]: enabled whenever an SST bloom is
///   configured, per STO-004 “Pin L0” column.
/// - [`Options::set_memtable_prefix_bloom_ratio`]: set only for [`BloomProfile::Prefix32`] when global
///   [`CoinStoreConfig::bloom_filter`] is on (STO-004 SHOULD).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sto004BloomPlan {
    /// SST bloom bits/key when enabled; `None` ⇒ do not call [`BlockBasedOptions::set_bloom_filter`].
    pub sst_bloom_bits_per_key: Option<i32>,
    /// Second argument to [`BlockBasedOptions::set_bloom_filter`] (full-key vs block-based builder).
    pub sst_bloom_uses_block_based_builder: bool,
    /// Whether L0 filter + index blocks stay resident in block cache (STO-004).
    pub pin_l0_filter_and_index_in_cache: bool,
    /// Memtable prefix bloom ratio for prefix-style CFs; `None` ⇒ leave RocksDB default (effectively off).
    pub memtable_prefix_bloom_ratio: Option<f64>,
}

/// Compute the **STO-004** bloom / memtable / L0-pin plan for `cf` under `config`.
///
/// [`column_family_descriptor`] must stay in lockstep with this function — if you add a CF or change a
/// profile, update **both** and extend [`tests/sto_004_tests.rs`](../../tests/sto_004_tests.rs).
///
/// When [`CoinStoreConfig::bloom_filter`] is `false` (**API-003** fast CI path), SST blooms, L0 pinning, and
/// memtable prefix blooms are all suppressed; fixed prefix extractors for puzzle-hash CFs remain enabled so
/// [`StorageBackend::prefix_scan`] semantics stay correct.
pub fn sto004_bloom_plan_for_column_family(cf: &str, config: &CoinStoreConfig) -> Sto004BloomPlan {
    if !config.bloom_filter {
        return Sto004BloomPlan {
            sst_bloom_bits_per_key: None,
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: false,
            memtable_prefix_bloom_ratio: None,
        };
    }

    match cf_bloom_profile(cf) {
        BloomProfile::None => Sto004BloomPlan {
            sst_bloom_bits_per_key: None,
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: false,
            memtable_prefix_bloom_ratio: None,
        },
        BloomProfile::Full => Sto004BloomPlan {
            sst_bloom_bits_per_key: Some(BLOOM_FILTER_BITS_PER_KEY),
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: true,
            memtable_prefix_bloom_ratio: None,
        },
        BloomProfile::Prefix32 => Sto004BloomPlan {
            sst_bloom_bits_per_key: Some(BLOOM_FILTER_BITS_PER_KEY),
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: true,
            memtable_prefix_bloom_ratio: Some(STO004_MEMTABLE_PREFIX_BLOOM_RATIO),
        },
    }
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

// ─────────────────────────────────────────────────────────────────────────────
// STO-006 — compaction style + Level tuning + FIFO byte cap + memtable depth
// ─────────────────────────────────────────────────────────────────────────────
// Normative tables: `docs/requirements/domains/storage/specs/STO-006.md`. The helpers below are the
// **single source of truth** asserted by `tests/sto_006_tests.rs` and applied from [`column_family_descriptor`]
// so production wiring cannot drift from the documented matrix without failing CI.

/// On-disk byte ceiling for FIFO SSTs before RocksDB evicts oldest files (**STO-006** FIFO table).
///
/// Checkpoint / archive CFs are append-mostly; a 1 GiB cap matches the prior `state_snapshots`-only default and
/// leaves headroom until PRF-008 adds operator-tunable caps.
pub const STO006_FIFO_MAX_TABLE_FILES_SIZE: u64 = 1024 * 1024 * 1024;

/// Which compaction style **STO-006** assigns to a logical column family name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sto006CompactionStyle {
    /// Leveled compaction with [`Sto006LevelCompactionParams::NORMATIVE`] knobs.
    Level,
    /// FIFO compaction with [`STO006_FIFO_MAX_TABLE_FILES_SIZE`] (append-mostly stores).
    Fifo,
}

/// Leveled-compaction tuning shared by every non-FIFO CF (**STO-006** “Level Compaction Configuration”).
///
/// These numbers are **normative** (not RocksDB defaults). [`sto006_apply_level_compaction_options`] applies them
/// verbatim so operators and tests can reason about read amplification vs write stalls from one struct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sto006LevelCompactionParams {
    pub max_bytes_for_level_base: u64,
    pub max_bytes_for_level_multiplier: f64,
    pub level0_file_num_compaction_trigger: i32,
    pub level0_slowdown_writes_trigger: i32,
    pub level0_stop_writes_trigger: i32,
    pub target_file_size_base: u64,
    pub num_levels: i32,
}

impl Sto006LevelCompactionParams {
    /// Table from **STO-006.md** § Level Compaction Configuration (MiB triggers, multiplier, L0 gates, levels).
    pub const NORMATIVE: Self = Self {
        max_bytes_for_level_base: 256 * 1024 * 1024,
        max_bytes_for_level_multiplier: 10.0,
        level0_file_num_compaction_trigger: 4,
        level0_slowdown_writes_trigger: 20,
        level0_stop_writes_trigger: 36,
        target_file_size_base: 64 * 1024 * 1024,
        num_levels: 7,
    };
}

/// **STO-006** compaction style for `cf` (must match [`ALL_COLUMN_FAMILIES`] spelling).
///
/// Append-mostly stores (`archive_coin_records`, `state_snapshots`) use FIFO; everything else is Level-tuned for
/// point / prefix / range workloads per the spec rationale column.
pub fn sto006_compaction_style_for_cf(cf: &str) -> Sto006CompactionStyle {
    match cf {
        CF_ARCHIVE_COIN_RECORDS | CF_STATE_SNAPSHOTS => Sto006CompactionStyle::Fifo,
        _ => Sto006CompactionStyle::Level,
    }
}

/// `max_write_buffer_number` for `cf` — delegates to [`STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER`] by index.
pub fn sto006_max_write_buffer_number_for_cf(cf: &str) -> i32 {
    let idx = ALL_COLUMN_FAMILIES
        .iter()
        .position(|&n| n == cf)
        .unwrap_or_else(|| panic!("STO-006: unknown column family name {cf:?}"));
    STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER[idx]
}

/// Apply **STO-006** Level compaction parameters to fresh per-CF [`Options`].
pub fn sto006_apply_level_compaction_options(o: &mut Options) {
    let p = Sto006LevelCompactionParams::NORMATIVE;
    o.set_compaction_style(DBCompactionStyle::Level);
    o.set_max_bytes_for_level_base(p.max_bytes_for_level_base);
    o.set_max_bytes_for_level_multiplier(p.max_bytes_for_level_multiplier);
    o.set_level_zero_file_num_compaction_trigger(p.level0_file_num_compaction_trigger);
    o.set_level_zero_slowdown_writes_trigger(p.level0_slowdown_writes_trigger);
    o.set_level_zero_stop_writes_trigger(p.level0_stop_writes_trigger);
    o.set_target_file_size_base(p.target_file_size_base);
    o.set_num_levels(p.num_levels);
}

/// Apply **STO-006** FIFO compaction parameters to fresh per-CF [`Options`].
///
/// **Binding note (`allow_compaction = false`):** upstream RocksDB exposes `allow_compaction` on FIFO options,
/// but **rust-rocksdb 0.22** [`FifoCompactOptions`] only binds `set_max_table_files_size`. Pure FIFO without
/// intra-style compactions is still the dominant behavior for append-only CFs at this cap; document any future
/// binding upgrade in **STO-006** verification when the crate adds the setter.
pub fn sto006_apply_fifo_compaction_options(o: &mut Options) {
    let mut fifo = FifoCompactOptions::default();
    fifo.set_max_table_files_size(STO006_FIFO_MAX_TABLE_FILES_SIZE);
    o.set_fifo_compaction_options(&fifo);
    o.set_compaction_style(DBCompactionStyle::Fifo);
}

/// Build a [`ColumnFamilyDescriptor`] for one logical store.
///
/// Applies write-buffer sizing (**STO-002** + **STO-006** `max_write_buffer_number`), bloom / prefix table options
/// (**STO-004**), and compaction style (**STO-006**): FIFO for append-mostly archive + snapshots, Level elsewhere.
fn column_family_descriptor(cf: &str, config: &CoinStoreConfig) -> ColumnFamilyDescriptor {
    let idx = ALL_COLUMN_FAMILIES
        .iter()
        .position(|&n| n == cf)
        .unwrap_or_else(|| panic!("STO-002: unknown column family name {cf:?}"));

    let mut o = Options::default();
    o.set_write_buffer_size(STO002_ROCKS_WRITE_BUFFER_BYTES[idx]);
    o.set_max_write_buffer_number(STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER[idx]);

    if cf_uses_fixed_prefix_32(cf) {
        o.set_prefix_extractor(SliceTransform::create_fixed_prefix(32));
    }

    let plan = sto004_bloom_plan_for_column_family(cf, config);
    if let Some(ratio) = plan.memtable_prefix_bloom_ratio {
        o.set_memtable_prefix_bloom_ratio(ratio);
    }

    let mut block = BlockBasedOptions::default();
    if let Some(bits) = plan.sst_bloom_bits_per_key {
        block.set_bloom_filter(f64::from(bits), plan.sst_bloom_uses_block_based_builder);
    }
    if plan.pin_l0_filter_and_index_in_cache {
        block.set_pin_l0_filter_and_index_blocks_in_cache(true);
    }
    o.set_block_based_table_factory(&block);

    match sto006_compaction_style_for_cf(cf) {
        Sto006CompactionStyle::Fifo => sto006_apply_fifo_compaction_options(&mut o),
        Sto006CompactionStyle::Level => sto006_apply_level_compaction_options(&mut o),
    }

    ColumnFamilyDescriptor::new(cf, o)
}

/// [`WriteOptions`] used for every successful [`StorageBackend::batch_write`] on RocksDB (**STO-005**).
///
/// **Normative intent:** [`STO-005.md`](../../docs/requirements/domains/storage/specs/STO-005.md) calls for
/// `write_opt.set_sync(true)` so a committed block batch is **durable** after `batch_write` returns — one
/// synchronous WAL flush for the whole native [`rocksdb::WriteBatch`] rather than relying on the library
/// default (`sync = false`), which may leave data in the OS page cache across crashes.
///
/// **Interaction with empty batches:** [`RocksDbBackend::batch_write`] returns early when the logical
/// [`super::WriteBatch`] is empty, so this helper is never consulted for no-op commits (no spurious fsync).
///
/// **Contrast with [`StorageBackend::put`]:** point `put` / `delete` still use RocksDB’s default write path
/// (no per-op `sync`); only the block-sized batch entry point opts into the stricter durability contract.
fn sto005_batch_write_options() -> WriteOptions {
    let mut o = WriteOptions::default();
    o.set_sync(true);
    o
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
    /// Uses RocksDB’s native write batch plus [`DB::write_opt`] with `sto005_batch_write_options()`
    /// so the commit is **durable** (synchronous WAL) and still a **single** engine-level write for all ops
    /// (**STO-005**). If building the native batch fails (e.g. unknown logical `cf` before `write_opt`), nothing
    /// is persisted — partial native batches never reach the WAL.
    ///
    /// **Throughput:** Block-sized batches amortize memtable + WAL work vs. issuing one tiny batch per row;
    /// see [`tests/sto_005_tests.rs`](../../tests/sto_005_tests.rs) for the “many tiny commits vs. one fat batch”
    /// performance proof obligation.
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

        let opts = sto005_batch_write_options();
        self.db
            .write_opt(rocks_batch, &opts)
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

    /// Manual compaction on one logical CF (**STO-006** § Manual Compaction, **STO-001** trait surface).
    ///
    /// Uses the full key range so RocksDB may merge overlapping SSTs for that CF; unknown `cf` fails before
    /// touching the engine ([`StorageError::UnknownColumnFamily`]).
    fn compact(&self, cf: &str) -> Result<(), StorageError> {
        let cf_handle = self.cf_handle(cf)?;
        self.db
            .compact_range_cf(cf_handle, None::<&[u8]>, None::<&[u8]>);
        Ok(())
    }
}
