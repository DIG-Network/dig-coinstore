# dig-coinstore

Persistent global coin state database for the DIG Network L2 blockchain.

Manages the authoritative database of all spent and unspent coins using the coinset model (UTXO-like). Accepts validated blocks, applies state transitions, and provides a rich query API for coin lookups by ID, puzzle hash, hint, parent, and height.

**Crate boundary:** Input = pre-validated `BlockData`. Output = `CoinRecord`s, `CoinState`s, state roots, Merkle proofs. This crate does NOT run CLVM, produce blocks, or manage the mempool.

Derived from Chia's [`CoinStore`](https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/coin_store.py) and [`HintStore`](https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/hint_store.py), with improvements: Merkle-committed state, embedded KV storage, snapshot/restore, tiered archival, and in-memory caching.

---

## Quick Start

```rust
use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::{BlockData, CoinAddition, Coin, Bytes32};

// Open or create the coinstate database.
let mut store = CoinStore::new("./data/coinstate")?;

// Bootstrap the chain with genesis coins.
let genesis_coin = Coin::new(parent_hash, puzzle_hash, 1_000_000);
let state_root = store.init_genesis(vec![(genesis_coin, false)], timestamp)?;

// Apply a block (pre-validated additions, removals, hints).
let result = store.apply_block(block_data)?;
println!("New state root: {:?}, coins created: {}", result.state_root, result.coins_created);

// Query coins.
let record = store.get_coin_record(&coin_id)?;
let by_puzzle = store.get_coin_records_by_puzzle_hash(true, &puzzle_hash, 0, u64::MAX)?;

// Rollback for chain reorganization.
let rollback = store.rollback_to_block(target_height)?;
```

---

## Storage Backends

Feature-gated at compile time:

| Feature | Backend | Default | Notes |
|---------|---------|---------|-------|
| `rocksdb-storage` | RocksDB | **Yes** | LSM tree, bloom filters, write-optimized |
| `lmdb-storage` | LMDB | No | Memory-mapped, read-optimized |
| `full-storage` | Both | No | LMDB preferred when both enabled |

```toml
[dependencies]
dig-coinstore = "0.1"                           # RocksDB (default)
dig-coinstore = { version = "0.1", features = ["lmdb-storage"] }  # LMDB
```

---

## Re-exported Chia Types

These types are re-exported at the crate root so consumers don't need direct `chia-protocol` or `dig-clvm` dependencies:

| Type | Source | Description |
|------|--------|-------------|
| `Coin` | `chia-protocol` via `dig-clvm` | Coin identity: `parent_coin_info`, `puzzle_hash`, `amount`. `coin_id() = sha256(parent \|\| puzzle_hash \|\| amount)` |
| `Bytes32` | `chia-protocol` via `dig-clvm` | 32-byte hash for coin IDs, puzzle hashes, block hashes, state roots |
| `CoinState` | `chia-protocol` via `dig-clvm` | Lightweight sync view: `coin`, `created_height: Option<u32>`, `spent_height: Option<u32>` |
| `CoinStateFilters` | `chia-protocol` | Batch query filters: `include_spent`, `include_unspent`, `include_hinted`, `min_amount` |

---

## Core Types

### `CoinRecord`

Full lifecycle state of one coin in the store. Persists after spending for history and rollback.

```rust
pub struct CoinRecord {
    pub coin: Coin,                  // Immutable coin identity and value
    pub confirmed_height: u64,       // Height where created
    pub spent_height: Option<u64>,   // None = unspent, Some(h) = spent at height h
    pub coinbase: bool,              // Block reward vs transaction output
    pub timestamp: u64,              // Block timestamp at creation
    pub ff_eligible: bool,           // Singleton fast-forward candidate
}
```

**Key methods:**
- `CoinRecord::new(coin, confirmed_height, timestamp, coinbase)` — new unspent coin
- `is_spent() -> bool` — `spent_height.is_some()`
- `spend(height)` — mark spent
- `coin_id() -> CoinId` — delegates to `Coin::coin_id()`
- `to_coin_state() -> CoinState` — lightweight sync view
- `from_chia_coin_record(ChiaCoinRecord) -> Self` — Chia interop (import)
- `to_chia_coin_record() -> ChiaCoinRecord` — Chia interop (export)

### `BlockData`

Input to `apply_block()`. Pre-extracted state changes from a validated block.

