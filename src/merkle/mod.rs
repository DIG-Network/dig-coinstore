//! Sparse Merkle tree for state root computation.
//!
//! Maintains a 256-level sparse Merkle tree over all coin records.
//! The root hash is committed in every block header, enabling light
//! client proofs and state verification.
//!
//! # Domain-separated hashing
//!
//! - **Leaf hash:** `SHA256(0x00 || value)` — the `0x00` prefix prevents
//!   second-preimage attacks by distinguishing leaves from internal nodes.
//! - **Node hash:** `SHA256(0x01 || left || right)` — the `0x01` prefix
//!   distinguishes internal nodes from leaves.
//!
//! This convention matches the existing `l2_driver_state_channel` implementation
//! (see `utils/hash.rs`: `merkle_leaf_hash`, `merkle_node_hash`).
//!
//! # Deferred root recomputation
//!
//! Mutation methods (`batch_insert`, `batch_update`, `batch_remove`) mark the
//! tree as dirty but do NOT recompute the root. The root is recomputed lazily
//! on the next call to `root()`. This ensures at most one expensive tree
//! traversal per block, regardless of how many coins are modified.
//!
//! # Requirements: STR-004, MRK-001, MRK-002
//! # Spec: docs/requirements/domains/merkle/specs/MRK-001.md
//! # SPEC.md: Section 9 (Merkle Tree)

pub mod persistent;
pub mod proof;

use std::collections::HashMap;
use std::sync::OnceLock;

use chia_protocol::Bytes32;
use chia_sha2::Sha256;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Height of the sparse Merkle tree: 256 levels for 256-bit (Bytes32) keys.
///
/// Each bit of a coin_id selects left (0) or right (1) at successive levels,
/// from bit 0 (MSB, root level) to bit 255 (LSB, leaf level).
pub const SMT_HEIGHT: usize = 256;

// ─────────────────────────────────────────────────────────────────────────────
// Domain-separated hashing
// ─────────────────────────────────────────────────────────────────────────────

/// Compute a leaf hash: `SHA256(0x00 || data)`.
///
/// The `0x00` domain separator prevents second-preimage attacks by ensuring
/// leaf hashes cannot collide with internal node hashes.
///
/// Matches `l2_driver_state_channel/src/utils/hash.rs:merkle_leaf_hash`.
#[inline]
pub fn merkle_leaf_hash(data: &[u8]) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(data);
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Bytes32::from(bytes)
}

