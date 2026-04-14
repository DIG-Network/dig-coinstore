# dig-coinstore Specification

**Version:** 0.1.0
**Status:** Draft
**Date:** 2026-04-13

## 1. Overview

`dig-coinstore` is a self-contained Rust crate that manages the **global coinstate** for the DIG Network L2 blockchain. It maintains the authoritative database of all spent and unspent coins across the entire blockchain using the coinset model (UTXO-like). The crate accepts validated blocks as input, applies their state transitions (creating new coins, marking spent coins), and provides a rich query API for looking up coins by ID, puzzle hash, hint, parent, and height.

The coinstate **does** perform:
- **Persistent storage** of all coin records (created, spent, and their metadata) using a dual-backend architecture (LMDB for fast point lookups, RocksDB as fallback).
- **Block application** — processing a block's additions (new coins from `CREATE_COIN` conditions) and removals (spent coins) to advance the global state.
- **Block validation** — verifying that a block forms a valid chain extension (correct parent hash, monotonic height, valid state root) before applying it.
- **State root computation** — maintaining a sparse Merkle tree over all coin records, producing a deterministic `state_root` committed in block headers.
- **Rollback** — reverting the last N blocks for chain reorganization recovery.
- **Hint indexing** — storing `CREATE_COIN` hints (typically the recipient's puzzle hash) for wallet-style lookups.
- **Rich queries** — coin lookup by ID, puzzle hash, hint, parent coin ID, creation height, spent height, and batch variants of each.
- **Snapshot/restore** — serializable state for fast sync and backup.

The coinstate does **not** perform:
- **CLVM execution** (puzzle running, condition parsing, signature verification) — this is the caller's responsibility via `dig-clvm`.
- **Block production** (selecting transactions, building generators, aggregating signatures).
- **Mempool management** (transaction ordering, fee estimation, conflict detection) — handled by `dig-mempool`.
- **Networking** (peer discovery, block gossip, sync protocols).
- **Consensus beyond chain validity** (fork choice, finality, validator set management).

The design is derived from Chia's production `CoinStore` ([`chia/full_node/coin_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py)) and `HintStore` ([`chia/full_node/hint_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py)), combined with the working Rust implementation in `l2_driver_state_channel` (`src/coinset/state.rs`, `src/storage/state_store.rs`, `src/storage/rocksdb.rs`), extracted to a standalone crate with clear boundaries.

**Hard boundary:** Inputs = validated `BlockData` (additions, removals, coinbase coins, hints). Outputs = `CoinRecord`s, `CoinState`s, state roots, Merkle proofs. CLVM execution, block production, and networking are outside this crate.

The coinstate operates on the **coinset model** (UTXO-like), where coins are created and destroyed atomically. A coin's identity is `sha256(parent_coin_id || puzzle_hash || amount)`. Coins are immutable once created — they can only be spent (destroyed), never modified. The coinstate tracks every coin that has ever existed on the chain, along with when it was created and when (if ever) it was spent.

### 1.1 Design Principles

- **Chia parity**: The coin record schema, query patterns, and rollback semantics match Chia's `CoinStore` ([`coin_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py)). Any query that works against Chia's coin store should have an equivalent in `dig-coinstore`.
- **Chain validation on insert**: Every block applied to the coinstate is validated for chain continuity (parent hash, height monotonicity, no double-spends, no spending nonexistent coins) before mutations occur. Invalid blocks are rejected atomically.
- **Persistence first**: Unlike Chia's SQLite-based `CoinStore`, `dig-coinstore` uses embedded key-value stores (LMDB primary, RocksDB fallback) optimized for the specific access patterns of coinstate: fast point lookups by coin ID, prefix scans by puzzle hash, and sequential reads by height.
- **Merkle-committed state**: A sparse Merkle tree is maintained over all coin records. The root is recomputed on every block and committed in block headers, enabling light client proofs and state verification.
- **Determinism**: Given the same sequence of blocks, the coinstate produces identical state roots, query results, and rollback behavior. No randomness, no wall-clock dependencies.
- **Batch-optimized**: Block application uses batch operations — all additions and removals are collected, validated together, and applied with a single Merkle root recomputation. This matches the batched approach in `CoinSetState::apply_block_batch()`.
- **Rollback-safe**: The coinstate supports rolling back the last N blocks by deleting coins created after a given height and un-spending coins spent after that height. This mirrors Chia's `CoinStore.rollback_to_block()` ([`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561)).

### 1.2 Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `dig-clvm` | **Types only**. Source of re-exported Chia types: `Coin` (via `chia-protocol`), `Bytes32`, `CoinState`, `SpendBundle`, `Program`. The coinstate does NOT call dig-clvm validation functions. |
| `dig-constants` | Network constants (`NetworkConstants`, `DIG_MAINNET`, `DIG_TESTNET`). Genesis challenge, block cost limits. |
| `chia-protocol` | Core types: `Coin` (+ `::coin_id()`), `Bytes32`, `CoinState`, `CoinStateFilters`, `CoinRecord` (for interop conversions only — see Design Decision 12). |
| `chia-sha2` | SHA-256 implementation (`Sha256` hasher). Used for Merkle leaf hashing (`coin_record_hash()`), Merkle internal node hashing, and any other SHA-256 operations in the crate. Ensures hash compatibility with the Chia ecosystem. Transitively depended on via `chia-protocol`, but declared explicitly for direct usage. |
| `chia-traits` | `Streamable` trait for Chia-canonical binary serialization. Used for wire-format serialization of `CoinState` in sync protocol responses and snapshot interchange. Internal storage continues to use `bincode` (see Design Decision 15). |
| `chia-consensus` | **Dev-dependency only.** Provides `compute_merkle_set_root()` and `MerkleSet` with proof generation/validation. Used in integration tests to cross-check Merkle computations against the Chia reference implementation. NOT used at runtime — the coinstate uses its own sparse Merkle tree (see Design Decision 13). |
| `chia-sdk-test` | **Dev-dependency only.** Provides `Simulator`, an in-memory coin store reference implementation. Used in integration tests as an oracle to verify query result parity with the Chia wallet protocol. |
| `heed` (LMDB) | Primary storage backend. Fast point lookups, ACID transactions, memory-mapped I/O. Feature-gated: `lmdb-storage`. |
| `rocksdb` | Fallback storage backend. Write-optimized LSM tree with bloom filters. Feature-gated: `rocksdb-storage`. |
| `bincode` | Compact binary serialization for coin records and snapshots (internal storage). |
| `serde` | Serialization framework for persistence and snapshot/restore. |
| `parking_lot` | `RwLock` for concurrent read access to in-memory indices. |
| `thiserror` | Error type derivation. |

**Key types used by the coinstate:**

| Type | From Crate | Coinstate Usage |
|------|-----------|----------------|
| `Coin` | chia-protocol | Coin identity. `::coin_id()` computes `sha256(parent \|\| puzzle_hash \|\| amount)`. Fields: `parent_coin_info`, `puzzle_hash`, `amount`. |
| `Bytes32` | chia-protocol | 32-byte hash for coin IDs, puzzle hashes, block hashes, state roots, hints. |
| `CoinState` | chia-protocol | Lightweight coin state for sync protocol: `coin`, `created_height`, `spent_height`. |
| `CoinStateFilters` | chia-protocol | Filter parameters for batch coin state queries: `include_spent`, `include_unspent`, `include_hinted`, `min_amount`. Adopted directly from Chia's protocol messages to ensure wire-level compatibility. |
| `CoinRecord` (chia) | chia-protocol | Chia's native coin record type (`confirmed_block_index: u32`, `spent_block_index: u32`, `coinbase`, `timestamp`). Used for interop conversions only — `dig-coinstore` defines its own `CoinRecord` with extended fields (see Design Decision 12). |
| `Sha256` | chia-sha2 | SHA-256 hasher used for Merkle leaf hashing and internal node hashing. Same implementation used by `Coin::coin_id()` internally. |

### 1.3 Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Dual storage backend (LMDB + RocksDB) | LMDB excels at point lookups (coin by ID); RocksDB at write-heavy workloads. Feature-gated choice. Matches `l2_driver_state_channel` storage architecture. |
| 2 | In-memory Merkle tree + persistent coin records | Merkle tree is rebuilt from persistent data on startup. Avoids storing intermediate nodes. Matches `CoinSetState` design. |
| 3 | `CoinRecord` stores `spent_height: Option<u64>` | `None` = unspent, `Some(h)` = spent at height h. Simpler than Chia's `spent_index` sentinel values (`0` = unspent, `-1` = FF-eligible unspent). Chia: [`coin_store.py:50`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L50). |
| 4 | Separate hint store | Hints are stored in a dedicated index (coin_id -> hint, hint -> coin_ids) matching Chia's `HintStore` ([`hint_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py)). Enables wallet-style lookups without polluting coin records. |
| 5 | Block validation before state mutation | All removals are validated (coin exists, coin unspent) before any mutations occur. Atomic: either the entire block applies or nothing changes. Matches `CoinSetState::apply_block_batch()`. |
| 6 | Rollback deletes + un-spends | Coins created after the rollback height are deleted; coins spent after the rollback height are un-spent. Matches Chia's [`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561). |
| 7 | Height-indexed secondary indices | Coins are indexed by `confirmed_height` and `spent_height` for efficient range queries. Matches Chia's [`coin_confirmed_index`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L60) and [`coin_spent_index`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L63). |
| 8 | `std` only | Full-node infrastructure. No `no_std` support needed. |
| 9 | Snapshot pruning with configurable retention | Old snapshots are automatically pruned to bound disk usage. Default: keep last 10 snapshots. |
| 10 | Puzzle hash index uses composite key | `puzzle_hash + coin_id` as key enables prefix scans for all coins with a given puzzle hash. Matches Chia's [`coin_puzzle_hash`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L66) index. |
| 11 | Parent coin index | Index by `parent_coin_info` for efficient child-coin lookups. Matches Chia's [`coin_parent_index`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L69). |
| 12 | Custom `CoinRecord` instead of `chia-protocol::CoinRecord` | `chia-protocol::CoinRecord` uses `u32` heights and a `spent_block_index: u32` sentinel (0 = unspent). dig-coinstore uses `u64` heights (future-proof beyond 4B blocks), `Option<u64>` for `spent_height` (idiomatic Rust, no sentinel ambiguity), and adds `ff_eligible: bool` (not present in any Chia crate). Interop conversion methods (`from_chia_coin_record()`, `to_chia_coin_record()`) are provided. The `chia-sdk-coinset::CoinRecord` was evaluated but rejected — it is an RPC client response type with a redundant `spent: bool` field and serde JSON annotations unsuitable for storage. |
| 13 | Custom sparse Merkle tree instead of `chia-consensus::MerkleSet` | `chia-consensus::MerkleSet` is a hash-sorted Merkle set designed for block-level coin commitments (recomputed from scratch each time). dig-coinstore needs a **persistent, incremental** sparse Merkle tree keyed by coin_id bit paths (fixed 256-level) that supports batch updates, dirty node tracking, lazy loading, and proof generation without full reconstruction. These are fundamentally different data structures. `chia-consensus` is used as a dev-dependency for cross-checking in tests. |
| 14 | `CoinStateFilters` from `chia-protocol` for batch query parameters | `batch_coin_states_by_puzzle_hashes()` accepts a `CoinStateFilters` struct from `chia-protocol` instead of four separate boolean/integer parameters. This ensures direct wire-level compatibility with Chia's peer protocol messages (`RequestPuzzleState`, `CoinStateFilters`) and reduces API surface. |
| 15 | `chia-sha2` for all SHA-256 operations | All SHA-256 operations (Merkle leaf hashing, internal node hashing) use `chia-sha2::Sha256` to ensure hash compatibility with the Chia ecosystem. `Coin::coin_id()` already uses `chia-sha2` internally. `bincode` remains the serialization format for internal storage; `chia-traits::Streamable` is available for wire-format serialization of protocol types like `CoinState`. |

### 1.4 Chia CoinStore Parity

The following Chia `CoinStore` operations are supported with equivalent semantics:

| Chia CoinStore Method | dig-coinstore Equivalent | Reference |
|----------------------|--------------------------|-----------|
| `new_block()` | `apply_block()` | [`coin_store.py:105-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L105) |
| `get_coin_record()` | `get_coin_record()` | [`coin_store.py:181-193`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L181) |
| `get_coin_records()` | `get_coin_records()` | [`coin_store.py:195-221`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L195) |
| `get_coins_added_at_height()` | `get_coins_added_at_height()` | [`coin_store.py:223-236`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L223) |
| `get_coins_removed_at_height()` | `get_coins_removed_at_height()` | [`coin_store.py:238-254`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L238) |
| `get_coin_records_by_puzzle_hash()` | `get_coin_records_by_puzzle_hash()` | [`coin_store.py:257-278`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L257) |
| `get_coin_records_by_puzzle_hashes()` | `get_coin_records_by_puzzle_hashes()` | [`coin_store.py:280-307`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L280) |
| `get_coin_records_by_names()` | `get_coin_records_by_names()` | [`coin_store.py:309-335`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L309) |
| `get_coin_records_by_parent_ids()` | `get_coin_records_by_parent_ids()` | [`coin_store.py:380-406`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L380) |
| `get_coin_states_by_ids()` | `get_coin_states_by_ids()` | [`coin_store.py:408-442`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L408) |
| `get_coin_states_by_puzzle_hashes()` | `get_coin_states_by_puzzle_hashes()` | [`coin_store.py:347-378`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L347) |
| `batch_coin_states_by_puzzle_hashes()` | `batch_coin_states_by_puzzle_hashes()` | [`coin_store.py:446-559`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L446) |
| `rollback_to_block()` | `rollback_to_block()` | [`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561) |
| `get_unspent_lineage_info_for_puzzle_hash()` | `get_unspent_lineage_info_for_puzzle_hash()` | [`coin_store.py:651-674`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L651) |
| `num_unspent()` | `num_unspent()` | [`coin_store.py:96-103`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L96) |

### 1.5 Chia Behaviors Adopted

The following Chia `CoinStore` and `HintStore` behaviors are explicitly adopted in dig-coinstore because they represent production-hardened patterns:

| # | Chia Behavior | Description | Reference |
|---|---------------|-------------|-----------|
| 1 | Strict spend count assertion | `_set_spent()` counts rows actually updated and fails with `ValueError` if the count doesn't match the expected number of removals. This catches coins that don't exist or are already spent at the storage layer, even after pre-validation. | [`coin_store.py:645-648`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L645) |
| 2 | `_set_spent` WHERE guard | The UPDATE only affects rows WHERE `spent_index <= 0`, so already-spent coins are silently skipped. The rowcount assertion then detects the mismatch. This is a defense-in-depth pattern. | [`coin_store.py:640-641`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L640) |
| 3 | FF-eligible at creation time | When `same_as_parent=True`, the coin is immediately stored with `spent_index=-1` (FF-eligible unspent) rather than `0` (normal unspent). This avoids a post-hoc migration scan. | [`coin_store.py:128-129`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L128) |
| 4 | FF-eligible recomputation on rollback | When un-spending coins during rollback, Chia re-evaluates FF eligibility by checking if the coin's parent has the same `puzzle_hash` and `amount` and is itself spent. This uses an EXISTS subquery with a self-join. | [`coin_store.py:598-623`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L598) |
| 5 | Block boundary-aware pagination | `batch_coin_states_by_puzzle_hashes()` refuses to split blocks across pages. When the result exceeds `max_items`, it pops trailing items that share the same `MAX(confirmed_height, spent_height)` as the overflow item, ensuring the caller can resume cleanly. | [`coin_store.py:541-558`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L541) |
| 6 | Deduplication across direct + hinted results | `batch_coin_states_by_puzzle_hashes()` uses a dict keyed by coin_id to prevent returning the same coin twice when it matches both by puzzle_hash and by hint. | [`coin_store.py:469-509`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L469) |
| 7 | Deterministic sort order for pagination | Results are sorted by `MAX(confirmed_height, spent_height) ASC` so that pagination cursors are stable and deterministic. | [`coin_store.py:497`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L497) |
| 8 | `MAX_PUZZLE_HASH_BATCH_SIZE` limit | Explicit upper bound on the number of puzzle hashes in a single `batch_coin_states_by_puzzle_hashes()` call. Prevents unbounded query expansion. | [`coin_store.py:444`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L444) |
| 9 | `min_amount` filter | Wallet queries can filter out dust coins by specifying a minimum amount. Avoids materializing millions of sub-mojo records. | [`coin_store.py:454`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L454) |
| 10 | Large input batching (`to_batches()`) | Large IN-clause queries are split into batches to avoid database parameter limits. dig-coinstore uses equivalent chunked iteration for KV-store multi-key lookups. | [`coin_store.py:203`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L203) |
| 11 | Height 0 reward coin assertion | Genesis block (height 0) must have 0 reward coins. All other blocks must have >= 2 (farmer + pool reward). | [`coin_store.py:138-141`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L138) |
| 12 | Height 0 special case in `get_coins_removed_at_height` | Returns empty vec immediately for height 0, avoiding a query that would match all unspent coins (since unspent coins have `spent_index=0` in Chia). | [`coin_store.py:240`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L240) |
| 13 | Hint length validation | Hints must be <= 32 bytes. Only 32-byte hints are treated as puzzle hash subscriptions for wallet notification. | [`hint_management.py:44-48`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py#L44) |
| 14 | Idempotent hint insertion | `INSERT OR IGNORE` on the unique `(coin_id, hint)` pair. Duplicate hints are silently ignored. | [`hint_store.py:79-80`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py#L79) |
| 15 | Performance logging | Block application logs a warning when it takes > 10 seconds, with a suggestion to use faster storage. | [`coin_store.py:164-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L164) |

### 1.6 Improvements Over Chia L1

| # | Improvement | Description |
|---|-------------|-------------|
| 1 | Merkle-committed state | Sparse Merkle tree over all coin records with state root in every block header. Chia has no cryptographic commitment over its UTXO set at the coin store layer. Enables light client proofs. |
| 2 | Chain validation on insert | `apply_block()` validates height continuity, parent hash, state root, removal existence, and addition uniqueness before any mutation. Chia's `new_block()` trusts the caller completely. |
| 3 | Merkle proofs | `get_coin_proof()` / `verify_coin_proof()` enable light clients to verify coin existence/absence against a trusted state root. Not available in Chia. |
| 4 | Embedded KV storage | LMDB (memory-mapped, zero-copy reads) and RocksDB (bloom filters, LSM compaction) replace SQLite. Better read latency for point lookups, better write throughput for block application. |
| 5 | Integrated hint store | Hints are stored atomically with block application inside `CoinStore`, not in a separate class. Simplifies rollback (hints are cleaned up in the same transaction). |
| 6 | Snapshot/restore | Built-in `snapshot()` / `restore()` with configurable retention. Chia has no built-in snapshot mechanism in its coin store. |
| 7 | Batch Merkle updates | All additions and removals are collected and applied with a single Merkle root recomputation per block. |
| 8 | Aggregate queries | `aggregate_unspent_by_puzzle_hash()`, `total_unspent_value()` — Chia requires full table scans for equivalent information. |
| 9 | Bloom filter optimization | RocksDB backend uses full bloom (10 bits/key) for point lookups and prefix bloom (32 bytes) for puzzle hash scans. Negative lookups avoid disk reads entirely. |
| 10 | `rollback_n_blocks()` convenience | Callers can specify a block count instead of computing target height. |
| 11 | Enriched rollback result | Returns `coins_deleted`, `coins_unspent` counts alongside the modified records map. |
| 12 | Tiered spent coin archival | Spent coins beyond the rollback window are moved to a minimal-index archive tier. Hot-tier secondary indices remain small regardless of chain age. Chia keeps all coins in one table forever. |
| 13 | In-memory unspent set | `HashSet<CoinId>` for O(1) lock-free "is unspent?" checks. Eliminates disk I/O for the single most frequent query (mempool validation). |
| 14 | LRU coin record cache | Recently accessed `CoinRecord`s served from memory. Absorbs repeat lookups from mempool, wallets, and block production without hitting storage. |
| 15 | Persistent Merkle tree | Internal nodes stored in dedicated CF with incremental persistence. Startup loads only the root — no full tree rebuild. |
| 16 | Unspent-only puzzle hash index | Dedicated `unspent_by_puzzle_hash` CF that is orders of magnitude smaller than the full index. Accelerates the dominant wallet query pattern. |
| 17 | WriteBatch atomic block commits | All writes for a block (coins, indices, hints, Merkle nodes, counters) committed in a single atomic WriteBatch with one WAL fsync. 10-50x faster than individual puts. |
| 18 | Materialized aggregate counters | `unspent_count`, `spent_count`, `total_value` maintained as running counters. O(1) instead of O(N) for aggregate queries. |
| 19 | MVCC reads during writes | Readers see consistent pre-block state while block application is in progress. No read blocking during writes. |
| 20 | Parallel removal validation | Phase 1 validation parallelized across cores using the lock-free in-memory unspent set. |
| 21 | Checkpoint-based fast sync | Verifiable snapshots with Merkle root proof allow new nodes to bootstrap without replaying the entire chain. |
| 22 | Height-partitioned indices | Height-keyed CFs use bucket partitioning so old data settles into cold LSM levels and is never rewritten during compaction. |
| 23 | Per-CF compaction strategy | Level compaction for read-optimized CFs, FIFO for append-mostly CFs. Tuned to each access pattern. |

---

## 2. Data Model

### 2.1 Coin Identity

Coins follow the Chia coinset model:

```
CoinId = sha256(parent_coin_info || puzzle_hash || amount)
```

A `CoinId` is a `Bytes32` (32-byte hash). Throughout this spec, "coin ID" refers to this derived identifier. The coin ID is computed via `Coin::coin_id()` from `chia-protocol`.

`PuzzleHash` is a type alias for `Bytes32`, representing the SHA256 hash of a serialized CLVM puzzle program.

### 2.2 CoinRecord

A `CoinRecord` represents the full lifecycle state of a coin. Once a coin is created, its record persists permanently in the coinstate (even after spending) to support historical queries and rollback.

Corresponds to Chia's `CoinRecord` ([`chia_rs::CoinRecord`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L46-L56)) with the same fields, adapted for Rust Option types.

```rust
/// A record of a coin with its full lifecycle state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinRecord {
    /// The coin (parent_coin_info, puzzle_hash, amount). Immutable once created.
    pub coin: Coin,
    /// Block height when this coin was created (confirmed).
    /// Chia: `confirmed_index` in coin_record table (coin_store.py:49).
    pub confirmed_height: u64,
    /// Block height when this coin was spent. `None` if unspent.
    /// Chia: `spent_index` (coin_store.py:50). Chia uses 0 for unspent, -1 for FF-eligible
    /// unspent; dig-coinstore uses `Option<u64>` for clarity.
    pub spent_height: Option<u64>,
    /// Whether the coin is a coinbase reward (block reward, not a transaction output).
    /// Chia: `coinbase` (coin_store.py:51).
    pub coinbase: bool,
    /// Timestamp of the block that created this coin.
    /// Chia: `timestamp` (coin_store.py:55).
    pub timestamp: u64,
    /// Whether this coin is a potential singleton fast-forward candidate.
    /// Set to `true` when `same_as_parent=true` at creation time (same puzzle_hash
    /// and amount as parent, non-coinbase). Recomputed during rollback.
    /// Chia equivalent: `spent_index = -1` (coin_store.py:128-129).
    pub ff_eligible: bool,
}
```

**Derived methods:**

```rust
impl CoinRecord {
    /// Create a new unspent coin record.
    pub fn new(coin: Coin, confirmed_height: u64, timestamp: u64, coinbase: bool) -> Self;

    /// Check if the coin has been spent.
    pub fn is_spent(&self) -> bool;

    /// Mark the coin as spent at the given height.
    pub fn spend(&mut self, height: u64);

    /// Get the coin ID (derived from coin fields).
    pub fn coin_id(&self) -> CoinId;

    /// Convert to a lightweight CoinState for sync protocol.
    pub fn to_coin_state(&self) -> CoinState;

    /// Convert from a `chia-protocol::CoinRecord` for interop.
    /// `ff_eligible` defaults to `false` (not present in Chia's CoinRecord).
    /// Heights are widened from `u32` to `u64`. `spent_block_index == 0` maps
    /// to `spent_height = None`.
    pub fn from_chia_coin_record(record: chia_protocol::CoinRecord) -> Self;

    /// Convert to a `chia-protocol::CoinRecord` for interop.
    /// Heights are narrowed from `u64` to `u32` (panics if height > u32::MAX).
    /// `spent_height = None` maps to `spent_block_index = 0`.
    /// `ff_eligible` is lost (not representable in Chia's CoinRecord).
    pub fn to_chia_coin_record(&self) -> chia_protocol::CoinRecord;
}
```

### 2.3 CoinState

A lightweight view of a coin's lifecycle, used in the sync protocol. Corresponds to Chia's `CoinState` ([`chia_rs::CoinState`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L340-L345)).

```rust
/// Lightweight coin state for sync protocol responses.
pub struct CoinState {
    pub coin: Coin,
    /// Height at which this coin was created. None if not yet confirmed.
    pub created_height: Option<u32>,
    /// Height at which this coin was spent. None if unspent.
    pub spent_height: Option<u32>,
}
```

### 2.4 BlockData

The input to `apply_block()`. This is not a full block — it is the pre-validated, pre-extracted set of state changes that the caller has already derived from CLVM execution. The coinstate does not run CLVM; it trusts the caller to provide correct additions and removals.

```rust
/// Pre-validated block state changes for coinstate application.
pub struct BlockData {
    /// Block height (must be exactly `current_height + 1`).
    pub height: u64,
    /// Block timestamp (unix seconds).
    pub timestamp: u64,
    /// Hash of this block's header (for chain tracking).
    pub block_hash: Bytes32,
    /// Hash of the parent block's header (must match current chain tip).
    pub parent_hash: Bytes32,
    /// Coins created by transactions in this block.
    /// Each entry: (coin_id, coin, is_same_puzzle_as_parent).
    /// Chia: `tx_additions` parameter in coin_store.py:121.
    pub additions: Vec<CoinAddition>,
    /// Coin IDs destroyed (spent) by transactions in this block.
    /// Chia: `tx_removals` parameter in coin_store.py:122.
    pub removals: Vec<CoinId>,
    /// Coinbase reward coins created at this height.
    /// Chia: `included_reward_coins` parameter in coin_store.py:110.
    pub coinbase_coins: Vec<Coin>,
    /// Hints extracted from CREATE_COIN conditions (coin_id -> hint bytes).
    /// Stored in the hint index for wallet-style lookups.
    /// Chia: processed by hint_management.py and stored in HintStore.
    pub hints: Vec<(CoinId, Bytes32)>,
    /// Expected state root after applying this block.
    /// If provided, the coinstate verifies its computed root matches. If it does not
    /// match, the block is rejected.
    pub expected_state_root: Option<Bytes32>,
}

/// A coin addition with metadata.
pub struct CoinAddition {
    /// The computed coin ID.
    pub coin_id: CoinId,
    /// The coin being created.
    pub coin: Coin,
    /// Whether this coin has the same puzzle hash and amount as its parent.
    /// Used for singleton fast-forward unspent tracking.
    /// Chia: `same_as_parent` flag in tx_additions (coin_store.py:121).
    pub same_as_parent: bool,
}
```

### 2.5 UnspentLineageInfo

Tracks singleton lineage for fast-forward optimization. Matches Chia's `UnspentLineageInfo` ([`mempool_item.py:18-22`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/types/mempool_item.py#L18)), used by the mempool for singleton fast-forward during block production.

```rust
/// Lineage information for the most recent unspent singleton coin
/// matching a puzzle hash. Used by the mempool for singleton fast-forward.
pub struct UnspentLineageInfo {
    /// The current unspent singleton coin ID.
    pub coin_id: Bytes32,
    /// The parent of the current singleton.
    pub parent_id: Bytes32,
    /// The grandparent (parent's parent).
    pub parent_parent_id: Bytes32,
}
```

### 2.6 CoinStoreConfig

Configuration for the coinstate store. Builder pattern with `with_*` methods.

```rust
pub struct CoinStoreConfig {
    /// Storage backend to use.
    /// Default: LMDB if `lmdb-storage` feature is enabled, else RocksDB.
    pub backend: StorageBackend,

    /// Path to the storage directory.
    pub storage_path: PathBuf,

    /// Maximum number of state snapshots to retain.
    /// Default: 10. Older snapshots are pruned automatically.
    pub max_snapshots: usize,

    /// Maximum number of results for batch queries.
    /// Default: 50,000. Matches Chia's `max_items` parameter
    /// (coin_store.py:355, coin_store.py:415).
    pub max_query_results: usize,

    /// LMDB map size (maximum database size).
    /// Default: 10 GB.
    pub lmdb_map_size: usize,

    /// RocksDB write buffer size.
    /// Default: 64 MB.
    pub rocksdb_write_buffer_size: usize,

    /// RocksDB max open files.
    /// Default: 1000.
    pub rocksdb_max_open_files: i32,

    /// Enable bloom filters for point lookups (RocksDB only).
    /// Default: true. 10 bits per key (~1% false positive rate).
    pub bloom_filter: bool,
}

pub enum StorageBackend {
    /// LMDB — optimized for read-heavy, fast point lookups.
    Lmdb,
    /// RocksDB — optimized for write-heavy workloads with bloom filters.
    RocksDb,
}
```

### 2.7 Constants

```rust
/// Maximum query results per batch (matches Chia's default max_items).
/// Chia: `max_items=50000` in coin_store.py:355, coin_store.py:415.
pub const DEFAULT_MAX_QUERY_RESULTS: usize = 50_000;

/// Maximum number of puzzle hashes in a single batch_coin_states_by_puzzle_hashes() call.
/// Chia: `MAX_PUZZLE_HASH_BATCH_SIZE = SQLITE_MAX_VARIABLE_NUMBER - 10` (coin_store.py:444).
/// We use a comparable limit to prevent unbounded query expansion.
pub const MAX_PUZZLE_HASH_BATCH_SIZE: usize = 990;

/// Maximum length of a hint in bytes.
/// Chia: hint_management.py:44 `assert len(hint) <= 32`.
/// Only 32-byte hints are eligible for puzzle-hash subscription matching.
pub const MAX_HINT_LENGTH: usize = 32;

/// Default number of snapshots to retain.
pub const DEFAULT_MAX_SNAPSHOTS: usize = 10;

/// Default LMDB map size (10 GB).
pub const DEFAULT_LMDB_MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;

/// Bloom filter bits per key for RocksDB (10 bits = ~1% false positive rate).
pub const BLOOM_FILTER_BITS: i32 = 10;

/// Default batch size for chunking large multi-key lookups.
/// Analogous to Chia's `to_batches(names, SQLITE_MAX_VARIABLE_NUMBER)` pattern
/// (coin_store.py:203). KV stores don't have SQL parameter limits, but
/// batching prevents unbounded memory allocation for intermediate results.
pub const DEFAULT_LOOKUP_BATCH_SIZE: usize = 999;

/// Block application performance warning threshold in seconds.
/// Chia: coin_store.py:165 `took_too_long = end - start > 10`.
pub const BLOCK_APPLY_WARN_SECONDS: f64 = 10.0;

/// Minimum reward coins per non-genesis block.
/// Chia: coin_store.py:141 `assert len(included_reward_coins) >= 2`.
pub const MIN_REWARD_COINS_PER_BLOCK: usize = 2;

// -- Performance & scalability (Section 14) --

/// Default LRU cache capacity for CoinRecord lookups.
/// At ~200 bytes per entry, 1M entries ≈ 200 MB.
pub const DEFAULT_COIN_CACHE_CAPACITY: usize = 1_000_000;

/// Rollback window: blocks beyond this depth are eligible for archive/pruning.
/// Coins spent more than this many blocks ago can be moved to the archive tier.
pub const DEFAULT_ROLLBACK_WINDOW: u64 = 1_000;

/// Batch size for materialization during snapshot restore.
/// Loading coins in chunks prevents OOM on large coin sets.
/// Matches l2_driver_state_channel pattern (50K-coin batches).
pub const MATERIALIZATION_BATCH_SIZE: usize = 50_000;

/// RocksDB prefix extractor length for puzzle hash column families.
/// First 32 bytes of composite key (puzzle_hash portion) used for prefix bloom.
pub const PUZZLE_HASH_PREFIX_LENGTH: usize = 32;
```

---

## 3. Public API

### 3.1 Construction

```rust
impl CoinStore {
    /// Create a new coinstate store with default configuration.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CoinStoreError>;

    /// Create a coinstate store with custom configuration.
    pub fn with_config(config: CoinStoreConfig) -> Result<Self, CoinStoreError>;

    /// Initialize genesis state with initial coins.
    /// Called once when bootstrapping a new chain.
    pub fn init_genesis(
        &mut self,
        initial_coins: Vec<(Coin, bool)>,  // (coin, is_coinbase)
        timestamp: u64,
    ) -> Result<Bytes32, CoinStoreError>;  // returns genesis state root
}
```

### 3.2 Block Application

The primary state transition. Corresponds to Chia's `CoinStore.new_block()` ([`coin_store.py:105-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L105)).

```rust
/// Result of applying a block to the coinstate.
pub struct ApplyBlockResult {
    /// The new state root after applying this block.
    pub state_root: Bytes32,
    /// Number of coins created (additions + coinbase).
    pub coins_created: usize,
    /// Number of coins spent (removals).
    pub coins_spent: usize,
    /// New chain tip height.
    pub height: u64,
}

impl CoinStore {
    /// Apply a block's state changes to the coinstate.
    ///
    /// # Validation performed
    ///
    /// Before any state mutation, the following checks are performed atomically:
    ///
    /// 1. **Height continuity**: `block.height == self.height() + 1`.
    /// 2. **Parent hash**: `block.parent_hash == self.tip_hash()`.
    /// 3. **Removal validity**: Every coin in `removals` must exist and be unspent.
    ///    - Coin not found → `CoinNotFound`.
    ///    - Coin already spent → `DoubleSpend`.
    /// 4. **No duplicate additions**: No coin in `additions` already exists.
    /// 5. **State root verification**: If `expected_state_root` is provided, the computed
    ///    root must match. Mismatch → `StateRootMismatch`.
    ///
    /// # Mutation order
    ///
    /// Matches Chia's CoinStore.new_block() (coin_store.py:105-178):
    /// 1. Insert all addition coin records (tx + coinbase).
    /// 2. Mark all removal coins as spent at this height.
    /// 3. Store hints in the hint index.
    /// 4. Batch-update the Merkle tree (single root recomputation).
    /// 5. Update chain tip metadata (height, hash, timestamp).
    ///
    /// If any validation check fails, no state is mutated (atomic).
    pub fn apply_block(&mut self, block: BlockData) -> Result<ApplyBlockResult, CoinStoreError>;
}
```

### 3.3 Rollback

Corresponds to Chia's `CoinStore.rollback_to_block()` ([`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561)).

```rust
/// Result of a rollback operation.
pub struct RollbackResult {
    /// Coin records that were modified during rollback.
    /// Includes both deleted coins (created after target) and
    /// un-spent coins (spent after target).
    /// Chia returns this as `dict[bytes32, CoinRecord]` (coin_store.py:567).
    pub modified_coins: HashMap<CoinId, CoinRecord>,
    /// Number of coins deleted (created after rollback height).
    pub coins_deleted: usize,
    /// Number of coins un-spent (spent after rollback height).
    pub coins_unspent: usize,
    /// New chain tip height after rollback.
    pub new_height: u64,
}

impl CoinStore {
    /// Roll back the coinstate to a target block height.
    ///
    /// All coins created after `target_height` are deleted.
    /// All coins spent after `target_height` are marked as unspent.
    /// The chain tip is updated to `target_height`.
    ///
    /// `target_height` can be negative (via i64), in which case everything
    /// is rolled back (empty coinstate). Matches Chia's behavior where
    /// `block_index` can be -1 (coin_store.py:562).
    ///
    /// # Chia parity
    ///
    /// Chia's rollback also handles the FF-eligible unspent marker
    /// (spent_index = -1 for coins whose parent has same puzzle_hash/amount
    /// and is spent). dig-coinstore tracks this via the `same_as_parent`
    /// flag in CoinAddition.
    ///
    /// Reference: coin_store.py:561-624.
    pub fn rollback_to_block(
        &mut self,
        target_height: i64,
    ) -> Result<RollbackResult, CoinStoreError>;

    /// Roll back exactly `n` blocks from the current tip.
    /// Convenience wrapper: `rollback_to_block(self.height() - n)`.
    pub fn rollback_n_blocks(&mut self, n: u64) -> Result<RollbackResult, CoinStoreError>;
}
```

### 3.4 Coin Queries by ID

```rust
impl CoinStore {
    /// Get a single coin record by its coin ID.
    /// Returns None if the coin has never existed.
    /// Chia: coin_store.py:181-193.
    pub fn get_coin_record(&self, coin_id: &CoinId) -> Result<Option<CoinRecord>, CoinStoreError>;

    /// Get multiple coin records by their IDs (batch).
    /// Returns records in arbitrary order. Missing IDs are silently skipped.
    /// Chia: coin_store.py:195-221.
    pub fn get_coin_records(
        &self,
        coin_ids: &[CoinId],
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    /// Get coin records by their IDs, filtering by spent status.
    /// Chia: coin_store.py:309-335.
    pub fn get_coin_records_by_names(
        &self,
        include_spent: bool,
        names: &[CoinId],
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;
}
```

### 3.5 Coin Queries by Puzzle Hash

```rust
impl CoinStore {
    /// Get coin records matching a puzzle hash.
    /// Chia: coin_store.py:257-278.
    pub fn get_coin_records_by_puzzle_hash(
        &self,
        include_spent: bool,
        puzzle_hash: &Bytes32,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    /// Get coin records matching any of the given puzzle hashes (batch).
    /// Chia: coin_store.py:280-307.
    pub fn get_coin_records_by_puzzle_hashes(
        &self,
        include_spent: bool,
        puzzle_hashes: &[Bytes32],
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    /// Get lightweight CoinStates for a set of puzzle hashes.
    /// Used by the sync/wallet protocol.
    /// Chia: coin_store.py:347-378.
    pub fn get_coin_states_by_puzzle_hashes(
        &self,
        include_spent: bool,
        puzzle_hashes: &[Bytes32],
        min_height: u64,
        max_items: usize,
    ) -> Result<Vec<CoinState>, CoinStoreError>;

    /// Paginated coin state query by puzzle hashes with full Chia parity.
    ///
    /// Returns `(results, next_height)` where `next_height` is `None` if all matching
    /// coins have been returned, or `Some(h)` to resume from height `h` in the next call.
    ///
    /// # Chia behaviors adopted
    ///
    /// - **Input size limit**: `puzzle_hashes.len()` must not exceed `MAX_PUZZLE_HASH_BATCH_SIZE`.
    ///   Chia: coin_store.py:464.
    /// - **`min_amount` filter**: Filters out coins below the specified amount, preventing
    ///   dust from bloating results. Chia: coin_store.py:454.
    /// - **`include_hinted` join**: When true, a second pass queries the hint index for coins
    ///   whose hints match the puzzle hashes, even if the coins have different puzzle hashes.
    ///   Results are deduplicated by coin_id (dict-keyed). Chia: coin_store.py:511-531.
    /// - **Deterministic sort**: Results are ordered by `MAX(confirmed_height, spent_height) ASC`
    ///   for stable pagination. Chia: coin_store.py:497.
    /// - **Block boundary preservation**: When results exceed `max_items`, trailing items that
    ///   share the same `MAX(confirmed_height, spent_height)` as the overflow item are removed.
    ///   This prevents blocks from being split across pages, ensuring the caller can resume
    ///   cleanly at `next_height` without missing or duplicating coins.
    ///   Chia: coin_store.py:549-558.
    /// - **Fetch max_items + 1**: Internally fetches one extra item to detect whether there are
    ///   more results. If exactly `max_items` are returned, the query is complete.
    ///   Chia: coin_store.py:498.
    ///
    /// Chia: coin_store.py:446-559.
    ///
    /// The `filters` parameter uses `chia_protocol::CoinStateFilters` directly,
    /// matching the Chia peer protocol wire format for `RequestPuzzleState`.
    /// This ensures callers can pass Chia protocol messages through without
    /// field-by-field decomposition.
    pub fn batch_coin_states_by_puzzle_hashes(
        &self,
        puzzle_hashes: &[Bytes32],
        min_height: u64,
        filters: CoinStateFilters,
        max_items: usize,
    ) -> Result<(Vec<CoinState>, Option<u64>), CoinStoreError>;
}
```

### 3.6 Coin Queries by Height

```rust
impl CoinStore {
    /// Get all coins created (confirmed) at a specific height.
    /// Chia: coin_store.py:223-236.
    pub fn get_coins_added_at_height(
        &self,
        height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;

    /// Get all coins spent (removed) at a specific height.
    /// Returns empty vec for height 0 (matches Chia's special case, coin_store.py:240).
    /// Chia: coin_store.py:238-254.
    pub fn get_coins_removed_at_height(
        &self,
        height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;
}
```

### 3.7 Coin Queries by Parent

```rust
impl CoinStore {
    /// Get coin records whose parent_coin_info matches one of the given IDs.
    /// Chia: coin_store.py:380-406.
    pub fn get_coin_records_by_parent_ids(
        &self,
        include_spent: bool,
        parent_ids: &[CoinId],
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError>;
}
```

### 3.8 Coin Queries by ID (CoinState)

```rust
impl CoinStore {
    /// Get lightweight CoinStates for a collection of coin IDs.
    /// Used by the sync/wallet protocol.
    /// Chia: coin_store.py:408-442.
    pub fn get_coin_states_by_ids(
        &self,
        include_spent: bool,
        coin_ids: &[CoinId],
        min_height: u64,
        max_height: u64,
        max_items: usize,
    ) -> Result<Vec<CoinState>, CoinStoreError>;
}
```

### 3.9 Hint Queries

Hints are typically the intended recipient's puzzle hash, embedded in `CREATE_COIN` conditions. The hint store enables wallet-style coin discovery without scanning all coins.

Corresponds to Chia's `HintStore` ([`hint_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py)).

**Hint validation rules** (adopted from Chia's [`hint_management.py:44-48`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py#L44)):
- Hints must be > 0 bytes and <= `MAX_HINT_LENGTH` (32 bytes). Empty hints are silently skipped.
- Only 32-byte hints are eligible for puzzle-hash subscription matching in `batch_coin_states_by_puzzle_hashes()`.
- Duplicate `(coin_id, hint)` pairs are idempotent (silently ignored on re-insert). Chia: `INSERT OR IGNORE` ([`hint_store.py:79`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py#L79)).

```rust
impl CoinStore {
    /// Get all coin IDs that have a specific hint.
    /// Chia: hint_store.py:34-39.
    pub fn get_coin_ids_by_hint(
        &self,
        hint: &Bytes32,
        max_items: usize,
    ) -> Result<Vec<CoinId>, CoinStoreError>;

    /// Get coin IDs matching any of the given hints (batch).
    /// Chia: hint_store.py:41-56.
    pub fn get_coin_ids_by_hints(
        &self,
        hints: &[Bytes32],
        max_items: usize,
    ) -> Result<Vec<CoinId>, CoinStoreError>;

    /// Get the hints associated with specific coin IDs.
    /// Chia: hint_store.py:58-72.
    pub fn get_hints_for_coin_ids(
        &self,
        coin_ids: &[CoinId],
    ) -> Result<Vec<Bytes32>, CoinStoreError>;

    /// Get the total number of hints stored.
    /// Chia: hint_store.py:85-93.
    pub fn count_hints(&self) -> Result<u64, CoinStoreError>;
}
```

### 3.10 Singleton Fast-Forward Support

```rust
impl CoinStore {
    /// Look up the most recent unspent singleton lineage matching a puzzle hash.
    /// Used by the mempool for singleton fast-forward admission.
    ///
    /// Returns lineage info only when exactly one unspent coin matches the puzzle
    /// hash AND its parent has the same puzzle_hash/amount AND the parent is spent.
    ///
    /// Chia: coin_store.py:651-674. Uses the `coin_record_ph_ff_unspent_idx`
    /// partial index for performance.
    pub fn get_unspent_lineage_info_for_puzzle_hash(
        &self,
        puzzle_hash: &Bytes32,
    ) -> Result<Option<UnspentLineageInfo>, CoinStoreError>;
}
```

### 3.11 Aggregate Queries

```rust
impl CoinStore {
    /// Count the total number of unspent coins.
    /// Chia: coin_store.py:96-103.
    pub fn num_unspent(&self) -> Result<u64, CoinStoreError>;

    /// Count the total number of coin records (spent + unspent).
    pub fn num_total(&self) -> Result<u64, CoinStoreError>;

    /// Get the total value locked across all unspent coins.
    pub fn total_unspent_value(&self) -> Result<u64, CoinStoreError>;

    /// Aggregate unspent balances grouped by puzzle hash.
    /// Returns map of puzzle_hash -> (total_amount, coin_count).
    pub fn aggregate_unspent_by_puzzle_hash(
        &self,
    ) -> Result<HashMap<Bytes32, (u64, usize)>, CoinStoreError>;

    /// Check if the coin store is empty.
    /// Chia: coin_store.py:676-683.
    pub fn is_empty(&self) -> Result<bool, CoinStoreError>;
}
```

### 3.12 Chain State

```rust
impl CoinStore {
    /// Get the current chain tip height.
    pub fn height(&self) -> u64;

    /// Get the current chain tip block hash.
    pub fn tip_hash(&self) -> Bytes32;

    /// Get the current state root (Merkle root of all coin records).
    pub fn state_root(&self) -> Bytes32;

    /// Get the current timestamp.
    pub fn timestamp(&self) -> u64;

    /// Get comprehensive statistics about the coinstate.
    pub fn stats(&self) -> CoinStoreStats;
}

pub struct CoinStoreStats {
    /// Current chain tip height.
    pub height: u64,
    /// Current timestamp.
    pub timestamp: u64,
    /// Number of unspent coins.
    pub unspent_count: u64,
    /// Number of spent coins (in history).
    pub spent_count: u64,
    /// Total value across all unspent coins.
    pub total_unspent_value: u64,
    /// Current state root.
    pub state_root: Bytes32,
    /// Current chain tip hash.
    pub tip_hash: Bytes32,
    /// Number of hints stored.
    pub hint_count: u64,
    /// Number of snapshots stored.
    pub snapshot_count: usize,
}
```

### 3.13 Merkle Proofs

```rust
impl CoinStore {
    /// Get a Merkle proof for a specific coin.
    /// Can be verified against the state root by a light client.
    pub fn get_coin_proof(&self, coin_id: &CoinId) -> Result<SparseMerkleProof, CoinStoreError>;

    /// Verify a Merkle proof against an expected state root.
    pub fn verify_coin_proof(proof: &SparseMerkleProof, expected_root: &Bytes32) -> bool;
}
```

### 3.14 Persistence (Snapshot/Restore)

```rust
impl CoinStore {
    /// Take a full snapshot of the current coinstate for backup or fast sync.
    /// The snapshot includes all coin records, hints, Merkle tree state,
    /// and chain metadata.
    pub fn snapshot(&self) -> Result<CoinStoreSnapshot, CoinStoreError>;

    /// Restore coinstate from a snapshot. Replaces all current state.
    pub fn restore(&mut self, snapshot: CoinStoreSnapshot) -> Result<(), CoinStoreError>;

    /// Save a snapshot to persistent storage (keyed by height).
    /// Automatically prunes old snapshots beyond `max_snapshots`.
    pub fn save_snapshot(&self) -> Result<(), CoinStoreError>;

    /// Load a snapshot from persistent storage by height.
    pub fn load_snapshot(&self, height: u64) -> Result<Option<CoinStoreSnapshot>, CoinStoreError>;

    /// Load the most recent snapshot from persistent storage.
    pub fn load_latest_snapshot(&self) -> Result<Option<CoinStoreSnapshot>, CoinStoreError>;

    /// Get available snapshot heights.
    pub fn available_snapshot_heights(&self) -> Vec<u64>;
}

#[derive(Serialize, Deserialize)]
pub struct CoinStoreSnapshot {
    /// Chain tip height at snapshot time.
    pub height: u64,
    /// Chain tip block hash.
    pub block_hash: Bytes32,
    /// State root at snapshot time.
    pub state_root: Bytes32,
    /// Timestamp at snapshot time.
    pub timestamp: u64,
    /// All coin records.
    pub coins: HashMap<CoinId, CoinRecord>,
    /// All hints.
    pub hints: Vec<(CoinId, Bytes32)>,
    /// Total coins.
    pub total_coins: u64,
    /// Total unspent value.
    pub total_value: u64,
}
```

### 3.15 Maintenance

```rust
impl CoinStore {
    /// Flush pending writes to disk.
    pub fn flush(&self) -> Result<(), CoinStoreError>;

    /// Compact the underlying storage (RocksDB only; no-op for LMDB).
    pub fn compact(&self) -> Result<(), CoinStoreError>;

    /// Prune data before a given height (snapshots only, not coin records).
    pub fn prune_snapshots_before(&self, height: u64) -> Result<usize, CoinStoreError>;
}
```

---

## 4. Error Types

```rust
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CoinStoreError {
    // -- Chain continuity --
    #[error("block height {got} does not follow current height {expected}")]
    HeightMismatch { expected: u64, got: u64 },

    #[error("parent hash mismatch: expected {expected}, got {got}")]
    ParentHashMismatch { expected: Bytes32, got: Bytes32 },

    #[error("state root mismatch: expected {expected}, computed {computed}")]
    StateRootMismatch { expected: Bytes32, computed: Bytes32 },

    // -- Coin existence --
    #[error("coin not found: {0}")]
    CoinNotFound(CoinId),

    #[error("coin already exists: {0}")]
    CoinAlreadyExists(CoinId),

    // -- Spend validity --
    #[error("double spend: coin {0} already spent")]
    DoubleSpend(CoinId),

    #[error("spend count mismatch: expected {expected} updates, got {actual}")]
    SpendCountMismatch { expected: usize, actual: usize },

    // -- Rollback --
    #[error("cannot rollback: target height {target} above current height {current}")]
    RollbackAboveTip { target: i64, current: u64 },

    // -- Storage --
    #[error("storage error: {0}")]
    StorageError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("deserialization error: {0}")]
    DeserializationError(String),

    // -- Block structure --
    #[error("invalid reward coin count: expected {expected}, got {got}")]
    InvalidRewardCoinCount { expected: String, got: usize },

    #[error("hint too long: {length} bytes exceeds maximum {max}")]
    HintTooLong { length: usize, max: usize },

    // -- Genesis --
    #[error("genesis already initialized")]
    GenesisAlreadyInitialized,

    #[error("coinstate not initialized (call init_genesis first)")]
    NotInitialized,

    // -- Query --
    #[error("puzzle hash batch size {size} exceeds maximum {max}")]
    PuzzleHashBatchTooLarge { size: usize, max: usize },
}
```

---

## 5. Block Application Pipeline

### 5.1 Overview

The block application pipeline is the core state transition of the coinstate. It takes a `BlockData` (pre-extracted state changes from a validated block) and atomically applies all additions and removals. This corresponds to Chia's `CoinStore.new_block()` ([`coin_store.py:105-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L105)).

```
apply_block(block: BlockData)
   │
   ├─ Phase 1: Validation (read-only)
   │   ├─ 1. Height continuity check
   │   ├─ 2. Parent hash check
   │   ├─ 3. Reward coin count assertion (Chia: coin_store.py:138-141)
   │   ├─ 4. Validate all removals exist and are unspent
   │   ├─ 5. Validate no duplicate additions
   │   └─ 6. Validate hint lengths (<= MAX_HINT_LENGTH)
   │
   ├─ Phase 2: Mutation (atomic)
   │   ├─ 7. Insert addition coin records (FF-eligible tracking via same_as_parent)
   │   ├─ 8. Insert coinbase coin records
   │   ├─ 9. Mark removal coins as spent (with strict count assertion)
   │   ├─ 10. Store hints in hint index (idempotent, skip empty)
   │   ├─ 11. Batch-update Merkle tree (single root recomputation)
   │   ├─ 12. Verify state root (if expected_state_root provided)
   │   └─ 13. Update chain tip metadata
   │
   └─ Phase 3: Observability
       └─ 14. Log warning if block application exceeded BLOCK_APPLY_WARN_SECONDS
```

### 5.2 Height Continuity (Phase 1)

The block height must be exactly `current_height + 1`. This ensures blocks are applied in strict sequential order. Any gap or regression is rejected.

```rust
if block.height != self.height() + 1 {
    return Err(CoinStoreError::HeightMismatch {
        expected: self.height() + 1,
        got: block.height,
    });
}
```

### 5.3 Parent Hash Verification (Phase 1)

The block's `parent_hash` must match the hash of the current chain tip block. This ensures the block forms a valid chain extension.

```rust
if block.parent_hash != self.tip_hash() {
    return Err(CoinStoreError::ParentHashMismatch {
        expected: self.tip_hash(),
        got: block.parent_hash,
    });
}
```

For genesis (height 0), the parent hash is the zero hash.

### 5.4 Reward Coin Count Assertion (Phase 1)

Adopted from Chia ([`coin_store.py:138-141`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L138)):

```rust
if block.height == 0 {
    assert!(block.coinbase_coins.is_empty());  // Genesis has no rewards
} else {
    assert!(block.coinbase_coins.len() >= MIN_REWARD_COINS_PER_BLOCK);  // >= 2 (farmer + pool)
}
```

This catches malformed blocks before any state mutation.

### 5.5 Removal Validation (Phase 1)

Every coin ID in `removals` must exist in the coinstate and be currently unspent. This is validated **before** any mutations occur, ensuring atomicity.

Chia performs the equivalent check in `_set_spent()` ([`coin_store.py:627-648`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L627)) where `rows_updated != len(coin_names)` raises `ValueError`.

```rust
for coin_id in &block.removals {
    let record = self.get_coin_record(coin_id)?
        .ok_or(CoinStoreError::CoinNotFound(*coin_id))?;
    if record.is_spent() {
        return Err(CoinStoreError::DoubleSpend(*coin_id));
    }
}
```

### 5.6 Addition Validation (Phase 1)

No coin in `additions` should already exist in the coinstate. While coin ID collisions are cryptographically unlikely (would require a SHA256 collision), this check catches logic errors.

### 5.7 Hint Length Validation (Phase 1)

All hints in `block.hints` are validated:
- Hints with length 0 are silently skipped (not stored).
- Hints with length > `MAX_HINT_LENGTH` (32 bytes) are rejected.

Adopted from Chia's [`hint_management.py:44-48`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py#L44).

### 5.8 Coin Insertion with FF-Eligible Tracking (Phase 2)

Transaction additions and coinbase coins are inserted as new `CoinRecord` entries. Each record is stored with:
- `confirmed_height = block.height`
- `spent_height = None` (unspent)
- `timestamp = block.timestamp`
- `coinbase = false` for transaction outputs, `true` for coinbase rewards

**FF-eligible tracking at creation time** (adopted from Chia [`coin_store.py:128-129`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L128)):

When `CoinAddition.same_as_parent = true`, the coin is immediately flagged as a potential singleton fast-forward candidate. In Chia, this is stored as `spent_index = -1` (vs `0` for normal unspent). dig-coinstore stores a `ff_eligible: bool` flag on the `CoinRecord`. This avoids a post-hoc migration scan to identify FF-eligible coins.

The `same_as_parent` check is performed by the caller (who has access to the parent coin during CLVM execution) and means: this coin has the same `puzzle_hash` and `amount` as its parent, and is not a coinbase reward. This pattern is characteristic of singleton spends.

Chia inserts all records in a single `executemany` ([`coin_store.py:161`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L161)). dig-coinstore batches the write similarly.

### 5.9 Spend Marking with Strict Count Assertion (Phase 2)

All removal coins are marked as spent at the current block height.

**Strict count assertion** (adopted from Chia [`coin_store.py:627-648`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L627)):

The spend operation uses a two-layer safety pattern:

1. **WHERE guard**: Only updates records where `spent_height` is `None` (unspent). Already-spent coins are silently skipped. Chia: `WHERE spent_index <= 0` ([`coin_store.py:640-641`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L640)).

2. **Post-mutation count assertion**: After all spend updates, the number of rows actually modified is compared against the expected number of removals. If they don't match, something went wrong (coin didn't exist, was already spent, or was silently skipped by the WHERE guard). This is a hard failure. Chia: `if rows_updated != len(coin_names): raise ValueError(...)` ([`coin_store.py:645-648`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L645)).

```rust
let rows_updated = self.storage.mark_coins_spent(&block.removals, block.height)?;
if rows_updated != block.removals.len() {
    return Err(CoinStoreError::SpendCountMismatch {
        expected: block.removals.len(),
        actual: rows_updated,
    });
}
```

This defense-in-depth approach catches edge cases that Phase 1 validation might miss due to TOCTOU races in concurrent environments.

**Batching for large removal sets** (adopted from Chia [`coin_store.py:635`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L635)): Large removal sets are processed in chunks of `DEFAULT_LOOKUP_BATCH_SIZE` to bound intermediate memory allocation.

### 5.10 Hint Storage (Phase 2)

Hints from `block.hints` are stored in the hint index. Each hint is a `(coin_id, hint_bytes)` pair. Duplicate `(coin_id, hint)` pairs are ignored (idempotent insert).

Chia stores hints via `HintStore.add_hints()` ([`hint_store.py:74-83`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py#L74)) using `INSERT OR IGNORE` for idempotent upserts. dig-coinstore replicates this: duplicate `(coin_id, hint)` pairs are silently skipped.

Hint extraction from conditions is done by `get_hints_and_subscription_coin_ids()` in [`hint_management.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py) — this is outside the coinstate's scope (the caller extracts hints from CLVM conditions and passes them in `BlockData.hints`).

### 5.11 Merkle Tree Update (Phase 2)

All coin record changes (new coins + updated spent coins) are collected into a batch and applied to the sparse Merkle tree with a single root recomputation. This matches the approach in `CoinSetState::apply_block_batch()`:

```rust
// Collect all merkle tree updates
let merkle_updates: Vec<(CoinId, Bytes32)> = /* all new + updated records */;
self.merkle_tree.batch_insert(merkle_updates);
```

### 5.12 State Root Verification (Phase 2)

If `BlockData.expected_state_root` is provided, the computed state root is compared against it. A mismatch indicates the block's claimed state root is inconsistent with the actual state changes, and the block is rejected.

### 5.13 Chain Tip Update (Phase 2)

The chain tip metadata is updated:
- `height = block.height`
- `tip_hash = block.block_hash`
- `timestamp = block.timestamp`

### 5.14 Performance Logging (Phase 3)

Adopted from Chia ([`coin_store.py:164-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L164)):

Block application is timed from start to finish. If the elapsed time exceeds `BLOCK_APPLY_WARN_SECONDS` (10 seconds), a warning is logged:

```
WARN: Height {height}: block application took {elapsed:.2f}s
      ({additions} additions, {removals} removals).
      Ensure coinstate database is on fast storage.
```

Normal application logs at DEBUG level with the same format. This early warning helps operators identify storage bottlenecks before they cause consensus delays.

---

## 6. Rollback Pipeline

### 6.1 Overview

Rollback reverts the coinstate to a previous block height. This is used for chain reorganization (reorg) recovery. The algorithm matches Chia's `CoinStore.rollback_to_block()` ([`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561)).

```
rollback_to_block(target_height)
   │
   ├─ 1. Collect coins confirmed after target_height
   │     → These will be deleted. Add to modified_coins map.
   │     Chia: coin_store.py:570-580
   │
   ├─ 2. Delete those coin records from storage
   │     Chia: "DELETE FROM coin_record WHERE confirmed_index > ?"
   │     (coin_store.py:583)
   │
   ├─ 3. Collect coins spent after target_height
   │     → These will be un-spent. Add to modified_coins map
   │     (only if not already present from step 1).
   │     Chia: coin_store.py:586-596
   │
   ├─ 4. Un-spend those coins with FF-eligible recomputation
   │     For each coin spent after target_height:
   │       IF coinbase = false
   │          AND EXISTS parent with same puzzle_hash AND same amount
   │          AND parent.spent_height > 0 (parent is spent)
   │       THEN mark as FF-eligible unspent
   │       ELSE mark as normal unspent
   │     Chia: coin_store.py:602-623 (UPDATE with CASE/EXISTS)
   │
   ├─ 5. Remove associated hints for deleted coins
   │
   ├─ 6. Rebuild Merkle tree for affected range
   │
   └─ 7. Update chain tip to target_height
```

### 6.2 Chia Rollback Parity

Chia's rollback ([`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561)) performs three SQL operations:

1. **Collect + Delete** coins confirmed after the target height ([`coin_store.py:570-583`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L570)). The pre-deletion records are saved into the modified_coins map so the caller can update caches.

2. **Collect + Un-spend** coins spent after the target height ([`coin_store.py:586-596`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L586)). These are added to modified_coins only if they weren't already captured in step 1 (a coin could be both created and spent after the target height — step 1's DELETE handles that case).

3. **FF-eligible recomputation** ([`coin_store.py:598-623`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L598)) — This is the most sophisticated part of the rollback. When un-spending coins, Chia re-evaluates whether each coin is a potential singleton fast-forward candidate using a self-join:

```sql
UPDATE coin_record INDEXED BY coin_spent_index
SET spent_index = CASE
    WHEN
        coinbase = 0 AND
        EXISTS (
            SELECT 1
            FROM coin_record AS parent
            WHERE parent.coin_name = coin_record.coin_parent
              AND parent.puzzle_hash = coin_record.puzzle_hash
              AND parent.amount = coin_record.amount
              AND parent.spent_index > 0
        )
    THEN -1    -- FF-eligible unspent
    ELSE 0     -- Normal unspent
END
WHERE spent_index > ?  -- All coins spent after target height
```

dig-coinstore replicates this exact logic: for each coin being un-spent, look up its parent coin record. If the parent exists, has the same puzzle_hash, the same amount, is not a coinbase reward, and the parent itself is still spent (not also being un-spent in this rollback), then mark the child as FF-eligible. Otherwise, mark as normal unspent.

This ensures that after a rollback, the FF-eligible index is consistent — singletons that still have a valid spent-parent lineage remain queryable via `get_unspent_lineage_info_for_puzzle_hash()`.

### 6.3 Return Value

The rollback returns a map of all modified coin records (both deleted and un-spent), matching Chia's return type of `dict[bytes32, CoinRecord]` ([`coin_store.py:567`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L567)). This allows the caller to update any in-memory caches.

---

## 7. Storage Architecture

### 7.1 Overview

The storage layer uses a dual-backend architecture inherited from `l2_driver_state_channel`, optimized for the coinstate's specific access patterns.

| Access Pattern | Primary Backend | Index/Structure |
|---------------|-----------------|-----------------|
| Coin by ID (point lookup) | LMDB / RocksDB | Primary key (`coin_name`). LRU-cached. |
| Is coin unspent? (boolean) | **In-memory** | `HashSet<CoinId>` unspent set (see 14.2) |
| Coins by puzzle hash (prefix scan) | LMDB / RocksDB | Composite key (`puzzle_hash + coin_id`). Prefix bloom. |
| Unspent coins by puzzle hash | RocksDB | **Unspent-only** index (see 14.5) |
| Coins by parent ID | RocksDB | `coin_parent` index |
| Coins by confirmed height | RocksDB | `confirmed_height` index |
| Coins by spent height | RocksDB | `spent_height` index |
| Hints by hint value | LMDB / RocksDB | `hint -> coin_id` index |
| Hints by coin ID | LMDB / RocksDB | `coin_id -> hint` table |
| Merkle internal nodes | RocksDB | Persistent Merkle tree (see 14.4) |
| Archived spent coins | RocksDB | Archive tier (see 14.1) |
| State snapshots | RocksDB | Height-keyed |
| Chain metadata | RocksDB | String-keyed, includes materialized counters |

### 7.2 Database Layout (RocksDB Column Families)

Matches the column family structure from `l2_driver_state_channel/src/storage/rocksdb.rs`:

| Column Family | Key | Value | Bloom | Compaction | Purpose |
|--------------|-----|-------|-------|------------|---------|
| `coin_records` | `coin_id` (32 bytes) | Serialized `CoinRecord` | Yes (10 bits) | Level | Primary coin storage |
| `coin_by_puzzle_hash` | `puzzle_hash + coin_id` (64 bytes) | `coin_id` (32 bytes) | Prefix (32 bytes) | Level | All coins by puzzle hash |
| `unspent_by_puzzle_hash` | `puzzle_hash + coin_id` (64 bytes) | `()` | Prefix (32 bytes) | Level | **Unspent-only** puzzle hash index (Section 14.5) |
| `coin_by_parent` | `parent_coin_info + coin_id` (64 bytes) | `coin_id` (32 bytes) | Yes (10 bits) | Level | Parent ID lookups |
| `coin_by_confirmed_height` | `height (8 bytes BE) + coin_id` (40 bytes) | `coin_id` (32 bytes) | No | FIFO | Height-range queries, rollback |
| `coin_by_spent_height` | `height (8 bytes BE) + coin_id` (40 bytes) | `coin_id` (32 bytes) | No | FIFO | Height-range queries, rollback |
| `hints` | `coin_id + hint` (64 bytes) | `()` | Yes (10 bits) | Level | Hint by coin ID |
| `hints_by_value` | `hint + coin_id` (64 bytes) | `()` | Prefix (32 bytes) | Level | Coin IDs by hint |
| `merkle_nodes` | `level (1 byte) + path_prefix (32 bytes)` | `hash` (32 bytes) | Yes (10 bits) | Level | Persistent Merkle tree internals (Section 14.4) |
| `archive_coin_records` | `coin_id` (32 bytes) | Serialized `CoinRecord` | Yes (10 bits) | Level | Archived spent coins (Section 14.1) |
| `state_snapshots` | `height` (8 bytes BE) | Serialized snapshot | No | FIFO | Snapshot storage |
| `metadata` | String key | Bytes value | Yes (10 bits) | Level | Chain tip, config, materialized counters |

### 7.3 Database Layout (LMDB)

When using the LMDB backend (feature `lmdb-storage`), the same logical structure is used across LMDB named databases:

| Database | Key | Value |
|----------|-----|-------|
| `coins` | `coin_id` (32 bytes) | Serialized `CoinRecord` |
| `coins_by_ph` | `puzzle_hash + coin_id` (64 bytes) | `()` |
| `hints` | `coin_id + hint` (64 bytes) | `()` |
| `hints_by_value` | `hint + coin_id` (64 bytes) | `()` |
| `snapshots` | `height` (8 bytes BE) | Serialized snapshot |
| `metadata` | String key | Bytes value |

### 7.4 Bloom Filters and Compaction (RocksDB)

**Full bloom filters** (10 bits per key, ~1% false positive rate) are used for point-lookup-heavy column families: `coin_records`, `coin_by_parent`, `hints`, `merkle_nodes`, `archive_coin_records`, `metadata`. These avoid unnecessary disk reads for non-existent keys.

**Prefix bloom filters** are used for prefix-scan-heavy column families: `coin_by_puzzle_hash`, `unspent_by_puzzle_hash`, `hints_by_value`. The prefix extractor uses the first `PUZZLE_HASH_PREFIX_LENGTH` (32) bytes of the composite key. This makes "does this puzzle hash have any coins?" nearly free — the bloom filter rejects non-matching prefixes without reading any SST blocks.

**No bloom filters** for append-mostly column families: `state_snapshots`, `coin_by_confirmed_height`, `coin_by_spent_height`. These are accessed by range scan, not point lookup.

**Compaction strategy per CF:**
- **Level compaction** for read-optimized CFs (`coin_records`, all indices, `merkle_nodes`). Optimizes point lookup and short range scan latency.
- **FIFO compaction** for append-mostly CFs (`coin_by_confirmed_height`, `coin_by_spent_height`, `state_snapshots`). Old data naturally ages out to higher levels without rewrite amplification.

**Write buffer and cache allocation:**
- `coin_records` gets the largest write buffer (64 MB) and block cache share since it's the hottest CF.
- `unspent_by_puzzle_hash` gets the second-largest allocation since it's the primary wallet query path.
- L0 filter and index blocks are pinned in the block cache to avoid eviction under memory pressure.

### 7.5 Chia Comparison

Chia uses SQLite with the following schema ([`coin_store.py:46-56`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L46)):

```sql
CREATE TABLE coin_record(
    coin_name blob PRIMARY KEY,
    confirmed_index bigint,
    spent_index bigint,
    coinbase int,
    puzzle_hash blob,
    coin_parent blob,
    amount blob,
    timestamp bigint
)
```

With indices on `confirmed_index`, `spent_index`, `puzzle_hash`, and `coin_parent` ([`coin_store.py:59-69`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L59)).

dig-coinstore uses embedded key-value stores instead of SQLite for:
- **Better write throughput**: LSM tree (RocksDB) and B+ tree (LMDB) handle high write volumes better than SQLite WAL.
- **No SQL parsing overhead**: Direct key-value operations avoid query compilation.
- **Memory-mapped reads (LMDB)**: Zero-copy reads for point lookups.
- **Bloom filters (RocksDB)**: Negative lookups (coin doesn't exist) are nearly free.

---

## 8. Hint Store

### 8.1 Overview

The hint store is a secondary index that maps hints (typically recipient puzzle hashes) to the coins that carry them. Hints are extracted from `CREATE_COIN` conditions during block processing by the caller and passed to the coinstate in `BlockData.hints`.

This corresponds to Chia's `HintStore` ([`hint_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py)).

### 8.2 Schema

Chia's hint store uses a single table ([`hint_store.py:29`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py#L29)):

```sql
CREATE TABLE hints(coin_id blob, hint blob, UNIQUE (coin_id, hint))
CREATE INDEX hint_index on hints(hint)
```

dig-coinstore uses two key-value indices for bidirectional lookup:
- **Forward index** (`coin_id + hint -> ()`): Given a coin, find its hints.
- **Reverse index** (`hint + coin_id -> ()`): Given a hint, find all coins with that hint.

### 8.3 Integration with batch_coin_states_by_puzzle_hashes

Chia's `batch_coin_states_by_puzzle_hashes()` ([`coin_store.py:446-559`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L446)) supports an `include_hinted` parameter. When true, it performs a second query joining `coin_record` with `hints` to include coins whose hints match the queried puzzle hashes, even if the coins themselves have different puzzle hashes.

dig-coinstore replicates this behavior:
1. **Direct query**: Find coins whose `puzzle_hash` matches any of the input hashes.
2. **Hinted query** (if `include_hinted`): Query the reverse hint index for coin IDs whose hints match the input hashes, then look up those coin records.
3. **Deduplication**: Results are merged into a `HashMap<CoinId, CoinState>` (keyed by coin_id) to prevent returning the same coin twice. Chia: [`coin_store.py:469-509`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L469).
4. **Sort + trim**: When `include_hinted` is true, the merged results are sorted by `MAX(confirmed_height, spent_height) ASC` and trimmed to `max_items + 1`. Chia: [`coin_store.py:535-538`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L535).

### 8.4 Hint-Aware Subscription Pattern

Chia's [`hint_management.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py) extracts hints from block state changes and determines which coin IDs need wallet notifications. This is outside the coinstate's scope, but the coinstate provides the building blocks:

- `get_coin_ids_by_hint()` — the wallet layer calls this with its subscribed puzzle hashes to discover coins hinted to it.
- Only 32-byte hints are eligible for puzzle-hash subscription matching ([`hint_management.py:44-45`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py#L44)). Shorter hints are stored but not used for subscription notifications.

### 8.5 Rollback Hint Cleanup

When a rollback deletes coins created after the target height, all associated hints for those coin IDs are also deleted. This maintains referential integrity between the coin record store and the hint store. Chia does not explicitly clean up hints on rollback (orphaned hints remain in the SQLite table), but dig-coinstore keeps the stores consistent because hints are integrated into the same atomic write path.

---

## 9. Merkle Tree

### 9.1 State Root

The coinstate maintains a sparse Merkle tree over all coin records. Each coin record is hashed and inserted at the position determined by its `coin_id`. The root of this tree is the `state_root` committed in every block header.

All hashing operations in the Merkle tree (leaf hashing, internal node hashing) use `chia_sha2::Sha256` to ensure hash compatibility with the Chia ecosystem. This is the same SHA-256 implementation used internally by `Coin::coin_id()`.

```rust
use chia_sha2::Sha256;

fn coin_record_hash(record: &CoinRecord) -> Bytes32 {
    let serialized = bincode::encode_to_vec(record, /* config */).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&serialized);
    Bytes32::from(hasher.finalize())
}
```

### 9.2 Batch Updates

Individual coin insertions/updates trigger a full path recomputation. For block application (which may touch hundreds or thousands of coins), all changes are collected and applied with a single root recomputation:

```rust
self.merkle_tree.batch_insert(updates);  // Single root computation
```

This matches `CoinSetState::apply_block_batch()` from `l2_driver_state_channel`.

### 9.3 Proofs

Light clients can request a Merkle proof for any coin and verify it against the `state_root` committed in a block header they trust:

```rust
let proof = store.get_coin_proof(&coin_id)?;
assert!(CoinStore::verify_coin_proof(&proof, &trusted_state_root));
```

---

## 10. Internal Data Structures

### 10.1 In-Memory State

The in-memory coinstate maintains several structures for fast access:

```
// -- Hot path (always in memory) --
unspent_set:     HashSet<CoinId>                 // O(1) "is unspent?" check (~32 bytes/coin)
coin_cache:      LruCache<CoinId, CoinRecord>    // Recently accessed records (Section 14.3)
merkle_tree:     SparseMerkleTree                // State root computation (dirty nodes tracked)

// -- Materialized counters (updated atomically per block) --
unspent_count:   u64                             // Running count of unspent coins
spent_count:     u64                             // Running count of spent coins
total_value:     u64                             // Running sum of unspent coin amounts

// -- Chain state --
height:          u64
tip_hash:        Bytes32
timestamp:       u64
```

The `unspent_set` is the single most important in-memory structure. At 10M unspent coins it consumes ~320 MB but eliminates all disk I/O for the most frequent query pattern (mempool validation). It is maintained incrementally: insert on coin creation, remove on coin spend, re-insert on rollback un-spend.

### 10.2 Persistent Indices

See Section 7 for the full database layout. The key indices mirror Chia's SQLite indices:

| dig-coinstore Index | Chia SQLite Index | Purpose |
|--------------------|-------------------|---------|
| `coin_records` (primary key) | `sqlite_autoindex_coin_record_1` | Coin by ID lookup |
| `coin_by_puzzle_hash` | `coin_puzzle_hash` ([`coin_store.py:66`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L66)) | Puzzle hash queries |
| `coin_by_parent` | `coin_parent_index` ([`coin_store.py:69`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L69)) | Parent ID queries |
| `coin_by_confirmed_height` | `coin_confirmed_index` ([`coin_store.py:60`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L60)) | Added-at-height queries, rollback |
| `coin_by_spent_height` | `coin_spent_index` ([`coin_store.py:63`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L63)) | Removed-at-height queries, rollback |
| `hints_by_value` | `hint_index` ([`hint_store.py:31`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_store.py#L31)) | Hint-to-coin lookups |

### 10.3 FF-Eligible Unspent Tracking

Chia uses a partial index for fast-forward singleton queries ([`coin_store.py:80-86`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L80)):

```sql
CREATE INDEX coin_record_ph_ff_unspent_idx
    ON coin_record(puzzle_hash, spent_index)
    WHERE spent_index = -1
```

dig-coinstore implements equivalent tracking using the `same_as_parent` flag stored per coin record, with a dedicated query path in `get_unspent_lineage_info_for_puzzle_hash()`.

---

## 11. Concurrency

### 11.1 Thread Safety

`CoinStore` is `Send + Sync`. Read operations can execute concurrently with each other and with writes (MVCC). Write operations (block application, rollback) are serialized.

| Operation | Lock | Contention |
|-----------|------|------------|
| `apply_block` Phase 1 (validation) | Shared read | Concurrent with other reads |
| `apply_block` Phase 2 (mutation) | Exclusive write | Sequential block processing |
| `apply_block` Phase 2 (WriteBatch) | Storage write | Single atomic commit |
| `rollback_to_block` | Exclusive write | Rare (reorg only) |
| `get_coin_record` | **Lock-free** (LRU cache hit) | No contention |
| `get_coin_record` | Shared read (cache miss) | Concurrent |
| `is_unspent` | **Lock-free** (in-memory HashSet) | No contention |
| `get_coin_records_by_*` | Shared read | Concurrent |
| `get_coin_states_by_*` | Shared read | Concurrent |
| `hint queries` | Shared read | Concurrent |
| `stats` / `height` / `state_root` | **Lock-free** (atomic counters) | No contention |
| `snapshot` | Shared read (RocksDB snapshot) | Concurrent |
| `restore` | Exclusive write | Rare (startup only) |

### 11.2 Storage-Level Concurrency

- **LMDB**: Uses MVCC (multi-version concurrency control). Readers never block writers; writers never block readers. One write transaction at a time. Readers see a consistent snapshot of the pre-write state while a write is in progress.
- **RocksDB**: Supports concurrent reads via immutable snapshots. During `apply_block` Phase 2, a `WriteBatch` is prepared without holding the read lock. Readers continue seeing the pre-block state. The batch is committed atomically, and then the in-memory state (unspent set, counters, cache, tip) is swapped. See Section 14.8 for details.

### 11.3 MVCC Read Isolation During Block Application

Readers should never see a partially-applied block. The block application pipeline achieves this by preparing all mutations into a `WriteBatch` (RocksDB) or write transaction (LMDB) without updating the in-memory query state. Only after the storage commit succeeds are the in-memory structures (unspent set, LRU cache, counters, tip) updated atomically via pointer swap or lock acquisition. This means:

- Readers during Phase 2 see the **previous block's state** (consistent, complete).
- Readers after Phase 2 see the **new block's state** (consistent, complete).
- No reader ever sees a state where some coins are spent but their replacements aren't yet created.

---

## 12. Compatibility Notes

### 12.1 Chia CoinStore Compatibility

All Chia `CoinStoreProtocol` methods ([`coin_store_protocol.py`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/consensus/coin_store_protocol.py)) have direct equivalents in dig-coinstore. The query semantics (filtering by height range, include_spent, max_items) match Chia's behavior.

### 12.2 DIG L2 vs Chia L1

| Aspect | Chia L1 | DIG L2 |
|--------|---------|--------|
| Storage engine | SQLite | LMDB / RocksDB (configurable) |
| Coin identity | `sha256(parent \|\| puzzle_hash \|\| amount)` | Same |
| Unspent tracking | `spent_index = 0` | `spent_height = None` |
| FF-eligible unspent | `spent_index = -1` | `ff_eligible: bool` on CoinRecord |
| FF-eligible at creation | **Yes** (`spent_index = -1` on insert) | **Yes** (`ff_eligible = true` on insert) |
| FF-eligible rollback recomputation | **Yes** (EXISTS subquery) | **Yes** (equivalent parent check) |
| State commitment | Not Merkle-committed (no state root in headers) | Sparse Merkle tree, root in every block header |
| Merkle proofs | Not available | Available via `get_coin_proof()` |
| Chain validation on insert | **None** (trusts caller) | Height, parent hash, state root, removals, additions |
| Strict spend count assertion | **Yes** (`rows_updated != len`) | **Yes** (same pattern) |
| Hint store | Separate `HintStore` class | Integrated into `CoinStore` |
| Hint rollback cleanup | **Not cleaned up** (orphaned hints remain) | **Cleaned up** atomically with coin deletion |
| Batch pagination | **Block-boundary-aware** (won't split blocks) | **Same** (adopted) |
| `min_amount` filter | **Yes** | **Yes** (adopted) |
| `MAX_PUZZLE_HASH_BATCH_SIZE` | **Yes** (`SQLITE_MAX_VAR - 10`) | **Yes** (990, adopted) |
| Deterministic sort for pagination | **Yes** (`ORDER BY MAX(confirmed, spent)`) | **Yes** (adopted) |
| Dedup across direct + hinted | **Yes** (dict-keyed by coin_id) | **Yes** (adopted) |
| Large input batching | **Yes** (`to_batches()`) | **Yes** (`DEFAULT_LOOKUP_BATCH_SIZE`) |
| Performance logging | **Yes** (warn > 10s) | **Yes** (adopted) |
| Reward coin assertion | **Yes** (0 at genesis, >= 2 otherwise) | **Yes** (adopted) |
| Query limits | `SQLITE_MAX_VARIABLE_NUMBER` batching | Configurable `max_query_results` |
| Rollback return | `dict[bytes32, CoinRecord]` | `RollbackResult` struct with same data + counts |
| Block application | `new_block()` (separate SQL ops) | `apply_block()` (batch Merkle update) |
| Snapshot/restore | Not built-in | Built-in with configurable retention |
| Aggregate queries | Not built-in | `aggregate_unspent_by_puzzle_hash()`, `total_unspent_value()` |
| Bloom filters | Not applicable (SQLite) | RocksDB: 10 bits/key for point lookups |
| Concurrency | async/await with SQLite | `RwLock` with LMDB MVCC or RocksDB locks |
| SHA-256 implementation | Python `hashlib` | `chia-sha2::Sha256` (same crate used by `chia-protocol`) |
| CoinRecord type | Python dataclass | Custom Rust struct with `from_chia_coin_record()`/`to_chia_coin_record()` interop |
| Batch query filters | Separate parameters | `chia_protocol::CoinStateFilters` struct (wire-compatible) |
| CoinState serialization | Python Streamable | `chia-protocol::CoinState` with `chia-traits::Streamable` for wire format |

### 12.3 Chia Crate Utilization

The following table summarizes how `dig-coinstore` uses each Chia ecosystem crate, and why certain crate features are used or intentionally not used:

| Chia Crate | What We Use | What We Don't Use (and Why) |
|-----------|-------------|----------------------------|
| `chia-protocol` | `Coin`, `Bytes32`, `CoinState`, `CoinStateFilters`, `CoinRecord` (interop only) | `SpendBundle`, `CoinSpend`, `Program` — coinstore doesn't execute CLVM or handle transactions |
| `chia-sha2` | `Sha256` for Merkle leaf/node hashing | — (full crate used) |
| `chia-traits` | `Streamable` for `CoinState` wire serialization | Not used for internal storage (bincode is more compact for KV storage) |
| `chia-consensus` (dev) | `compute_merkle_set_root()`, `MerkleSet` for test cross-checks | NOT used at runtime — its `MerkleSet` is hash-sorted (recomputed from scratch), incompatible with our persistent sparse Merkle tree |
| `chia-sdk-test` (dev) | `Simulator` as oracle for query parity tests | Not used at runtime |
| `dig-clvm` | Type re-exports (`Coin`, `Bytes32`, `CoinState`) | Validation functions — coinstore receives pre-validated data |

### 12.4 Crate Boundary

`dig-coinstore` is a **library crate** (`lib`). It is strictly a **coinstate manager**: inputs are pre-validated block state changes, outputs are coin records and state roots. It does **not** include (all are outside this crate and handled by other DIG or Chia crates):

- **CLVM execution** (puzzle running, condition parsing). The caller extracts additions, removals, and hints from CLVM conditions before calling `apply_block()`.
- **Block production** (transaction selection, generator building). Handled by `dig-mempool` + block producer.
- **Block validation** (CLVM block generator execution, signature verification). The caller validates the block before extracting `BlockData`.
- **Mempool management**. Handled by `dig-mempool`, which queries the coinstate for `CoinRecord`s during transaction validation.
- **Networking** (peer discovery, block sync, gossip).
- **Consensus rules beyond chain continuity** (fork choice, finality, validator management).

The coinstate's contract is:
- **Input**: `BlockData` (pre-validated state changes) via `apply_block()`
- **Output**: `CoinRecord`s, `CoinState`s, `Bytes32` state roots, `SparseMerkleProof`s via query methods

---

## 13. Performance and Scalability

### 13.1 Tiered Coin Storage (Spent Coin Archival)

**Problem:** Both Chia and `l2_driver_state_channel` keep every spent coin forever. Chia's own code acknowledges the "huge coin records table" ([`coin_store.py:77`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L77)). At 100M+ total coins, secondary indices (puzzle hash, parent, height) become enormous and every prefix scan degrades.

**Solution:** Three-tier retention model:

| Tier | Contents | Indexing | Storage |
|------|----------|----------|---------|
| **Hot** | All unspent coins + coins spent within `DEFAULT_ROLLBACK_WINDOW` (last 1000 blocks) | Full indexing: by ID, puzzle hash, parent, height, hint. Unspent-only index. In-memory unspent set. | `coin_records` CF + all secondary CFs |
| **Archive** | Coins spent beyond the rollback window | Minimal: by coin ID only. No puzzle hash, parent, or height indices. | `archive_coin_records` CF |
| **Pruned** | Discarded entirely (operator opt-in) | None | Not stored |

**Archive migration** runs as a background task after each block:
1. Scan `coin_by_spent_height` for coins spent at `height - DEFAULT_ROLLBACK_WINDOW`.
2. Move each coin record from `coin_records` → `archive_coin_records`.
3. Delete secondary index entries (puzzle hash, parent, height indices) for the moved coin.
4. The LRU cache evicts archived coins naturally.

**Query behavior:**
- `get_coin_record(id)` checks hot tier first, then archive. Transparent to caller.
- `get_coin_records_by_puzzle_hash()` only searches the hot tier (fast).
- A separate `get_archived_coin_record(id)` method is available for explicit archive queries.

**Configuration:**

```rust
pub struct ArchiveConfig {
    /// Enable tiered storage. Default: true.
    pub enabled: bool,
    /// Blocks before spent coins are archived. Default: DEFAULT_ROLLBACK_WINDOW.
    pub archive_after_blocks: u64,
    /// Enable pruning (discard archived coins). Default: false.
    /// WARNING: Pruned coins cannot be recovered. Only for nodes that
    /// don't need historical queries.
    pub prune_archived: bool,
}
```

### 13.2 In-Memory Unspent Set

**Problem:** The most frequent coinstate query is "is this coin unspent?" — called for every coin in every `SpendBundle` submitted to the mempool. Hitting disk for this is unnecessary.

**Solution:** Maintain a `HashSet<CoinId>` of all unspent coin IDs in memory.

```rust
unspent_set: HashSet<CoinId>
```

**Memory cost:** 32 bytes per coin ID + HashMap overhead ≈ 40-50 bytes per coin. At 10M unspent coins = ~400-500 MB. At 1M unspent coins = ~40-50 MB.

**Maintenance:**
- `apply_block()`: Insert each addition's coin_id. Remove each removal's coin_id.
- `rollback_to_block()`: Reverse the above (remove additions, re-insert removals).
- Startup: Populated by scanning the `coin_records` CF for records where `spent_height = None`. Done in `MATERIALIZATION_BATCH_SIZE` chunks to bound memory during loading.

**Query acceleration:**

```rust
impl CoinStore {
    /// O(1) unspent check. No disk I/O.
    pub fn is_unspent(&self, coin_id: &CoinId) -> bool {
        self.unspent_set.contains(coin_id)
    }
}
```

This replaces the read-lock + KV-lookup path for the hottest query.

### 13.3 LRU Coin Record Cache

**Problem:** Point lookups for `CoinRecord` dominate the read workload. Recently accessed coins (by the mempool, by wallets, by the block producer) are very likely to be accessed again.

**Solution:** An LRU cache in front of the KV store.

```rust
coin_cache: LruCache<CoinId, CoinRecord>  // capacity: DEFAULT_COIN_CACHE_CAPACITY
```

**Behavior:**
- `get_coin_record(id)`: Check cache first. On hit, return immediately (no lock, no disk). On miss, read from KV store and populate cache.
- `apply_block()`: Write-through — new coins are inserted into both the cache and the KV store. Spent coins are updated in the cache (spent_height set).
- `rollback_to_block()`: Invalidate all cache entries for affected coins (conservative — let them repopulate on demand).
- Cache is **not** included in snapshots (ephemeral, rebuilt from KV store).

**Memory cost:** ~200 bytes per cached CoinRecord. At 1M entries = ~200 MB.

### 13.4 Persistent Merkle Tree

**Problem:** The current `SparseMerkleTree` in `CoinSetState` is fully in-memory and must be rebuilt from all coin records on startup. At 10M+ coins this takes seconds to minutes. The Merkle tree also consumes significant memory (intermediate nodes for a 256-level tree).

**Solution:** Store Merkle internal nodes in a dedicated column family (`merkle_nodes`), with incremental persistence.

**Design:**
- **Dirty node tracking:** During `apply_block()`, batch Merkle updates mark affected internal nodes as "dirty." Only dirty nodes are flushed to the `merkle_nodes` CF in the same `WriteBatch` as coin record updates.
- **Lazy loading:** On startup, only the root node is read. Internal nodes are loaded on demand during proof generation or state root computation. The LRU cache covers frequently-accessed subtrees.
- **Memoized empty hashes:** Pre-computed 257-level empty hash array (matching `l2_driver_state_channel/src/utils/merkle.rs` pattern) avoids recomputing empty subtrees. Stored as a `OnceLock<[Bytes32; 257]>`.

**Key format in `merkle_nodes` CF:**
```
Key:   level (1 byte, 0=root, 255=leaf parent) || path_prefix (up to 32 bytes)
Value: hash (32 bytes)
```

**Startup cost:** O(1) — read root hash from metadata, verify against stored tip. Full tree is demand-loaded.

### 13.5 Unspent-Only Puzzle Hash Index

**Problem:** The `coin_by_puzzle_hash` index contains all coins (spent and unspent), but the dominant query pattern is "give me unspent coins for this puzzle hash" (wallet sync, balance queries). For a popular puzzle hash (exchange hot wallet) with millions of historical coins, scanning the full index and filtering out spent coins is wasteful.

**Solution:** A dedicated `unspent_by_puzzle_hash` column family that only contains currently-unspent coin IDs.

**Maintenance cost per block:**
- For each addition: `put(puzzle_hash + coin_id, ())`
- For each removal: `delete(puzzle_hash + coin_id)`
- On rollback: reverse (delete additions, re-insert removals)

This is a small constant cost per coin per block — the same cost as the existing full index, applied to a much smaller dataset.

**Query acceleration:** `get_coin_records_by_puzzle_hash(include_spent=false, ...)` scans `unspent_by_puzzle_hash` instead of `coin_by_puzzle_hash`. The unspent index is orders of magnitude smaller for long-running chains.

**The full `coin_by_puzzle_hash` index is retained** for `include_spent=true` queries and archive-tier historical lookups.

### 13.6 WriteBatch for Atomic Block Application

**Problem:** The existing RocksDB code in `l2_driver_state_channel/src/storage/rocksdb.rs` does individual `put_cf()` calls. A block with 1000 additions generates 1000+ individual writes across multiple column families (coin records, puzzle hash index, parent index, height index, hints, Merkle nodes). Each write may trigger a WAL fsync.

**Solution:** Accumulate all writes for a single block into a `rocksdb::WriteBatch` and commit atomically.

```rust
fn apply_block_storage(&self, block: &BlockData, ...) -> Result<()> {
    let mut batch = WriteBatch::default();

    // Additions: coin_records + all secondary indices
    for addition in &block.additions {
        batch.put_cf(&coin_records_cf, &addition.coin_id, &serialize(&record));
        batch.put_cf(&coin_by_ph_cf, &ph_key, &addition.coin_id);
        batch.put_cf(&unspent_by_ph_cf, &ph_key, &[]);
        batch.put_cf(&coin_by_parent_cf, &parent_key, &addition.coin_id);
        batch.put_cf(&coin_by_height_cf, &height_key, &addition.coin_id);
    }

    // Removals: update coin_records + remove from unspent index
    for removal in &block.removals {
        batch.put_cf(&coin_records_cf, &removal, &serialize(&updated_record));
        batch.delete_cf(&unspent_by_ph_cf, &ph_key);
        batch.put_cf(&coin_by_spent_height_cf, &spent_height_key, &removal);
    }

    // Hints
    for (coin_id, hint) in &block.hints { ... }

    // Merkle dirty nodes
    for (key, hash) in &dirty_merkle_nodes { ... }

    // Materialized counters
    batch.put_cf(&metadata_cf, "unspent_count", &new_count.to_le_bytes());
    batch.put_cf(&metadata_cf, "total_value", &new_value.to_le_bytes());

    // Single atomic commit — one WAL fsync for the entire block
    self.db.write(batch)?;
    Ok(())
}
```

**Impact:** Reduces WAL fsyncs from O(coins_per_block) to O(1). Typical improvement: 10-50x faster block application for large blocks.

### 13.7 Materialized Aggregate Counters

**Problem:** `num_unspent()`, `total_unspent_value()`, and `aggregate_unspent_by_puzzle_hash()` currently require full table scans. At 10M+ coins these are seconds-long operations.

**Solution:** Maintain running counters updated atomically during block application.

```rust
// Updated in the same WriteBatch as block application
unspent_count += additions.len() + coinbase_coins.len();
unspent_count -= removals.len();
total_value += sum(addition.amount for each addition + coinbase);
total_value -= sum(removed_coin.amount for each removal);
```

**Stored in `metadata` CF:**
- `"unspent_count"` → `u64`
- `"spent_count"` → `u64`
- `"total_value"` → `u64`

**Cost:** Two additions and one subtraction per block. **Savings:** Eliminates O(N) scans for the three most common aggregate queries.

`aggregate_unspent_by_puzzle_hash()` cannot be cheaply materialized (too many puzzle hashes to track individually). It remains a full scan but operates over the smaller `unspent_by_puzzle_hash` CF instead of the full `coin_records` CF.

### 13.8 MVCC-Style Reads During Block Application

**Problem:** The naive approach holds an exclusive write lock for the entire duration of `apply_block()`. During this time, all readers (mempool, wallet sync, RPC) are blocked. A block with 5000 transactions could block readers for hundreds of milliseconds.

**Solution:** Split block application so that readers continue seeing the previous block's state while the new block is being written.

**Mechanism (RocksDB):**
1. Take a `rocksdb::Snapshot` before starting Phase 2. All concurrent readers use this snapshot.
2. Prepare the `WriteBatch` (no lock needed — this is just memory allocation).
3. Commit the `WriteBatch` atomically.
4. Swap the in-memory state (unspent set, counters, cache, tip) under a brief exclusive lock.
5. Release the snapshot.

**Mechanism (LMDB):**
LMDB provides this naturally. Its copy-on-write B-tree means readers always see a consistent snapshot. The write transaction commits atomically, and subsequent readers see the new state.

**Impact:** Reader latency during block application drops from "blocked for the entire write duration" to "briefly paused for an atomic pointer swap" (~microseconds).

### 13.9 Parallel Block Validation

**Problem:** Phase 1 validation (checking that each removal coin exists and is unspent) does N independent lookups. These are serialized in the current design.

**Solution:** Parallelize removal validation using the in-memory unspent set.

```rust
// Phase 1: Parallel removal validation (lock-free, read-only)
let validation_errors: Vec<_> = block.removals.par_iter()
    .filter_map(|coin_id| {
        if !self.unspent_set.contains(coin_id) {
            if self.storage.get_coin_record(coin_id).is_none() {
                Some(CoinStoreError::CoinNotFound(*coin_id))
            } else {
                Some(CoinStoreError::DoubleSpend(*coin_id))
            }
        } else {
            None
        }
    })
    .collect();

if !validation_errors.is_empty() {
    return Err(validation_errors[0].clone());
}
```

Since `unspent_set` is a `HashSet` (read-only during validation), this is fully lock-free. The storage fallback (for distinguishing CoinNotFound from DoubleSpend) is only hit on the error path.

**Impact:** On a block with 1000 removals and 8 cores, validation is ~4-8x faster.

### 13.10 Snapshot-Based Fast Sync

**Problem:** New nodes must either replay the entire chain from genesis (slow — hours to days at scale) or trust a snapshot from a peer. The current snapshot mechanism stores all coin records, which works but doesn't leverage the Merkle tree for trustless verification.

**Solution:** Publish periodic **checkpoint snapshots** that can be verified against trusted block headers.

**Checkpoint snapshot contents:**
1. All unspent coin records at a specific height (sorted by coin_id).
2. The Merkle root at that height (matches the block header's `state_root`).
3. The block header hash and height.
4. Optionally: Merkle proofs for a random subset of coins (spot-check verification).

**Fast sync protocol:**
1. New node obtains a trusted block header (from a checkpoint, a peer, or hardcoded).
2. Downloads the checkpoint snapshot.
3. Verifies the Merkle root of the snapshot matches the trusted header's `state_root`.
4. (Optional) Spot-checks individual coin Merkle proofs.
5. Loads the snapshot via `restore()`.
6. Begins processing blocks from the snapshot height.

**Snapshot generation:**
- Runs in background after each epoch boundary (configurable interval).
- Uses a RocksDB snapshot for consistent reads while the node continues processing.
- Compressed with zstd before storage/distribution.

**This is the single largest scalability unlock for the network.** Without it, every new node must replay the entire chain history.

### 13.11 Height-Partitioned Indices

**Problem:** The `coin_by_confirmed_height` and `coin_by_spent_height` column families grow monotonically forever. RocksDB must compact them periodically, rewriting old data that is rarely accessed.

**Solution:** Partition the key space by height bucket:

```
Key: height_bucket (4 bytes BE) || height (8 bytes BE) || coin_id (32 bytes)
where height_bucket = height / BUCKET_SIZE  (e.g., BUCKET_SIZE = 10_000)
```

**Benefits:**
- Old buckets (low height values) settle into the lowest LSM levels and are never rewritten.
- RocksDB's level compaction can trivially skip old buckets during compaction.
- Range queries for recent heights only touch the most recent bucket(s).
- FIFO compaction can be configured to drop the oldest bucket when disk space is constrained.

### 13.12 Performance Targets

| Operation | Target Latency | Mechanism |
|-----------|---------------|-----------|
| `is_unspent(coin_id)` | < 1 μs | In-memory HashSet |
| `get_coin_record(coin_id)` (cache hit) | < 5 μs | LRU cache |
| `get_coin_record(coin_id)` (cache miss) | < 100 μs | RocksDB point lookup + bloom |
| `get_coin_records_by_puzzle_hash(unspent)` | < 1 ms (100 coins) | Unspent-only index + prefix bloom |
| `apply_block(1000 additions, 500 removals)` | < 50 ms | WriteBatch + batch Merkle update |
| `rollback_to_block(1 block)` | < 100 ms | Height index scan + WriteBatch |
| `num_unspent()` / `total_unspent_value()` | < 1 μs | Materialized counters |
| `state_root()` | < 1 μs | Cached root hash |
| `get_coin_proof(coin_id)` | < 10 ms | Persistent Merkle tree demand-load |
| Startup (10M coins) | < 5 s | Persistent Merkle tree + batch unspent set load |
| Snapshot generation (10M unspent) | < 30 s | RocksDB snapshot + zstd compression |

---

## 14. Testing Strategy

### 14.1 Unit Tests

- **Genesis**: initialization with coins, empty genesis, duplicate genesis rejection, height 0 has 0 reward coins.
- **Block application**: single block, sequential blocks, additions only, removals only, mixed.
- **Chain validation**: height mismatch, parent hash mismatch, state root mismatch, reward coin count (0 at genesis, >= 2 otherwise).
- **Coin existence**: spend nonexistent coin, double spend, duplicate addition.
- **Strict spend assertion**: spend count mismatch detection, WHERE guard on already-spent coins.
- **FF-eligible tracking**: `same_as_parent=true` sets `ff_eligible` at creation, `same_as_parent=false` does not, coinbase never FF-eligible.
- **Rollback**: single block, multiple blocks, full rollback (negative height), rollback with FF-eligible recomputation (parent exists with same puzzle_hash/amount/is spent → FF-eligible, otherwise normal unspent).
- **Queries by ID**: single, batch, missing, include_spent filtering, large batch chunking.
- **Queries by puzzle hash**: single hash, multiple hashes, height range, include_spent.
- **Queries by parent**: single parent, multiple parents, height range.
- **Queries by height**: coins added at height, coins removed at height, height 0 special case (empty result).
- **CoinState queries**: pagination, min_height, max_height, max_items.
- **Batch coin states**: include_hinted join + dedup, min_amount filter, pagination with next_height, block boundary preservation (no block split across pages), `MAX_PUZZLE_HASH_BATCH_SIZE` limit enforcement, deterministic sort order.
- **Hint store**: add hints, query by hint, query by coin ID, batch queries, hint count, idempotent insert (duplicate ignored), hint length validation (reject > 32 bytes, skip empty), rollback cleans up hints for deleted coins.
- **Singleton lineage**: FF-eligible lookup, no match, multiple matches (ambiguous → None), FF-eligible after rollback.
- **Merkle proofs**: proof generation, verification, proof after block application, proof after rollback.
- **Snapshots**: save, load, restore, pruning, latest snapshot, persistence across restart.
- **Concurrency**: concurrent reads during writes, read consistency.
- **Storage backends**: LMDB tests, RocksDB tests, backend parity.
- **Performance**: block application timing, warning threshold at > 10s.
- **LRU cache**: hit/miss behavior, write-through on apply, invalidation on rollback, eviction under capacity pressure.
- **Unspent set**: insert/remove/contains correctness, startup population, rollback re-insertion.
- **Materialized counters**: counter accuracy after apply, rollback, genesis. Matches full-scan computation.
- **WriteBatch atomicity**: crash during commit doesn't leave partial state. Verify all-or-nothing.
- **Archive tier**: coins migrate after rollback window, archived coins queryable by ID, not by puzzle hash.
- **Unspent-only index**: only contains unspent coins, correctly maintained through apply + rollback.
- **Prefix bloom**: negative lookups for nonexistent puzzle hashes don't hit disk (verify via RocksDB stats).

### 14.2 Integration Tests

- **Full chain replay**: Apply 1000+ blocks, verify state roots at each height.
- **Rollback + re-apply**: Apply blocks, rollback, re-apply different fork, verify consistency.
- **Snapshot round-trip**: Apply blocks, snapshot, restore on fresh store, verify identical state.
- **Mempool integration**: Query coin records for mempool validation, verify query correctness.
- **Large-scale queries**: 100K+ coins, batch queries, pagination correctness.
- **Fast sync**: Generate checkpoint snapshot, restore on fresh node, verify state root, resume block processing.
- **Archive migration**: Apply 2000+ blocks, verify coins beyond rollback window are archived, verify hot-tier index sizes shrink.
- **Concurrent reads during writes**: Readers see consistent pre-block state while apply_block is in progress. No partial state visible.

### 14.3 Benchmark Tests

- **Block application throughput**: Blocks with 100, 1000, 5000 transactions. Measure ms/block and verify < 50ms for 1500-coin blocks.
- **Point lookup latency**: Cache-hit vs cache-miss latency distribution over 1M lookups.
- **Unspent check throughput**: `is_unspent()` ops/second (target: >10M/s).
- **Puzzle hash scan**: Latency for puzzle hashes with 1, 100, 10000 unspent coins.
- **Startup time**: Time to load unspent set + Merkle root at 1M, 10M, 50M coins.
- **Snapshot generation**: Time and size at 1M, 10M unspent coins.

### 14.4 Property Tests

- **Conservation**: Total unspent value after each block = previous total + additions - removals (accounting for fees).
- **Determinism**: Same sequence of blocks always produces the same state root.
- **Rollback identity**: `apply_block(B); rollback(h-1)` produces identical state to before `apply_block(B)`.
- **Query consistency**: `get_coins_added_at_height(h)` returns exactly the coins added by block h.
