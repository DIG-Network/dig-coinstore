//! # MRK-005 — Proof verification (`verify_coin_proof` / [`SparseMerkleProof::verify`])
//!
//! **Normative:** `docs/requirements/domains/merkle/NORMATIVE.md#MRK-005`  
//! **Spec + test plan:** `docs/requirements/domains/merkle/specs/MRK-005.md`  
//! **Master SPEC:** `docs/resources/SPEC.md` (Merkle proofs / light-client verification)
//!
//! ## What this requirement demands
//!
//! Static verification: given only a [`dig_coinstore::merkle::SparseMerkleProof`] and a trusted
//! [`Bytes32`] root, decide whether the proof is consistent with that root. No database, no
//! [`dig_coinstore::merkle::SparseMerkleTree`] instance, and no hidden global state may participate
//! (MRK-005 §Behavior item 7). Both inclusion (`value = Some(leaf_hash)`) and exclusion (`value =
//! None`, empty-leaf sentinel via MRK-002) must use the **same** walk as MRK-004 sibling ordering.
//!
//! ## How passing tests prove satisfaction
//!
//! Each test maps to rows in MRK-005.md §Test Plan: valid inclusion / exclusion return `true`;
//! tampered siblings or leaf material return `false`; wrong roots return `false`; the empty-tree
//! edge matches MRK-002 `empty_hash(SMT_HEIGHT)`; snapshot semantics show a proof remains valid for
//! the root it was minted under after further mutations. A second, test-local reimplementation of
//! the walk (same `merkle_node_hash` / `empty_hash` / MSB-first bit rule) must agree with
//! production code — that is our “independent reference” within the repo until a Chia-native SMT
//! oracle is wired (chia-consensus is a dev-dep today but does not expose this exact sparse shape).
//!
//! # Requirement: MRK-005

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{
    empty_hash, merkle_leaf_hash, merkle_node_hash, verify_coin_proof, SparseMerkleProof,
    SparseMerkleTree, SMT_HEIGHT,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test-local reference verifier (MRK-005 § cross-implementation)
// ─────────────────────────────────────────────────────────────────────────────

/// Duplicate MRK-005’s walk using only public hashing helpers — must match [`SparseMerkleProof::verify`].
///
/// Rationale: MRK-005 asks for agreement with an independent implementation; duplicating the
/// algorithm here catches accidental drift between proof generation (`build_sparse_proof_for_key`)
/// and verification ordering.
fn reference_verify(proof: &SparseMerkleProof, expected_root: &Bytes32) -> bool {
    if proof.siblings.len() != SMT_HEIGHT {
        return false;
    }
    let leaf_hash = match &proof.value {
        Some(v) => *v,
        None => empty_hash(0),
    };
    let mut current = leaf_hash;
    for depth in (0..SMT_HEIGHT).rev() {
        let bit = SparseMerkleTree::get_bit_public(&proof.key, depth);
        let sibling = &proof.siblings[depth];
        current = if bit {
            merkle_node_hash(sibling, &current)
        } else {
            merkle_node_hash(&current, sibling)
        };
    }
    current == *expected_root
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-005 §Test Plan — positive paths
// ─────────────────────────────────────────────────────────────────────────────

/// MRK-005 / test plan `test_verify_inclusion_valid`: inclusion proof verifies against the tree root.
#[test]
fn vv_req_mrk_005_verify_inclusion_valid() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x11u8; 32]);
    let leaf = merkle_leaf_hash(b"coin-payload");
    tree.batch_insert(&[(key, leaf)]).unwrap();
    let root = tree.root();
    let proof = tree.get_coin_proof(&key).unwrap();
    assert_eq!(proof.value, Some(leaf));
    assert!(proof.verify(&root));
    assert!(verify_coin_proof(&proof, &root));
    assert!(reference_verify(&proof, &root));
}

/// MRK-005 / test plan `test_verify_non_inclusion_valid`: absent key → `value = None`, still verifies.
#[test]
fn vv_req_mrk_005_verify_non_inclusion_valid() {
    let mut tree = SparseMerkleTree::new();
    let present = Bytes32::from([0x01u8; 32]);
    let absent = Bytes32::from([0x02u8; 32]);
    let leaf = merkle_leaf_hash(b"v");
    tree.batch_insert(&[(present, leaf)]).unwrap();
    let root = tree.root();
    let proof = tree.get_coin_proof(&absent).unwrap();
    assert!(proof.value.is_none());
    assert!(proof.verify(&root));
    assert!(verify_coin_proof(&proof, &root));
}

/// MRK-005 / test plan `test_verify_empty_tree_proof`: exclusion proof for any key vs empty-tree root.
#[test]
fn vv_req_mrk_005_verify_empty_tree_proof() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    assert_eq!(root, empty_hash(SMT_HEIGHT), "MRK-002 empty-tree root");
    let key = Bytes32::from([0xABu8; 32]);
    let proof = tree.get_coin_proof(&key).unwrap();
    assert!(proof.verify(&root));
    assert!(verify_coin_proof(&proof, &root));
}

