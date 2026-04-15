//! # STR-004 Tests — Merkle module is part of the crate surface
//!
//! Verifies **STR-004**: the `merkle` module is compiled, exported from the crate root, and exposes the
//! sparse Merkle tree type used for L2 state roots ([`SPEC.md`](../../docs/resources/SPEC.md) §9, §1.6 #1).
//!
//! **MRK-001 behavioral V&V** lives in the dedicated requirement file [`tests/mrk_001_tests.rs`](mrk_001_tests.rs)
//! (batch insert/update/remove, deferred `root()`, errors, 256-level keys) per
//! [`MRK-001.md`](../../docs/requirements/domains/merkle/specs/MRK-001.md) — STR-004 here only proves the module
//! boundary compiles and [`SparseMerkleTree::new`] / [`SparseMerkleTree::root`] are reachable.
//!
//! # Requirement: STR-004
//! # Spec: docs/requirements/domains/crate_structure/specs/STR-004.md
//! # SPEC.md: §1.3 #13 (custom SMT), §9 (Merkle tree)

use dig_coinstore::merkle::SparseMerkleTree;

/// Verifies STR-004: `SparseMerkleTree` is a public type with the core API used by [`dig_coinstore::CoinStore`].
///
/// **Proof:** If the merkle module were removed or renamed, this integration test binary would fail to compile.
/// MRK-001 acceptance is satisfied by [`mrk_001_tests`](mrk_001_tests.rs), not duplicated here.
#[test]
fn vv_req_str_004_smt_struct_exists() {
    let mut tree = SparseMerkleTree::new();
    let _root = tree.root();
    let _len = tree.len();
    let _empty = tree.is_empty();
}
