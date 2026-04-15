//! CoinStore — the primary public API struct for dig-coinstore.
//!
//! Orchestrates block application, rollback, queries, hints, caching,
//! and Merkle tree operations. All public methods are defined here or
//! in domain-specific modules that add `impl CoinStore` blocks.
//!
//! # Construction
//!
//! ```ignore
//! let store = CoinStore::new("./data/coinstate")?;
//! store.init_genesis(vec![(genesis_coin, true)], timestamp)?;
//! ```
//!
//! # Requirement: API-001
//! # Spec: docs/requirements/domains/crate_api/specs/API-001.md
//! # SPEC.md: Section 3.1

use std::path::Path;

use chia_protocol::Bytes32;

use crate::config::{CoinStoreConfig, StorageBackend as ConfiguredEngine};
use crate::error::CoinStoreError;
use crate::merkle::{merkle_leaf_hash, SparseMerkleTree};
#[cfg(feature = "lmdb-storage")]
use crate::storage::lmdb::LmdbBackend;
#[cfg(feature = "rocksdb-storage")]
use crate::storage::rocksdb::RocksDbBackend;
use crate::storage::schema;
use crate::storage::{StorageBackend as KvStore, WriteBatch};
use crate::types::{ApplyBlockResult, BlockData, RollbackResult};

/// Metadata keys stored in the `metadata` column family.
const META_HEIGHT: &[u8] = b"chain_height";
const META_TIP_HASH: &[u8] = b"chain_tip_hash";
const META_TIMESTAMP: &[u8] = b"chain_timestamp";
const META_INITIALIZED: &[u8] = b"initialized";

/// The primary public API for the dig-coinstore crate.
///
/// Manages the global coin state for the DIG Network L2 blockchain.
/// Provides block application, rollback, queries, hints, Merkle proofs,
/// and snapshot/restore functionality.
///
/// # Thread safety
///
/// `CoinStore` is designed to be `Send + Sync` (CON-001). Internal
/// mutability is managed via `parking_lot::RwLock` (CON-002, added later).
///
/// # Storage
///
/// All persistent state is stored via the [`KvStore`] trait (`storage::StorageBackend`).
/// The concrete engine comes from [`CoinStoreConfig::backend`] ([`ConfiguredEngine`]) and
/// must match enabled Cargo features (`rocksdb-storage`, `lmdb-storage`).
///
/// # Requirement: API-001
/// # Spec: docs/requirements/domains/crate_api/specs/API-001.md
pub struct CoinStore {
    /// Effective configuration (path, limits, engine choice). Immutable after open.
    config: CoinStoreConfig,

    /// The storage backend (RocksDB or LMDB).
    backend: Box<dyn KvStore>,

    /// In-memory sparse Merkle tree for state root computation.
    /// Persisted incrementally via dirty node flushing (MRK-003).
    merkle_tree: SparseMerkleTree,

    /// Current chain tip height. 0 after genesis, incremented per block.
    height: u64,

    /// Current chain tip block hash.
    tip_hash: Bytes32,

    /// Current chain tip timestamp.
    timestamp: u64,

    /// Whether init_genesis() has been called.
    initialized: bool,
}

impl CoinStore {
    /// Create a new coinstate store with default configuration at the given path.
    ///
    /// Opens or creates the storage backend, initializes internal data structures.
    /// The store is empty until `init_genesis()` is called.
    ///
    /// # Errors
    ///
    /// Returns `CoinStoreError::StorageError` if the storage backend cannot be
    /// opened (e.g., path permissions, lock file conflict).
    ///
    /// # Requirement: API-001
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CoinStoreError> {
        let config = CoinStoreConfig::default_with_path(path);
        Self::with_config(config)
    }

