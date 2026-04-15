//! Storage backend abstraction for dig-coinstore.
//!
//! Defines the [`StorageBackend`] trait that abstracts over LMDB and RocksDB.
//! Both backends implement this trait, and the rest of the crate interacts
//! with storage exclusively through this interface.
//!
//! Backend selection is compile-time via Cargo feature gates:
//! - `rocksdb-storage` → [`rocksdb`] module
//! - `lmdb-storage` → [`lmdb`] module
//!
//! # Design rationale
//!
//! The trait uses `&str` for column family names (matching the string constants
//! in [`schema`]) and `&[u8]` / `Vec<u8>` for keys and values. This avoids
//! generic type parameters that would complicate dynamic dispatch while keeping
//! the API flexible enough for any key encoding scheme.
//!
//! # Requirements: STR-003, STO-001
//! # Spec: docs/requirements/domains/storage/specs/STO-001.md
//! # SPEC.md: Section 7 (Storage Architecture)
//!
//! **Verification:** behavioral acceptance for [`StorageBackend`] lives in [`tests/sto_001_tests.rs`](../../tests/sto_001_tests.rs)
//! (RocksDB by default; run with `--no-default-features --features lmdb-storage` for the LMDB slice).
//! **STO-003 (six LMDB databases, MVCC, `MapFull`):** [`tests/sto_003_tests.rs`](../../tests/sto_003_tests.rs).
//! **STO-002 (Rocks column families):** [`tests/sto_002_tests.rs`](../../tests/sto_002_tests.rs).
//! **STO-004 (RocksDB bloom / prefix / L0 pin):** [`tests/sto_004_tests.rs`](../../tests/sto_004_tests.rs).

#[cfg(feature = "rocksdb-storage")]
pub mod rocksdb;

#[cfg(feature = "lmdb-storage")]
pub mod lmdb;

pub mod schema;

// ─────────────────────────────────────────────────────────────────────────────
// StorageError
// ─────────────────────────────────────────────────────────────────────────────

/// Errors from storage backend operations.
///
/// This is a low-level error type used by [`StorageBackend`] implementations.
/// Higher-level code wraps this into `CoinStoreError::StorageError`.
#[derive(Debug, Clone, thiserror::Error)]
pub enum StorageError {
    /// The specified column family does not exist.
    #[error("unknown column family: {0}")]
    UnknownColumnFamily(String),

    /// A database I/O or internal error.
    #[error("backend error: {0}")]
    BackendError(String),

    /// LMDB mapped region is full (`MDB_MAP_FULL`); increase [`crate::config::CoinStoreConfig::lmdb_map_size`] and reopen.
    ///
    /// # Requirement: STO-003
    /// # Spec: docs/requirements/domains/storage/specs/STO-003.md
    #[error("LMDB map full: increase lmdb_map_size and reopen the environment")]
    MapFull,

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

/// A key-value pair returned from prefix scans.
pub type KvPair = (Vec<u8>, Vec<u8>);

// ─────────────────────────────────────────────────────────────────────────────
// WriteBatch
// ─────────────────────────────────────────────────────────────────────────────

/// A single operation within a [`WriteBatch`].
///
/// Operations are accumulated in memory and then committed atomically
/// via [`StorageBackend::batch_write`].
#[derive(Debug, Clone)]
pub enum WriteOp {
    /// Insert or update a key-value pair.
    Put {
        cf: String,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    /// Delete a key.
    Delete { cf: String, key: Vec<u8> },
}

/// An atomic batch of write operations.
///
/// Accumulates [`WriteOp`]s in memory, then commits them all at once
/// via [`StorageBackend::batch_write`]. This ensures either all writes
/// succeed or none do (atomicity).
///
/// # Usage
///
/// ```ignore
/// let mut batch = WriteBatch::new();
/// batch.put("coin_records", &coin_key, &serialized_record);
/// batch.put("coin_by_puzzle_hash", &ph_key, &coin_id);
/// batch.delete("unspent_by_puzzle_hash", &old_ph_key);
/// backend.batch_write(batch)?;
/// ```
///
/// # Requirement: STO-005
/// # Spec: docs/requirements/domains/storage/specs/STO-005.md
#[derive(Debug, Clone, Default)]
pub struct WriteBatch {
    /// The accumulated write operations.
    pub ops: Vec<WriteOp>,
}

impl WriteBatch {
    /// Create an empty write batch.
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    /// Create a write batch with pre-allocated capacity.
    ///
    /// Use when the number of operations is known in advance (e.g.,
    /// block application knows additions.len() + removals.len()).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ops: Vec::with_capacity(capacity),
        }
    }

