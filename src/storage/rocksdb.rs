//! RocksDB storage backend implementation.
//!
//! Implements [`StorageBackend`] using RocksDB with column families for each
//! index type. Column families are created on first open based on the names
//! defined in [`super::schema`].
//!
//! # Requirements: STR-003, STO-002, STO-005
//! # Spec: docs/requirements/domains/storage/specs/STO-002.md
//! # SPEC.md: Section 7.2 (RocksDB Column Families)
//!
//! # Chia comparison
//! Chia uses SQLite with indices for coin lookups. RocksDB provides better
//! write throughput (LSM tree) and bloom filter support for negative lookups.

use std::path::Path;
use std::sync::Arc;

use rocksdb::{
    BlockBasedOptions, ColumnFamilyDescriptor, IteratorMode, Options,
    WriteBatch as RocksWriteBatch, DB,
};

use crate::config::{CoinStoreConfig, BLOOM_FILTER_BITS_PER_KEY};

use super::schema::ALL_COLUMN_FAMILIES;
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

impl RocksDbBackend {
    /// Open (or create) a RocksDB database using [`CoinStoreConfig`] tuning.
    ///
    /// Applies `rocksdb_write_buffer_size`, `rocksdb_max_open_files`, and optional block-based
    /// bloom filters per API-003 / SPEC §2.6. Column families follow [`ALL_COLUMN_FAMILIES`].
    ///
    /// # Errors
    ///
    /// Returns `StorageError::BackendError` if the database cannot be opened
    /// (e.g., path doesn't exist and can't be created, lock file conflict).
    pub fn open(config: &CoinStoreConfig) -> Result<Self, StorageError> {
        let path: &Path = config.storage_path.as_path();

        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_write_buffer_size(config.rocksdb_write_buffer_size);
        opts.set_max_open_files(config.rocksdb_max_open_files);

        let mut block_opts = BlockBasedOptions::default();
        if config.bloom_filter {
            block_opts.set_bloom_filter(f64::from(BLOOM_FILTER_BITS_PER_KEY), false);
        }
        opts.set_block_based_table_factory(&block_opts);

        // Column families share the same block/table factory from `opts` for new CFs.
        let cf_descriptors: Vec<ColumnFamilyDescriptor> = ALL_COLUMN_FAMILIES
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
            .collect();

        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)
            .map_err(|e| StorageError::BackendError(format!("Failed to open RocksDB: {}", e)))?;

        Ok(Self { db: Arc::new(db) })
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

    /// Prefix scan: return all KV pairs where key starts with `prefix`.
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