```rust
pub struct BlockData {
    pub height: u64,                           // Must be current_height + 1
    pub timestamp: u64,                        // Unix seconds
    pub block_hash: Bytes32,                   // This block's header hash
    pub parent_hash: Bytes32,                  // Must match current tip hash
    pub additions: Vec<CoinAddition>,          // Transaction-created coins
    pub removals: Vec<CoinId>,                 // Spent coin IDs
    pub coinbase_coins: Vec<Coin>,             // Block rewards (≥ 2 for non-genesis)
    pub hints: Vec<(CoinId, Bytes32)>,         // CREATE_COIN hints for wallet indexing
    pub expected_state_root: Option<Bytes32>,  // Optional post-apply root check
}
```

### `CoinAddition`

```rust
pub struct CoinAddition {
    pub coin_id: CoinId,        // sha256(parent || puzzle_hash || amount)
    pub coin: Coin,             // The created coin
    pub same_as_parent: bool,   // true → ff_eligible (singleton fast-forward)
}
```

**Constructor:** `CoinAddition::from_coin(coin, same_as_parent)` — computes `coin_id` via `Coin::coin_id()`.

### `ApplyBlockResult`

Returned on successful `apply_block()`.

```rust
pub struct ApplyBlockResult {
    pub state_root: Bytes32,   // Merkle root after this block
    pub coins_created: usize,  // additions.len() + coinbase_coins.len()
    pub coins_spent: usize,    // removals.len()
    pub height: u64,           // New chain tip height
}
```

### `RollbackResult`

Returned on successful `rollback_to_block()`.

```rust
pub struct RollbackResult {
    pub modified_coins: HashMap<CoinId, CoinRecord>,  // Pre-rollback snapshots
    pub coins_deleted: usize,    // Coins created after target (removed)
    pub coins_unspent: usize,    // Coins spent after target (un-spent)
    pub new_height: u64,         // Chain tip after rollback
}
```

### `CoinStoreStats`

Aggregated chain metrics from `stats()`.

```rust
pub struct CoinStoreStats {
    pub height: u64,
    pub timestamp: u64,
    pub unspent_count: u64,
    pub spent_count: u64,
    pub total_unspent_value: u64,
    pub state_root: Bytes32,
    pub tip_hash: Bytes32,
    pub hint_count: u64,
    pub snapshot_count: usize,
}
```

### `CoinStoreSnapshot`

Serializable checkpoint for fast sync / backup / restore.

```rust
pub struct CoinStoreSnapshot {
    pub height: u64,
    pub block_hash: Bytes32,
    pub state_root: Bytes32,
    pub timestamp: u64,
    pub coins: HashMap<CoinId, CoinRecord>,
    pub hints: Vec<(CoinId, Bytes32)>,
    pub total_coins: u64,
    pub total_value: u64,
}
```

### Type Aliases

```rust
pub type CoinId = Bytes32;       // sha256(parent || puzzle_hash || amount)
pub type PuzzleHash = Bytes32;   // sha256(serialized CLVM puzzle)
```

---

## CoinStoreError

All fallible methods return `Result<T, CoinStoreError>`. Variants:

| Variant | Trigger | Fields |
|---------|---------|--------|
| `HeightMismatch` | `block.height != current + 1` | `expected: u64, got: u64` |
| `ParentHashMismatch` | `block.parent_hash != tip_hash` | `expected: Bytes32, got: Bytes32` |
| `StateRootMismatch` | Computed root != `expected_state_root` | `expected: Bytes32, computed: Bytes32` |
| `CoinNotFound` | Removal references missing coin | `CoinId` |
| `CoinAlreadyExists` | Addition duplicates existing coin | `CoinId` |
| `DoubleSpend` | Removal references already-spent coin | `CoinId` |
| `SpendCountMismatch` | Updated rows != expected removals | `expected: usize, actual: usize` |
| `InvalidRewardCoinCount` | Wrong coinbase count for height | `expected: String, got: usize` |
| `HintTooLong` | Hint > 32 bytes | `length: usize, max: usize` |
| `GenesisAlreadyInitialized` | Double `init_genesis()` call | — |
| `NotInitialized` | Operation before `init_genesis()` | — |
| `RollbackAboveTip` | `target_height > current_height` | `target: i64, current: u64` |
| `PuzzleHashBatchTooLarge` | Batch query exceeds limit | `size: usize, max: usize` |
| `StorageError` | Backend I/O failure | `String` |
| `SerializationError` | Bincode encode failure | `String` |
| `DeserializationError` | Bincode decode failure | `String` |

