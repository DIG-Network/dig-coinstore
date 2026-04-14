# Concurrency — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Section 11, Section 13.8-13.9

---

## &sect;1 Thread Safety

<a id="CON-001"></a>**CON-001** `CoinStore` MUST implement `Send + Sync`. All public methods MUST be callable from any thread without external synchronization.
> **Spec:** [`CON-001.md`](specs/CON-001.md)

---

## &sect;2 RwLock Strategy

<a id="CON-002"></a>**CON-002** The implementation MUST use a reader-writer lock strategy. All query methods MUST acquire a shared read lock. `apply_block` and `rollback` MUST acquire an exclusive write lock. The implementation SHOULD use `parking_lot::RwLock` for performance.
> **Spec:** [`CON-002.md`](specs/CON-002.md)

---

## &sect;3 MVCC Reads During Writes

<a id="CON-003"></a>**CON-003** Readers MUST see a consistent pre-block state while `apply_block` Phase 2 is executing. For RocksDB, the implementation MUST take a snapshot before the write and readers MUST use that snapshot. For LMDB, natural MVCC semantics MUST be leveraged. The tip MUST be swapped atomically after the write batch commits.
> **Spec:** [`CON-003.md`](specs/CON-003.md)

---

## &sect;4 Parallel Removal Validation

<a id="CON-004"></a>**CON-004** Phase 1 removal checks SHOULD be parallelized via `rayon` `par_iter` over the in-memory unspent set. This MUST be lock-free since the unspent set is read-only during the validation phase.
> **Spec:** [`CON-004.md`](specs/CON-004.md)
