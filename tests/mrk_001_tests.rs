//! # MRK-001 Tests — `SparseMerkleTree` batch mutations and deferred root
//!
//! **Normative:** [`MRK-001`](../../docs/requirements/domains/merkle/NORMATIVE.md#MRK-001)
//! **Spec:** [`MRK-001.md`](../../docs/requirements/domains/merkle/specs/MRK-001.md) (behavior §, acceptance criteria, test plan table)
//! **Implementation:** [`dig_coinstore::merkle::SparseMerkleTree`](../../src/merkle/mod.rs) — 256-level SMT, domain-separated
//! hashing ([`merkle_leaf_hash`] / [`merkle_node_hash`]), empty subtrees via [`empty_hash`] (MRK-002 memoization).
//!
//! ## What MRK-001 mandates (and how we prove it)
//!
//! | MRK-001 rule | Evidence in this file |
//! |--------------|------------------------|
//! | `batch_insert` / `batch_update` / `batch_remove` / `root()` exist | compile-time use + each `vv_req_mrk_001_*` |
//! | Duplicate insert → error | [`vv_req_mrk_001_duplicate_insert_error`] |
//! | Update/remove missing → error | [`vv_req_mrk_001_batch_update_missing_key_error`], [`vv_req_mrk_001_remove_missing_error`] |
//! | Mutations defer root work | [`vv_req_mrk_001_deferred_recomputation`] ([`SparseMerkleTree::is_dirty`]) |
//! | `root()` idempotent when clean | [`vv_req_mrk_001_idempotent_root`] |
//! | Empty tree root = MRK-002 sentinel | [`vv_req_mrk_001_empty_tree_root_standalone`], [`vv_req_mrk_001_insert_then_remove_equals_empty`] |
//! | 256-bit key paths | [`vv_req_mrk_001_256_level_depth`] (keys differ only at final bit) |
//! | Large batch, one recompute | [`vv_req_mrk_001_batch_insert_hundred_single_root_call`] |
//!
//! **SocratiCode:** not connected in this workspace; discovery used repo search + Repomix packs per `docs/prompt/start.md`.
//! **GitNexus:** `npx gitnexus status` before edits; `analyze` after commit.

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{
    empty_hash, merkle_leaf_hash, MerkleError, SparseMerkleTree, SMT_HEIGHT,
};

/// **MRK-001 / test plan `test_empty_tree_root_standalone`:** Fresh tree → [`SparseMerkleTree::root`] equals the
/// all-empty 256-level root from [`empty_hash`]`(SMT_HEIGHT)` (MRK-002; NORMATIVE MRK-001 §7 bullet).
#[test]
fn vv_req_mrk_001_empty_tree_root_standalone() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    assert_eq!(
        root,
        empty_hash(SMT_HEIGHT),
        "empty tree commitment must match precomputed empty root"
    );
    assert!(!tree.is_dirty(), "after root(), cached digest should satisfy is_dirty == false for empty leaves");
}

/// **MRK-001 / `test_batch_insert_single`:** One leaf changes the root from the empty-tree root.
#[test]
fn vv_req_mrk_001_batch_insert_single() {
    let mut tree = SparseMerkleTree::new();
    let empty_root = tree.root();

    let key = Bytes32::from([0x01u8; 32]);
    let value = merkle_leaf_hash(b"coin_record_data");
    tree.batch_insert(&[(key, value)]).unwrap();

    let new_root = tree.root();
    assert_ne!(new_root, empty_root, "non-empty leaf set must change the state root");
}

/// **MRK-001 / `test_batch_insert_multiple`:** Many distinct keys, one [`root`] after inserts — deterministic root.
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

    let mut tree2 = SparseMerkleTree::new();
    tree2.batch_insert(&entries).unwrap();
    assert_eq!(tree2.root(), root, "same multiset of (key, leaf_hash) must yield identical root");
}

/// **MRK-001 test plan `test_batch_insert_multiple` (100 leaves):** scales the batch path without assuming root internals.
#[test]
fn vv_req_mrk_001_batch_insert_hundred_single_root_call() {
    let mut tree = SparseMerkleTree::new();
    let entries: Vec<(Bytes32, Bytes32)> = (0..100u16)
        .map(|i| {
            let mut kb = [0u8; 32];
            kb[0..2].copy_from_slice(&i.to_be_bytes());
            let key = Bytes32::from(kb);
            (key, merkle_leaf_hash(&i.to_le_bytes()))
        })
        .collect();
    tree.batch_insert(&entries).unwrap();
    let _ = tree.root();
    assert_eq!(tree.len(), 100);
}

/// **MRK-001 / `test_batch_update`:** [`batch_update`] replaces leaf digest; second [`root`] reflects new value.
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

    assert_ne!(root_v1, root_v2, "changing leaf hash must change the committed root");
}

