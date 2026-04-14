//! Storage backend abstraction for dig-coinstore.
//!
//! Defines the [`StorageBackend`] trait that abstracts over LMDB and RocksDB.
//! Both backends implement this trait, and the rest of the crate interacts
//! with storage exclusively through this interface.
//!
//! Backend selection is compile-time via Cargo feature gates:
//! - `rocksdb-storage` → [`rocksdb`] module
//! - `lmdb-storage` → [`lmdb`] module
//!
//! # Requirements: STO-001 through STO-008
//! # Spec: docs/requirements/domains/storage/specs/
//! # SPEC.md: Section 7 (Storage Architecture)

#[cfg(feature = "rocksdb-storage")]
pub mod rocksdb;

#[cfg(feature = "lmdb-storage")]
pub mod lmdb;

pub mod schema;
