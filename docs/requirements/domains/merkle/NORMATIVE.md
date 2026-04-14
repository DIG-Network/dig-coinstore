# Merkle Tree — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Section 9 (Merkle Tree), Section 13.4 (Persistent Merkle Tree)

---

## &sect;1 Sparse Merkle Tree

<a id="MRK-001"></a>**MRK-001** `SparseMerkleTree` MUST support `batch_insert`, `batch_update`, `batch_remove`, and `root()`. Root recomputation MUST be deferred until explicitly requested. The tree MUST use 256 levels for `Bytes32` keys. There MUST be at most one root recomputation per block.
> **Spec:** [`MRK-001.md`](specs/MRK-001.md)

<a id="MRK-002"></a>**MRK-002** Empty subtree hashes MUST be pre-computed into a `OnceLock<[Bytes32; 257]>` array. Lookup of an empty subtree hash at any level MUST be O(1). The implementation MUST NOT use recursive computation at query time.
> **Spec:** [`MRK-002.md`](specs/MRK-002.md)

---

## &sect;2 Persistent Storage

<a id="MRK-003"></a>**MRK-003** Internal nodes MUST be persisted in a `merkle_nodes` column family. Dirty nodes MUST be tracked during batch updates. Dirty nodes MUST be flushed incrementally in the same `WriteBatch` as coin records. On startup, the implementation MUST load only the root hash.
> **Spec:** [`MRK-003.md`](specs/MRK-003.md)

---

## &sect;3 Proof Generation & Verification

<a id="MRK-004"></a>**MRK-004** `get_coin_proof(coin_id)` MUST return a `SparseMerkleProof` containing sibling hashes along the 256-level path. The method MUST work for both existing coins (inclusion proof) and non-existing coins (non-inclusion proof).
> **Spec:** [`MRK-004.md`](specs/MRK-004.md)

<a id="MRK-005"></a>**MRK-005** `verify(proof, expected_root) -> bool` MUST be a static method. It MUST verify inclusion proofs and non-inclusion proofs against any trusted state root. Verification MUST NOT require access to the tree's internal state.
> **Spec:** [`MRK-005.md`](specs/MRK-005.md)

---

## &sect;4 Leaf Hash Function

<a id="MRK-006"></a>**MRK-006** The leaf hash function MUST be `coin_record_hash(record) = sha256(bincode::serialize(record))`. This function MUST be deterministic -- the same `CoinRecord` always produces the same hash regardless of platform or serialization version. This hash is used as the leaf value in the sparse Merkle tree.
> **Spec:** [`MRK-006.md`](specs/MRK-006.md)
