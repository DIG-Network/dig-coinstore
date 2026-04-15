//! Persistent Merkle tree node storage ([SPEC.md §1.6 #15](../../docs/resources/SPEC.md)).
//!
//! Stores internal tree nodes in the [`CF_MERKLE_NODES`](crate::storage::schema::CF_MERKLE_NODES)
//! column family ([SPEC.md §7.2](../../docs/resources/SPEC.md)) with dirty node tracking and
//! incremental flush. On startup, only the root node is loaded; internal nodes are demand-loaded
//! during proof generation ([SPEC.md §3.13](../../docs/resources/SPEC.md)).
//!
//! # Key format
//!
//! Each internal node is keyed by `level(1 byte) || path(32 bytes)` = 33 bytes.
//! The value is the 32-byte hash of `merkle_node_hash(left, right)`.
//!
//! # Current status
//!
//! **Not yet implemented.** The current [`SparseMerkleTree`](crate::merkle::SparseMerkleTree) in
//! `merkle/mod.rs` works in-memory ([SPEC.md §1.3 #2](../../docs/resources/SPEC.md)).
//! MRK-003 (Phase 3) will add persistent node storage so the tree survives restarts
//! without full reconstruction from coin records.
//!
//! # Requirements: MRK-003
//! # Spec: docs/requirements/domains/merkle/specs/MRK-003.md
//! # SPEC.md: §1.6 #15 (Persistent Merkle Tree), §7.2 (CF_MERKLE_NODES), §1.3 #2
