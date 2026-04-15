# Queries — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [QRY-001](NORMATIVE.md#QRY-001) | :white_check_mark: | Point lookups by coin ID | 5 tests in `qry_001_tests.rs`: existing, missing, spent, batch mixed, empty. |
| [QRY-002](NORMATIVE.md#QRY-002) | :white_check_mark: | Puzzle hash queries with filtering | 5 tests in `qry_002_tests.rs`: include_spent true/false, height range, no match, batch. |
| [QRY-003](NORMATIVE.md#QRY-003) | :white_check_mark: | Height-based addition/removal queries | 5 tests in `qry_003_tests.rs`: genesis, block 1, removals, height 0 empty, nonexistent. |
| [QRY-004](NORMATIVE.md#QRY-004) | :white_check_mark: | Parent ID queries | 4 tests in `qry_004_tests.rs`: by parent, no match, include_spent false, multiple parents. |
| [QRY-005](NORMATIVE.md#QRY-005) | :white_check_mark: | Name queries with filtering | 4 tests in `qry_005_tests.rs`: include_spent true/false, height range, empty names. |
| [QRY-006](NORMATIVE.md#QRY-006) | :white_check_mark: | Lightweight CoinState queries | 5 tests in `qry_006_tests.rs`. |
| [QRY-007](NORMATIVE.md#QRY-007) | :white_check_mark: | Batch coin state pagination | 5 tests in `qry_007_tests.rs`. |
| [QRY-008](NORMATIVE.md#QRY-008) | :white_check_mark: | Singleton lineage lookup | 4 tests in `qry_008_tests.rs`. |
| [QRY-009](NORMATIVE.md#QRY-009) | :white_check_mark: | Aggregate queries | 5 tests in `qry_009_tests.rs`. |
| [QRY-010](NORMATIVE.md#QRY-010) | :white_check_mark: | Chain state metadata | 4 tests in `qry_010_tests.rs`. |
| [QRY-011](NORMATIVE.md#QRY-011) | :white_check_mark: | Input slice chunking | 4 tests in `qry_011_tests.rs`. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