---

## Public API Reference

### Construction

```rust
impl CoinStore {
    /// Open/create store with default config at path.
    fn new(path: impl AsRef<Path>) -> Result<Self, CoinStoreError>;

    /// Open/create with custom configuration.
    fn with_config(config: CoinStoreConfig) -> Result<Self, CoinStoreError>;

    /// Bootstrap chain with genesis coins. Called once.
    /// Returns the genesis state root.
    fn init_genesis(
        &mut self,
        initial_coins: Vec<(Coin, bool)>,  // (coin, is_coinbase)
        timestamp: u64,
    ) -> Result<Bytes32, CoinStoreError>;
}
```

### Block Application

```rust
impl CoinStore {
    /// Apply a validated block. Atomic: all-or-nothing.
    ///
    /// Phase 1 (validation): height, parent hash, reward count, removals exist+unspent,
    ///   additions unique, hints valid.
    /// Phase 2 (mutation): insert coins, mark spends, store hints, update Merkle tree,
    ///   commit chain tip. All in one WriteBatch.
    /// Phase 3 (observability): performance logging.
    fn apply_block(&mut self, block: BlockData) -> Result<ApplyBlockResult, CoinStoreError>;
}
```

### Rollback

```rust
impl CoinStore {
    /// Revert to target_height. Negative = full reset.
    /// Deletes coins confirmed after target, un-spends coins spent after target,
    /// cleans up hints, rebuilds Merkle tree.
    fn rollback_to_block(&mut self, target_height: i64) -> Result<RollbackResult, CoinStoreError>;

    /// Convenience: rollback_to_block(height - n).
    fn rollback_n_blocks(&mut self, n: u64) -> Result<RollbackResult, CoinStoreError>;
}
```

### Coin Queries

```rust
impl CoinStore {
    // --- Point lookups (QRY-001) ---
    fn get_coin_record(&self, coin_id: &CoinId) -> Result<Option<CoinRecord>, CoinStoreError>;
    fn get_coin_records(&self, coin_ids: &[CoinId]) -> Result<Vec<CoinRecord>, CoinStoreError>;

    // --- By puzzle hash (QRY-002) ---
    fn get_coin_records_by_puzzle_hash(
        &self, include_spent: bool, puzzle_hash: &Bytes32,
        start_height: u64, end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;
    fn get_coin_records_by_puzzle_hashes(
        &self, include_spent: bool, puzzle_hashes: &[Bytes32],
        start_height: u64, end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    // --- By height (QRY-003) ---
    fn get_coins_added_at_height(&self, height: u64) -> Result<Vec<CoinRecord>, CoinStoreError>;
    fn get_coins_removed_at_height(&self, height: u64) -> Result<Vec<CoinRecord>, CoinStoreError>;

    // --- By parent (QRY-004) ---
    fn get_coin_records_by_parent_ids(
        &self, include_spent: bool, parent_ids: &[CoinId],
        start_height: u64, end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    // --- By names with filters (QRY-005) ---
    fn get_coin_records_by_names(
        &self, include_spent: bool, names: &[CoinId],
        start_height: u64, end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    // --- Lightweight CoinState (QRY-006) ---
    fn get_coin_states_by_ids(
        &self, include_spent: bool, coin_ids: &[CoinId],
        min_height: u64, max_height: u64, max_items: usize,
    ) -> Result<Vec<CoinState>, CoinStoreError>;
    fn get_coin_states_by_puzzle_hashes(
        &self, include_spent: bool, puzzle_hashes: &[Bytes32],
        min_height: u64, max_items: usize,
    ) -> Result<Vec<CoinState>, CoinStoreError>;

    // --- Paginated batch with CoinStateFilters (QRY-007) ---
    fn batch_coin_states_by_puzzle_hashes(
        &self, puzzle_hashes: &[Bytes32], min_height: u64,
        filters: CoinStateFilters, max_items: usize,
    ) -> Result<(Vec<CoinState>, Option<u64>), CoinStoreError>;
    // Returns (results, next_height). next_height=None means last page.
    // Enforces MAX_PUZZLE_HASH_BATCH_SIZE (990).
    // Supports include_hinted join, min_amount, deterministic sort, block boundary preservation.

    // --- Singleton lineage (QRY-008) ---
    fn get_unspent_lineage_info_for_puzzle_hash(
        &self, puzzle_hash: &Bytes32,
    ) -> Result<Option<UnspentLineageInfo>, CoinStoreError>;
    // Returns None if != exactly 1 unspent coin matches the puzzle hash.
}
```

