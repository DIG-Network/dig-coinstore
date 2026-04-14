# Queries — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [QRY-001](NORMATIVE.md#QRY-001) | :x: | Point lookups by coin ID | Tests: single lookup, batch lookup, missing coin returns None. |
| [QRY-002](NORMATIVE.md#QRY-002) | :x: | Puzzle hash queries with filtering | Tests: include_spent filter, height range, batch puzzle hashes. |
| [QRY-003](NORMATIVE.md#QRY-003) | :x: | Height-based addition/removal queries | Tests: coins added at height, coins removed at height, height 0 removal empty. |
| [QRY-004](NORMATIVE.md#QRY-004) | :x: | Parent ID queries | Tests: by parent IDs, include_spent, height range filtering. |
| [QRY-005](NORMATIVE.md#QRY-005) | :x: | Name queries with filtering | Tests: by coin IDs with include_spent and height range. |
| [QRY-006](NORMATIVE.md#QRY-006) | :x: | Lightweight CoinState queries | Tests: by IDs, by puzzle hashes, CoinState struct correctness. |
| [QRY-007](NORMATIVE.md#QRY-007) | :x: | Batch coin state pagination | Tests: batch size limit, hinted join, dedup, min_amount, sort order, block boundary, pagination detection. |
| [QRY-008](NORMATIVE.md#QRY-008) | :x: | Singleton lineage lookup | Tests: exactly one match returns info, zero/multiple matches return None. |
| [QRY-009](NORMATIVE.md#QRY-009) | :x: | Aggregate queries | Tests: num_unspent, total_value, by_puzzle_hash aggregation, num_total. |
| [QRY-010](NORMATIVE.md#QRY-010) | :x: | Chain state metadata | Tests: height, tip_hash, state_root, timestamp, stats, is_empty. |
| [QRY-011](NORMATIVE.md#QRY-011) | :x: | Input slice chunking | Tests: chunking under/over/at limit, dedup across chunks, error propagation, all 5 affected methods. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