/// MRK-005 / test plan `test_verify_roundtrip`: MRK-004 `get_coin_proof` + MRK-005 verify share one root.
#[test]
fn vv_req_mrk_005_verify_roundtrip_multi_leaf() {
    let mut tree = SparseMerkleTree::new();
    let entries: Vec<(Bytes32, Bytes32)> = (0..5u8)
        .map(|i| (Bytes32::from([i; 32]), merkle_leaf_hash(&[i])))
        .collect();
    tree.batch_insert(&entries).unwrap();
    let root = tree.root();

    for (k, _) in &entries {
        let p = tree.get_coin_proof(k).unwrap();
        assert!(verify_coin_proof(&p, &root), "inclusion {:?}", k);
        assert_eq!(reference_verify(&p, &root), verify_coin_proof(&p, &root));
    }
    let absent = Bytes32::from([0xFFu8; 32]);
    let p = tree.get_coin_proof(&absent).unwrap();
    assert!(verify_coin_proof(&p, &root), "exclusion");
    assert!(reference_verify(&p, &root));
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-005 §Test Plan — negative paths
// ─────────────────────────────────────────────────────────────────────────────

/// MRK-005 / test plan `test_verify_tampered_sibling`: flip one sibling digest → verification fails.
#[test]
fn vv_req_mrk_005_verify_tampered_sibling() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x33u8; 32]);
    let leaf = merkle_leaf_hash(b"x");
    tree.batch_insert(&[(key, leaf)]).unwrap();
    let root = tree.root();
    let mut proof = tree.get_coin_proof(&key).unwrap();
    let idx = 120usize;
    let mut bytes = proof.siblings[idx].as_ref().to_vec();
    bytes[0] ^= 0x01;
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    proof.siblings[idx] = Bytes32::from(arr);
    assert!(!proof.verify(&root));
    assert!(!verify_coin_proof(&proof, &root));
}

/// MRK-005 / test plan `test_verify_tampered_leaf`: change claimed leaf hash while siblings stay valid.
#[test]
fn vv_req_mrk_005_verify_tampered_leaf() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x44u8; 32]);
    let leaf = merkle_leaf_hash(b"good");
    tree.batch_insert(&[(key, leaf)]).unwrap();
    let root = tree.root();
    let mut proof = tree.get_coin_proof(&key).unwrap();
    proof.value = Some(merkle_leaf_hash(b"evil"));
    assert!(!proof.verify(&root));
    assert!(!verify_coin_proof(&proof, &root));
}

/// MRK-005 / test plan `test_verify_wrong_root`: structurally valid proof, untrusted root mismatch.
#[test]
fn vv_req_mrk_005_verify_wrong_root() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x55u8; 32]);
    tree.batch_insert(&[(key, merkle_leaf_hash(b"data"))])
        .unwrap();
    let _root = tree.root();
    let proof = tree.get_coin_proof(&key).unwrap();
    let wrong = Bytes32::from([0xFFu8; 32]);
    assert!(!verify_coin_proof(&proof, &wrong));
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-005 §Test Plan — integration / snapshot semantics
// ─────────────────────────────────────────────────────────────────────────────

/// MRK-005 / test plan `test_verify_after_mutation`: proof stays valid for the **old** committed root.
#[test]
fn vv_req_mrk_005_verify_snapshot_old_root_still_valid() {
    let mut tree = SparseMerkleTree::new();
    let k1 = Bytes32::from([0x01u8; 32]);
    tree.batch_insert(&[(k1, merkle_leaf_hash(b"a"))]).unwrap();
    let root_v1 = tree.root();
    let proof_v1 = tree.get_coin_proof(&k1).unwrap();
    assert!(verify_coin_proof(&proof_v1, &root_v1));

    let k2 = Bytes32::from([0x02u8; 32]);
    tree.batch_insert(&[(k2, merkle_leaf_hash(b"b"))]).unwrap();
    let root_v2 = tree.root();
    assert_ne!(root_v1, root_v2);

    // Same bytes as right after v1 commit — must still authenticate state v1.
    assert!(verify_coin_proof(&proof_v1, &root_v1));
    // Must not authenticate as the head state without a fresh proof.
    assert!(!verify_coin_proof(&proof_v1, &root_v2));
}

/// MRK-005 / test plan `test_verify_against_wrong_tree_state`: alias of wrong-root after heavy mutation.
#[test]
fn vv_req_mrk_005_verify_against_wrong_tree_state() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x77u8; 32]);
    tree.batch_insert(&[(key, merkle_leaf_hash(b"only"))])
        .unwrap();
    let root_a = tree.root();
    let proof = tree.get_coin_proof(&key).unwrap();

    tree.batch_insert(&[(Bytes32::from([0x88u8; 32]), merkle_leaf_hash(b"extra"))])
        .unwrap();
    let root_b = tree.root();

    assert!(verify_coin_proof(&proof, &root_a));
    assert!(!verify_coin_proof(&proof, &root_b));
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-005 — free function vs method (SPEC naming)
// ─────────────────────────────────────────────────────────────────────────────

/// MRK-005 acceptance: `verify_coin_proof` is callable without a tree; bitwise same as [`SparseMerkleProof::verify`].
#[test]
fn vv_req_mrk_005_verify_coin_proof_matches_method() {
    let mut tree = SparseMerkleTree::new();
    let key = Bytes32::from([0x99u8; 32]);
    tree.batch_insert(&[(key, merkle_leaf_hash(b"z"))]).unwrap();
    let root = tree.root();
    let proof = tree.get_coin_proof(&key).unwrap();
    assert_eq!(proof.verify(&root), verify_coin_proof(&proof, &root));
}

/// Degenerate key / sibling pattern: all-zero key on empty tree still hits MRK-002 root only when root matches.
#[test]
fn vv_req_mrk_005_verify_with_degenerate_input() {
    let mut tree = SparseMerkleTree::new();
    let root = tree.root();
    let key = Bytes32::default();
    let proof = tree.get_coin_proof(&key).unwrap();
    assert!(verify_coin_proof(&proof, &root));
    assert!(!verify_coin_proof(&proof, &Bytes32::from([0xEEu8; 32])));
}
