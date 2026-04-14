# Performance and Scalability — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Section 13

---

## &sect;1 In-Memory Unspent Set

<a id="PRF-001"></a>**PRF-001** The implementation MUST maintain an in-memory `HashSet<CoinId>` providing O(1) `is_unspent()` lookups. Each entry MUST consume approximately 40 bytes. The set MUST insert on coin creation, remove on spend, and re-insert on rollback. On startup the set MUST be populated from storage in `MATERIALIZATION_BATCH_SIZE` (50,000) chunks.
> **Spec:** [`PRF-001.md`](specs/PRF-001.md)

---

## &sect;2 LRU Coin Record Cache

<a id="PRF-002"></a>**PRF-002** The implementation MUST maintain an LRU cache of coin records with a default capacity of `DEFAULT_COIN_CACHE_CAPACITY` (1,000,000) entries. The cache MUST use write-through semantics on `apply_block` and MUST be fully invalidated on rollback. The cache MUST NOT be persisted to disk. A cache miss MUST fall through to the underlying storage layer.
> **Spec:** [`PRF-002.md`](specs/PRF-002.md)

---

## &sect;3 Materialized Aggregate Counters

<a id="PRF-003"></a>**PRF-003** The implementation MUST maintain materialized aggregate counters (`unspent_count`, `spent_count`, `total_value`) that are updated atomically in the same `WriteBatch` as the block application. Counters MUST be stored in the metadata column family. Counter reads MUST be O(1).
> **Spec:** [`PRF-003.md`](specs/PRF-003.md)

---

## &sect;4 Unspent-Only Puzzle Hash Index

<a id="PRF-004"></a>**PRF-004** The implementation MUST maintain a dedicated `unspent_by_puzzle_hash` column family indexing only currently-unspent coins by puzzle hash. Entries MUST be inserted on coin creation, deleted on spend, and re-inserted on rollback. This index SHOULD be substantially smaller than a full (spent + unspent) puzzle hash index.
> **Spec:** [`PRF-004.md`](specs/PRF-004.md)

---

## &sect;5 Tiered Spent Coin Archival

<a id="PRF-005"></a>**PRF-005** The implementation SHOULD support tiered spent coin archival via `ArchiveConfig`. Coins spent more than `archive_after_blocks` (default: `DEFAULT_ROLLBACK_WINDOW` = 1,000) blocks ago MAY be migrated to a dedicated `archive_coin_records` column family by a background process. If `prune_archived` is true (default: false), archived coins MAY be deleted from the hot tier entirely. The hot tier MUST retain full indices for non-archived coins.
> **Spec:** [`PRF-005.md`](specs/PRF-005.md)

---

## &sect;6 Snapshot-Based Fast Sync

<a id="PRF-006"></a>**PRF-006** The implementation SHOULD support checkpoint snapshots with a Merkle root. New nodes MUST be able to download a snapshot, verify its root against a trusted header, restore state from the snapshot, and resume block application from the snapshot height.
> **Spec:** [`PRF-006.md`](specs/PRF-006.md)

---

## &sect;7 Height-Partitioned Indices

<a id="PRF-007"></a>**PRF-007** The `coin_by_confirmed_height` and `coin_by_spent_height` indices MUST use a compound key of the form `height_bucket||height||coin_id`. Old buckets SHOULD naturally settle into cold LSM levels, enabling efficient range scans on recent heights while minimizing read amplification for historical queries.
> **Spec:** [`PRF-007.md`](specs/PRF-007.md)

---

## &sect;8 Snapshot/Restore Persistence

<a id="PRF-008"></a>**PRF-008** The implementation MUST provide `save_snapshot()`, `load_snapshot(height)`, `load_latest_snapshot()`, and `available_snapshot_heights()` methods. Old snapshots beyond `max_snapshots` MUST be automatically pruned.
> **Spec:** [`PRF-008.md`](specs/PRF-008.md)

---

## &sect;9 Benchmark Tests

<a id="PRF-009"></a>**PRF-009** The crate MUST include `criterion` benchmark tests that measure and verify the performance targets from SPEC Section 13.12. The following 11 targets MUST have corresponding benchmarks: `is_unspent(coin_id)` < 1 &mu;s; `get_coin_record(coin_id)` cache hit < 5 &mu;s; `get_coin_record(coin_id)` cache miss < 100 &mu;s; `get_coin_records_by_puzzle_hash(unspent)` < 1 ms for 100 coins; `apply_block(1000 additions, 500 removals)` < 50 ms; `rollback_to_block(1 block)` < 100 ms; `num_unspent()` / `total_unspent_value()` < 1 &mu;s; `state_root()` < 1 &mu;s; `get_coin_proof(coin_id)` < 10 ms; Startup (10M coins) < 5 s; Snapshot generation (10M unspent) < 30 s.
> **Spec:** [`PRF-009.md`](specs/PRF-009.md)
