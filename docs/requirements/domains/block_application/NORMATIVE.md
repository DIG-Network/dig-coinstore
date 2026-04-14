# Block Application Pipeline — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Section 5

---

## &sect;1 Entry Point

<a id="BLK-001"></a>**BLK-001** `apply_block()` MUST accept a `BlockData` and MUST return `Result<ApplyBlockResult, CoinStoreError>`. The operation MUST be atomic: either the entire block applies successfully or no state changes occur.
> **Spec:** [`BLK-001.md`](specs/BLK-001.md)

---

## &sect;2 Height and Chain Continuity

<a id="BLK-002"></a>**BLK-002** `block.height` MUST equal `self.height() + 1`. Any gap or regression MUST return `CoinStoreError::HeightMismatch { expected, got }`.
> **Spec:** [`BLK-002.md`](specs/BLK-002.md)

<a id="BLK-003"></a>**BLK-003** `block.parent_hash` MUST match `self.tip_hash()`. A mismatch MUST return `CoinStoreError::ParentHashMismatch { expected, got }`. For genesis (height 0), the parent hash is the zero hash.
> **Spec:** [`BLK-003.md`](specs/BLK-003.md)

---

## &sect;3 Reward Coin Assertion

<a id="BLK-004"></a>**BLK-004** Height 0 MUST have 0 coinbase coins (`coinbase_coins.is_empty()`). All other heights MUST have `coinbase_coins.len() >= MIN_REWARD_COINS_PER_BLOCK` (2). Adopted from Chia `coin_store.py:138-141`.
> **Spec:** [`BLK-004.md`](specs/BLK-004.md)

---

## &sect;4 Removal and Addition Validation

<a id="BLK-005"></a>**BLK-005** Every coin ID in `removals` MUST exist in the coinstate and be currently unspent. Validation MUST occur BEFORE any mutations. Missing coins MUST return `CoinStoreError::CoinNotFound`. Already-spent coins MUST return `CoinStoreError::DoubleSpend`.
> **Spec:** [`BLK-005.md`](specs/BLK-005.md)

<a id="BLK-006"></a>**BLK-006** No coin in `additions` MUST already exist in the coinstate. Duplicate additions MUST return `CoinStoreError::CoinAlreadyExists`.
> **Spec:** [`BLK-006.md`](specs/BLK-006.md)

---

## &sect;5 Coin Mutation

<a id="BLK-007"></a>**BLK-007** Addition coins MUST be stored with `confirmed_height = block.height`, `spent_height = None`, `timestamp = block.timestamp`. When `CoinAddition.same_as_parent = true`, the coin MUST be flagged `ff_eligible = true`. Coinbase coins MUST always have `ff_eligible = false`. Adopted from Chia `coin_store.py:128-129`.
> **Spec:** [`BLK-007.md`](specs/BLK-007.md)

<a id="BLK-008"></a>**BLK-008** Removal coins MUST be marked as spent at `block.height`. The update MUST use a WHERE guard that only modifies unspent records. After mutation, `rows_updated` MUST equal `removals.len()`. A mismatch MUST return `CoinStoreError::SpendCountMismatch { expected, actual }`. Adopted from Chia `coin_store.py:627-648`.
> **Spec:** [`BLK-008.md`](specs/BLK-008.md)

---

## &sect;6 State Root Verification

<a id="BLK-009"></a>**BLK-009** If `BlockData.expected_state_root` is provided, the computed Merkle state root MUST match the expected value. A mismatch MUST return `CoinStoreError::StateRootMismatch { expected, computed }`.
> **Spec:** [`BLK-009.md`](specs/BLK-009.md)

---

## &sect;7 Observability

<a id="BLK-010"></a>**BLK-010** Block application MUST be timed from start to finish. If elapsed time exceeds `BLOCK_APPLY_WARN_SECONDS` (10 seconds), a warning MUST be logged including the block height, additions count, and removals count. Normal application MUST log at DEBUG level. Adopted from Chia `coin_store.py:164-178`.
> **Spec:** [`BLK-010.md`](specs/BLK-010.md)

---

## &sect;8 Hint Processing

<a id="BLK-011"></a>**BLK-011** `apply_block()` MUST validate all hints in `block.hints` during Phase 1. Empty hints (0 bytes) MUST be silently skipped. Hints longer than `MAX_HINT_LENGTH` (32 bytes) MUST cause the entire block to be rejected with `CoinStoreError::HintTooLong`. This validation MUST occur before any Phase 2 mutations.
> **Spec:** [`BLK-011.md`](specs/BLK-011.md)

<a id="BLK-012"></a>**BLK-012** `apply_block()` MUST store all validated hints from `block.hints` during Phase 2. Hint storage MUST be included in the same `WriteBatch` as coin record writes. Duplicate `(coin_id, hint)` pairs MUST be silently ignored (idempotent). See HNT-002.
> **Spec:** [`BLK-012.md`](specs/BLK-012.md)

---

## &sect;9 Merkle Tree Update

<a id="BLK-013"></a>**BLK-013** `apply_block()` MUST batch-update the Merkle tree during Phase 2 with all new/modified coin record hashes. The update MUST use `SparseMerkleTree::batch_insert()` for a single root recomputation. Dirty Merkle nodes MUST be included in the same `WriteBatch`. See MRK-001, MRK-003.
> **Spec:** [`BLK-013.md`](specs/BLK-013.md)

---

## &sect;10 Chain Tip and Atomic Commit

<a id="BLK-014"></a>**BLK-014** `apply_block()` MUST atomically update the chain tip metadata (height, tip_hash, timestamp) in the same `WriteBatch` as all other Phase 2 mutations. After successful commit, the in-memory state (unspent set, LRU cache, counters, chain tip) MUST be swapped atomically.
> **Spec:** [`BLK-014.md`](specs/BLK-014.md)
