//! Tiered spent coin archival for dig-coinstore.
//!
//! Manages the hot/archive/prune tiers for spent coin records. Coins spent
//! beyond the rollback window are migrated from the hot tier (full indexing)
//! to the archive tier (coin ID only) as a background operation.
//!
//! # Requirements: PRF-005
//! # Spec: docs/requirements/domains/performance/specs/PRF-005.md
//! # SPEC.md: Section 13.1 (Tiered Coin Storage)