    /// Create a coinstate store with custom configuration.
    ///
    /// All configuration values from `config` are respected. The storage
    /// backend is selected based on compile-time feature flags:
    /// - `rocksdb-storage` (default): RocksDB backend
    /// - `lmdb-storage`: LMDB backend
    ///
    /// # Errors
    ///
    /// Returns `CoinStoreError::StorageError` if the backend cannot be opened.
    ///
    /// # Requirement: API-001
    pub fn with_config(config: CoinStoreConfig) -> Result<Self, CoinStoreError> {
        // Create the storage directory if it doesn't exist.
        std::fs::create_dir_all(&config.storage_path).map_err(|e| {
            CoinStoreError::StorageError(format!(
                "Failed to create storage directory {}: {}",
                config.storage_path.display(),
                e
            ))
        })?;

        // Open the storage backend.
        let backend: Box<dyn KvStore> = Self::open_backend(&config)?;

        // Load persisted state from metadata CF (for re-open scenario).
        let initialized = backend
            .get(schema::CF_METADATA, META_INITIALIZED)?
            .is_some();

        let height = backend
            .get(schema::CF_METADATA, META_HEIGHT)?
            .map(|v| u64::from_le_bytes(v.try_into().unwrap_or([0u8; 8])))
            .unwrap_or(0);

        let tip_hash = backend
            .get(schema::CF_METADATA, META_TIP_HASH)?
            .map(|v| {
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&v[..32.min(v.len())]);
                Bytes32::from(bytes)
            })
            .unwrap_or_else(|| Bytes32::from([0u8; 32]));

        let timestamp = backend
            .get(schema::CF_METADATA, META_TIMESTAMP)?
            .map(|v| u64::from_le_bytes(v.try_into().unwrap_or([0u8; 8])))
            .unwrap_or(0);

        // Reconstruct the Merkle tree.
        // For now: if initialized, rebuild from coin records. If not, empty tree.
        // MRK-003 (persistent nodes) will optimize this to O(1) startup.
        let merkle_tree = if initialized {
            Self::rebuild_merkle_tree(&*backend)?
        } else {
            SparseMerkleTree::new()
        };

