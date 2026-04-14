//! Materialized aggregate counters.
//!
//! Running totals of `unspent_count`, `spent_count`, and `total_value`
//! updated atomically in the same WriteBatch as block application.
//! Stored in the metadata CF for persistence across restarts.
//!
//! Provides O(1) reads for `num_unspent()`, `total_unspent_value()`,
//! and `stats()` instead of full-table scans.
//!
//! # Requirements: PRF-003
//! # Spec: docs/requirements/domains/performance/specs/PRF-003.md
//! # SPEC.md: Section 13.7 (Materialized Aggregate Counters)
