//! Merkle proof generation and verification.
//!
//! [`SparseMerkleProof`] contains sibling hashes along a 256-level path.
//! Supports both inclusion proofs (coin exists with a specific value) and
//! non-inclusion proofs (coin does not exist in the tree).
//!
//! # Verification
//!
//! `verify()` is a **static method** — it requires no tree state. A light
//! client can verify a proof against any trusted state root using only the
//! proof data, the key, and the claimed value (or None for exclusion).
//!
//! # Requirements: MRK-004, MRK-005
//! # Spec: `docs/requirements/domains/merkle/specs/MRK-004.md`, `specs/MRK-005.md`

use std::collections::HashMap;

use chia_protocol::Bytes32;
use serde::{Deserialize, Serialize};

use super::{child_path, empty_hash, merkle_node_hash, MerkleError, SparseMerkleTree, SMT_HEIGHT};

/// A proof of inclusion or exclusion in the sparse Merkle tree.
///
/// Contains the 256 sibling hashes along the path from the leaf to the root.
/// For an inclusion proof, `value` is `Some(leaf_hash)`. For an exclusion
/// proof, `value` is `None` (the leaf at this position is empty).
///
/// **MRK-004 wire shape:** NORMATIVE text names a single `leaf_value: Bytes32` (inclusion digest
/// vs empty-leaf hash at level 0 for absence; see [`empty_hash`](crate::merkle::empty_hash)). We
/// keep `Option<Bytes32>` for serde/back-compat and expose the canonical 32-byte leaf via
/// [`Self::leaf_value`].
///
/// # Verification
///
/// Given a trusted root hash, call [`Self::verify`] or [`verify_coin_proof`] to check whether the
/// proof is consistent with that root.
///
/// # Size
///
/// A proof is exactly 256 * 32 = 8,192 bytes of sibling hashes, plus the
/// key (32 bytes), optional value (32 bytes), and a small header.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SparseMerkleProof {
    /// The key (coin_id) this proof is for.
    pub key: Bytes32,
    /// The leaf value if the key exists (inclusion proof), or None (exclusion proof).
    pub value: Option<Bytes32>,
    /// Sibling hashes along the path from leaf to root.
    /// `siblings[0]` is the sibling at depth 0 (root level).
    /// `siblings[255]` is the sibling at depth 255 (leaf level).
    pub siblings: Vec<Bytes32>,
}

impl SparseMerkleProof {
    /// Verify this proof against an expected root hash.
    ///
    /// Returns `true` if the proof is consistent with the given root.
    /// This is a **static computation** — no tree state is needed.
    ///
    /// For inclusion proofs (`value = Some(leaf_hash)`): verifies the key
    /// exists with the claimed value.
    /// For exclusion proofs (`value = None`): verifies the key does not exist.
    ///
    /// # Algorithm
    ///
    /// Starts at the leaf level (depth 255) with either the claimed leaf hash
    /// or the empty leaf hash. Walks up to the root, combining with sibling
    /// hashes at each level. The key bits determine whether we're the left
    /// or right child at each level.
    ///
    /// # Requirement: MRK-005
    pub fn verify(&self, expected_root: &Bytes32) -> bool {
        if self.siblings.len() != SMT_HEIGHT {
            return false;
        }

        // Start with the leaf hash at depth 256.
        let leaf_hash = match &self.value {
            Some(value) => *value, // The stored value IS the leaf hash
            None => empty_hash(0), // Empty leaf → empty leaf hash
        };

        // Walk from leaf (depth 255) to root (depth 0), combining with siblings.
        let mut current = leaf_hash;
        for depth in (0..SMT_HEIGHT).rev() {
            let bit = SparseMerkleTree::get_bit_public(&self.key, depth);
            let sibling = &self.siblings[depth];

            current = if bit {
                // This node is on the right; sibling is on the left.
                merkle_node_hash(sibling, &current)
            } else {
                // This node is on the left; sibling is on the right.
                merkle_node_hash(&current, sibling)
            };
        }

        current == *expected_root
    }

    /// MRK-004: canonical 32-byte leaf payload for wire / light-client consumers.
    ///
    /// Maps `value = Some(h)` → `h`, and `value = None` → MRK-002 empty leaf hash
    /// [`empty_hash`](crate::merkle::empty_hash)`(0)` (non-inclusion at this key).
    #[must_use]
    pub fn leaf_value(&self) -> Bytes32 {
        match self.value {
            Some(h) => h,
            None => empty_hash(0),
        }
    }
}

/// MRK-005 / `docs/resources/SPEC.md` §3.13: verify a coin Merkle proof against a **trusted** state
/// root without any tree or storage handle.
///
/// This is the free-function spelling expected by `docs/requirements/IMPLEMENTATION_ORDER.md`
/// (“`verify_coin_proof`”) and mirrors the future `CoinStore` static API from the master SPEC. It
/// delegates to [`SparseMerkleProof::verify`] — no duplicated logic, no I/O, no globals.
///
/// # When to use
///
/// - Prefer [`SparseMerkleProof::verify`] when you already have a proof value in scope (`proof.verify(&root)`).
/// - Use `verify_coin_proof(&proof, &root)` when naming parity with SPEC / RPC docs matters, or when
///   threading a `fn(Proof, Root) -> bool` callback without a method receiver.
#[inline]
#[must_use]
pub fn verify_coin_proof(proof: &SparseMerkleProof, expected_root: &Bytes32) -> bool {
    proof.verify(expected_root)
}

