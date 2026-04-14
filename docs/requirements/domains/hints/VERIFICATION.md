# Hints — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [HNT-001](NORMATIVE.md#HNT-001) | :x: | Hint validation (empty skip, >32 reject) | Tests: empty hint skipped, oversized rejected with HintTooLong, 32-byte accepted. |
| [HNT-002](NORMATIVE.md#HNT-002) | :x: | Idempotent hint insertion | Tests: duplicate (coin_id, hint) pair silently ignored, no duplicates in store. |
| [HNT-003](NORMATIVE.md#HNT-003) | :x: | Bidirectional KV indices | Tests: forward lookup by coin_id, reverse lookup by hint, composite key structure. |
| [HNT-004](NORMATIVE.md#HNT-004) | :x: | Hint query functions | Tests: single hint lookup, batch lookup, reverse lookup by coin_ids, count. |
| [HNT-005](NORMATIVE.md#HNT-005) | :x: | Rollback hint cleanup | Tests: hints deleted when coins rolled back, both indices clean, no orphans. |
| [HNT-006](NORMATIVE.md#HNT-006) | :x: | Variable-length hint keys | Tests: short hints padded/prefixed, no prefix-scan collisions, only 32-byte in include_hinted joins. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
