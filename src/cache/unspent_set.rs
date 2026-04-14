//! In-memory unspent coin ID set.
//!
//! `HashSet<CoinId>` providing O(1) lock-free "is this coin unspent?" checks.
//! At ~40 bytes per coin, 10M unspent coins consume ~400 MB.
//!
//! Maintained incrementally: insert on creation, remove on spend,
//! re-insert on rollback un-spend. Populated from storage on startup
//! in `MATERIALIZATION_BATCH_SIZE` chunks.
//!
//! # Requirements: PRF-001
//! # Spec: docs/requirements/domains/performance/specs/PRF-001.md
//! # SPEC.md: Section 13.2 (In-Memory Unspent Set)
