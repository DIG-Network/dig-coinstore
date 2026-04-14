# Queries — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md)

---

## &sect;1 Point Lookups

<a id="QRY-001"></a>**QRY-001** `get_coin_record(coin_id)` MUST return `Option<CoinRecord>` for a single coin. `get_coin_records(coin_ids)` MUST return records for multiple coin IDs in a single batch lookup.
> **Spec:** [`QRY-001.md`](specs/QRY-001.md)

---

## &sect;2 Puzzle Hash Queries

<a id="QRY-002"></a>**QRY-002** `get_coin_records_by_puzzle_hash()` and `get_coin_records_by_puzzle_hashes()` MUST support `include_spent`, `start_height`, and `end_height` filtering parameters.
> **Spec:** [`QRY-002.md`](specs/QRY-002.md)

---

## &sect;3 Height Queries

<a id="QRY-003"></a>**QRY-003** `get_coins_added_at_height()` MUST return all coins confirmed at a given height. `get_coins_removed_at_height()` MUST return all coins spent at a given height. Height 0 for removals MUST return an empty result.
> **Spec:** [`QRY-003.md`](specs/QRY-003.md)

---

## &sect;4 Parent ID Queries

<a id="QRY-004"></a>**QRY-004** `get_coin_records_by_parent_ids()` MUST support `include_spent` and height range filtering parameters.
> **Spec:** [`QRY-004.md`](specs/QRY-004.md)

---

## &sect;5 Name Queries

<a id="QRY-005"></a>**QRY-005** `get_coin_records_by_names()` MUST look up coins by coin IDs with `include_spent` and height range filtering.
> **Spec:** [`QRY-005.md`](specs/QRY-005.md)

---

## &sect;6 Coin State Queries

<a id="QRY-006"></a>**QRY-006** `get_coin_states_by_ids(include_spent, coin_ids, min_height, max_height, max_items)` (SPEC Section 3.8) and `get_coin_states_by_puzzle_hashes(include_spent, puzzle_hashes, min_height, max_items)` (SPEC Section 3.6) MUST return lightweight `CoinState` structs suitable for wallet sync.
> **Spec:** [`QRY-006.md`](specs/QRY-006.md)

---

## &sect;7 Batch Coin State Pagination

<a id="QRY-007"></a>**QRY-007** `batch_coin_states_by_puzzle_hashes(puzzle_hashes, min_height, filters: CoinStateFilters, max_items)` MUST return `(Vec<CoinState>, Option<u64>)` per SPEC Section 3.5. The `filters` parameter MUST use `chia_protocol::CoinStateFilters` directly (re-exported via `dig-clvm`) to ensure wire-level compatibility with Chia's `RequestPuzzleState`. MUST enforce `MAX_PUZZLE_HASH_BATCH_SIZE`, MUST support `include_spent`/`include_unspent`/`include_hinted` filters, MUST support `min_amount` filtering, MUST sort by `MAX(confirmed_height, spent_height)` ASC with deterministic tiebreaking, MUST preserve block boundaries (never split a block across pages), and MUST fetch `max_items + 1` to detect pagination continuation via `Option<u64>` cursor.
> **Spec:** [`QRY-007.md`](specs/QRY-007.md)

---

## &sect;8 Singleton Lineage

<a id="QRY-008"></a>**QRY-008** `get_unspent_lineage_info_for_puzzle_hash()` MUST return `Option<UnspentLineageInfo>` with fields `coin_id`, `parent_id`, `parent_parent_id` (SPEC Section 2.5). MUST return `None` if the puzzle hash does not match exactly one unspent coin.
> **Spec:** [`QRY-008.md`](specs/QRY-008.md)

---

## &sect;9 Aggregate Queries

<a id="QRY-009"></a>**QRY-009** `num_unspent()`, `total_unspent_value()`, `aggregate_unspent_by_puzzle_hash() -> HashMap<Bytes32, (u64, usize)>` (SPEC Section 3.11, full-table aggregation), and `num_total()` MUST be provided. Implementations SHOULD use materialized counters where possible for O(1) performance.
> **Spec:** [`QRY-009.md`](specs/QRY-009.md)

---

## &sect;10 Chain State

<a id="QRY-010"></a>**QRY-010** `height()`, `tip_hash()`, `state_root()`, `timestamp()`, `stats()`, and `is_empty()` MUST be provided for querying current chain state metadata.
> **Spec:** [`QRY-010.md`](specs/QRY-010.md)

---

## &sect;11 Input Slice Chunking

<a id="QRY-011"></a>**QRY-011** All query methods that accept `&[T]` input slices (`get_coin_records`, `get_coin_records_by_names`, `get_coin_records_by_puzzle_hashes`, `get_coin_records_by_parent_ids`, `get_coin_ids_by_hints`) MUST chunk large inputs in batches of `DEFAULT_LOOKUP_BATCH_SIZE` (1000) to prevent unbounded memory allocation. Adopted from Chia's `to_batches()` pattern.
> **Spec:** [`QRY-011.md`](specs/QRY-011.md)
