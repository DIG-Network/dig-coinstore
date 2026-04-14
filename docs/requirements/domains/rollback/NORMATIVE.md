# Rollback — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md)

---

## &sect;1 Core Rollback

<a id="RBK-001"></a>**RBK-001** `rollback_to_block(target_height: i64)` MUST return a `RollbackResult`. `target_height` MAY be negative, which MUST roll back all blocks (full reset).
> **Spec:** [`RBK-001.md`](specs/RBK-001.md)

<a id="RBK-002"></a>**RBK-002** Coins confirmed after `target_height` MUST be deleted. Pre-deletion records MUST be saved to the `modified_coins` map in the `RollbackResult`.
> **Spec:** [`RBK-002.md`](specs/RBK-002.md)

<a id="RBK-003"></a>**RBK-003** Coins spent after `target_height` MUST be marked unspent (`spent_height = None`). Coins MUST be added to `modified_coins` only if not already captured by the deletion pass (RBK-002).
> **Spec:** [`RBK-003.md`](specs/RBK-003.md)

---

## &sect;2 Fast-Forward Eligibility

<a id="RBK-004"></a>**RBK-004** When un-spending a coin, if `coinbase == false` AND a parent coin exists with the same `puzzle_hash` and `amount` AND the parent is spent, the coin MUST be marked `ff_eligible = true`. Otherwise, the coin MUST be marked `ff_eligible = false`.
> **Spec:** [`RBK-004.md`](specs/RBK-004.md)

---

## &sect;3 Convenience Wrapper

<a id="RBK-005"></a>**RBK-005** `rollback_n_blocks(n: u64)` MUST be a convenience wrapper that calls `rollback_to_block(self.height() - n)`.
> **Spec:** [`RBK-005.md`](specs/RBK-005.md)

---

## &sect;4 Merkle Tree Rebuild

<a id="RBK-006"></a>**RBK-006** Rollback MUST batch-update the Merkle tree by removing entries for deleted coins and re-hashing un-spent coins. The update MUST use batch operations for a single root recomputation. See MRK-001.
> **Spec:** [`RBK-006.md`](specs/RBK-006.md)

---

## &sect;5 Atomicity

<a id="RBK-007"></a>**RBK-007** Rollback MUST be atomic. If any step fails (storage error during deletion, un-spending, or Merkle rebuild), the entire rollback MUST be aborted with no partial state changes.
> **Spec:** [`RBK-007.md`](specs/RBK-007.md)
