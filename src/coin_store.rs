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

use std::collections::{HashMap, HashSet};
use std::path::Path;

use chia_protocol::Bytes32;

use crate::config::CoinStoreConfig;
use crate::error::CoinStoreError;
use crate::merkle::{merkle_leaf_hash, SparseMerkleTree};
use crate::storage::schema;
use crate::storage::{StorageBackend as KvStore, WriteBatch};
use crate::types::{
    ApplyBlockResult, BlockData, CoinId, CoinRecord, CoinStoreSnapshot, CoinStoreStats,
    RollbackResult,
};

/// Metadata keys stored in the `metadata` column family.
const META_HEIGHT: &[u8] = b"chain_height";
const META_TIP_HASH: &[u8] = b"chain_tip_hash";
const META_TIMESTAMP: &[u8] = b"chain_timestamp";
const META_INITIALIZED: &[u8] = b"initialized";

/// Max [`WriteBatch`] operations per [`StorageBackend::batch_write`] flush inside [`CoinStore::restore`].
///
/// Mirrors SPEC.md §2.7 `MATERIALIZATION_BATCH_SIZE` (50_000) so restoring huge snapshots does not
/// build a single multi-gigabyte batch ([`API-008`](../../docs/requirements/domains/crate_api/specs/API-008.md) implementation notes).
const SNAPSHOT_RESTORE_BATCH_OPS: usize = 50_000;

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
/// The concrete engine comes from [`CoinStoreConfig::backend`] ([`crate::config::StorageBackend`]) and
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

    /// In-memory unspent coin IDs for O(1) [`Self::is_unspent`] (PRF-001 seed; API-010).
    ///
    /// **Population:** Filled on [`Self::init_genesis`] for each inserted genesis coin, rebuilt from disk
    /// when reopening an initialized store ([`Self::with_config`]), and replaced from [`CoinStoreSnapshot`]
    /// on [`Self::restore`]. BLK-008+ will remove IDs on spend and re-insert on rollback.
    ///
    /// # Requirement: API-010 (public `is_unspent`), PRF-001 (full incremental maintenance)
    unspent_ids: HashSet<CoinId>,
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

        let unspent_ids = if initialized {
            Self::rebuild_unspent_ids_from_backend(&*backend)?
        } else {
            HashSet::new()
        };

        Ok(Self {
            config,
            backend,
            merkle_tree,
            height,
            tip_hash,
            timestamp,
            initialized,
            unspent_ids,
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

        // Insert each genesis coin as a coin record (legacy 97-byte row until STO-008 bincode-only).
        for (coin, is_coinbase) in &initial_coins {
            let coin_id = coin.coin_id();
            let rec = CoinRecord::new(*coin, 0, timestamp, *is_coinbase);
            let record_bytes = Self::serialize_coin_record_storage_bytes(&rec)?;

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

        self.unspent_ids.clear();
        for (coin, _) in &initial_coins {
            self.unspent_ids.insert(coin.coin_id());
        }

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

    /// Returns `true` if `coin_id` is present in the in-memory unspent set (API-010; PRF-001).
    ///
    /// **Semantics:** `true` only when the ID was inserted after [`Self::init_genesis`], [`Self::restore`],
    /// or store reopen (same decode rules as [`Self::stats`]). `false` covers spent rows, unknown coins, and
    /// coins not yet reflected until BLK mutates this set on spend.
    ///
    /// **Performance:** Single [`HashSet::contains`] — no storage I/O, no extra mutex beyond future CON-002
    /// `RwLock` wrapping the whole [`CoinStore`] (API-010 implementation notes).
    ///
    /// # Requirement: API-010
    pub fn is_unspent(&self, coin_id: &CoinId) -> bool {
        self.unspent_ids.contains(coin_id)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Chain statistics (API-007)
    // ─────────────────────────────────────────────────────────────────────

    /// Aggregate [`CoinStoreStats`] for monitoring and QRY-010 chain-state reads.
    ///
    /// **Sources:** `height`, `timestamp`, `tip_hash` mirror in-memory tip metadata; `state_root` uses
    /// [`SparseMerkleTree::root_observed`](crate::merkle::SparseMerkleTree::root_observed) so this stays `&self`
    /// per [`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) §3.12. `unspent_count`, `spent_count`,
    /// and `total_unspent_value` are derived from `coin_records` rows (bincode [`CoinRecord`] or the
    /// temporary genesis byte layout from [`Self::serialize_legacy_coin_record`]) until PRF-003 materialized
    /// counters replace the scan ([`API-007`](../../docs/requirements/domains/crate_api/specs/API-007.md#performance)).
    ///
    /// **`hint_count` / `snapshot_count`:** key counts in [`schema::CF_HINTS`] and [`schema::CF_STATE_SNAPSHOTS`].
    /// Scan failures log a warning and contribute `0` so stats never panic on I/O.
    ///
    /// **Uninitialized store:** Before [`Self::init_genesis`], returns zeros / empty-tree root / zero hashes
    /// for tip fields matching a fresh open (see `tests/api_007_tests.rs`).
    ///
    /// # Requirement: API-007
    pub fn stats(&self) -> CoinStoreStats {
        let state_root = self.merkle_tree.root_observed();
        let mut unspent_count: u64 = 0;
        let mut spent_count: u64 = 0;
        let mut total_unspent_value: u64 = 0;

        if self.initialized {
            match self.backend.prefix_scan(schema::CF_COIN_RECORDS, &[]) {
                Ok(entries) => {
                    for (_key, value) in entries {
                        if let Some(rec) = Self::decode_coin_record_bytes(&value) {
                            if rec.spent_height.is_none() {
                                unspent_count = unspent_count.saturating_add(1);
                                total_unspent_value =
                                    total_unspent_value.saturating_add(rec.coin.amount);
                            } else {
                                spent_count = spent_count.saturating_add(1);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "stats: coin_records prefix_scan failed; aggregate counts defaulting to 0"
                    );
                }
            }
        }

        let hint_count = Self::cf_entry_count_u64(&*self.backend, schema::CF_HINTS);
        let snapshot_count = Self::cf_entry_count_usize(&*self.backend, schema::CF_STATE_SNAPSHOTS);

        CoinStoreStats {
            height: self.height,
            timestamp: self.timestamp,
            unspent_count,
            spent_count,
            total_unspent_value,
            state_root,
            tip_hash: self.tip_hash,
            hint_count,
            snapshot_count,
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Snapshots & checkpoints (API-008)
    // ─────────────────────────────────────────────────────────────────────

    /// Capture a serde-stable [`CoinStoreSnapshot`] of the current coinstate.
    ///
    /// **Decode path:** Scans [`schema::CF_COIN_RECORDS`] with [`Self::decode_coin_record_bytes`] — the same
    /// logic as [`Self::stats`] — so legacy genesis rows and future bincode [`CoinRecord`] values both
    /// appear as typed [`CoinRecord`] entries in `snapshot.coins`.
    ///
    /// **`state_root`:** [`SparseMerkleTree::root_observed`](crate::merkle::SparseMerkleTree::root_observed)
    /// matches [`CoinStoreStats::state_root`] for the same instant (API-007 / API-008 alignment).
    ///
    /// **`hints`:** Always empty until HNT-* persists reversible `(coin_id, hint)` pairs we can scan here
    /// ([`API-008`](../../docs/requirements/domains/crate_api/specs/API-008.md) field table).
    ///
    /// # Errors
    /// [`CoinStoreError::NotInitialized`] before [`Self::init_genesis`].
    ///
    /// # Requirement: API-008
    pub fn snapshot(&self) -> Result<CoinStoreSnapshot, CoinStoreError> {
        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }
        let entries = self
            .backend
            .prefix_scan(schema::CF_COIN_RECORDS, &[])
            .map_err(|e| {
                CoinStoreError::StorageError(format!("snapshot: coin_records scan: {e}"))
            })?;

        let mut coins = HashMap::new();
        for (_key, value) in entries {
            if let Some(rec) = Self::decode_coin_record_bytes(&value) {
                coins.insert(rec.coin_id(), rec);
            }
        }

        let total_coins = coins.len() as u64;
        let total_value: u64 = coins
            .values()
            .filter(|r| r.spent_height.is_none())
            .map(|r| r.coin.amount)
            .sum();

        Ok(CoinStoreSnapshot {
            height: self.height,
            block_hash: self.tip_hash,
            state_root: self.merkle_tree.root_observed(),
            timestamp: self.timestamp,
            coins,
            hints: Vec::new(),
            total_coins,
            total_value,
        })
    }

    /// Replace all column-family coinstate with `snapshot` (destructive; clears every CF in
    /// [`schema::ALL_COLUMN_FAMILIES`] before rewrite).
    ///
    /// **Algorithm (API-008 normative restore):**
    /// 1. Validate `total_coins`, `total_value`, and per-row `HashMap` keys vs [`CoinRecord::coin_id`].
    /// 2. Recompute the sparse Merkle root from **canonical on-disk encodings** chosen by
    ///    [`Self::serialize_coin_record_storage_bytes`] *before* mutating storage — catches tampered
    ///    snapshots without leaving a half-cleared DB on failure.
    /// 3. Clear all CFs, flush batched inserts (≤ [`SNAPSHOT_RESTORE_BATCH_OPS`] ops per batch), rebuild
    ///    secondary indices exactly like [`Self::init_genesis`] plus [`schema::CF_COIN_BY_SPENT_HEIGHT`]
    ///    for spent rows.
    /// 4. Persist chain metadata from `snapshot.{height, block_hash, timestamp}` and mark initialized.
    ///
    /// **Encoding contract:** Legacy **97-byte** rows when [`CoinRecord::ff_eligible`] is `false`
    /// (matches [`Self::decode_legacy_genesis_coin_record`]); **bincode** when `ff_eligible` is `true`
    /// so fast-forward metadata survives round-trips once BLK writes such rows.
    ///
    /// # Errors
    /// [`CoinStoreError::StateRootMismatch`] when the recomputed Merkle root disagrees with
    /// `snapshot.state_root`.
    ///
    /// # Requirement: API-008
    pub fn restore(&mut self, snapshot: CoinStoreSnapshot) -> Result<(), CoinStoreError> {
        if snapshot.total_coins != snapshot.coins.len() as u64 {
            return Err(CoinStoreError::StorageError(format!(
                "restore: total_coins {} != coins.len() {}",
                snapshot.total_coins,
                snapshot.coins.len()
            )));
        }
        let recomputed_value: u64 = snapshot
            .coins
            .values()
            .filter(|r| r.spent_height.is_none())
            .map(|r| r.coin.amount)
            .sum();
        if recomputed_value != snapshot.total_value {
            return Err(CoinStoreError::StorageError(format!(
                "restore: total_value {} != recomputed unspent sum {recomputed_value}",
                snapshot.total_value
            )));
        }
        for (map_id, rec) in &snapshot.coins {
            if *map_id != rec.coin_id() {
                return Err(CoinStoreError::StorageError(format!(
                    "restore: map key {map_id:?} != record coin_id {:?}",
                    rec.coin_id()
                )));
            }
        }

        let mut rows: Vec<(CoinRecord, Vec<u8>)> = Vec::with_capacity(snapshot.coins.len());
        for rec in snapshot.coins.values() {
            let bytes = Self::serialize_coin_record_storage_bytes(rec)?;
            rows.push((rec.clone(), bytes));
        }

        let mut merkle_entries: Vec<(Bytes32, Bytes32)> = Vec::with_capacity(rows.len());
        for (rec, bytes) in &rows {
            merkle_entries.push((rec.coin_id(), merkle_leaf_hash(bytes)));
        }

        let mut tree = SparseMerkleTree::new();
        if !merkle_entries.is_empty() {
            tree.batch_insert(&merkle_entries).map_err(|e| {
                CoinStoreError::StorageError(format!("restore: merkle batch_insert: {e}"))
            })?;
        }
        let computed_root = tree.root();
        if computed_root != snapshot.state_root {
            return Err(CoinStoreError::StateRootMismatch {
                expected: snapshot.state_root,
                computed: computed_root,
            });
        }

        for cf in schema::ALL_COLUMN_FAMILIES {
            Self::clear_column_family(&*self.backend, cf)?;
        }

        let mut batch = WriteBatch::new();
        for (rec, bytes) in &rows {
            Self::append_coin_record_to_batch(&mut batch, rec, bytes);
            if batch.len() >= SNAPSHOT_RESTORE_BATCH_OPS {
                self.backend.batch_write(std::mem::take(&mut batch))?;
            }
        }

        for (coin_id, hint) in &snapshot.hints {
            // STO-008: fixed-width hint keys via [`schema::coin_hint_key`] / [`schema::hint_coin_key`]
            // (same 64-byte layout as the previous inline concat — keys are now centralized for MRK/HNT proofs).
            let fwd = schema::coin_hint_key(coin_id, hint);
            batch.put(schema::CF_HINTS, fwd.as_slice(), &[]);
            let rev = schema::hint_coin_key(hint, coin_id);
            batch.put(schema::CF_HINTS_BY_VALUE, rev.as_slice(), &[]);
            if batch.len() >= SNAPSHOT_RESTORE_BATCH_OPS {
                self.backend.batch_write(std::mem::take(&mut batch))?;
            }
        }

        batch.put(schema::CF_METADATA, META_INITIALIZED, &[1u8]);
        batch.put(
            schema::CF_METADATA,
            META_HEIGHT,
            &snapshot.height.to_le_bytes(),
        );
        batch.put(
            schema::CF_METADATA,
            META_TIP_HASH,
            snapshot.block_hash.as_ref(),
        );
        batch.put(
            schema::CF_METADATA,
            META_TIMESTAMP,
            &snapshot.timestamp.to_le_bytes(),
        );

        if !batch.is_empty() {
            self.backend.batch_write(batch)?;
        }

        self.merkle_tree = tree;
        self.height = snapshot.height;
        self.tip_hash = snapshot.block_hash;
        self.timestamp = snapshot.timestamp;
        self.initialized = true;

        self.unspent_ids = snapshot
            .coins
            .values()
            .filter(|r| r.spent_height.is_none())
            .map(|r| r.coin_id())
            .collect();

        Ok(())
    }

    /// Serialize [`Self::snapshot`] and persist it under [`schema::CF_STATE_SNAPSHOTS`] keyed by
    /// [`Self::height`] (big-endian height key via [`schema::snapshot_key`]).
    ///
    /// **STO-008:** payload uses [`crate::storage::kv_bincode::encode_coin_store_snapshot`] (fixint + big-endian).
    /// [`Self::load_snapshot`] accepts both that layout and pre-STO-008 default-bincode blobs via
    /// [`crate::storage::kv_bincode::decode_coin_store_snapshot_storage`].
    ///
    /// Prunes older checkpoints when more than [`CoinStoreConfig::max_snapshots`] rows exist (oldest
    /// heights deleted first). If `max_snapshots == 0`, pruning is skipped (treated as “do not auto-prune”).
    ///
    /// # Requirement: API-008, API-003 (`max_snapshots`)
    pub fn save_snapshot(&self) -> Result<(), CoinStoreError> {
        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }
        let snap = self.snapshot()?;
        let payload = crate::storage::kv_bincode::encode_coin_store_snapshot(&snap)?;
        let key = schema::snapshot_key(self.height);
        self.backend
            .put(schema::CF_STATE_SNAPSHOTS, &key, &payload)?;
        Self::prune_snapshots_to_limit(&*self.backend, self.config.max_snapshots)?;
        Ok(())
    }

    /// Load a retained checkpoint by height, if present.
    ///
    /// # Requirement: API-008
    pub fn load_snapshot(&self, height: u64) -> Result<Option<CoinStoreSnapshot>, CoinStoreError> {
        let key = schema::snapshot_key(height);
        match self.backend.get(schema::CF_STATE_SNAPSHOTS, &key)? {
            None => Ok(None),
            Some(bytes) => Ok(Some(
                crate::storage::kv_bincode::decode_coin_store_snapshot_storage(&bytes)
                    .map_err(CoinStoreError::from_bincode_deserialize)?,
            )),
        }
    }

    /// Load the newest retained checkpoint (lexicographic max on big-endian height keys).
    ///
    /// # Requirement: API-008
    pub fn load_latest_snapshot(&self) -> Result<Option<CoinStoreSnapshot>, CoinStoreError> {
        let heights = self.available_snapshot_heights();
        let Some(&h) = heights.last() else {
            return Ok(None);
        };
        self.load_snapshot(h)
    }

    /// Heights currently present in [`schema::CF_STATE_SNAPSHOTS`], ascending.
    ///
    /// On scan failure logs a warning and returns an empty list (same resilience pattern as [`Self::stats`]).
    ///
    /// # Requirement: API-008
    pub fn available_snapshot_heights(&self) -> Vec<u64> {
        match self.backend.prefix_scan(schema::CF_STATE_SNAPSHOTS, &[]) {
            Ok(entries) => {
                let mut hs: Vec<u64> = entries
                    .into_iter()
                    .map(|(k, _)| schema::height_from_snapshot_key(&k))
                    .collect();
                hs.sort_unstable();
                hs
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "available_snapshot_heights: prefix_scan failed; returning empty"
                );
                Vec::new()
            }
        }
    }

    /// Decode a `coin_records` value written either as bincode [`CoinRecord`] (STO-008 target) or the
    /// genesis-era fixed layout from [`Self::serialize_legacy_coin_record`].
    fn decode_coin_record_bytes(bytes: &[u8]) -> Option<CoinRecord> {
        // STO-008 normative bincode (fixint + BE) first, then legacy default bincode (`ff_eligible` rows),
        // then the fixed 97-byte genesis tuple ([`Self::decode_legacy_genesis_coin_record`]).
        if let Ok(rec) = crate::storage::kv_bincode::decode_coin_record_storage(bytes) {
            return Some(rec);
        }
        Self::decode_legacy_genesis_coin_record(bytes)
    }

    /// Legacy 97-byte genesis row: parent(32) + puzzle_hash(32) + amount(8) + confirmed_height(8)
    /// + spent_height_sentinel(8) + coinbase(1) + timestamp(8). `spent_height == 0` means unspent.
    fn decode_legacy_genesis_coin_record(bytes: &[u8]) -> Option<CoinRecord> {
        const LEN: usize = 32 + 32 + 8 + 8 + 8 + 1 + 8;
        if bytes.len() != LEN {
            return None;
        }
        let parent = Bytes32::from(*<&[u8; 32]>::try_from(&bytes[0..32]).ok()?);
        let puzzle = Bytes32::from(*<&[u8; 32]>::try_from(&bytes[32..64]).ok()?);
        let amount = u64::from_le_bytes(bytes[64..72].try_into().ok()?);
        let confirmed_height = u64::from_le_bytes(bytes[72..80].try_into().ok()?);
        let spent_raw = u64::from_le_bytes(bytes[80..88].try_into().ok()?);
        let coinbase = bytes[88] != 0;
        let timestamp = u64::from_le_bytes(bytes[89..97].try_into().ok()?);
        let coin = chia_protocol::Coin::new(parent, puzzle, amount);
        let spent_height = (spent_raw != 0).then_some(spent_raw);
        let mut rec = CoinRecord::new(coin, confirmed_height, timestamp, coinbase);
        if let Some(h) = spent_height {
            rec.spend(h);
        }
        Some(rec)
    }

    fn cf_entry_count_u64(backend: &dyn KvStore, cf: &str) -> u64 {
        match backend.prefix_scan(cf, &[]) {
            Ok(entries) => entries.len() as u64,
            Err(e) => {
                tracing::warn!(cf, error = %e, "stats: prefix_scan failed; returning 0");
                0
            }
        }
    }

    fn cf_entry_count_usize(backend: &dyn KvStore, cf: &str) -> usize {
        match backend.prefix_scan(cf, &[]) {
            Ok(entries) => entries.len(),
            Err(e) => {
                tracing::warn!(cf, error = %e, "stats: prefix_scan failed; returning 0");
                0
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Block application & rollback (API-006 signatures; BLK-001+, RBK-001+ pipelines)
    // ─────────────────────────────────────────────────────────────────────

    /// Apply validated [`BlockData`] to this store ([SPEC.md §3.2](../../docs/resources/SPEC.md)).
    ///
    /// Orchestrates the full block application pipeline:
    /// - **Phase 1 — Validation (no writes):** BLK-002 height, BLK-003 parent hash, BLK-004 reward
    ///   count, BLK-005 removal validity, BLK-006 addition uniqueness, BLK-011 hint validation.
    /// - **Phase 2 — Mutation (atomic [`WriteBatch`]):** BLK-007 coin insertion, BLK-008 spend marking,
    ///   BLK-012 hint storage, BLK-013 Merkle update, BLK-014 chain tip commit.
    /// - **Phase 3 — Observability:** BLK-010 performance logging.
    ///
    /// On success returns [`ApplyBlockResult`] with new state root, counts, and height.
    /// On failure returns the appropriate [`CoinStoreError`] variant and **no** state changes occur.
    ///
    /// # Chia reference ([SPEC.md §1.4](../../docs/resources/SPEC.md))
    ///
    /// Corresponds to `CoinStore.new_block()` ([`coin_store.py:105-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L105)).
    ///
    /// # Requirements: BLK-001 through BLK-014
    pub fn apply_block(&mut self, block: BlockData) -> Result<ApplyBlockResult, CoinStoreError> {
        let start = std::time::Instant::now();

        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }

        // ─── Phase 1: Validation (read-only, no mutations) ───────────────

        // BLK-002: Height continuity — block.height must be exactly current + 1.
        // SPEC.md §1.1 "Chain validation on insert" — defense-in-depth.
        if block.height != self.height + 1 {
            return Err(CoinStoreError::HeightMismatch {
                expected: self.height + 1,
                got: block.height,
            });
        }

        // BLK-003: Parent hash — block.parent_hash must match current tip hash.
        // Genesis tip is zero hash; subsequent tips are the previous block's hash.
        if block.parent_hash != self.tip_hash {
            return Err(CoinStoreError::ParentHashMismatch {
                expected: self.tip_hash,
                got: block.parent_hash,
            });
        }

        // BLK-004: Reward coin count — genesis (height 1 after init_genesis at 0) needs ≥ 2 coinbase.
        // Height 0 is handled by init_genesis, so apply_block always gets height ≥ 1.
        // Chia: coin_store.py:138-141 — `assert len(reward_coins) >= 2` for height > 0.
        // SPEC.md §2.7: MIN_REWARD_COINS_PER_BLOCK = 2.
        {
            let min_rewards = 2usize; // MIN_REWARD_COINS_PER_BLOCK
            if block.coinbase_coins.len() < min_rewards {
                return Err(CoinStoreError::InvalidRewardCoinCount {
                    expected: format!(">= {}", min_rewards),
                    got: block.coinbase_coins.len(),
                });
            }
        }

        // BLK-005: Removal validation — every removal must exist and be unspent.
        // Validation occurs BEFORE any mutations (SPEC.md §1.5 #5, §1.3 #5).
        // Collect existing records for removals so we can mark them spent in Phase 2.
        let mut removal_records: Vec<CoinRecord> = Vec::with_capacity(block.removals.len());
        for removal_id in &block.removals {
            let key = schema::coin_key(removal_id);
            match self.backend.get(schema::CF_COIN_RECORDS, &key)? {
                None => return Err(CoinStoreError::CoinNotFound(*removal_id)),
                Some(bytes) => {
                    let rec = Self::decode_coin_record_bytes(&bytes).ok_or_else(|| {
                        CoinStoreError::StorageError(format!(
                            "apply_block: corrupt coin record for {:?}",
                            removal_id
                        ))
                    })?;
                    if rec.is_spent() {
                        return Err(CoinStoreError::DoubleSpend(*removal_id));
                    }
                    removal_records.push(rec);
                }
            }
        }

        // BLK-006: Addition validation — no addition coin_id already exists in the store.
        // Check both transaction additions AND coinbase coins.
        let mut all_addition_ids: HashSet<Bytes32> = HashSet::new();
        for addition in &block.additions {
            if !all_addition_ids.insert(addition.coin_id) {
                return Err(CoinStoreError::CoinAlreadyExists(addition.coin_id));
            }
            let key = schema::coin_key(&addition.coin_id);
            if self.backend.get(schema::CF_COIN_RECORDS, &key)?.is_some() {
                return Err(CoinStoreError::CoinAlreadyExists(addition.coin_id));
            }
        }
        for coinbase in &block.coinbase_coins {
            let cb_id = coinbase.coin_id();
            if !all_addition_ids.insert(cb_id) {
                return Err(CoinStoreError::CoinAlreadyExists(cb_id));
            }
            let key = schema::coin_key(&cb_id);
            if self.backend.get(schema::CF_COIN_RECORDS, &key)?.is_some() {
                return Err(CoinStoreError::CoinAlreadyExists(cb_id));
            }
        }

        // BLK-011: Hint validation — hints > 32 bytes reject the block; empty hints are skipped.
        // SPEC.md §1.5 #13, §2.7 MAX_HINT_LENGTH = 32.
        for (_, hint) in &block.hints {
            let hint_len = hint.as_ref().len();
            if hint_len > 32 {
                return Err(CoinStoreError::HintTooLong {
                    length: hint_len,
                    max: 32,
                });
            }
        }

        // ─── Phase 2: Mutation (atomic WriteBatch) ───────────────────────
        // All writes go into a single WriteBatch for atomicity (SPEC.md §1.6 #17).

        let mut batch = WriteBatch::new();
        let mut merkle_inserts: Vec<(Bytes32, Bytes32)> = Vec::new();
        let mut merkle_updates: Vec<(Bytes32, Bytes32)> = Vec::new();
        let mut coins_created: usize = 0;

        // BLK-007: Insert coinbase coins — always ff_eligible = false.
        // Chia: coin_store.py:128 — coinbase coins are never FF-eligible.
        for coinbase in &block.coinbase_coins {
            let rec = CoinRecord::new(*coinbase, block.height, block.timestamp, true);
            let record_bytes = Self::serialize_coin_record_storage_bytes(&rec)?;
            let coin_id = rec.coin_id();
            let leaf_hash = merkle_leaf_hash(&record_bytes);

            Self::append_coin_record_to_batch(&mut batch, &rec, &record_bytes);
            merkle_inserts.push((coin_id, leaf_hash));
            coins_created += 1;
        }

        // BLK-007: Insert transaction addition coins — ff_eligible from same_as_parent.
        // Chia: coin_store.py:128-129 — `same_as_parent=True` → spent_index = -1 (FF-eligible).
        for addition in &block.additions {
            let mut rec =
                CoinRecord::new(addition.coin, block.height, block.timestamp, false);
            if addition.same_as_parent {
                rec.ff_eligible = true;
            }
            let record_bytes = Self::serialize_coin_record_storage_bytes(&rec)?;
            let coin_id = rec.coin_id();
            let leaf_hash = merkle_leaf_hash(&record_bytes);

            Self::append_coin_record_to_batch(&mut batch, &rec, &record_bytes);
            merkle_inserts.push((coin_id, leaf_hash));
            coins_created += 1;
        }

        // BLK-008: Spend marking — mark each removal as spent at block.height.
        // Update the coin record, remove from unspent puzzle hash index, add to spent height index.
        // Chia: coin_store.py:627-648 — strict count assertion.
        let coins_spent = removal_records.len();
        for rec in &mut removal_records {
            rec.spend(block.height);
            let record_bytes = Self::serialize_coin_record_storage_bytes(rec)?;
            let coin_id = rec.coin_id();

            // Update primary record.
            let key = schema::coin_key(&coin_id);
            batch.put(schema::CF_COIN_RECORDS, &key, &record_bytes);

            // Remove from unspent puzzle hash index.
            let ph_key = schema::puzzle_hash_coin_key(&rec.coin.puzzle_hash, &coin_id);
            batch.delete(schema::CF_UNSPENT_BY_PUZZLE_HASH, &ph_key);

            // Add to spent height index.
            let sh_key = schema::height_coin_key(block.height, &coin_id);
            batch.put(schema::CF_COIN_BY_SPENT_HEIGHT, &sh_key, coin_id.as_ref());

            // Update Merkle leaf with new (spent) record bytes.
            let leaf_hash = merkle_leaf_hash(&record_bytes);
            merkle_updates.push((coin_id, leaf_hash));
        }

        // BLK-012: Hint storage — store validated hints in forward + reverse indices.
        // SPEC.md §1.5 #14: idempotent insertion (duplicate (coin_id, hint) is a no-op).
        for (coin_id, hint) in &block.hints {
            // Skip empty hints (all zeros) per BLK-011.
            if hint.as_ref() == &[0u8; 32] {
                continue;
            }
            let mut fwd = Vec::with_capacity(64);
            fwd.extend_from_slice(coin_id.as_ref());
            fwd.extend_from_slice(hint.as_ref());
            batch.put(schema::CF_HINTS, &fwd, &[]);

            let mut rev = Vec::with_capacity(64);
            rev.extend_from_slice(hint.as_ref());
            rev.extend_from_slice(coin_id.as_ref());
            batch.put(schema::CF_HINTS_BY_VALUE, &rev, &[]);
        }

        // BLK-013: Merkle tree batch update — single root recomputation.
        // SPEC.md §1.6 #7: batch Merkle updates.
        if !merkle_inserts.is_empty() {
            self.merkle_tree
                .batch_insert(&merkle_inserts)
                .map_err(|e| CoinStoreError::StorageError(format!("Merkle insert: {}", e)))?;
        }
        if !merkle_updates.is_empty() {
            self.merkle_tree
                .batch_update(&merkle_updates)
                .map_err(|e| CoinStoreError::StorageError(format!("Merkle update: {}", e)))?;
        }
        let state_root = self.merkle_tree.root();

        // BLK-009: State root verification — if expected_state_root is set, verify match.
        if let Some(expected) = block.expected_state_root {
            if state_root != expected {
                // Rollback Merkle tree changes by reconstructing from storage.
                // This maintains atomicity — no partial Merkle state.
                self.merkle_tree = Self::rebuild_merkle_tree(&*self.backend)?;
                return Err(CoinStoreError::StateRootMismatch {
                    expected,
                    computed: state_root,
                });
            }
        }

        // BLK-014: Chain tip atomic commit — height, tip_hash, timestamp in the same WriteBatch.
        batch.put(
            schema::CF_METADATA,
            META_HEIGHT,
            &block.height.to_le_bytes(),
        );
        batch.put(
            schema::CF_METADATA,
            META_TIP_HASH,
            block.block_hash.as_ref(),
        );
        batch.put(
            schema::CF_METADATA,
            META_TIMESTAMP,
            &block.timestamp.to_le_bytes(),
        );

        // Atomic commit — all writes in one batch (SPEC.md §1.6 #17).
        self.backend.batch_write(batch)?;

        // Update in-memory state to match persisted state.
        self.height = block.height;
        self.tip_hash = block.block_hash;
        self.timestamp = block.timestamp;

        // ─── Phase 3: Observability ──────────────────────────────────────

        // BLK-010: Performance logging — warn if > 10s (SPEC.md §1.5 #15, §2.7).
        let elapsed = start.elapsed();
        if elapsed.as_secs_f64() > 10.0 {
            tracing::warn!(
                height = block.height,
                additions = coins_created,
                removals = coins_spent,
                elapsed_secs = elapsed.as_secs_f64(),
                "apply_block took > 10s — consider faster storage"
            );
        } else {
            tracing::debug!(
                height = block.height,
                additions = coins_created,
                removals = coins_spent,
                elapsed_ms = elapsed.as_millis() as u64,
                "apply_block complete"
            );
        }

        Ok(ApplyBlockResult {
            state_root,
            coins_created,
            coins_spent,
            height: block.height,
        })
    }

    /// Roll the coinstate back to `target_height` (may be negative for full reset per RBK-001).
    ///
    /// **Return type (API-006 / RBK-001):** `Result<RollbackResult, CoinStoreError>` with enriched
    /// deleted / un-spent counts vs Chia's raw map alone ([`RollbackResult`]).
    ///
    /// **Status:** Rollback scan + Merkle rebuild (RBK-002..007) are not wired yet. Without genesis,
    /// returns [`CoinStoreError::NotInitialized`]. If `target_height` is strictly greater than
    /// [`Self::height`], returns [`CoinStoreError::RollbackAboveTip`] (API-010). Otherwise returns
    /// [`CoinStoreError::StorageError`] with a `"rollback_to_block:"` prefix until RBK ships.
    ///
    /// # Requirement: API-006 (type surface), API-010 (`RollbackAboveTip`), RBK-001 (full behavior)
    pub fn rollback_to_block(
        &mut self,
        target_height: i64,
    ) -> Result<RollbackResult, CoinStoreError> {
        if !self.initialized {
            return Err(CoinStoreError::NotInitialized);
        }
        if target_height > self.height as i64 {
            return Err(CoinStoreError::RollbackAboveTip {
                target: target_height,
                current: self.height,
            });
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
    ///
    /// Delegates to [`crate::storage::open_storage_backend`] (STO-007) so the factory and [`CoinStore`] cannot diverge.
    fn open_backend(config: &CoinStoreConfig) -> Result<Box<dyn KvStore>, CoinStoreError> {
        crate::storage::open_storage_backend(config.backend, config).map_err(Into::into)
    }

    /// Encode [`CoinRecord`] the same way [`Self::restore`] will write `coin_records` values.
    ///
    /// **Legacy path (`ff_eligible == false`):** fixed 97-byte layout consumed by
    /// [`Self::decode_legacy_genesis_coin_record`] — this keeps Merkle leaf hashes identical to the
    /// pre-API-008 genesis writer so [`Self::snapshot`] → [`Self::restore`] is lossless for normal coins.
    ///
    /// **Bincode path (`ff_eligible == true`):** STO-008 forward encoding for rows that cannot be
    /// represented in the legacy tuple (fast-forward eligibility bit).
    fn serialize_coin_record_storage_bytes(rec: &CoinRecord) -> Result<Vec<u8>, CoinStoreError> {
        if rec.ff_eligible {
            Ok(crate::storage::kv_bincode::encode_coin_record(rec)?)
        } else {
            Ok(Self::serialize_legacy_coin_record(rec))
        }
    }

    /// Legacy on-disk `coin_records` layout: parent(32) + puzzle_hash(32) + amount(8) + confirmed_height(8)
    /// + spent_height_raw(8) + coinbase(1) + timestamp(8) = 97 bytes (`spent_height_raw == 0` means unspent).
    fn serialize_legacy_coin_record(rec: &CoinRecord) -> Vec<u8> {
        let mut buf = Vec::with_capacity(97);
        buf.extend_from_slice(rec.coin.parent_coin_info.as_ref());
        buf.extend_from_slice(rec.coin.puzzle_hash.as_ref());
        buf.extend_from_slice(&rec.coin.amount.to_le_bytes());
        buf.extend_from_slice(&rec.confirmed_height.to_le_bytes());
        let spent_raw = rec.spent_height.unwrap_or(0);
        buf.extend_from_slice(&spent_raw.to_le_bytes());
        buf.push(if rec.coinbase { 1 } else { 0 });
        buf.extend_from_slice(&rec.timestamp.to_le_bytes());
        buf
    }

    /// Append primary + secondary index writes for one coin row (shared by genesis + restore).
    fn append_coin_record_to_batch(batch: &mut WriteBatch, rec: &CoinRecord, record_bytes: &[u8]) {
        let coin_id = rec.coin_id();
        let key = schema::coin_key(&coin_id);
        batch.put(schema::CF_COIN_RECORDS, &key, record_bytes);

        let ph_key = schema::puzzle_hash_coin_key(&rec.coin.puzzle_hash, &coin_id);
        batch.put(schema::CF_COIN_BY_PUZZLE_HASH, &ph_key, coin_id.as_ref());
        if rec.spent_height.is_none() {
            batch.put(schema::CF_UNSPENT_BY_PUZZLE_HASH, &ph_key, &[]);
        }

        let parent_key = schema::parent_coin_key(&rec.coin.parent_coin_info, &coin_id);
        batch.put(schema::CF_COIN_BY_PARENT, &parent_key, coin_id.as_ref());

        let ch_key = schema::height_coin_key(rec.confirmed_height, &coin_id);
        batch.put(
            schema::CF_COIN_BY_CONFIRMED_HEIGHT,
            &ch_key,
            coin_id.as_ref(),
        );

        if let Some(spent_h) = rec.spent_height {
            let sh_key = schema::height_coin_key(spent_h, &coin_id);
            batch.put(schema::CF_COIN_BY_SPENT_HEIGHT, &sh_key, coin_id.as_ref());
        }
    }

    /// Delete every key in `cf` (used by [`Self::restore`] before full rewrite).
    fn clear_column_family(backend: &dyn KvStore, cf: &str) -> Result<(), CoinStoreError> {
        let pairs = backend
            .prefix_scan(cf, &[])
            .map_err(|e| CoinStoreError::StorageError(format!("clear_column_family({cf}): {e}")))?;
        for (k, _) in pairs {
            backend.delete(cf, &k)?;
        }
        Ok(())
    }

    /// Keep at most `max_snapshots` rows in [`schema::CF_STATE_SNAPSHOTS`] (delete smallest heights first).
    fn prune_snapshots_to_limit(
        backend: &dyn KvStore,
        max_snapshots: usize,
    ) -> Result<(), CoinStoreError> {
        if max_snapshots == 0 {
            return Ok(());
        }
        let entries = backend.prefix_scan(schema::CF_STATE_SNAPSHOTS, &[])?;
        if entries.len() <= max_snapshots {
            return Ok(());
        }
        let mut heights: Vec<u64> = entries
            .iter()
            .map(|(k, _)| schema::height_from_snapshot_key(k))
            .collect();
        heights.sort_unstable();
        let excess = entries.len() - max_snapshots;
        for h in heights.iter().take(excess) {
            let key = schema::snapshot_key(*h);
            backend.delete(schema::CF_STATE_SNAPSHOTS, &key)?;
        }
        Ok(())
    }

    /// Rebuild [`CoinStore::unspent_ids`] from `coin_records` (startup path for reopened stores).
    ///
    /// Uses the same decode predicate as [`Self::stats`] (`spent_height == None` ⇒ unspent). Full PRF-001
    /// will incrementally maintain this set during BLK/RBK instead of full rescans where possible.
    fn rebuild_unspent_ids_from_backend(
        backend: &dyn KvStore,
    ) -> Result<HashSet<CoinId>, CoinStoreError> {
        let mut set = HashSet::new();
        let entries = backend
            .prefix_scan(schema::CF_COIN_RECORDS, &[])
            .map_err(|e| {
                CoinStoreError::StorageError(format!("rebuild_unspent_ids_from_backend: {e}"))
            })?;
        for (_key, value) in entries {
            if let Some(rec) = Self::decode_coin_record_bytes(&value) {
                if rec.spent_height.is_none() {
                    set.insert(rec.coin_id());
                }
            }
        }
        Ok(set)
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
