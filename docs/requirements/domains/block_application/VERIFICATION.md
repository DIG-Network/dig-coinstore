# Block Application Pipeline — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [BLK-001](NORMATIVE.md#BLK-001) | ✅ | apply_block() entry point, atomic semantics | 16 tests in `blk_001_tests.rs`: signature, valid block, sequential blocks, height mismatch, parent hash, reward count, coin not found, double spend, duplicate addition, atomicity on failure, state root, ff_eligible, removal marks spent, not initialized. |
| [BLK-002](NORMATIVE.md#BLK-002) | ✅ | Height continuity check | 5 tests in `blk_002_tests.rs`: correct height, too high, too low, duplicate, gap of 2. |
| [BLK-003](NORMATIVE.md#BLK-003) | ✅ | Parent hash verification | 4 tests in `blk_003_tests.rs`: correct parent, wrong parent, genesis zero hash, multi-block chaining. |
| [BLK-004](NORMATIVE.md#BLK-004) | ✅ | Reward coin count assertion | 5 tests in `blk_004_tests.rs`: 2 ok, 0 fails, 1 fails, 3 ok, 5 ok. |
| [BLK-005](NORMATIVE.md#BLK-005) | ✅ | Removal validation (exist + unspent) | 5 tests in `blk_005_tests.rs`: valid removal, missing coin, double spend, atomicity, multiple removals. |
| [BLK-006](NORMATIVE.md#BLK-006) | ✅ | Addition validation (no duplicates) | 4 tests in `blk_006_tests.rs`: new additions, existing coin, duplicate in block, coinbase duplicate. |
| [BLK-007](NORMATIVE.md#BLK-007) | ✅ | Coin insertion with FF-eligible tracking | 5 tests in `blk_007_tests.rs`: ff true/false, coinbase false, confirmed_height, spent_height None. |
| [BLK-008](NORMATIVE.md#BLK-008) | ✅ | Spend marking with strict count assertion | 4 tests in `blk_008_tests.rs`: spend height, count matches, unspent index, spent index. |
| [BLK-009](NORMATIVE.md#BLK-009) | ✅ | State root verification | 4 tests in `blk_009_tests.rs`: None skips, correct root, wrong root mismatch, atomicity. |
| [BLK-010](NORMATIVE.md#BLK-010) | ✅ | Performance logging | 3 tests in `blk_010_tests.rs`: timing runs, full pipeline, sequential resets. |
| [BLK-011](NORMATIVE.md#BLK-011) | ✅ | Hint validation in Phase 1 | 4 tests in `blk_011_tests.rs`: valid hints, empty vec, zero-filled skipped, before mutations. |
| [BLK-012](NORMATIVE.md#BLK-012) | ✅ | Hint storage in Phase 2 | 4 tests in `blk_012_tests.rs`: forward index, duplicate idempotent, multiple hints, accumulate. |
| [BLK-013](NORMATIVE.md#BLK-013) | ✅ | Merkle tree batch update | 5 tests in `blk_013_tests.rs`: additions, removals, deterministic, coinbase-only, cumulative. |
| [BLK-014](NORMATIVE.md#BLK-014) | ✅ | Chain tip atomic commit | 5 tests in `blk_014_tests.rs`: height, tip_hash, timestamp, sequential, failure unchanged. |

**Status legend:** ✅ verified · ⚠️ partial · -- gap
