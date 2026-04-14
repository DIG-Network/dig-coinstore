# Crate API — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Sections 2, 3, 4

---

## &sect;1 Construction

<a id="API-001"></a>**API-001** `CoinStore::new(path)` MUST create a coinstate store with default `CoinStoreConfig` at the given path. `CoinStore::with_config(config)` MUST create a coinstate store with custom configuration. `init_genesis()` MUST bootstrap the chain with initial coins and return `Result<Bytes32, CoinStoreError>`.
> **Spec:** [`API-001.md`](specs/API-001.md)

---

## &sect;2 Core Types

<a id="API-002"></a>**API-002** `CoinRecord` MUST be a public struct deriving `Debug, Clone, Serialize, Deserialize` with fields: coin (Coin), confirmed_height (u64), spent_height (Option<u64>), coinbase (bool), timestamp (u64), ff_eligible (bool). It MUST provide methods: `new()`, `is_spent()`, `spend()`, `coin_id()`, `to_coin_state()`.
> **Spec:** [`API-002.md`](specs/API-002.md)

<a id="API-003"></a>**API-003** `CoinStoreConfig` MUST be a public struct with a builder pattern (`with_*` methods) and documented default values for all fields: backend (StorageBackend), storage_path, max_snapshots, max_query_results, lmdb_map_size, rocksdb_write_buffer_size, rocksdb_max_open_files, bloom_filter.
> **Spec:** [`API-003.md`](specs/API-003.md)

---

## &sect;3 Error Types

<a id="API-004"></a>**API-004** `CoinStoreError` MUST be a public enum deriving `Debug + Clone + PartialEq + thiserror::Error` with all specified variants: HeightMismatch, ParentHashMismatch, StateRootMismatch, CoinNotFound, CoinAlreadyExists, DoubleSpend, SpendCountMismatch, InvalidRewardCoinCount, HintTooLong, GenesisAlreadyInitialized, NotInitialized, PuzzleHashBatchTooLarge, StorageError, SerializationError, DeserializationError.
> **Spec:** [`API-004.md`](specs/API-004.md)

---

## &sect;4 Block Input Types

<a id="API-005"></a>**API-005** `BlockData` MUST be a public struct with fields: height, timestamp, block_hash, parent_hash, additions (Vec<CoinAddition>), removals (Vec<CoinId>), coinbase_coins (Vec<Coin>), hints (Vec<(CoinId, Bytes32)>), expected_state_root (Option<Bytes32>). `CoinAddition` MUST be a public struct with fields: coin_id, coin, same_as_parent.
> **Spec:** [`API-005.md`](specs/API-005.md)

---

## &sect;5 Result Types

<a id="API-006"></a>**API-006** `ApplyBlockResult` MUST be a public struct with fields: state_root (Bytes32), coins_created (usize), coins_spent (usize), height (u64). `RollbackResult` MUST be a public struct with fields: modified_coins (HashMap<CoinId, CoinRecord>), coins_deleted (usize), coins_unspent (usize), new_height (u64).
> **Spec:** [`API-006.md`](specs/API-006.md)

---

## &sect;6 Statistics

<a id="API-007"></a>**API-007** `CoinStoreStats` MUST be a public struct with fields: height (u64), timestamp (u64), unspent_count (u64), spent_count (u64), total_unspent_value (u64), state_root (Bytes32), tip_hash (Bytes32), hint_count (u64), snapshot_count (usize). Returned by `CoinStore::stats()`.
> **Spec:** [`API-007.md`](specs/API-007.md)

---

## &sect;7 Snapshot Types

<a id="API-008"></a>**API-008** `CoinStoreSnapshot` MUST be a public struct deriving `Serialize, Deserialize` with fields: height (u64), block_hash (Bytes32), state_root (Bytes32), timestamp (u64), coins (HashMap<CoinId, CoinRecord>), hints (Vec<(CoinId, Bytes32)>), total_coins (u64), total_value (u64).
> **Spec:** [`API-008.md`](specs/API-008.md)

---

## &sect;8 Type Aliases and Lineage Info

<a id="API-009"></a>**API-009** The crate MUST define and publicly export type aliases `CoinId = Bytes32` and `PuzzleHash = Bytes32`. These MUST be used consistently throughout the API surface. The `UnspentLineageInfo` struct (coin_id, parent_id, parent_parent_id) MUST be defined and publicly exported.
> **Spec:** [`API-009.md`](specs/API-009.md)

---

## &sect;9 RollbackAboveTip and is_unspent

<a id="API-010"></a>**API-010** The `CoinStoreError` enum MUST include the `RollbackAboveTip` variant: `#[error("cannot rollback: target height {target} above current height {current}")] RollbackAboveTip { target: i64, current: u64 }`. The `is_unspent(&self, coin_id: &CoinId) -> bool` method MUST be a public CoinStore method with O(1) lock-free semantics.
> **Spec:** [`API-010.md`](specs/API-010.md)
