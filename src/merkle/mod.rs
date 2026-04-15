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
//! # Persistent internal nodes (MRK-003)
//!
//! After [`SparseMerkleTree::root`] recomputes from leaves, the implementation records which
//! `(level, path)` internal rows differ from the canonical empty subtree so
//! [`SparseMerkleTree::flush_to_batch`] can append `merkle_nodes` puts/deletes plus the metadata
//! root row in one [`crate::storage::WriteBatch`]. See [`persistent`](persistent) and
//! `docs/requirements/domains/merkle/specs/MRK-003.md`.
//!
//! # Requirements: STR-004, MRK-001, MRK-002, MRK-003
//! # Spec: docs/requirements/domains/merkle/specs/MRK-001.md
//! # SPEC.md: Section 9 (Merkle Tree), Section 13.4 (Persistent Merkle Tree)

pub mod persistent;
pub mod proof;

use std::collections::HashMap;
use std::sync::OnceLock;

use chia_protocol::Bytes32;
use chia_sha2::Sha256;

use crate::storage::schema;
use crate::storage::{StorageBackend, WriteBatch};

pub use persistent::{MerkleNodePersistOp, MERKLE_STATE_ROOT_META_KEY};

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
/// **Storage shape:** fixed `[Bytes32; 257]` inside [`OnceLock`], not `Vec`, so the table is
/// stack-sized at init time and matches **MRK-002 / NORMATIVE** (`OnceLock<[Bytes32; 257]>`).
/// Lookup remains a single bounds-checked index — O(1), no heap per read, no recursive work
/// on the query path (see `docs/requirements/domains/merkle/specs/MRK-002.md`).
///
/// Initialized lazily via [`OnceLock::get_or_init`] on first access. All 257 values are computed
/// bottom-up in one pass. Rust guarantees concurrent first callers block until a single init
/// completes, then all observe the same static slice.
static EMPTY_HASHES: OnceLock<[Bytes32; 257]> = OnceLock::new();

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