### Aggregate Queries

```rust
impl CoinStore {
    fn num_unspent(&self) -> Result<u64, CoinStoreError>;
    fn total_unspent_value(&self) -> Result<u128, CoinStoreError>;
    fn num_total(&self) -> Result<u64, CoinStoreError>;
    fn aggregate_unspent_by_puzzle_hash(
        &self,
    ) -> Result<HashMap<Bytes32, (u64, usize)>, CoinStoreError>;
    // Returns puzzle_hash → (total_amount, coin_count) for all puzzle hashes with unspent coins.
}
```

### Chain State

```rust
impl CoinStore {
    fn height(&self) -> u64;
    fn tip_hash(&self) -> Bytes32;
    fn timestamp(&self) -> u64;
    fn state_root(&mut self) -> Bytes32;  // Recomputes if dirty
    fn is_initialized(&self) -> bool;
    fn is_empty(&self) -> bool;
    fn is_unspent(&self, coin_id: &CoinId) -> bool;  // O(1) HashSet lookup
    fn config(&self) -> &CoinStoreConfig;
    fn stats(&self) -> CoinStoreStats;
}
```

### Hint Store

```rust
impl CoinStore {
    /// Insert hint for a coin. Idempotent (duplicate = no-op).
    fn add_hint(&self, coin_id: &CoinId, hint: &[u8]) -> Result<(), CoinStoreError>;

    /// Coins associated with a 32-byte hint.
    fn get_coin_ids_by_hint(&self, hint: &Bytes32, max_items: usize) -> Result<Vec<CoinId>, CoinStoreError>;

    /// Batch: coins associated with any of the hints.
    fn get_coin_ids_by_hints(&self, hints: &[Bytes32], max_items: usize) -> Result<Vec<CoinId>, CoinStoreError>;

    /// Hints associated with the given coin IDs.
    fn get_hints_for_coin_ids(&self, coin_ids: &[CoinId]) -> Result<HashMap<CoinId, Vec<Bytes32>>, CoinStoreError>;

    /// Total hint count.
    fn count_hints(&self) -> Result<u64, CoinStoreError>;

    /// Remove all hints for given coins (used during rollback).
    fn remove_hints_for_coins(&self, coin_ids: &[CoinId]) -> Result<u64, CoinStoreError>;

    /// Query by variable-length hint (1-32 bytes).
    fn get_coin_ids_by_hint_bytes(&self, hint: &[u8], max_items: usize) -> Result<Vec<CoinId>, CoinStoreError>;
}

/// Standalone hint validation.
pub fn validate_hint(hint: &[u8]) -> Result<HintAction, HintError>;
pub const MAX_HINT_LENGTH: usize = 32;
```

### Snapshot / Restore

```rust
impl CoinStore {
    /// Capture full coinstate snapshot.
    fn snapshot(&self) -> Result<CoinStoreSnapshot, CoinStoreError>;

    /// Replace all state from snapshot. Validates Merkle root.
    fn restore(&mut self, snapshot: CoinStoreSnapshot) -> Result<(), CoinStoreError>;

    /// Persist snapshot to storage, auto-prune old ones.
    fn save_snapshot(&self) -> Result<(), CoinStoreError>;

    /// Load snapshot by height.
    fn load_snapshot(&self, height: u64) -> Result<Option<CoinStoreSnapshot>, CoinStoreError>;

    /// Load the most recent snapshot.
    fn load_latest_snapshot(&self) -> Result<Option<CoinStoreSnapshot>, CoinStoreError>;

    /// Available snapshot heights (ascending).
    fn available_snapshot_heights(&self) -> Vec<u64>;
}
```

---

## Configuration

```rust
let config = CoinStoreConfig::default()           // or CoinStoreConfig::default_with_path("./data")
    .with_backend(StorageBackend::RocksDb)         // or StorageBackend::Lmdb
    .with_storage_path("./data/coinstate")
    .with_max_snapshots(10)                        // auto-prune older
    .with_max_query_results(50_000)                // batch query cap
    .with_lmdb_map_size(10 * 1024 * 1024 * 1024)  // 10 GiB
    .with_rocksdb_write_buffer_size(64 * 1024 * 1024)
    .with_rocksdb_max_open_files(1000)
    .with_bloom_filter(true);                      // 10 bits/key, ~1% FP

let store = CoinStore::with_config(config)?;
```

