//! # MRK-004 Tests — Proof generation (`get_coin_proof`)
//!
//! **Normative:** [`MRK-004`](../../docs/requirements/domains/merkle/NORMATIVE.md#MRK-004)
//! **Spec:** [`MRK-004.md`](../../docs/requirements/domains/merkle/specs/MRK-004.md) (method
//! signature, 256 siblings, inclusion vs non-inclusion, dirty-tree rule)
//! **Verification matrix:** [`VERIFICATION.md`](../../docs/requirements/domains/merkle/VERIFICATION.md)
//!
//! MRK-005 static verification is exercised here only where it closes the MRK-004 causal chain
//! (“generated proof verifies against the same `root()`”). Dedicated MRK-005 edge cases live in
//! [`tests/mrk_005_tests.rs`](./mrk_005_tests.rs).
//!
//! ## How these tests prove MRK-004
//!
//! | MRK-004 acceptance / test-plan idea | Evidence in this file |
//! |-------------------------------------|-------------------------|
//! | `get_coin_proof` exists with `Result<_, MerkleError>` | Every happy path `unwrap()`; [`vv_req_mrk_004_dirty_tree_returns_error`] matches [`MerkleError::ProofRequiresCleanTree`] |
//! | Exactly 256 sibling digests | [`vv_req_mrk_004_proof_sibling_count_is_256`] on arbitrary key |
//! | Inclusion: canonical leaf equals inserted leaf hash | [`vv_req_mrk_004_inclusion_leaf_value_matches_inserted_hash`] uses [`SparseMerkleProof::leaf_value`] + `value == Some` |
//! | Non-inclusion: leaf equals MRK-002 empty leaf hash | [`vv_req_mrk_004_exclusion_leaf_value_is_empty_hash`] compares to [`empty_hash`](dig_coinstore::merkle::empty_hash)`(0)` |
//! | Proof verifies vs `root()` (MRK-005) | [`vv_req_mrk_004_inclusion_proof_verifies`], multi-leaf + empty-tree cases |
//! | After `batch_update`, proof tracks new leaf | [`vv_req_mrk_004_proof_after_leaf_update`] |
//! | Dirty tree → error (no silent stale root) | [`vv_req_mrk_004_dirty_tree_returns_error`] |
//! | Persist → reload → `get_coin_proof` (MRK-003 wire + resident leaves) | [`rocks_mrk004::vv_req_mrk_004_get_coin_proof_after_persist_and_load`] behind `rocksdb-storage` |
//!
//! **Lazy `merkle_nodes` sibling reads without a resident leaf map** are not exercised here: with
//! the authoritative leaf `HashMap` loaded from disk, sibling recomputation from that multiset is
//! equivalent to the persisted state root (see MRK-003 tracking notes). A future enhancement can
//! thread `StorageBackend` into the walk for partial hydration.

mod helpers;

use std::collections::HashMap;

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{
    empty_hash, merkle_leaf_hash, MerkleError, SparseMerkleTree, SMT_HEIGHT,
};

// ─────────────────────────────────────────────────────────────────────────────
// MRK-004: `get_coin_proof` + `SparseMerkleProof::leaf_value`
// ─────────────────────────────────────────────────────────────────────────────

/// MRK-004 §Method + MRK-005: Inclusion proof from [`SparseMerkleTree::get_coin_proof`] verifies
/// against the same [`SparseMerkleTree::root`] that defined the trusted commitment.
#[test]
fn vv_req_mrk_004_inclusion_proof_verifies() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x42u8; 32]);
    let value = merkle_leaf_hash(b"coin_data");

    tree.batch_insert(&[(key, value)]).unwrap();
    let root = tree.root();

    let proof = tree.get_coin_proof(&key).unwrap();
    assert_eq!(proof.value, Some(value));
    assert_eq!(proof.leaf_value(), value);
    assert!(
        proof.verify(&root),
        "MRK-004 proof must be MRK-005-verifiable against the tree root"
    );
}

/// MRK-004 test plan `test_proof_sibling_count`: sibling vector length is exactly `SMT_HEIGHT`.
#[test]
fn vv_req_mrk_004_proof_sibling_count_is_256() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x17u8; 32]);
    tree.batch_insert(&[(key, merkle_leaf_hash(b"x"))]).unwrap();
    let _ = tree.root();

    let proof = tree.get_coin_proof(&key).unwrap();
    assert_eq!(
        proof.siblings.len(),
        SMT_HEIGHT,
        "MRK-004 requires one sibling digest per tree level"
    );
}

/// MRK-004 acceptance: for an existing coin, [`SparseMerkleProof::leaf_value`] is the stored
/// record digest (here: [`merkle_leaf_hash`] output used as the SMT leaf).
#[test]
fn vv_req_mrk_004_inclusion_leaf_value_matches_inserted_hash() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x03u8; 32]);
    let leaf = merkle_leaf_hash(b"leaf_payload");
    tree.batch_insert(&[(key, leaf)]).unwrap();
    let _ = tree.root();

    let proof = tree.get_coin_proof(&key).unwrap();
    assert_eq!(proof.leaf_value(), leaf);
}

