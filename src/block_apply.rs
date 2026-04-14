//! Block application pipeline for dig-coinstore.
//!
//! Implements the `apply_block()` method on [`CoinStore`]: Phase 1 validation
//! (height, parent hash, reward coins, removals, additions, hints) followed by
//! Phase 2 atomic mutation (coin insertion, spend marking, hint storage,
//! Merkle update, chain tip commit).
//!
//! # Requirements: BLK-001 through BLK-014
//! # Spec: docs/requirements/domains/block_application/specs/
//! # SPEC.md: Section 5 (Block Application Pipeline)
