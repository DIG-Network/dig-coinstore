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
//! # Spec: docs/requirements/domains/merkle/specs/MRK-004.md

use chia_protocol::Bytes32;
use serde::{Deserialize, Serialize};

use super::{empty_hash, merkle_node_hash, SparseMerkleTree, SMT_HEIGHT};

/// A proof of inclusion or exclusion in the sparse Merkle tree.
///
/// Contains the 256 sibling hashes along the path from the leaf to the root.
/// For an inclusion proof, `value` is `Some(leaf_hash)`. For an exclusion
/// proof, `value` is `None` (the leaf at this position is empty).
///
/// # Verification
///
/// Given a trusted root hash, call `SparseMerkleProof::verify()` to check
/// whether the proof is consistent with that root.
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

    /// Generate a proof for a key (inclusion or exclusion).
    ///
    /// If the key exists in the tree, generates an inclusion proof.
    /// If the key does not exist, generates an exclusion proof (value = None).
    ///
    /// # Requirement: MRK-004
    pub fn get_proof(&self, key: &Bytes32) -> SparseMerkleProof {
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
            let sibling_hash = Self::compute_subtree_hash(sibling_leaves, depth + 1);
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
}
