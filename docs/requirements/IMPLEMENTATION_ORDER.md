# Implementation Order

Phased checklist for dig-coinstore requirements. Work top-to-bottom within each phase.
After completing a requirement: write tests, verify they pass, update TRACKING.yaml, VERIFICATION.md, and check off here.

**A requirement is NOT complete until comprehensive tests verify it.**

---

## Phase 0: Crate Structure & Foundation

- [x] STR-001 — Cargo.toml with dependencies (chia-protocol, chia-sha2, chia-traits + chia-consensus/chia-sdk-test dev-deps), feature gates, and metadata
- [x] STR-002 — Module hierarchy (`src/lib.rs` root, submodule layout)
- [x] STR-003 — Storage module (`src/storage/`) with backend trait and implementations
- [x] STR-004 — Merkle module (`src/merkle/`) with sparse Merkle tree
- [x] STR-005 — Re-export strategy (Coin, Bytes32, CoinState, CoinStateFilters via `dig-clvm`; ChiaCoinRecord alias)
- [x] STR-006 — Test infrastructure (`tests/` layout, helpers, fixtures)

## Phase 1: Crate API Types

- [x] API-001 — CoinStore constructor (`new`, `with_config`)
- [x] API-002 — CoinRecord struct (coin, confirmed_height, spent_height, coinbase, timestamp, ff_eligible) + from/to_chia_coin_record() interop
- [x] API-003 — CoinStoreConfig with builder pattern and defaults
- [x] API-004 — CoinStoreError enum (15 variants per NORMATIVE API-004)
- [x] API-005 — BlockData and CoinAddition structs
- [x] API-006 — ApplyBlockResult and RollbackResult structs
- [x] API-007 — CoinStoreStats struct
- [x] API-008 — CoinStoreSnapshot struct (serde Serialize/Deserialize)
- [x] API-009 — CoinId/PuzzleHash type aliases and UnspentLineageInfo struct
- [x] API-010 — RollbackAboveTip error variant and is_unspent() method

## Phase 2: Storage Backends

- [x] STO-001 — Storage trait (backend-agnostic interface)
- [x] STO-002 — RocksDB backend with column families
- [x] STO-003 — LMDB backend with named databases
- [x] STO-004 — Bloom filter configuration (full bloom + prefix bloom)
- [x] STO-005 — WriteBatch atomic block commits (RocksDB)
- [x] STO-006 — Compaction strategy per column family
- [x] STO-007 — Feature gates (`lmdb-storage`, `rocksdb-storage`)
- [x] STO-008 — Serialization (bincode for coin records, snapshots)

## Phase 3: Merkle Tree

- [x] MRK-001 — Sparse Merkle tree with batch insert/update/remove
- [x] MRK-002 — Memoized empty hash array (257 levels, OnceLock)
- [x] MRK-003 — Persistent internal nodes (merkle_nodes column family)
- [x] MRK-004 — Proof generation (`get_coin_proof`)
- [x] MRK-005 — Proof verification (`verify_coin_proof`)
- [x] MRK-006 — Leaf hash function (`coin_record_hash` using `chia_sha2::Sha256`, determinism)

## Phase 4: Block Application Pipeline

- [x] BLK-001 — `apply_block()` entry point signature and return type
- [x] BLK-002 — Height continuity validation
- [x] BLK-003 — Parent hash validation
- [x] BLK-004 — Reward coin count assertion (0 at genesis, >= 2 otherwise)
- [x] BLK-005 — Removal validation (exists + unspent, pre-mutation)
- [x] BLK-006 — Addition validation (no duplicates)
- [x] BLK-007 — Coin insertion with FF-eligible tracking (same_as_parent)
- [x] BLK-008 — Spend marking with strict count assertion
- [x] BLK-009 — State root verification (optional expected_state_root)
- [x] BLK-010 — Performance logging (warn > 10s)
- [x] BLK-011 — Hint validation in Phase 1 (length check, empty skip, block rejection)
- [x] BLK-012 — Hint storage in Phase 2 (WriteBatch, idempotent)
- [x] BLK-013 — Merkle tree batch update in Phase 2 (single root recomputation)
- [x] BLK-014 — Chain tip atomic commit (height, tip_hash, timestamp swap)

