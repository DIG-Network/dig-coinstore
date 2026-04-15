# Hints — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [HNT-001](NORMATIVE.md#HNT-001) | :white_check_mark: | Hint validation (empty skip, >32 reject) | 8 tests in `hnt_001_tests.rs`: constant, empty skip, 33/64-byte reject, 1/16/31/32-byte accept. |
| [HNT-002](NORMATIVE.md#HNT-002) | :white_check_mark: | Idempotent hint insertion | 6 tests in `hnt_002_tests.rs`: double insert Ok, query returns once, same coin different hints, different coins same hint, repeated duplicates, empty hint skip. |
| [HNT-003](NORMATIVE.md#HNT-003) | :white_check_mark: | Bidirectional KV indices | 4 tests in `hnt_003_tests.rs`: forward lookup, reverse lookup, consistency, many-to-many. |
| [HNT-004](NORMATIVE.md#HNT-004) | :white_check_mark: | Hint query functions | 8 tests in `hnt_004_tests.rs`: by_hint basic, max_items, batch dedup, hints_for_coin_ids, count, empty store, nonexistent hint, unknown coin. |
| [HNT-005](NORMATIVE.md#HNT-005) | :white_check_mark: | Rollback hint cleanup | 8 tests in `hnt_005_tests.rs`: both indices cleaned, reverse empty, forward empty, shared hint preserved, no-hints noop, multi-hint all removed, count decremented, empty slice noop. |
| [HNT-006](NORMATIVE.md#HNT-006) | :white_check_mark: | Variable-length hint keys | 5 tests in `hnt_006_tests.rs`: 32-byte baseline, short hint stored+queried, no prefix collision, two lengths same coin, Bytes32 API only matches 32-byte. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
