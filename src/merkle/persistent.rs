//! Persistent Merkle internal nodes (**MRK-003**).
//!
//! Internal sparse-Merkle nodes are stored under [`crate::storage::schema::CF_MERKLE_NODES`] using
//! [`crate::storage::schema::merkle_node_key`] (`level || path`, 33 bytes). The committed state root
//! is stored separately in [`crate::storage::schema::CF_METADATA`] under [`MERKLE_STATE_ROOT_META_KEY`]
//! so startup can satisfy NORMATIVE MRK-003 (“load only the root hash”) with a single metadata `get`
//! before any `merkle_nodes` prefix scan.
//!
//! # Value encoding
//!
//! Each `merkle_nodes` value is **32 bytes**: the domain-separated internal digest
//! [`crate::merkle::merkle_node_hash`] of the two child subtrees (same wire shape as in-memory
//! [`crate::merkle::SparseMerkleTree`] recomputation). We do **not** store `(left || right)` raw
//! concatenation in the table; MRK-003’s “left/right” semantics are implicit in the hash function.
//!
//! # Lazy loading (MRK-004)
//!
//! Proof generation will read missing siblings via [`StorageBackend::get`]. This module only
//! defines the stable metadata key and the [`MerkleNodePersistOp`] surface used by
//! [`crate::merkle::SparseMerkleTree::flush_to_batch`](crate::merkle::SparseMerkleTree::flush_to_batch).
//!
//! # Requirements
//!
//! - **MRK-003** — `docs/requirements/domains/merkle/specs/MRK-003.md`
//! - **STO-008** — key helpers in `docs/requirements/domains/storage/specs/STO-008.md`
//! - **SPEC.md** — Section 13.4 (Persistent Merkle Tree)

use chia_protocol::Bytes32;

/// UTF-8 metadata row in [`crate::storage::schema::CF_METADATA`] holding the 32-byte state root.
///
/// **Why a string key:** metadata CF uses [`crate::storage::schema::metadata_key`] (STO-008); this
/// stays human-auditable in `rocksdb::DB::get` dumps and matches the MRK-003 implementation note
/// (“well-known metadata key”).
pub const MERKLE_STATE_ROOT_META_KEY: &str = "merkle_state_root";

/// What to apply for one `merkle_nodes` row during [`crate::merkle::SparseMerkleTree::flush_to_batch`].
///
/// **Delete:** Subtree hash collapsed to the canonical empty digest at this `(level, path)` — the
/// row is removed so disk does not retain redundant empty material (MRK-003 behavior §2 / spec
/// “empty subtree nodes are not persisted”).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MerkleNodePersistOp {
    /// Upsert the 32-byte node hash at this `(level, path)` key.
    Put(Bytes32),
    /// Remove the key from `merkle_nodes`.
    Delete,
}
