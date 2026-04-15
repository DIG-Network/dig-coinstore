# Queries — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [QRY-001](NORMATIVE.md#QRY-001) | :white_check_mark: | Point lookups by coin ID | 5 tests in `qry_001_tests.rs`: existing, missing, spent, batch mixed, empty. |
| [QRY-002](NORMATIVE.md#QRY-002) | :white_check_mark: | Puzzle hash queries with filtering | 5 tests in `qry_002_tests.rs`: include_spent true/false, height range, no match, batch. |
| [QRY-003](NORMATIVE.md#QRY-003) | :white_check_mark: | Height-based addition/removal queries | 5 tests in `qry_003_tests.rs`: genesis, block 1, removals, height 0 empty, nonexistent. |
| [QRY-004](NORMATIVE.md#QRY-004) | :white_check_mark: | Parent ID queries | 4 tests in `qry_004_tests.rs`: by parent, no match, include_spent false, multiple parents. |
| [QRY-005](NORMATIVE.md#QRY-005) | :white_check_mark: | Name queries with filtering | 4 tests in `qry_005_tests.rs`: include_spent true/false, height range, empty names. |
| [QRY-006](NORMATIVE.md#QRY-006) | :x: | Lightweight CoinState queries | Tests: by IDs, by puzzle hashes, CoinState struct correctness. |
| [QRY-007](NORMATIVE.md#QRY-007) | :x: | Batch coin state pagination | Tests: batch size limit, hinted join, dedup, min_amount, sort order, block boundary, pagination detection. |
| [QRY-008](NORMATIVE.md#QRY-008) | :x: | Singleton lineage lookup | Tests: exactly one match returns info, zero/multiple matches return None. |
| [QRY-009](NORMATIVE.md#QRY-009) | :x: | Aggregate queries | Tests: num_unspent, total_value, by_puzzle_hash aggregation, num_total. |
| [QRY-010](NORMATIVE.md#QRY-010) | :x: | Chain state metadata | Tests: height, tip_hash, state_root, timestamp, stats, is_empty. |
| [QRY-011](NORMATIVE.md#QRY-011) | :x: | Input slice chunking | Tests: chunking under/over/at limit, dedup across chunks, error propagation, all 5 affected methods. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