---

## Storage Schema

12 column families (RocksDB) / named databases (LMDB):

| CF | Key | Value | Purpose |
|----|-----|-------|---------|
| `coin_records` | `coin_id` (32B) | bincode `CoinRecord` | Primary store |
| `coin_by_puzzle_hash` | `puzzle_hash \|\| coin_id` (64B) | `coin_id` | Puzzle hash index |
| `unspent_by_puzzle_hash` | `puzzle_hash \|\| coin_id` (64B) | empty | Unspent-only index |
| `coin_by_parent` | `parent_id \|\| coin_id` (64B) | `coin_id` | Parent index |
| `coin_by_confirmed_height` | `height_BE \|\| coin_id` (40B) | `coin_id` | Creation height index |
| `coin_by_spent_height` | `height_BE \|\| coin_id` (40B) | `coin_id` | Spend height index |
| `hints` | `coin_id \|\| hint` (up to 64B) | empty | Forward hint index |
| `hints_by_value` | `hint \|\| coin_id` (up to 64B) | empty | Reverse hint index |
| `merkle_nodes` | `level(1B) \|\| path(32B)` | `hash` (32B) | Merkle internal nodes |
| `archive_coin_records` | `coin_id` (32B) | bincode `CoinRecord` | Archived spent coins |
| `state_snapshots` | `height_BE` (8B) | bincode `CoinStoreSnapshot` | Checkpoints |
| `metadata` | string key | bytes | Chain tip, counters, config |

All height keys use **big-endian** encoding so lexicographic byte comparison matches numeric order.

---

## Merkle Tree

256-level sparse Merkle tree over all coin records, providing cryptographic state commitment.

- **Leaf hash:** `SHA256(0x00 || coin_record_bytes)` — domain-separated
- **Node hash:** `SHA256(0x01 || left || right)` — domain-separated
- **Empty subtree:** pre-computed for all 257 levels via `OnceLock` (O(1) lookup)
- **Deferred root recomputation:** mutations mark the tree dirty; root computed lazily on `root()` call
- **Proof generation:** `SparseMerkleProof` with 256 sibling hashes for inclusion/exclusion proofs
- **Proof verification:** static computation — no tree state needed, only proof + trusted root

---

## apply_block Pipeline

Phase 1 — **Validation** (no writes):
1. Height continuity: `block.height == current + 1`
2. Parent hash: `block.parent_hash == tip_hash`
3. Reward coins: ≥ 2 coinbase for non-genesis
4. Removals: each exists and is unspent
5. Additions: no duplicates
6. Hints: length ≤ 32 bytes

Phase 2 — **Mutation** (atomic `WriteBatch`):
7. Insert coinbase + addition records with FF-eligible tracking
8. Mark removals as spent
9. Verify `expected_state_root` if provided
10. Store hints in forward + reverse indices
11. Batch update Merkle tree
12. Commit chain tip (height, hash, timestamp)

Phase 3 — **Observability**:
13. Log warning if elapsed > 10 seconds

**Atomicity:** If any validation fails, no mutations occur. If Phase 2 fails, the WriteBatch is not committed.

---

## rollback_to_block Pipeline

1. Delete coins confirmed after target height (all indices)
2. Clean up hints for deleted coins (forward + reverse)
3. Un-spend coins spent after target height (clear `spent_height`, re-add to unspent index)
4. Recompute FF-eligible for un-spent coins (parent EXISTS check)
5. Rebuild Merkle tree (batch remove + update)
6. Atomic commit via WriteBatch

Negative `target_height` triggers full reset to height 0.

---

## Thread Safety

`CoinStore` is `Send + Sync`. The Rust borrow checker enforces shared reads (`&self`) and exclusive writes (`&mut self`) at compile time. For runtime concurrency via `Arc<RwLock<CoinStore>>`, `parking_lot::RwLock` is recommended.

---

## Specification

Full specification: [`docs/resources/SPEC.md`](docs/resources/SPEC.md)
Requirements: [`docs/requirements/IMPLEMENTATION_ORDER.md`](docs/requirements/IMPLEMENTATION_ORDER.md)

81 requirements across 9 phases, verified by 493 tests in 75 test files.