/// **MRK-001 / `test_batch_remove` + insert/remove inverse:** After remove, root matches never-inserted empty tree.
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
        "removing the last leaf must restore the canonical empty-tree root"
    );
}

/// **MRK-001 implementation notes:** insert then remove **before** any [`root`] between mutations still yields empty root.
#[test]
fn vv_req_mrk_001_insert_remove_without_intervening_root_still_empty_commitment() {
    let mut tree = SparseMerkleTree::new();
    let empty_root = empty_hash(SMT_HEIGHT);
    let key = Bytes32::from([0x33u8; 32]);
    let value = merkle_leaf_hash(b"ephemeral");
    tree.batch_insert(&[(key, value)]).unwrap();
    tree.batch_remove(&[key]).unwrap();
    assert!(tree.is_empty());
    assert_eq!(tree.root(), empty_root);
}

/// **MRK-001 / `test_deferred_recomputation`:** [`batch_insert`] must not synchronously recompute [`root`]; [`is_dirty`]
/// reports stale cache while leaves are non-empty.
#[test]
fn vv_req_mrk_001_deferred_recomputation() {
    let mut tree = SparseMerkleTree::new();
    let _ = tree.root();
    assert!(!tree.is_dirty());

    let key = Bytes32::from([0xAAu8; 32]);
    let value = merkle_leaf_hash(b"test");
    tree.batch_insert(&[(key, value)]).unwrap();

    assert!(
        tree.is_dirty(),
        "MRK-001: after mutation, root_hash stays invalidated until root() recomputes"
    );
}

/// **MRK-001 / `test_idempotent_root`:** Two consecutive [`root`] calls with no mutations return the same [`Bytes32`].
#[test]
fn vv_req_mrk_001_idempotent_root() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xBBu8; 32]);
    let value = merkle_leaf_hash(b"test");
    tree.batch_insert(&[(key, value)]).unwrap();

    let root1 = tree.root();
    let root2 = tree.root();
    assert_eq!(root1, root2);
    assert!(!tree.is_dirty());
}

/// **MRK-001 / `test_duplicate_insert_error`:** Second insert of same coin id → [`MerkleError::KeyAlreadyExists`].
#[test]
fn vv_req_mrk_001_duplicate_insert_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xCCu8; 32]);
    let value = merkle_leaf_hash(b"test");

    tree.batch_insert(&[(key, value)]).unwrap();
    let result = tree.batch_insert(&[(key, value)]);

    assert!(matches!(result, Err(MerkleError::KeyAlreadyExists(_))));
}

/// **MRK-001 / `test_remove_missing_error`:** [`batch_remove`] for unknown id → [`MerkleError::KeyNotFound`].
#[test]
fn vv_req_mrk_001_remove_missing_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xDDu8; 32]);

    let result = tree.batch_remove(&[key]);
    assert!(matches!(result, Err(MerkleError::KeyNotFound(_))));
}

/// **MRK-001 / `test_batch_update_missing_key_error`:** [`batch_update`] without prior insert → `KeyNotFound`.
#[test]
fn vv_req_mrk_001_batch_update_missing_key_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xEEu8; 32]);
    let value = merkle_leaf_hash(b"test");

    let result = tree.batch_update(&[(key, value)]);
    assert!(matches!(result, Err(MerkleError::KeyNotFound(_))));
}

/// **MRK-001 / `test_256_level_depth`:** Keys equal except bit 255 (last bit of the 32-byte key) map to distinct leaves.
#[test]
fn vv_req_mrk_001_256_level_depth() {
    let mut tree = SparseMerkleTree::new();

    let key_a = [0u8; 32];
    let mut key_b = [0u8; 32];
    key_b[31] = 0x01;

    let a = Bytes32::from(key_a);
    let b = Bytes32::from(key_b);
    let va = merkle_leaf_hash(b"a");
    let vb = merkle_leaf_hash(b"b");

    tree.batch_insert(&[(a, va), (b, vb)]).unwrap();

    assert_eq!(tree.len(), 2);
    assert_eq!(tree.get(&a), Some(&va));
    assert_eq!(tree.get(&b), Some(&vb));

    let root = tree.root();
    assert_ne!(root, empty_hash(SMT_HEIGHT));
}

/// **MRK-001 / `test_idempotent_update`:** [`batch_update`] with an unchanged leaf hash must not move the root.
#[test]
fn vv_req_mrk_001_idempotent_update() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x77u8; 32]);
    let value = merkle_leaf_hash(b"same");

    tree.batch_insert(&[(key, value)]).unwrap();
    let root_before = tree.root();

    tree.batch_update(&[(key, value)]).unwrap();
    let root_after = tree.root();

    assert_eq!(root_before, root_after);
}
