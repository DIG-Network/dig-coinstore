//! Hint store for puzzle hash hints on coins.
//!
//! Manages the bidirectional hint index: forward (coin_id -> hints) and
//! reverse (hint -> coin_ids). Supports hint validation, idempotent insertion,
//! variable-length keys, and rollback cleanup.
//!
//! # Requirements: HNT-001 through HNT-006
//! # Spec: docs/requirements/domains/hints/specs/
//! # SPEC.md: Section 8 (Hint Store)
//! # Chia reference: hint_store.py, hint_management.py