## Phase 5: Hint Store

- [x] HNT-001 — Hint validation (length <= 32 bytes, skip empty)
- [x] HNT-002 — Hint storage with idempotent insertion
- [x] HNT-003 — Forward index (coin_id -> hints) and reverse index (hint -> coin_ids)
- [x] HNT-004 — Hint queries (by hint, by coin_id, batch, count)
- [x] HNT-005 — Rollback hint cleanup (delete hints for deleted coins)
- [x] HNT-006 — Variable-length hint keys (length-prefixed encoding, no prefix collisions)

## Phase 6: Queries

- [x] QRY-001 — `get_coin_record()` and `get_coin_records()` (by ID, batch)
- [x] QRY-002 — `get_coin_records_by_puzzle_hash()` and `get_coin_records_by_puzzle_hashes()`
- [x] QRY-003 — `get_coins_added_at_height()` and `get_coins_removed_at_height()`
- [x] QRY-004 — `get_coin_records_by_parent_ids()`
- [x] QRY-005 — `get_coin_records_by_names()` (with include_spent, height range)
- [ ] QRY-006 — `get_coin_states_by_ids()` and `get_coin_states_by_puzzle_hashes()`
- [ ] QRY-007 — `batch_coin_states_by_puzzle_hashes()` with `CoinStateFilters` from chia-protocol (pagination, block boundary, dedup, min_amount)
- [ ] QRY-008 — `get_unspent_lineage_info_for_puzzle_hash()` (singleton FF support)
- [ ] QRY-009 — Aggregate queries (`num_unspent`, `total_unspent_value`, `aggregate_unspent_by_puzzle_hash`)
- [ ] QRY-010 — Chain state queries (`height`, `tip_hash`, `state_root`, `stats`, `is_empty`)
- [ ] QRY-011 — Large input batching (chunk slices by DEFAULT_LOOKUP_BATCH_SIZE)

## Phase 7: Rollback

- [ ] RBK-001 — `rollback_to_block()` entry point and return type
- [ ] RBK-002 — Coin deletion (confirmed after target height)
- [ ] RBK-003 — Coin un-spending (spent after target height)
- [ ] RBK-004 — FF-eligible recomputation during rollback (parent EXISTS check)
- [ ] RBK-005 — `rollback_n_blocks()` convenience wrapper
- [ ] RBK-006 — Merkle tree batch rebuild during rollback
- [ ] RBK-007 — Rollback atomicity (all-or-nothing on failure)

## Phase 8: Concurrency

- [ ] CON-001 — `CoinStore` is `Send + Sync`
- [ ] CON-002 — RwLock strategy (shared reads, exclusive writes)
- [ ] CON-003 — MVCC reads during block application (snapshot isolation)
- [ ] CON-004 — Parallel removal validation via in-memory unspent set

## Phase 9: Performance & Scalability

- [ ] PRF-001 — In-memory unspent set (`HashSet<CoinId>`)
- [ ] PRF-002 — LRU coin record cache
- [ ] PRF-003 — Materialized aggregate counters (unspent_count, total_value)
- [ ] PRF-004 — Unspent-only puzzle hash index
- [ ] PRF-005 — Tiered spent coin archival (hot/archive/prune)
- [ ] PRF-006 — Snapshot-based fast sync (checkpoint snapshots with Merkle root verification)
- [ ] PRF-007 — Height-partitioned indices
- [ ] PRF-008 — Snapshot/restore persistence (save, load, prune)
- [ ] PRF-009 — Performance benchmark targets (11 criterion benchmarks from SPEC 13.12)

---

## Summary

| Phase | Domain | Count |
|-------|--------|-------|
| 0 | Crate Structure | 6 |
| 1 | Crate API | 10 |
| 2 | Storage | 8 |
| 3 | Merkle Tree | 6 |
| 4 | Block Application | 14 |
| 5 | Hints | 6 |
| 6 | Queries | 11 |
| 7 | Rollback | 7 |
| 8 | Concurrency | 4 |
| 9 | Performance | 9 |
| **Total** | | **81** |