/// Access the full pre-computed empty hash table (257 entries, indices `0..=SMT_HEIGHT`).
fn get_empty_hashes() -> &'static [Bytes32; 257] {
    EMPTY_HASHES.get_or_init(|| {
        let mut hashes = [Bytes32::default(); SMT_HEIGHT + 1];

        // Level 0: domain-separated empty leaf (MRK-002 § leaf sentinel).
        hashes[0] = merkle_leaf_hash(&EMPTY_LEAF_SENTINEL);

        // Levels 1..256: parent = H(left || right) with both children the same empty subtree.
        for i in 1..=SMT_HEIGHT {
            hashes[i] = merkle_node_hash(&hashes[i - 1], &hashes[i - 1]);
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

    /// `merkle_state_root` metadata row missing during [`SparseMerkleTree::load_from_store`].
    #[error("persisted merkle state root missing from metadata column family")]
    PersistedRootMissing,

    /// Metadata root bytes are not a 32-byte [`Bytes32`] wire encoding.
    #[error("persisted merkle state root has invalid length: {0} bytes (expected 32)")]
    InvalidPersistedRootLength(usize),

    /// Recomputed root from the provided leaf map does not match the single metadata read (data
    /// corruption or partial write).
    #[error("persisted merkle root mismatch: disk={disk:?} recomputed={recomputed:?}")]
    PersistedRootMismatch {
        disk: Bytes32,
        recomputed: Bytes32,
    },

    /// Backend I/O failure while loading persisted Merkle metadata.
    #[error("storage error during merkle load: {0}")]
    Storage(String),
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
/// # Requirements: MRK-001, MRK-003
/// # Spec: docs/requirements/domains/merkle/specs/MRK-001.md, specs/MRK-003.md
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

    /// Pending `merkle_nodes` writes since the last successful [`Self::flush_to_batch`].
    ///
    /// **MRK-003:** Populated during [`Self::root`] recomputation (same traversal as MRK-001) so we
    /// never pay an extra tree walk for persistence. Keys are exactly [`schema::merkle_node_key`]
    /// outputs (`[u8; 33]`). Values are [`MerkleNodePersistOp::Put`] for non-empty-subtree digests
    /// or [`MerkleNodePersistOp::Delete`] when the canonical empty hash applies at that `(level,
    /// path)` — enabling empty-subtree pruning on disk per MRK-003 spec §Behavior item 2.
    dirty_merkle_nodes: HashMap<[u8; 33], MerkleNodePersistOp>,
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
            dirty_merkle_nodes: HashMap::new(),
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
    /// # Requirement: MRK-001, MRK-003 (dirty map refresh)
    pub fn root(&mut self) -> Bytes32 {
        if let Some(cached) = self.root_hash {
            return cached;
        }

        // Recompute from leaves and repopulate MRK-003 dirty set in the same traversal.
        let leaf_refs: Vec<(&Bytes32, &Bytes32)> = self.leaves.iter().collect();
        self.dirty_merkle_nodes.clear();
        let path_root = Bytes32::default();
        let root = Self::compute_subtree_hash_core(
            &leaf_refs,
            0,
            &path_root,
            &mut self.dirty_merkle_nodes,
            true,
        );
        self.root_hash = Some(root);
        root
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
        let mut sink = HashMap::new();
        Self::compute_subtree_hash_core(
            &leaf_refs,
            0,
            &Bytes32::default(),
            &mut sink,
            false,
        )
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

    /// MRK-003: read-only view of pending `merkle_nodes` writes (primarily tests and diagnostics).
    pub fn dirty_nodes(&self) -> &HashMap<[u8; 33], MerkleNodePersistOp> {
        &self.dirty_merkle_nodes
    }

    /// MRK-003: drop pending persistence rows without touching disk.
    ///
    /// Normal production flow uses [`Self::flush_to_batch`], which clears dirty after enqueueing
    /// ops. This helper exists for rollback simulations and tests that need a clean dirty slate
    /// without committing a batch.
    pub fn clear_dirty(&mut self) {
        self.dirty_merkle_nodes.clear();
    }

    /// MRK-003: enqueue dirty internal nodes plus the metadata state root into `batch`.
    ///
    /// **Atomicity:** Callers MUST pass the same [`WriteBatch`] they use for coin-record mutations
    /// so one [`StorageBackend::batch_write`] satisfies MRK-003 / BLK-014 “all-or-nothing” commits.
    ///
    /// **Ordering:** Invokes [`Self::root`] first so the dirty map matches the latest leaf multiset,
    /// then drains [`Self::dirty_merkle_nodes`] into `merkle_nodes` puts/deletes, then appends the
    /// metadata root row ([`MERKLE_STATE_ROOT_META_KEY`]).
    pub fn flush_to_batch(&mut self, batch: &mut WriteBatch) -> Result<(), MerkleError> {
        let root = self.root();
        for (key, op) in std::mem::take(&mut self.dirty_merkle_nodes) {
            match op {
                MerkleNodePersistOp::Put(h) => {
                    batch.put(schema::CF_MERKLE_NODES, &key, h.as_ref());
                }
                MerkleNodePersistOp::Delete => {
                    batch.delete(schema::CF_MERKLE_NODES, &key);
                }
            }
        }
        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        batch.put(schema::CF_METADATA, &meta_key, root.as_ref());
        Ok(())
    }

    /// MRK-003: single metadata `get` for the committed root, then validate against `leaves`.
    ///
    /// Performs **exactly one** storage read on [`schema::CF_METADATA`] (no `merkle_nodes` scan),
    /// matching NORMATIVE MRK-003. Recomputes from `leaves` via [`Self::root_observed`] to detect
    /// corruption before accepting the disk root into [`Self::root_hash`].
    pub fn load_from_store(
        store: &dyn StorageBackend,
        leaves: HashMap<Bytes32, Bytes32>,
    ) -> Result<Self, MerkleError> {
        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        let disk_bytes = store
            .get(schema::CF_METADATA, &meta_key)
            .map_err(|e| MerkleError::Storage(e.to_string()))?
            .ok_or(MerkleError::PersistedRootMissing)?;
        if disk_bytes.len() != 32 {
            return Err(MerkleError::InvalidPersistedRootLength(disk_bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&disk_bytes);
        let disk_root = Bytes32::from(arr);

        let mut tree = Self {
            leaves,
            root_hash: None,
            dirty_merkle_nodes: HashMap::new(),
        };
        let recomputed = tree.root_observed();
        if recomputed != disk_root {
            return Err(MerkleError::PersistedRootMismatch {
                disk: disk_root,
                recomputed,
            });
        }
        tree.root_hash = Some(disk_root);
        Ok(tree)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Internal: recursive subtree hash computation
    // ─────────────────────────────────────────────────────────────────────

    /// Recursive subtree hash with optional MRK-003 dirty recording.
    ///
    /// `path` carries the MSB-first prefix (bits `0..depth-1`) identifying the position of the
    /// current node in the global 256-bit key space. When `record_dirty` is true, every visited
    /// internal level records a [`MerkleNodePersistOp`] into `dirty_out` (keyed by
    /// [`schema::merkle_node_key`]). Callers that only need the digest (e.g. MRK-004 sibling
    /// recomputation) pass `record_dirty: false` and a throwaway map so both recursive children
    /// can share one writer without moving `Option<&mut HashMap<…>>` twice.
    fn compute_subtree_hash_core(
        leaves: &[(&Bytes32, &Bytes32)],
        depth: usize,
        path: &Bytes32,
        dirty_out: &mut HashMap<[u8; 33], MerkleNodePersistOp>,
        record_dirty: bool,
    ) -> Bytes32 {
        if leaves.is_empty() {
            let h = empty_hash(SMT_HEIGHT - depth);
            if record_dirty {
                record_merkle_persist_op(dirty_out, depth, path, h);
            }
            return h;
        }

        if depth == SMT_HEIGHT {
            return *leaves[0].1;
        }

        let (left, right): (Vec<_>, Vec<_>) = leaves
            .iter()
            .partition(|(key, _)| !Self::get_bit(key, depth));

        let path_left = child_path(path, depth, false);
        let path_right = child_path(path, depth, true);

        let left_hash = Self::compute_subtree_hash_core(
            &left,
            depth + 1,
            &path_left,
            dirty_out,
            record_dirty,
        );
        let right_hash = Self::compute_subtree_hash_core(
            &right,
            depth + 1,
            &path_right,
            dirty_out,
            record_dirty,
        );

        let node_hash = merkle_node_hash(&left_hash, &right_hash);
        if record_dirty {
            record_merkle_persist_op(dirty_out, depth, path, node_hash);
        }
        node_hash
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

/// Child path for the sparse walk: copy `base` bits `0..depth-1`, clear suffix, then set bit `depth`.
///
/// **Invariant:** All keys in the recursive `leaves` slice at `(depth, path)` agree on the first
/// `depth` bits; `go_right` selects the `1` branch at bit index `depth` (MRK-001 MSB-first rule).
fn child_path(base: &Bytes32, depth: usize, go_right: bool) -> Bytes32 {
    let mut arr: [u8; 32] = base.as_ref().try_into().expect("Bytes32 is 32 bytes");
    for bit in depth..256 {
        let bi = bit / 8;
        let bj = 7 - (bit % 8);
        arr[bi] &= !(1 << bj);
    }
    let bi = depth / 8;
    let bj = 7 - (depth % 8);
    if go_right {
        arr[bi] |= 1 << bj;
    }
    Bytes32::from(arr)
}

/// Queue one `merkle_nodes` row for flush (MRK-003 empty-subtree pruning).
fn record_merkle_persist_op(
    dirty: &mut HashMap<[u8; 33], MerkleNodePersistOp>,
    depth: usize,
    path: &Bytes32,
    hash: Bytes32,
) {
    if depth >= SMT_HEIGHT {
        return;
    }
    let key = schema::merkle_node_key(depth as u8, path);
    let empty = empty_hash(SMT_HEIGHT - depth);
    if hash == empty {
        dirty.insert(key, MerkleNodePersistOp::Delete);
    } else {
        dirty.insert(key, MerkleNodePersistOp::Put(hash));
    }
}
