//! Rollback pipeline for chain reorganization recovery.
//!
//! Implements `rollback_to_block()` and `rollback_n_blocks()` on [`CoinStore`]:
//! coin deletion, un-spending, FF-eligible recomputation, hint cleanup,
//! Merkle rebuild, and atomic state revert.
//!
//! # Requirements: RBK-001 through RBK-007
//! # Spec: docs/requirements/domains/rollback/specs/
//! # SPEC.md: Section 6 (Rollback Pipeline)
//! # Chia reference: coin_store.py:561-624
