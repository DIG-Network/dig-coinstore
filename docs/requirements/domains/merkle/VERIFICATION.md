# Merkle Tree — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [MRK-001](NORMATIVE.md#MRK-001) | :white_check_mark: | SparseMerkleTree batch ops and deferred root | `tests/mrk_001_tests.rs`: batch ops, `is_dirty`/deferred recompute, `root()` idempotence, MRK-002 empty root, 100-leaf batch, error paths, 256-bit key split, insert/remove net empty. |
| [MRK-002](NORMATIVE.md#MRK-002) | :white_check_mark: | Memoized empty hashes | `tests/mrk_002_tests.rs`: leaf/root levels, parent consistency for all levels, O(1) repeat lookup, cross-check vs naive recursion, concurrent `Barrier` reads vs serial baseline, out-of-bounds panic. |
| [MRK-003](NORMATIVE.md#MRK-003) | :white_check_mark: | Persistent internal nodes | `tests/mrk_003_tests.rs`: dirty map after `root()`, `flush_to_batch` → `merkle_nodes` + metadata root, atomic batch with `coin_records`, `load_from_store` single-read + mismatch errors, empty-root startup, delete ops on remove-all, proof verify after load; optional LMDB row under `lmdb-storage`. Lazy CF reads during proof remain MRK-004 follow-up. |
| [MRK-004](NORMATIVE.md#MRK-004) | :white_check_mark: | Proof generation | `tests/mrk_004_tests.rs`: `get_coin_proof` + `SparseMerkleProof::leaf_value`, 256 siblings, inclusion/exclusion vs `empty_hash(0)`, MRK-005 verify vs `root()`, dirty-tree `ProofRequiresCleanTree`, post-`batch_update` proof; Rocks flush+`load_from_store` path under `rocksdb-storage`. CF-only sibling hydration without resident leaves remains future work (see TRACKING MRK-004 notes). |
| [MRK-005](NORMATIVE.md#MRK-005) | :x: | Proof verification | Tests: static method, inclusion verify returns true, non-inclusion verify returns true, tampered proof returns false, works against any root. |
| [MRK-006](NORMATIVE.md#MRK-006) | :x: | Leaf hash function | Tests: deterministic hash, mutation sensitivity, spent vs unspent, cross-platform regression vector, integration with tree. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
