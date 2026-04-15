# Merkle Tree — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [MRK-001](NORMATIVE.md#MRK-001) | :white_check_mark: | SparseMerkleTree batch ops and deferred root | `tests/mrk_001_tests.rs`: batch ops, `is_dirty`/deferred recompute, `root()` idempotence, MRK-002 empty root, 100-leaf batch, error paths, 256-bit key split, insert/remove net empty. |
| [MRK-002](NORMATIVE.md#MRK-002) | :x: | Memoized empty hashes | Tests: OnceLock initialization, array has 257 entries, O(1) lookup, values match recursive computation. |
| [MRK-003](NORMATIVE.md#MRK-003) | :x: | Persistent internal nodes | Tests: merkle_nodes CF exists, dirty tracking during batch ops, flush in same WriteBatch as coins, startup loads only root hash. |
| [MRK-004](NORMATIVE.md#MRK-004) | :x: | Proof generation | Tests: inclusion proof for existing coin, non-inclusion proof for missing coin, proof contains 256 sibling hashes. |
| [MRK-005](NORMATIVE.md#MRK-005) | :x: | Proof verification | Tests: static method, inclusion verify returns true, non-inclusion verify returns true, tampered proof returns false, works against any root. |
| [MRK-006](NORMATIVE.md#MRK-006) | :x: | Leaf hash function | Tests: deterministic hash, mutation sensitivity, spent vs unspent, cross-platform regression vector, integration with tree. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
