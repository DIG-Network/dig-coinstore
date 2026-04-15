# Block Application Pipeline — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [BLK-001](NORMATIVE.md#BLK-001) | ✅ | apply_block() entry point, atomic semantics | 16 tests in `blk_001_tests.rs`: signature, valid block, sequential blocks, height mismatch, parent hash, reward count, coin not found, double spend, duplicate addition, atomicity on failure, state root, ff_eligible, removal marks spent, not initialized. |
| [BLK-002](NORMATIVE.md#BLK-002) | -- | Height continuity check | Tests: sequential height succeeds, gap rejected, regression rejected, height=0 genesis case. |
| [BLK-003](NORMATIVE.md#BLK-003) | -- | Parent hash verification | Tests: correct parent hash succeeds, wrong hash rejected, genesis zero hash accepted. |
| [BLK-004](NORMATIVE.md#BLK-004) | -- | Reward coin count assertion | Tests: genesis with 0 coinbase passes, genesis with coinbase rejected, non-genesis with >=2 passes, non-genesis with <2 rejected. |
| [BLK-005](NORMATIVE.md#BLK-005) | -- | Removal validation (exist + unspent) | Tests: valid removals pass, missing coin rejected, already-spent coin rejected, validation before mutation. |
| [BLK-006](NORMATIVE.md#BLK-006) | -- | Addition validation (no duplicates) | Tests: new additions pass, existing coin_id rejected with CoinAlreadyExists. |
| [BLK-007](NORMATIVE.md#BLK-007) | -- | Coin insertion with FF-eligible tracking | Tests: confirmed_height set, spent_height=None, timestamp set, same_as_parent sets ff_eligible, coinbase always ff_eligible=false. |
| [BLK-008](NORMATIVE.md#BLK-008) | -- | Spend marking with strict count assertion | Tests: removals marked spent, WHERE guard skips already-spent, count mismatch returns SpendCountMismatch. |
| [BLK-009](NORMATIVE.md#BLK-009) | -- | State root verification | Tests: matching root succeeds, mismatched root rejected, None root skips verification. |
| [BLK-010](NORMATIVE.md#BLK-010) | -- | Performance logging | Tests: timing occurs, slow block triggers warning log, normal block logs at DEBUG. |
| [BLK-011](NORMATIVE.md#BLK-011) | -- | Hint validation in Phase 1 | Tests: empty hints skipped, >32 byte hints reject block, validation before mutations. |
| [BLK-012](NORMATIVE.md#BLK-012) | -- | Hint storage in Phase 2 | Tests: hints stored in WriteBatch, duplicate pairs idempotent, rollback removes hints. |
| [BLK-013](NORMATIVE.md#BLK-013) | -- | Merkle tree batch update | Tests: batch_insert called, single root recomputation, dirty nodes in WriteBatch. |
| [BLK-014](NORMATIVE.md#BLK-014) | -- | Chain tip atomic commit | Tests: tip metadata in WriteBatch, in-memory state swap, partial failure rolls back. |

**Status legend:** ✅ verified · ⚠️ partial · -- gap
