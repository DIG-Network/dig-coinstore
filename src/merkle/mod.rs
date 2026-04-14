//! Sparse Merkle tree for state root computation.
//!
//! Maintains a 256-level sparse Merkle tree over all coin records.
//! The root hash is committed in every block header, enabling light
//! client proofs and state verification.
//!
//! Key operations: `batch_insert`, `batch_update`, `batch_remove`, `root()`.
//! All mutations are deferred until `root()` is called, enabling a single
//! root recomputation per block.
//!
//! # Requirements: MRK-001 through MRK-006
//! # Spec: docs/requirements/domains/merkle/specs/
//! # SPEC.md: Section 9 (Merkle Tree)

pub mod persistent;
pub mod proof;
