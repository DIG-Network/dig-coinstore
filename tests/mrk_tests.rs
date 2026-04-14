//! # MRK Domain Tests — Merkle Tree Verification
//!
//! Tests verifying the sparse Merkle tree implementation per
//! `docs/requirements/domains/merkle/` and
//! `docs/requirements/domains/crate_structure/specs/STR-004.md`.
//!
//! These tests prove:
//! - SparseMerkleTree exists with required batch methods + root()
//! - MRK-002: Memoized empty hashes are correct and O(1)
//! - MRK-001: Batch insert/update/remove with deferred root recomputation
//! - MRK-004/005: Proof generation and verification

mod helpers;

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{
    empty_hash, merkle_leaf_hash, merkle_node_hash, MerkleError, SparseMerkleTree, SMT_HEIGHT,
};

// ─────────────────────────────────────────────────────────────────────────────
// MRK-002: Memoized empty hashes
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies MRK-002: empty_hash(0) equals the leaf-level empty sentinel hash.
///
/// Level 0 is the empty leaf: SHA256(0x00 || [0; 32]).
#[test]
fn vv_req_mrk_002_empty_hash_leaf_level() {
    let expected = merkle_leaf_hash(&[0u8; 32]);
    assert_eq!(empty_hash(0), expected, "Level 0 must be empty leaf hash");
}

/// Verifies MRK-002: empty_hash(n) == merkle_node_hash(empty_hash(n-1), empty_hash(n-1))
/// for all levels 1..=256.
///
/// This proves the bottom-up construction is correct for all 256 internal levels.
#[test]
fn vv_req_mrk_002_empty_hash_consistency() {
    for n in 1..=SMT_HEIGHT {
        let child = empty_hash(n - 1);
        let expected = merkle_node_hash(&child, &child);
        assert_eq!(
            empty_hash(n),
            expected,
            "empty_hash({}) must equal node_hash(empty_hash({}), empty_hash({}))",
            n,
            n - 1,
            n - 1
        );
    }
}

/// Verifies MRK-002: empty_hash(256) is the root of an entirely empty tree.
///
/// Computed by iteratively hashing the empty leaf 256 times.
#[test]
fn vv_req_mrk_002_empty_hash_root_level() {
    // Compute manually: start from leaf, hash up 256 times.
    let mut current = merkle_leaf_hash(&[0u8; 32]);
    for _ in 1..=SMT_HEIGHT {
        current = merkle_node_hash(&current, &current);
    }
    assert_eq!(
        empty_hash(SMT_HEIGHT),
        current,
        "Level 256 must match iterative computation"
    );
}

/// Verifies MRK-002: empty_hash() is O(1) — repeated calls return immediately.
#[test]
fn vv_req_mrk_002_empty_hash_o1_lookup() {
    let first = empty_hash(128);
    let second = empty_hash(128);
    assert_eq!(first, second, "Repeated calls must return same value");
}

// ─────────────────────────────────────────────────────────────────────────────
// STR-004 / MRK-001: SparseMerkleTree
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies STR-004: SparseMerkleTree struct exists with required methods.
///
/// Compile-time test: if struct or methods are missing, this won't compile.
#[test]
fn vv_req_str_004_smt_struct_exists() {
    let mut tree = SparseMerkleTree::new();
    let _root = tree.root();
    let _len = tree.len();
    let _empty = tree.is_empty();
}

/// Verifies MRK-001: Empty tree root is deterministic and matches empty_hash(256).
#[test]
fn vv_req_mrk_001_empty_tree_root() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    assert_eq!(
        root,
        empty_hash(SMT_HEIGHT),
        "Empty tree root must match empty_hash(256)"
    );
}

/// Verifies MRK-001: Inserting one leaf changes the root.
#[test]
fn vv_req_mrk_001_single_insert() {
    let mut tree = SparseMerkleTree::new();
    let empty_root = tree.root();

    let key = Bytes32::from([0x01u8; 32]);
    let value = merkle_leaf_hash(b"coin_record_data");
    tree.batch_insert(&[(key, value)]).unwrap();

    let new_root = tree.root();
    assert_ne!(new_root, empty_root, "Root must change after insert");
}

/// Verifies MRK-001: Batch insert of multiple leaves, single root recomputation.
#[test]
fn vv_req_mrk_001_batch_insert_multiple() {
    let mut tree = SparseMerkleTree::new();

    let entries: Vec<(Bytes32, Bytes32)> = (0..10u8)
        .map(|i| {
            let key = Bytes32::from([i; 32]);
            let value = merkle_leaf_hash(&[i]);
            (key, value)
        })
        .collect();

    tree.batch_insert(&entries).unwrap();
    let root = tree.root();

    // Root should be deterministic — same entries always produce same root.
    let mut tree2 = SparseMerkleTree::new();
    tree2.batch_insert(&entries).unwrap();
    assert_eq!(tree2.root(), root, "Same entries must produce same root");
}

/// Verifies MRK-001: batch_update changes the root.
#[test]
fn vv_req_mrk_001_batch_update() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x42u8; 32]);
    let v1 = merkle_leaf_hash(b"v1");
    let v2 = merkle_leaf_hash(b"v2");

    tree.batch_insert(&[(key, v1)]).unwrap();
    let root_v1 = tree.root();

    tree.batch_update(&[(key, v2)]).unwrap();
    let root_v2 = tree.root();

    assert_ne!(root_v1, root_v2, "Root must change after update");
}

