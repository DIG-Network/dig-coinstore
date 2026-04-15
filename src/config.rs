//! Configuration types and constants for dig-coinstore.
//!
//! [`CoinStoreConfig`] holds every tunable parameter for opening a [`crate::coin_store::CoinStore`],
//! with a [`Default`] implementation and `with_*` builder methods (API-003). Named constants
//! mirror [`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) Section 2.7 so defaults stay
//! traceable to the master spec and to Chia’s `coin_store.py` where noted.
//!
//! # Naming: `config::StorageBackend` vs `storage::StorageBackend`
//!
//! The **enum** [`StorageBackend`] selects which *engine* to open (LMDB vs RocksDB). The **trait**
//! [`crate::storage::StorageBackend`](crate::storage::StorageBackend) is the key-value abstraction
//! implemented by both engines (STO-001). Same English name, different namespaces—callers choose
//! `dig_coinstore::config::StorageBackend` for configuration and `dig_coinstore::storage::StorageBackend`
//! only when implementing or taking a trait object.
//!
//! # Requirement: API-003
//! # Spec: docs/requirements/domains/crate_api/specs/API-003.md
//! # SPEC.md: Sections 1.3, 2.6, 2.7

use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// Constants (SPEC Section 2.7; API-003 default table)
// ─────────────────────────────────────────────────────────────────────────────

/// Default number of snapshots to retain before pruning.
///
/// # Spec link
/// SPEC.md §2.7 `DEFAULT_MAX_SNAPSHOTS`
pub const DEFAULT_MAX_SNAPSHOTS: usize = 10;

/// Maximum query results per batch. Matches Chia’s default `max_items=50000`.
///
/// # Spec link
/// SPEC.md §2.7 `DEFAULT_MAX_QUERY_RESULTS`; Chia
/// [`coin_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/coin_store.py)
pub const DEFAULT_MAX_QUERY_RESULTS: usize = 50_000;

/// Default LMDB environment map size (10 GiB).
///
/// # Spec link
/// SPEC.md §2.7 `DEFAULT_LMDB_MAP_SIZE`
pub const DEFAULT_LMDB_MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;

/// Default RocksDB write buffer size (64 MiB).
///
/// # Spec link
/// SPEC.md §2.6
pub const DEFAULT_ROCKSDB_WRITE_BUFFER_SIZE: usize = 64 * 1024 * 1024;

/// Default RocksDB `max_open_files`.
///
/// # Spec link
/// SPEC.md §2.6
pub const DEFAULT_ROCKSDB_MAX_OPEN_FILES: i32 = 1000;

/// Bloom filter bits per key for RocksDB block-based tables (~1% FP rate at 10 bits).
///
/// # Spec link
/// SPEC.md §2.7 `BLOOM_FILTER_BITS`
pub const BLOOM_FILTER_BITS_PER_KEY: i32 = 10;

/// Default: bloom filters enabled for RocksDB point lookups (STO-004).
pub const DEFAULT_BLOOM_FILTER_ENABLED: bool = true;

// ─────────────────────────────────────────────────────────────────────────────
// StorageBackend (configuration enum — not the KV trait)
// ─────────────────────────────────────────────────────────────────────────────

/// Which persistent storage engine [`crate::coin_store::CoinStore`] should open.
///
/// Compile-time features must align: opening [`StorageBackend::Lmdb`] requires the
/// `lmdb-storage` feature; [`StorageBackend::RocksDb`] requires `rocksdb-storage`.
/// When **both** features are enabled, [`CoinStoreConfig::default`] chooses LMDB per
/// SPEC §2.6 / API-003 implementation notes (LMDB preferred for dual builds).
///
/// # Requirement: API-003
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    /// LMDB — read-optimized, memory-mapped (`heed`).
    Lmdb,
    /// RocksDB — write-optimized LSM with bloom-capable SSTs.
    RocksDb,
}

