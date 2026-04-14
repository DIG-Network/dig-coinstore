# Hints — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Section 8 (Hint Store), Section 3.9 (Hint Queries)

---

## &sect;1 Hint Validation

<a id="HNT-001"></a>**HNT-001** Empty hints (0 bytes) MUST be silently skipped. Hints exceeding `MAX_HINT_LENGTH` (32 bytes) MUST be rejected with a `HintTooLong` error. Only 32-byte hints are eligible for puzzle-hash subscription matching.
> **Spec:** [`HNT-001.md`](specs/HNT-001.md)

---

## &sect;2 Idempotent Insertion

<a id="HNT-002"></a>**HNT-002** Duplicate `(coin_id, hint)` pairs MUST be silently ignored (insert-or-ignore semantics). Re-inserting an existing hint association MUST NOT produce an error or duplicate entries.
> **Spec:** [`HNT-002.md`](specs/HNT-002.md)

---

## &sect;3 Bidirectional Indices

<a id="HNT-003"></a>**HNT-003** The hint store MUST maintain a forward index keyed by `(coin_id, hint)` for "get hints for coin" lookups and a reverse index keyed by `(hint, coin_id)` for "get coins for hint" lookups. Both indices MUST use KV composite keys with empty values.
> **Spec:** [`HNT-003.md`](specs/HNT-003.md)

---

## &sect;4 Hint Queries

<a id="HNT-004"></a>**HNT-004** The hint store MUST expose `get_coin_ids_by_hint(hint, max_items)`, `get_coin_ids_by_hints(hints, max_items)` (batch), `get_hints_for_coin_ids(coin_ids)`, and `count_hints()`. Batch queries SHOULD aggregate results across all provided keys. `max_items` MUST cap the total number of returned coin IDs.
> **Spec:** [`HNT-004.md`](specs/HNT-004.md)

---

## &sect;5 Rollback Cleanup

<a id="HNT-005"></a>**HNT-005** When coins are deleted during rollback, all associated hints for those coin IDs MUST also be deleted from both the forward and reverse indices. No orphaned hint entries MAY remain after rollback.
> **Spec:** [`HNT-005.md`](specs/HNT-005.md)

---

## &sect;6 Variable-Length Hint Keys

<a id="HNT-006"></a>**HNT-006** The hint store MUST support variable-length hints (1-32 bytes). For non-32-byte hints, composite keys MUST be padded or length-prefixed to prevent prefix-scan collisions. Only 32-byte hints MUST participate in `include_hinted` joins in `batch_coin_states_by_puzzle_hashes()`.
> **Spec:** [`HNT-006.md`](specs/HNT-006.md)
