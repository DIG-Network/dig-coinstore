//! RocksDB storage backend implementation.
//!
//! Implements [`StorageBackend`] using RocksDB with 12 column families,
//! bloom filters, prefix bloom, WriteBatch atomic commits, and per-CF
//! compaction strategies.
//!
//! # Requirements: STO-002, STO-004, STO-005, STO-006
//! # Spec: docs/requirements/domains/storage/specs/STO-002.md
//! # SPEC.md: Section 7.2 (RocksDB Column Families)
//! # Chia comparison: coin_store.py uses SQLite; we use RocksDB for better
//!   write throughput and bloom filter support.