    /// Add a put (insert/update) operation.
    pub fn put(&mut self, cf: &str, key: &[u8], value: &[u8]) {
        self.ops.push(WriteOp::Put {
            cf: cf.to_string(),
            key: key.to_vec(),
            value: value.to_vec(),
        });
    }

    /// Add a delete operation.
    pub fn delete(&mut self, cf: &str, key: &[u8]) {
        self.ops.push(WriteOp::Delete {
            cf: cf.to_string(),
            key: key.to_vec(),
        });
    }

    /// Number of operations in this batch.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether this batch is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StorageBackend trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait abstracting over key-value storage backends (RocksDB, LMDB).
///
/// All storage access in dig-coinstore goes through this trait. [`crate::coin_store::CoinStore`]
/// holds `Box<dyn StorageBackend>` (aliased internally as `KvStore`) so block application, queries,
/// and snapshots stay engine-agnostic ([`STO-001.md`](../../docs/requirements/domains/storage/specs/STO-001.md)).
///
/// # Method semantics (normative summary)
///
/// | Method | Contract |
/// |--------|----------|
/// | [`Self::get`] | `Ok(None)` if key missing — **not** an error ([`STO-001`](../../docs/requirements/domains/storage/specs/STO-001.md) § Method Semantics). |
/// | [`Self::put`] | Upsert; replaces existing value. |
/// | [`Self::delete`] | **Idempotent:** missing key is `Ok(())` (both engines normalize this). |
/// | [`Self::batch_write`] | All-or-nothing; empty batch is a no-op ([`STO-005`](../../docs/requirements/domains/storage/specs/STO-005.md) for WAL/fsync depth). |
/// | [`Self::prefix_scan`] | Keys with prefix `prefix`, ordered lexicographically by key (iterator contract). |
/// | [`Self::flush`] | Best-effort durability hint (Rocks WAL flush; LMDB `force_sync`). |
/// | [`Self::compact`] | Engine-specific maintenance (Rocks manual compaction; LMDB currently no-op). |
///
/// # Thread safety
///
/// Implementations MUST be `Send + Sync` so callers can share `Arc<dyn StorageBackend>` across
/// threads and satisfy [`CON-001`](../../docs/requirements/domains/concurrency/specs/CON-001.md).
///
/// # Column families
///
/// The `cf` parameter names logical stores from [`schema`] (same string table for Rocks column
/// families and LMDB named databases). Unknown `cf` → [`StorageError::UnknownColumnFamily`].
///
/// # Verification
///
/// Behavioral contract: [`tests/sto_001_tests.rs`](../../tests/sto_001_tests.rs) (per-domain file for STO-001).
/// Compile-time surface: [`tests/str_003_tests.rs`](../../tests/str_003_tests.rs) (STR-003 overlap).
///
/// # Requirement: STO-001
/// # Spec: docs/requirements/domains/storage/specs/STO-001.md
pub trait StorageBackend: Send + Sync {
    /// Retrieve a value by column family and key.
    ///
    /// Returns `Ok(None)` if the key does not exist (not an error).
    /// Returns `Err` only on I/O or backend failures.
    fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;

    /// Insert or update a key-value pair in the specified column family.
    fn put(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError>;

    /// Remove a key from the specified column family.
    ///
    /// Idempotent: no error if the key does not exist.
    fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StorageError>;

    /// Atomically apply all operations in a [`WriteBatch`].
    ///
    /// Either all operations succeed or none do. This is the primary
    /// mechanism for atomic block application (BLK-014, STO-005).
    ///
    /// An empty batch is a no-op (no error).
    fn batch_write(&self, batch: WriteBatch) -> Result<(), StorageError>;

    /// Return all key-value pairs where the key starts with `prefix`.
    ///
    /// Results are ordered by key (lexicographic). Used for puzzle hash
    /// lookups, height range scans, and hint queries.
    ///
    /// An empty prefix returns all entries in the column family (expensive).
    fn prefix_scan(&self, cf: &str, prefix: &[u8]) -> Result<Vec<KvPair>, StorageError>;

    /// Force WAL and memtable flush to persistent storage.
    fn flush(&self) -> Result<(), StorageError>;

    /// Trigger compaction on the specified column family.
    fn compact(&self, cf: &str) -> Result<(), StorageError>;
}
