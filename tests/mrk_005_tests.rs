//! # MRK-005 Tests — Proof Verification
//!
//! Verifies **MRK-005**: `SparseMerkleProof::verify()` validates proofs against a trusted root.
//! Verification is the counterpart to MRK-004 (proof generation).
//!
//! # Requirement: MRK-005
//! # SPEC.md: §3.13 (Merkle Proofs API), §1.6 #3 (Proofs — Improvement Over Chia)
//!
//! ## How these tests prove the requirement
//!
//! - **Exclusion verification:** Non-existent key with sibling present verifies with `value = None`.
//! - **Wrong root rejection:** Valid proof does NOT verify against an arbitrary root.
//! - **Multi-leaf verification:** All inclusion + exclusion proofs verify against the correct root.
//!
//! `verify()` is a **static method** — no tree state needed, only proof data + trusted root
//! ([SPEC.md §3.13](../../docs/resources/SPEC.md)). Critical for light clients.

mod helpers;

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{merkle_leaf_hash, SparseMerkleTree};

// ─────────────────────────────────────────────────────────────────────────────
// MRK-005: Proof verification
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies MRK-004/005: Exclusion proof for a non-existing key verifies correctly.
#[test]
fn vv_req_mrk_005_exclusion_proof() {
    let mut tree = SparseMerkleTree::new();
    let key_present = Bytes32::from([0x01u8; 32]);
    let key_absent = Bytes32::from([0x02u8; 32]);
    let value = merkle_leaf_hash(b"data");

    tree.batch_insert(&[(key_present, value)]).unwrap();
    let root = tree.root();

    let proof = tree.get_coin_proof(&key_absent).unwrap();
    assert_eq!(proof.value, None, "Exclusion proof must have None value");
    assert!(
        proof.verify(&root),
        "Exclusion proof must verify against the tree root"
    );
}

/// Verifies MRK-005: Proof against wrong root returns false.
#[test]
fn vv_req_mrk_005_proof_invalid_root() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x55u8; 32]);
    let value = merkle_leaf_hash(b"data");

    tree.batch_insert(&[(key, value)]).unwrap();
    let _root = tree.root();

    let proof = tree.get_coin_proof(&key).unwrap();
    let wrong_root = Bytes32::from([0xFFu8; 32]);
    assert!(
        !proof.verify(&wrong_root),
        "Proof must NOT verify against wrong root"
    );
}

/// Verifies MRK-005: Proof verification works with multiple leaves in tree.
#[test]
fn vv_req_mrk_005_proof_with_multiple_leaves() {
    let mut tree = SparseMerkleTree::new();

    // Insert 5 leaves.
    let entries: Vec<(Bytes32, Bytes32)> = (0..5u8)
        .map(|i| (Bytes32::from([i; 32]), merkle_leaf_hash(&[i])))
        .collect();
    tree.batch_insert(&entries).unwrap();
    let root = tree.root();

    // Verify inclusion proof for each inserted leaf.
    for (key, value) in &entries {
        let proof = tree.get_coin_proof(key).unwrap();
        assert_eq!(proof.value, Some(*value));
        assert!(
            proof.verify(&root),
            "Inclusion proof must verify for key {:?}",
            key
        );
    }

    // Verify exclusion proof for a key NOT in the tree.
    let absent = Bytes32::from([0xFFu8; 32]);
    let proof = tree.get_coin_proof(&absent).unwrap();
    assert_eq!(proof.value, None);
    assert!(
        proof.verify(&root),
        "Exclusion proof must verify for absent key"
    );
}