/// MRK-004 acceptance: absent key → non-inclusion; canonical leaf is MRK-002 empty leaf hash.
#[test]
fn vv_req_mrk_004_exclusion_leaf_value_is_empty_hash() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    let absent = Bytes32::from([0x99u8; 32]);

    let proof = tree.get_coin_proof(&absent).unwrap();
    assert!(proof.value.is_none());
    assert_eq!(
        proof.leaf_value(),
        empty_hash(0),
        "exclusion proofs must expose the empty-leaf sentinel hash at level 0"
    );
    assert!(proof.verify(&root));
}

/// MRK-004: Proof on an empty committed tree (clean, no inserts) — exclusion for any `coin_id`.
#[test]
fn vv_req_mrk_004_proof_empty_tree_exclusion() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    let key = Bytes32::from([0xEEu8; 32]);

    let proof = tree.get_coin_proof(&key).unwrap();
    assert!(proof.value.is_none());
    assert_eq!(proof.leaf_value(), empty_hash(0));
    assert!(proof.verify(&root));
}

/// MRK-004 test plan `test_proof_with_multiple_leaves`: several inclusion proofs + one exclusion,
/// all against one `root()`.
#[test]
fn vv_req_mrk_004_proof_with_multiple_leaves() {
    let mut tree = SparseMerkleTree::new();

    let entries: Vec<(Bytes32, Bytes32)> = (0..5u8)
        .map(|i| (Bytes32::from([i; 32]), merkle_leaf_hash(&[i])))
        .collect();
    tree.batch_insert(&entries).unwrap();
    let root = tree.root();

    for (key, value) in &entries {
        let proof = tree.get_coin_proof(key).unwrap();
        assert_eq!(proof.value, Some(*value));
        assert!(proof.verify(&root));
    }

    let absent = Bytes32::from([0xFFu8; 32]);
    let proof = tree.get_coin_proof(&absent).unwrap();
    assert!(proof.value.is_none());
    assert!(proof.verify(&root));
}

/// MRK-004 test plan `test_proof_after_update`: mutate leaf, re-`root()`, proof leaf tracks digest.
#[test]
fn vv_req_mrk_004_proof_after_leaf_update() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xAAu8; 32]);
    let v0 = merkle_leaf_hash(b"v0");
    let v1 = merkle_leaf_hash(b"v1_replaced");
    tree.batch_insert(&[(key, v0)]).unwrap();
    let _ = tree.root();
    tree.batch_update(&[(key, v1)]).unwrap();
    let root = tree.root();

    let proof = tree.get_coin_proof(&key).unwrap();
    assert_eq!(proof.leaf_value(), v1);
    assert!(proof.verify(&root));
}

/// MRK-004 §Behavior item 6: mutations without an intervening [`SparseMerkleTree::root`] keep the
/// tree “dirty”; `get_coin_proof` must not fabricate a proof tied to a stale cached root.
#[test]
fn vv_req_mrk_004_dirty_tree_returns_error() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0xC0u8; 32]);
    tree.batch_insert(&[(key, merkle_leaf_hash(b"d"))]).unwrap();
    assert!(
        tree.is_dirty(),
        "precondition: MRK-001 defers recompute until root()"
    );

    let err = tree.get_coin_proof(&key).unwrap_err();
    assert_eq!(err, MerkleError::ProofRequiresCleanTree);
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-003 × MRK-004 integration (RocksDB): proof after metadata + `merkle_nodes` flush
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "rocksdb-storage")]
mod rocks_mrk004 {
    use super::*;
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::rocksdb::RocksDbBackend;
    use dig_coinstore::storage::{StorageBackend, WriteBatch};

    /// MRK-004 test plan `test_proof_with_lazy_loading` (adapted): after MRK-003 flush + reload,
    /// [`SparseMerkleTree::get_coin_proof`] still yields an MRK-005-verifiable path against the
    /// metadata root while the leaf map is fully resident (see module-level note on CF-only walks).
    #[test]
    fn vv_req_mrk_004_get_coin_proof_after_persist_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CoinStoreConfig::default_with_path(dir.path());
        let backend = RocksDbBackend::open(&cfg).unwrap();

        let key = Bytes32::from([0x55u8; 32]);
        let lh = merkle_leaf_hash(b"persist_proof");
        let leaves = HashMap::from([(key, lh)]);

        let mut tree = SparseMerkleTree::new();
        tree.batch_insert(&[(key, lh)]).unwrap();
        let expect_root = tree.root();
        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        backend.batch_write(batch).unwrap();

        let mut loaded = SparseMerkleTree::load_from_store(&backend, leaves).unwrap();
        let root = loaded.root();
        assert_eq!(root, expect_root);

        let proof = loaded.get_coin_proof(&key).unwrap();
        assert_eq!(proof.leaf_value(), lh);
        assert!(proof.verify(&root));
    }
}