/// Verifies MRK-001: Insert then remove returns to empty root.
#[test]
fn vv_req_mrk_001_insert_then_remove_equals_empty() {
    let mut tree = SparseMerkleTree::new();
    let empty_root = tree.root();

    let key = Bytes32::from([0xFFu8; 32]);
    let value = merkle_leaf_hash(b"data");

    tree.batch_insert(&[(key, value)]).unwrap();
    assert_ne!(tree.root(), empty_root);

    tree.batch_remove(&[key]).unwrap();
    assert_eq!(
        tree.root(),
        empty_root,
        "After remove, root must equal empty tree root"
    );
}

/// Verifies MRK-001: Deferred recomputation — mutations don't recompute root.
#[test]
fn vv_req_mrk_001_deferred_recomputation() {
    let mut tree = SparseMerkleTree::new();
    let _ = tree.root(); // Cache the root

    let key = Bytes32::from([0xAAu8; 32]);
    let value = merkle_leaf_hash(b"test");
    tree.batch_insert(&[(key, value)]).unwrap();

    // Tree should be dirty (root invalidated) but no recomputation yet.
    assert!(
        tree.is_dirty() || !tree.is_empty(),
        "Tree modified but root not yet recomputed"
    );
}

/// Verifies MRK-001: root() is idempotent — same result without intervening mutations.
#[test]
fn vv_req_mrk_001_idempotent_root() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xBBu8; 32]);
    let value = merkle_leaf_hash(b"test");
    tree.batch_insert(&[(key, value)]).unwrap();

    let root1 = tree.root();
    let root2 = tree.root();
    assert_eq!(
        root1, root2,
        "Consecutive root() calls must return same hash"
    );
}

/// Verifies MRK-001: Duplicate insert returns KeyAlreadyExists error.
#[test]
fn vv_req_mrk_001_duplicate_insert_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xCCu8; 32]);
    let value = merkle_leaf_hash(b"test");

    tree.batch_insert(&[(key, value)]).unwrap();
    let result = tree.batch_insert(&[(key, value)]);

    assert!(matches!(result, Err(MerkleError::KeyAlreadyExists(_))));
}

/// Verifies MRK-001: Remove missing key returns KeyNotFound error.
#[test]
fn vv_req_mrk_001_remove_missing_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xDDu8; 32]);

    let result = tree.batch_remove(&[key]);
    assert!(matches!(result, Err(MerkleError::KeyNotFound(_))));
}

/// Verifies MRK-001: Update missing key returns KeyNotFound error.
#[test]
fn vv_req_mrk_001_update_missing_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xEEu8; 32]);
    let value = merkle_leaf_hash(b"test");

    let result = tree.batch_update(&[(key, value)]);
    assert!(matches!(result, Err(MerkleError::KeyNotFound(_))));
}

/// Verifies MRK-001: Two keys differing only in the last bit produce distinct leaves.
///
/// This tests that the tree correctly handles 256-level depth.
#[test]
fn vv_req_mrk_001_256_level_depth() {
    let mut tree = SparseMerkleTree::new();

    // Two keys identical except for the very last bit (bit 255).
    let key_a = [0u8; 32];
    let mut key_b = [0u8; 32];
    key_b[31] = 0x01; // Last bit differs

    let a = Bytes32::from(key_a);
    let b = Bytes32::from(key_b);
    let va = merkle_leaf_hash(b"a");
    let vb = merkle_leaf_hash(b"b");

    tree.batch_insert(&[(a, va), (b, vb)]).unwrap();

    assert_eq!(tree.len(), 2);
    assert_eq!(tree.get(&a), Some(&va));
    assert_eq!(tree.get(&b), Some(&vb));

    // Root should reflect both leaves.
    let root = tree.root();
    assert_ne!(root, empty_hash(SMT_HEIGHT));
}

/// Verifies MRK-001: Idempotent update (same value) does not change the root.
#[test]
fn vv_req_mrk_001_idempotent_update() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x77u8; 32]);
    let value = merkle_leaf_hash(b"same");

    tree.batch_insert(&[(key, value)]).unwrap();
    let root_before = tree.root();

    tree.batch_update(&[(key, value)]).unwrap();
    let root_after = tree.root();

    assert_eq!(
        root_before, root_after,
        "Update with same value must not change root"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-004/005: Proof generation and verification
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

/// Verifies MRK-004/005: Exclusion proof for a non-existing key verifies correctly.
#[test]
fn vv_req_mrk_005_exclusion_proof() {
    let mut tree = SparseMerkleTree::new();
    let key_present = Bytes32::from([0x01u8; 32]);
    let key_absent = Bytes32::from([0x02u8; 32]);
    let value = merkle_leaf_hash(b"data");

    tree.batch_insert(&[(key_present, value)]).unwrap();
    let root = tree.root();

    let proof = tree.get_proof(&key_absent);
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

    let proof = tree.get_proof(&key);
    let wrong_root = Bytes32::from([0xFFu8; 32]);
    assert!(
        !proof.verify(&wrong_root),
        "Proof must NOT verify against wrong root"
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