/// Compute an internal node hash: `SHA256(0x01 || left || right)`.
///
/// The `0x01` domain separator distinguishes internal nodes from leaves.
///
/// Matches `l2_driver_state_channel/src/utils/hash.rs:merkle_node_hash`.
#[inline]
pub fn merkle_node_hash(left: &Bytes32, right: &Bytes32) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left.as_ref());
    hasher.update(right.as_ref());
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Bytes32::from(bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-002: Memoized empty hashes
// ─────────────────────────────────────────────────────────────────────────────
// Pre-computed empty subtree hashes for all 257 levels (0..=256).
// Level 0 = empty leaf hash, Level 256 = root of entirely empty 256-level tree.
//
// Requirement: MRK-002
// Spec: docs/requirements/domains/merkle/specs/MRK-002.md

/// Sentinel value for an empty leaf. All zeros, 32 bytes.
const EMPTY_LEAF_SENTINEL: [u8; 32] = [0u8; 32];

/// Pre-computed empty hashes for sparse Merkle tree levels 0..=256.
///
/// - `EMPTY_HASHES[0]` = `merkle_leaf_hash(EMPTY_LEAF_SENTINEL)` (empty leaf)
/// - `EMPTY_HASHES[n]` = `merkle_node_hash(EMPTY_HASHES[n-1], EMPTY_HASHES[n-1])` for n > 0
/// - `EMPTY_HASHES[256]` = root hash of an entirely empty 256-level tree
///
/// Initialized lazily via `OnceLock` on first access. All 257 values are
/// computed bottom-up in a single pass. Thread-safe: concurrent first-access
/// from multiple threads initializes exactly once.
static EMPTY_HASHES: OnceLock<Vec<Bytes32>> = OnceLock::new();

/// Get the pre-computed empty hash at the given tree level.
///
/// Level 0 = empty leaf hash.
/// Level 256 = root of entirely empty tree.
///
/// # Panics
///
/// Panics if `level > SMT_HEIGHT` (256).
///
/// # Performance
///
/// O(1) — direct array index after one-time initialization.
#[inline]
pub fn empty_hash(level: usize) -> Bytes32 {
    assert!(
        level <= SMT_HEIGHT,
        "level {} exceeds SMT_HEIGHT {}",
        level,
        SMT_HEIGHT
    );
    get_empty_hashes()[level]
}

/// Access the full pre-computed empty hash array.
fn get_empty_hashes() -> &'static Vec<Bytes32> {
    EMPTY_HASHES.get_or_init(|| {
        let mut hashes = Vec::with_capacity(SMT_HEIGHT + 1);

        // Level 0: empty leaf hash
        hashes.push(merkle_leaf_hash(&EMPTY_LEAF_SENTINEL));

        // Levels 1..256: each level is the node hash of two copies of the level below.
        for _ in 1..=SMT_HEIGHT {
            let child = *hashes.last().unwrap();
            hashes.push(merkle_node_hash(&child, &child));
        }

        hashes
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// MerkleError
// ─────────────────────────────────────────────────────────────────────────────

/// Errors from sparse Merkle tree operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MerkleError {
    /// Attempted to insert a key that already exists in the tree.
    #[error("key already exists: {0}")]
    KeyAlreadyExists(Bytes32),

    /// Attempted to update or remove a key that does not exist.
    #[error("key not found: {0}")]
    KeyNotFound(Bytes32),
}

// ─────────────────────────────────────────────────────────────────────────────
// MRK-001: SparseMerkleTree
// ─────────────────────────────────────────────────────────────────────────────

/// A 256-level sparse Merkle tree for coin state commitment.
///
/// Leaves are keyed by `Bytes32` (coin IDs) and hold the leaf hash of the
/// coin record. The tree uses deferred root recomputation: mutations mark
/// the tree as dirty, and the root is only recomputed when `root()` is called.
///
/// # Design
///
/// The tree stores only non-empty leaves in a `HashMap`. Internal nodes are
/// computed on-the-fly during root recomputation by recursively partitioning
/// leaves by their key bits at each level. Empty subtrees are replaced with
/// pre-computed empty hashes from [`empty_hash`] (MRK-002).
///
/// This is memory-efficient for sparse trees: a tree with 10M coins stores
/// only 10M leaf entries, not 2^256 nodes.
///
/// # Requirements: MRK-001
/// # Spec: docs/requirements/domains/merkle/specs/MRK-001.md
/// # SPEC.md: Section 9 (Merkle Tree)
///
/// # Reference
/// Derived from `l2_driver_state_channel/src/utils/merkle.rs:SparseMerkleTree`.
#[derive(Debug, Clone)]
pub struct SparseMerkleTree {
    /// Non-empty leaf values: coin_id -> leaf_hash.
    /// Only populated (non-empty) leaves are stored.
    leaves: HashMap<Bytes32, Bytes32>,

    /// Cached root hash. `None` when dirty (needs recomputation).
    root_hash: Option<Bytes32>,
}

impl Default for SparseMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl SparseMerkleTree {
    /// Create a new empty sparse Merkle tree.
    ///
    /// The root of an empty tree is `empty_hash(SMT_HEIGHT)` — the pre-computed
    /// root of a 256-level tree with all-empty leaves.
    pub fn new() -> Self {
        Self {
            leaves: HashMap::new(),
            root_hash: Some(empty_hash(SMT_HEIGHT)),
        }
    }

    /// Insert new leaves into the tree. Does NOT recompute the root.
    ///
    /// Each entry is `(coin_id, leaf_hash)`. Inserting a key that already
    /// exists returns `MerkleError::KeyAlreadyExists`.
    ///
    /// Call `root()` after all mutations to get the updated root hash.
    ///
    /// # Requirement: MRK-001
    pub fn batch_insert(&mut self, entries: &[(Bytes32, Bytes32)]) -> Result<(), MerkleError> {
        for (key, _) in entries {
            if self.leaves.contains_key(key) {
                return Err(MerkleError::KeyAlreadyExists(*key));
            }
        }
        for (key, value) in entries {
            self.leaves.insert(*key, *value);
        }
        if !entries.is_empty() {
            self.root_hash = None; // Mark dirty
        }
        Ok(())
    }

    /// Update existing leaves with new values. Does NOT recompute the root.
    ///
    /// Updating a key that does not exist returns `MerkleError::KeyNotFound`.
    ///
    /// # Requirement: MRK-001
    pub fn batch_update(&mut self, entries: &[(Bytes32, Bytes32)]) -> Result<(), MerkleError> {
        for (key, _) in entries {
            if !self.leaves.contains_key(key) {
                return Err(MerkleError::KeyNotFound(*key));
            }
        }
        for (key, value) in entries {
            self.leaves.insert(*key, *value);
        }
        if !entries.is_empty() {
            self.root_hash = None; // Mark dirty
        }
        Ok(())
    }

    /// Remove leaves from the tree. Does NOT recompute the root.
    ///
    /// Removing a key that does not exist returns `MerkleError::KeyNotFound`.
    ///
    /// # Requirement: MRK-001
    pub fn batch_remove(&mut self, keys: &[Bytes32]) -> Result<(), MerkleError> {
        for key in keys {
            if !self.leaves.contains_key(key) {
                return Err(MerkleError::KeyNotFound(*key));
            }
        }
        for key in keys {
            self.leaves.remove(key);
        }
        if !keys.is_empty() {
            self.root_hash = None; // Mark dirty
        }
        Ok(())
    }

    /// Return the current Merkle root, recomputing if dirty.
    ///
    /// After this call, the tree is no longer dirty. Subsequent calls
    /// without intervening mutations return the cached root in O(1).
    ///
    /// # Algorithm
    ///
    /// Recursively partitions leaves by their key bits at each tree level.
    /// At each level, bit `depth` of the key determines left (0) or right (1).
    /// Empty subtrees are replaced with pre-computed `empty_hash(height)`.
    ///
    /// # Requirement: MRK-001
    pub fn root(&mut self) -> Bytes32 {
        if let Some(cached) = self.root_hash {
            return cached;
        }

        // Recompute from leaves.
        let leaf_refs: Vec<(&Bytes32, &Bytes32)> = self.leaves.iter().collect();
        let root = Self::compute_subtree_hash(&leaf_refs, 0);
        self.root_hash = Some(root);
        root
    }

    /// Merkle root for read-only callers (e.g. [`crate::coin_store::CoinStore::stats`] / API-007).
    ///
    /// **Why not reuse [`Self::root`]?** `root()` takes `&mut self` to cache the recomputed root when
    /// the tree is dirty. [`CoinStore::stats`](crate::coin_store::CoinStore::stats) is `&self` in the
    /// public API ([`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) §3.12), so we expose this
    /// snapshot path that **does not** write `root_hash` back: dirty trees stay dirty for the next
    /// mutating `root()` call, but observability still sees the correct digest.
    ///
    /// **Complexity:** O(leaves) when dirty (same work as `root()` recompute); O(1) when clean.
    ///
    /// # Requirement: API-007 (chain stats), MRK-001
    #[must_use]
    pub fn root_readonly(&self) -> Bytes32 {
        if let Some(cached) = self.root_hash {
            return cached;
        }
        let leaf_refs: Vec<(&Bytes32, &Bytes32)> = self.leaves.iter().collect();
        Self::compute_subtree_hash(&leaf_refs, 0)
    }

    /// Read the Merkle root without `&mut self` (for [`crate::coin_store::CoinStore::stats`] and other `&self` APIs).
    ///
    /// **Difference vs [`Self::root`]:** [`Self::root`] caches the digest in [`Self::root_hash`] after a
    /// dirty recompute. This method returns the same mathematical root but **does not** update the cache
    /// when dirty, so it can run on a shared `&` reference. The next [`Self::root`] call may still pay the
    /// full recompute cost once a writer mutates the tree.
    ///
    /// **Empty tree:** Returns [`empty_hash`](empty_hash)([`SMT_HEIGHT`]) — identical to [`Self::new`]'s
    /// initial root.
    ///
    /// # Requirement: API-007 (stats reads state root), MRK-001
    pub fn root_observed(&self) -> Bytes32 {
        if self.leaves.is_empty() {
            return empty_hash(SMT_HEIGHT);
        }
        if let Some(cached) = self.root_hash {
            return cached;
        }
        let leaf_refs: Vec<(&Bytes32, &Bytes32)> = self.leaves.iter().collect();
        Self::compute_subtree_hash(&leaf_refs, 0)
    }

    /// Check if the tree has been modified since the last `root()` call.
    pub fn is_dirty(&self) -> bool {
        self.root_hash.is_none() && !self.leaves.is_empty()
    }

    /// Number of non-empty leaves in the tree.
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Whether the tree has no leaves.
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// Check if a key exists in the tree.
    pub fn contains_key(&self, key: &Bytes32) -> bool {
        self.leaves.contains_key(key)
    }

    /// Get the leaf hash for a key, if it exists.
    pub fn get(&self, key: &Bytes32) -> Option<&Bytes32> {
        self.leaves.get(key)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Internal: recursive subtree hash computation
    // ─────────────────────────────────────────────────────────────────────

    /// Recursively compute the hash of a subtree at the given depth.
    ///
    /// `depth` ranges from 0 (root level) to SMT_HEIGHT (leaf level).
    /// At depth == SMT_HEIGHT, we're at the leaf level.
    fn compute_subtree_hash(leaves: &[(&Bytes32, &Bytes32)], depth: usize) -> Bytes32 {
        // Base case: no leaves in this subtree → empty subtree hash.
        if leaves.is_empty() {
            return empty_hash(SMT_HEIGHT - depth);
        }

        // Base case: at leaf level (depth 256).
        if depth == SMT_HEIGHT {
            // There should be exactly one leaf at this position.
            // Return its pre-computed leaf hash (the value stored is already
            // the leaf hash, not the raw record).
            return leaves[0].1.to_owned();
        }

        // Partition leaves by bit `depth` of their key.
        // Bit 0 = MSB of the first byte.
        let (left, right): (Vec<_>, Vec<_>) = leaves
            .iter()
            .partition(|(key, _)| !Self::get_bit(key, depth));

        let left_hash = Self::compute_subtree_hash(&left, depth + 1);
        let right_hash = Self::compute_subtree_hash(&right, depth + 1);

        merkle_node_hash(&left_hash, &right_hash)
    }

    /// Get bit `n` of a Bytes32 key (MSB-first ordering).
    ///
    /// Bit 0 = MSB of byte 0 (root level decision).
    /// Bit 255 = LSB of byte 31 (leaf level decision).
    #[inline]
    fn get_bit(key: &Bytes32, n: usize) -> bool {
        let byte_index = n / 8;
        let bit_index = 7 - (n % 8); // MSB-first within each byte
        (key.as_ref()[byte_index] >> bit_index) & 1 == 1
    }
}