// ─────────────────────────────────────────────────────────────────────────────
// Default backend selection (feature-gated)
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the spec default for [`StorageBackend`] from Cargo features.
///
/// Precedence: if `lmdb-storage` **and** `rocksdb-storage` → [`StorageBackend::Lmdb`];
/// else if only `lmdb-storage` → [`StorageBackend::Lmdb`]; else → [`StorageBackend::RocksDb`]
/// (covers default crate features and “neither feature” placeholder).
#[must_use]
pub fn default_storage_backend_for_features() -> StorageBackend {
    #[cfg(feature = "lmdb-storage")]
    {
        #[cfg(feature = "rocksdb-storage")]
        {
            StorageBackend::Lmdb
        }
        #[cfg(not(feature = "rocksdb-storage"))]
        {
            StorageBackend::Lmdb
        }
    }
    #[cfg(not(feature = "lmdb-storage"))]
    {
        StorageBackend::RocksDb
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CoinStoreConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Tunable parameters for constructing a [`crate::coin_store::CoinStore`].
///
/// Use [`Default::default`] for SPEC defaults, then chain `with_*` to override only what you need.
/// Field semantics follow SPEC §2.6; backend-specific fields apply only when that engine is open
/// (e.g. `lmdb_map_size` is ignored for RocksDB-only runs).
///
/// # Requirement: API-003
/// # Spec: docs/requirements/domains/crate_api/specs/API-003.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoinStoreConfig {
    /// Storage engine to open (LMDB vs RocksDB).
    pub backend: StorageBackend,

    /// Root directory for database files.
    pub storage_path: PathBuf,

    /// Retention cap for on-disk state snapshots (future PRF-008).
    pub max_snapshots: usize,

    /// Upper bound for batch query result size (QRY domain; Chia `max_items` parity).
    pub max_query_results: usize,

    /// LMDB map size (`heed` / `EnvOpenOptions::map_size`).
    pub lmdb_map_size: usize,

    /// RocksDB `write_buffer_size` (memtable budget per column family group).
    pub rocksdb_write_buffer_size: usize,

    /// RocksDB `max_open_files` (API-003 / SPEC §2.6 default `1000`).
    ///
    /// **RocksDB engine note:** `RocksDbBackend` in `src/storage/rocksdb.rs` opens with FIFO compaction on the
    /// `state_snapshots` column family (STO-002 / STO-006). The upstream
    /// `rocksdb` crate requires `max_open_files = -1` in that configuration, so the backend **does not**
    /// apply this field when opening the DB. The field and [`Self::with_rocksdb_max_open_files`] remain for
    /// forward compatibility, tests that assert config round-trips, and any alternate layout that drops FIFO.
    pub rocksdb_max_open_files: i32,

    /// When true, RocksDB uses a block-based bloom filter (see [`BLOOM_FILTER_BITS_PER_KEY`]).
    pub bloom_filter: bool,
}

impl Default for CoinStoreConfig {
    fn default() -> Self {
        Self {
            backend: default_storage_backend_for_features(),
            storage_path: PathBuf::from("./coinstate"),
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            max_query_results: DEFAULT_MAX_QUERY_RESULTS,
            lmdb_map_size: DEFAULT_LMDB_MAP_SIZE,
            rocksdb_write_buffer_size: DEFAULT_ROCKSDB_WRITE_BUFFER_SIZE,
            rocksdb_max_open_files: DEFAULT_ROCKSDB_MAX_OPEN_FILES,
            bloom_filter: DEFAULT_BLOOM_FILTER_ENABLED,
        }
    }
}

impl CoinStoreConfig {
    /// Defaults with a caller-supplied storage path (the common case for `CoinStore::new`).
    ///
    /// Equivalent to `CoinStoreConfig::default()` then [`Self::with_storage_path`].
    pub fn default_with_path(path: impl AsRef<Path>) -> Self {
        Self {
            storage_path: path.as_ref().to_path_buf(),
            ..Self::default()
        }
    }

    /// Builder: select LMDB vs RocksDB.
    pub fn with_backend(mut self, backend: StorageBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Builder: filesystem directory for the database.
    pub fn with_storage_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage_path = path.into();
        self
    }

    /// Builder: snapshot retention count.
    pub fn with_max_snapshots(mut self, count: usize) -> Self {
        self.max_snapshots = count;
        self
    }

    /// Builder: batch query result ceiling.
    pub fn with_max_query_results(mut self, count: usize) -> Self {
        self.max_query_results = count;
        self
    }

    /// Builder: LMDB environment map size in bytes.
    pub fn with_lmdb_map_size(mut self, size: usize) -> Self {
        self.lmdb_map_size = size;
        self
    }

    /// Builder: RocksDB write buffer size in bytes.
    pub fn with_rocksdb_write_buffer_size(mut self, size: usize) -> Self {
        self.rocksdb_write_buffer_size = size;
        self
    }

    /// Builder: RocksDB `max_open_files` (stored on config; see [`CoinStoreConfig::rocksdb_max_open_files`] for when it applies).
    pub fn with_rocksdb_max_open_files(mut self, count: i32) -> Self {
        self.rocksdb_max_open_files = count;
        self
    }

    /// Builder: enable or disable RocksDB bloom filters (ignored for LMDB).
    pub fn with_bloom_filter(mut self, enabled: bool) -> Self {
        self.bloom_filter = enabled;
        self
    }
}
