# Rollback — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [RBK-001](NORMATIVE.md#RBK-001) | :x: | rollback_to_block() entry point | Tests: signature, negative target full reset, return type. |
| [RBK-002](NORMATIVE.md#RBK-002) | :x: | Coin deletion after target height | Tests: coins confirmed after target deleted, modified_coins populated. |
| [RBK-003](NORMATIVE.md#RBK-003) | :x: | Coin un-spending after target height | Tests: spent coins marked unspent, dedup with deletion pass. |
| [RBK-004](NORMATIVE.md#RBK-004) | :x: | FF-eligible recomputation | Tests: coinbase false + parent match -> ff_eligible true, otherwise false. |
| [RBK-005](NORMATIVE.md#RBK-005) | :x: | rollback_n_blocks() convenience wrapper | Tests: delegates to rollback_to_block with correct height. |
| [RBK-006](NORMATIVE.md#RBK-006) | :x: | Merkle tree batch rebuild during rollback | Tests: deleted coins removed from tree, un-spent coins re-hashed, single root recomputation. |
| [RBK-007](NORMATIVE.md#RBK-007) | :x: | Rollback atomicity | Tests: partial failure aborts all changes, no state modifications on error, storage error during deletion/unspend/Merkle rolls back. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
