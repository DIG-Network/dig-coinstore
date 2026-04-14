//! LMDB storage backend implementation.
//!
//! Implements [`StorageBackend`] using LMDB (via heed bindings) with
//! memory-mapped I/O, MVCC concurrency, and named databases.
//!
//! # Requirements: STO-003
//! # Spec: docs/requirements/domains/storage/specs/STO-003.md
//! # SPEC.md: Section 7.3 (LMDB Database Layout)