        Ok(Self {
            config,
            backend,
            merkle_tree,
            height,
            tip_hash,
            timestamp,
            initialized,
        })
    }

    /// Initialize genesis state with initial coins.
    ///
    /// Called once when bootstrapping a new chain. Inserts all genesis coins
    /// at height 0, builds the initial Merkle tree, and persists the state.
    ///
    /// # Errors
    ///
    /// - `GenesisAlreadyInitialized` if the store has already been initialized.
    ///
    /// # Returns
    ///
    /// The genesis state root (Merkle root of all genesis coins).
    ///
    /// # Requirement: API-001
    /// # Chia comparison: Chia handles genesis via the first `new_block()` call.
    ///   dig-coinstore separates it for explicit bootstrap control.
    pub fn init_genesis(
        &mut self,
        initial_coins: Vec<(chia_protocol::Coin, bool)>,
        timestamp: u64,
    ) -> Result<Bytes32, CoinStoreError> {
        if self.initialized {
            return Err(CoinStoreError::GenesisAlreadyInitialized);
        }

        let mut batch = WriteBatch::new();
        let mut merkle_entries: Vec<(Bytes32, Bytes32)> = Vec::new();

        // Insert each genesis coin as a coin record.
        for (coin, is_coinbase) in &initial_coins {
            let coin_id = coin.coin_id();

            // Serialize a minimal coin record representation.
            // Full CoinRecord struct (API-002) will replace this.
            // For now: store the raw coin bytes + metadata.
            let record_bytes = Self::serialize_genesis_record(coin, *is_coinbase, timestamp)?;

            // Primary coin record.
            let key = schema::coin_key(&coin_id);
            batch.put(schema::CF_COIN_RECORDS, &key, &record_bytes);

            // Puzzle hash index.
            let ph_key = schema::puzzle_hash_coin_key(&coin.puzzle_hash, &coin_id);
            batch.put(schema::CF_COIN_BY_PUZZLE_HASH, &ph_key, coin_id.as_ref());

            // Unspent puzzle hash index.
            batch.put(schema::CF_UNSPENT_BY_PUZZLE_HASH, &ph_key, &[]);

            // Parent index.
            let parent_key = schema::parent_coin_key(&coin.parent_coin_info, &coin_id);
            batch.put(schema::CF_COIN_BY_PARENT, &parent_key, coin_id.as_ref());

            // Height index (confirmed at height 0).
            let height_key = schema::height_coin_key(0, &coin_id);
            batch.put(
                schema::CF_COIN_BY_CONFIRMED_HEIGHT,
                &height_key,
                coin_id.as_ref(),
            );

            // Merkle leaf: hash of the coin record data.
            let leaf_hash = merkle_leaf_hash(&record_bytes);
            merkle_entries.push((coin_id, leaf_hash));
        }

        // Build Merkle tree from genesis coins.
        if !merkle_entries.is_empty() {
            self.merkle_tree
                .batch_insert(&merkle_entries)
                .map_err(|e| CoinStoreError::StorageError(format!("Merkle insert error: {}", e)))?;
        }
        let state_root = self.merkle_tree.root();

        // Store metadata.
        batch.put(schema::CF_METADATA, META_INITIALIZED, &[1u8]);
        batch.put(schema::CF_METADATA, META_HEIGHT, &0u64.to_le_bytes());
        batch.put(
            schema::CF_METADATA,
            META_TIP_HASH,
            Bytes32::from([0u8; 32]).as_ref(),
        );
        batch.put(
            schema::CF_METADATA,
            META_TIMESTAMP,
            &timestamp.to_le_bytes(),
        );

        // Atomic commit.
        self.backend.batch_write(batch)?;

        // Update in-memory state.
        self.height = 0;
        self.tip_hash = Bytes32::from([0u8; 32]);
        self.timestamp = timestamp;
        self.initialized = true;

        Ok(state_root)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Accessors
    // ─────────────────────────────────────────────────────────────────────

    /// Current chain tip height. 0 after genesis, incremented per block.
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Current chain tip block hash.
    pub fn tip_hash(&self) -> Bytes32 {
        self.tip_hash
    }

    /// Current chain tip timestamp.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Current state root (Merkle root of all coin records).
    pub fn state_root(&mut self) -> Bytes32 {
        self.merkle_tree.root()
    }

    /// Whether the store has been initialized (genesis applied).
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Whether the store is empty (no coins, no genesis).
    ///
    /// Returns true if the store has not been initialized OR if genesis
    /// was called with zero coins and no blocks have been applied.
    pub fn is_empty(&self) -> bool {
        !self.initialized
    }

    /// Configuration used to open this store (path, engine, tuning knobs).
    ///
    /// # Requirement: API-003
    pub fn config(&self) -> &CoinStoreConfig {
        &self.config
    }

    // ─────────────────────────────────────────────────────────────────────
    // Block application & rollback (API-006 signatures; BLK-001+, RBK-001+ pipelines)
    // ─────────────────────────────────────────────────────────────────────

    /// Apply validated [`BlockData`] to this store.
    ///
    /// **Return type (API-006 / BLK-001):** `Result<ApplyBlockResult, CoinStoreError>`. On success,
    /// callers receive the new state root and counts; on failure, **no** persistent state change (atomic).
    ///
    /// **Status:** The full validation + mutation pipeline (BLK-002..014) ships in later requirements.
    /// Until [`CoinStore::init_genesis`] has run, returns [`CoinStoreError::NotInitialized`]. After
    /// genesis, returns [`CoinStoreError::StorageError`] with a stable `"apply_block:"` prefix and the
    /// block height in the message until BLK-001+ wires real behavior (see `tests/api_006_tests.rs`).
    ///
    /// # Requirement: API-006 (type surface), BLK-001 (full behavior)
    pub fn apply_block(&mut self, block: BlockData) -> Result<ApplyBlockResult, CoinStoreError> {
        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }
        Err(CoinStoreError::StorageError(format!(
            "apply_block: not implemented - block height {} (BLK-001..BLK-014)",
            block.height
        )))
    }

    /// Roll the coinstate back to `target_height` (may be negative for full reset per RBK-001).
    ///
    /// **Return type (API-006 / RBK-001):** `Result<RollbackResult, CoinStoreError>` with enriched
    /// deleted / un-spent counts vs Chia's raw map alone ([`RollbackResult`]).
    ///
    /// **Status:** Rollback scan + Merkle rebuild (RBK-002..007) are not wired yet. Without genesis,
    /// returns [`CoinStoreError::NotInitialized`]. Otherwise returns [`CoinStoreError::StorageError`]
    /// with a `"rollback_to_block:"` prefix.
    ///
    /// # Requirement: API-006 (type surface), RBK-001 (full behavior)
    pub fn rollback_to_block(
        &mut self,
        target_height: i64,
    ) -> Result<RollbackResult, CoinStoreError> {
        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }
        Err(CoinStoreError::StorageError(format!(
            "rollback_to_block: not implemented - target_height {target_height} (RBK-001..RBK-007)"
        )))
    }

    /// Convenience: roll back exactly `n` blocks from the current tip.
    ///
    /// **Return type:** Same as [`Self::rollback_to_block`]. Implementation will compute the target
    /// height from [`Self::height()`] and delegate (RBK-005).
    pub fn rollback_n_blocks(&mut self, n: u64) -> Result<RollbackResult, CoinStoreError> {
        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }
        Err(CoinStoreError::StorageError(format!(
            "rollback_n_blocks: not implemented - n {n} (RBK-005 delegates to rollback_to_block)"
        )))
    }

    // ─────────────────────────────────────────────────────────────────────
    // Internal: backend selection
    // ─────────────────────────────────────────────────────────────────────

    /// Open the storage backend selected by [`CoinStoreConfig::backend`].
    #[cfg(any(feature = "rocksdb-storage", feature = "lmdb-storage"))]
    fn open_backend(config: &CoinStoreConfig) -> Result<Box<dyn KvStore>, CoinStoreError> {
        match config.backend {
            ConfiguredEngine::RocksDb => {
                #[cfg(feature = "rocksdb-storage")]
                {
                    Ok(Box::new(RocksDbBackend::open(config)?))
                }
                #[cfg(not(feature = "rocksdb-storage"))]
                {
                    Err(CoinStoreError::StorageError(
                        "CoinStoreConfig.backend is RocksDb but the crate was built without \
                         `rocksdb-storage`."
                            .into(),
                    ))
                }
            }
            ConfiguredEngine::Lmdb => {
                #[cfg(feature = "lmdb-storage")]
                {
                    Ok(Box::new(LmdbBackend::open(config)?))
                }
                #[cfg(not(feature = "lmdb-storage"))]
                {
                    Err(CoinStoreError::StorageError(
                        "CoinStoreConfig.backend is Lmdb but the crate was built without \
                         `lmdb-storage`."
                            .into(),
                    ))
                }
            }
        }
    }

    /// Fallback when no storage backend feature is enabled.
    #[cfg(not(any(feature = "rocksdb-storage", feature = "lmdb-storage")))]
    fn open_backend(_config: &CoinStoreConfig) -> Result<Box<dyn KvStore>, CoinStoreError> {
        Err(CoinStoreError::StorageError(
            "No storage backend enabled. Enable 'rocksdb-storage' or 'lmdb-storage' feature."
                .to_string(),
        ))
    }

    /// Serialize a genesis coin record as bytes.
    ///
    /// This is a temporary serialization format used until the full CoinRecord
    /// struct is defined (API-002). It stores: parent(32) + puzzle_hash(32) +
    /// amount(8) + confirmed_height(8) + spent_height(8) + coinbase(1) + timestamp(8).
    fn serialize_genesis_record(
        coin: &chia_protocol::Coin,
        is_coinbase: bool,
        timestamp: u64,
    ) -> Result<Vec<u8>, CoinStoreError> {
        let mut buf = Vec::with_capacity(97);
        buf.extend_from_slice(coin.parent_coin_info.as_ref());
        buf.extend_from_slice(coin.puzzle_hash.as_ref());
        buf.extend_from_slice(&coin.amount.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes()); // confirmed_height = 0
        buf.extend_from_slice(&0u64.to_le_bytes()); // spent_height = 0 (unspent)
        buf.push(if is_coinbase { 1 } else { 0 });
        buf.extend_from_slice(&timestamp.to_le_bytes());
        Ok(buf)
    }

    /// Rebuild the Merkle tree from persisted coin records.
    ///
    /// Scans all entries in the `coin_records` CF and inserts their leaf hashes
    /// into a fresh SparseMerkleTree. This is O(N) in the number of coins.
    ///
    /// TODO: MRK-003 will replace this with persistent node loading for O(1) startup.
    fn rebuild_merkle_tree(backend: &dyn KvStore) -> Result<SparseMerkleTree, CoinStoreError> {
        let mut tree = SparseMerkleTree::new();

        // Scan all coin records.
        let entries = backend.prefix_scan(schema::CF_COIN_RECORDS, &[])?;

        let mut merkle_entries: Vec<(Bytes32, Bytes32)> = Vec::with_capacity(entries.len());
        for (key, value) in &entries {
            let coin_id = schema::coin_id_from_key(key);
            let leaf_hash = merkle_leaf_hash(value);
            merkle_entries.push((coin_id, leaf_hash));
        }

        if !merkle_entries.is_empty() {
            tree.batch_insert(&merkle_entries).map_err(|e| {
                CoinStoreError::StorageError(format!("Merkle rebuild error: {}", e))
            })?;
        }

        Ok(tree)
    }
}