// Add a public accessor for get_bit so proofs can use it.
impl SparseMerkleTree {
    /// Public accessor for bit extraction (used by proof verification).
    ///
    /// Bit 0 = MSB of byte 0 (root level).
    /// Bit 255 = LSB of byte 31 (leaf level).
    #[inline]
    pub fn get_bit_public(key: &Bytes32, n: usize) -> bool {
        Self::get_bit(key, n)
    }

    /// Build a sparse Merkle path proof for `key` from the **in-memory** leaf multiset.
    ///
    /// Sibling digests for empty opposing subtrees resolve through [`empty_hash`] / subtree
    /// recomputation (MRK-002). When the full leaf map is resident—typical after
    /// [`SparseMerkleTree::load_from_store`] with the authoritative coinset—this matches the
    /// persisted state root without reading `merkle_nodes` rows on the hot path. Pulling internal
    /// digests from disk for **partial** leaf hydration remains a follow-up optimization tied to
    /// MRK-003 lazy reads (see `docs/requirements/domains/merkle/TRACKING.yaml` MRK-003 notes).
    fn build_sparse_proof_for_key(&self, key: &Bytes32) -> SparseMerkleProof {
        let mut siblings = Vec::with_capacity(SMT_HEIGHT);
        let leaf_refs: Vec<(&Bytes32, &Bytes32)> = self.leaves.iter().collect();
        let mut current_leaves = leaf_refs;

        for depth in 0..SMT_HEIGHT {
            let bit = Self::get_bit(key, depth);

            // Partition leaves by their bit at this depth.
            let (left_leaves, right_leaves): (Vec<_>, Vec<_>) = current_leaves
                .into_iter()
                .partition(|(k, _)| !Self::get_bit(k, depth));

            // The sibling is the subtree we're NOT descending into.
            let sibling_leaves = if bit { &left_leaves } else { &right_leaves };
            // MSB-first path to the sibling subtree root (must match MRK-001 / MRK-003 traversal).
            let mut path = Bytes32::default();
            for d in 0..depth {
                path = child_path(&path, d, Self::get_bit(key, d));
            }
            let sibling_path = child_path(&path, depth, !bit);
            let mut sink = HashMap::new();
            let sibling_hash = Self::compute_subtree_hash_core(
                sibling_leaves,
                depth + 1,
                &sibling_path,
                &mut sink,
                false,
            );
            siblings.push(sibling_hash);

            // Continue down the path.
            current_leaves = if bit { right_leaves } else { left_leaves };
        }

        SparseMerkleProof {
            key: *key,
            value: self.leaves.get(key).copied(),
            siblings,
        }
    }

    /// MRK-004 normative API: generate a Merkle proof for `coin_id` against the **current** tree.
    ///
    /// Returns [`MerkleError::ProofRequiresCleanTree`] if MRK-001 deferred recompute has not run
    /// since the last mutation (`is_dirty()`). This prevents serving proofs against an ambiguous
    /// “pending” state—call [`SparseMerkleTree::root`] first (block boundary), then `get_coin_proof`.
    ///
    /// **Inclusion vs exclusion:** existing keys yield `value = Some(leaf_hash)`; absent keys use
    /// `value = None`, with [`SparseMerkleProof::leaf_value`] exposing MRK-004’s always-32-byte
    /// wire field (empty leaf hash when absent).
    ///
    /// # Errors
    ///
    /// - [`MerkleError::ProofRequiresCleanTree`] — leaf map changed since last `root()`.
    ///
    /// # Spec
    ///
    /// `docs/requirements/domains/merkle/specs/MRK-004.md`
    pub fn get_coin_proof(&self, coin_id: &Bytes32) -> Result<SparseMerkleProof, MerkleError> {
        if self.is_dirty() {
            return Err(MerkleError::ProofRequiresCleanTree);
        }
        Ok(self.build_sparse_proof_for_key(coin_id))
    }

    /// Generate a proof for a key (inclusion or exclusion) **without** enforcing a clean tree.
    ///
    /// Prefer [`Self::get_coin_proof`] for production paths that must respect MRK-004’s dirty-tree
    /// rule. This helper remains for internal diagnostics and tests that intentionally read proofs
    /// mid-mutation (still deterministic given the current partial leaf map).
    ///
    /// # Requirement: MRK-004 (structural sibling walk; see also [`Self::get_coin_proof`])
    pub fn get_proof(&self, key: &Bytes32) -> SparseMerkleProof {
        self.build_sparse_proof_for_key(key)
    }
}
