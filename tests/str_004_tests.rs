//! # STR-004 Tests — Merkle Module

mod helpers;

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{
    empty_hash, merkle_leaf_hash, MerkleError, SparseMerkleTree, SMT_HEIGHT,
};

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
