//! Persistent Merkle tree node storage.
//!
//! Stores internal tree nodes in the `merkle_nodes` column family with
//! dirty node tracking and incremental flush. On startup, only the root
//! node is loaded; internal nodes are demand-loaded during proof generation.
//!
//! # Requirements: MRK-003
//! # Spec: docs/requirements/domains/merkle/specs/MRK-003.md
//! # SPEC.md: Section 13.4 (Persistent Merkle Tree)
