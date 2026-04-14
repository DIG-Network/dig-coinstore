# Performance and Scalability — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [PRF-001](NORMATIVE.md#PRF-001) | -- | In-memory unspent set (HashSet) | Tests: O(1) is_unspent() lookup, insert on creation, remove on spend, re-insert on rollback, startup population in 50K chunks, ~40 bytes/coin memory budget. |
| [PRF-002](NORMATIVE.md#PRF-002) | -- | LRU coin record cache | Tests: cache hit returns record without storage access, cache miss falls through to storage, write-through on apply_block, full invalidation on rollback, capacity bounded at 1M entries, not persisted across restarts. |
| [PRF-003](NORMATIVE.md#PRF-003) | -- | Materialized aggregate counters | Tests: counters updated atomically with block, O(1) reads from metadata CF, counters consistent after apply_block, counters consistent after rollback. |
| [PRF-004](NORMATIVE.md#PRF-004) | -- | Unspent-only puzzle hash index | Tests: index populated on creation, entry removed on spend, entry re-inserted on rollback, index only contains unspent coins, smaller than full index. |
| [PRF-005](NORMATIVE.md#PRF-005) | -- | Tiered spent coin archival | Tests: coins older than archive_after_blocks migrated, prune_archived=false keeps hot copy, prune_archived=true removes from hot tier, hot tier indices intact for non-archived coins. |
| [PRF-006](NORMATIVE.md#PRF-006) | -- | Snapshot-based fast sync | Tests: snapshot created with Merkle root, snapshot downloadable and restorable, root verification against trusted header, resume block application from snapshot height. |
| [PRF-007](NORMATIVE.md#PRF-007) | -- | Height-partitioned indices | Tests: compound key format correct, range scan on recent heights efficient, old buckets in cold LSM levels, coin_by_confirmed_height and coin_by_spent_height both use partitioned keys. |
| [PRF-008](NORMATIVE.md#PRF-008) | -- | Snapshot/restore persistence | Tests: save_snapshot() persists state, load_snapshot(height) restores correct state, load_latest_snapshot() returns most recent, available_snapshot_heights() lists all, auto-prune removes oldest beyond max_snapshots. |
| [PRF-009](NORMATIVE.md#PRF-009) | -- | Benchmark tests | Benchmarks: criterion benchmarks for all 11 SPEC Section 13.12 performance targets: is_unspent < 1 us, get_coin_record hit < 5 us, get_coin_record miss < 100 us, puzzle hash query < 1 ms, apply_block < 50 ms, rollback < 100 ms, counters < 1 us, state_root < 1 us, coin proof < 10 ms, startup 10M < 5 s, snapshot 10M < 30 s. |

**Status legend:** ✅ verified · ⚠️ partial · -- gap
