//! # MRK-004 Tests — Proof Generation

mod helpers;

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{merkle_leaf_hash, SparseMerkleTree};

// ─────────────────────────────────────────────────────────────────────────────
// MRK-004: Proof generation
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies MRK-004/005: Inclusion proof for an existing key verifies correctly.
#[test]
fn vv_req_mrk_004_inclusion_proof() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x42u8; 32]);
    let value = merkle_leaf_hash(b"coin_data");

    tree.batch_insert(&[(key, value)]).unwrap();
    let root = tree.root();

    let proof = tree.get_proof(&key);
    assert_eq!(
        proof.value,
        Some(value),
        "Inclusion proof must carry the leaf value"
    );
    assert!(
        proof.verify(&root),
        "Inclusion proof must verify against the tree root"
    );
}

/// Verifies MRK-004: Proof for empty tree (exclusion proof for any key).
#[test]
fn vv_req_mrk_004_proof_empty_tree() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    let key = Bytes32::from([0x99u8; 32]);

    let proof = tree.get_proof(&key);
    assert_eq!(proof.value, None);
    assert!(
        proof.verify(&root),
        "Exclusion proof in empty tree must verify"
    );
}

/// Verifies MRK-004/005: Proof still valid after multiple inserts.
#[test]
fn vv_req_mrk_004_proof_with_multiple_leaves() {
    let mut tree = SparseMerkleTree::new();

    // Insert 5 leaves.
    let entries: Vec<(Bytes32, Bytes32)> = (0..5u8)
        .map(|i| (Bytes32::from([i; 32]), merkle_leaf_hash(&[i])))
        .collect();
    tree.batch_insert(&entries).unwrap();
    let root = tree.root();

    // Verify inclusion proof for each inserted leaf.
    for (key, value) in &entries {
        let proof = tree.get_proof(key);
        assert_eq!(proof.value, Some(*value));
        assert!(
            proof.verify(&root),
            "Inclusion proof must verify for key {:?}",
            key
        );
    }

    // Verify exclusion proof for a key NOT in the tree.
    let absent = Bytes32::from([0xFFu8; 32]);
    let proof = tree.get_proof(&absent);
    assert_eq!(proof.value, None);
    assert!(
        proof.verify(&root),
        "Exclusion proof must verify for absent key"
    );
}
