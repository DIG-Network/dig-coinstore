//! Merkle proof generation and verification.
//!
//! [`SparseMerkleProof`] contains sibling hashes along a 256-level path.
//! Supports both inclusion proofs (coin exists) and non-inclusion proofs
//! (coin does not exist) against any trusted state root.
//!
//! # Requirements: MRK-004, MRK-005
//! # Spec: docs/requirements/domains/merkle/specs/MRK-004.md
