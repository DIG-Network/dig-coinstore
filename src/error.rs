//! Error types for dig-coinstore.
//!
//! Defines [`CoinStoreError`] with all error variants for chain validation,
//! coin existence, spend validity, rollback, storage, and query operations.
//!
//! The full set of variants is defined by API-004. This initial implementation
//! includes the variants needed for API-001 (constructors) and will be extended
//! as subsequent requirements are implemented.
//!
//! # Requirement: API-004 (full enum), API-001 (used by constructors)
//! # Spec: docs/requirements/domains/crate_api/specs/API-004.md
//! # SPEC.md: Section 4 (Error Types)

use crate::storage::StorageError;

/// Errors from CoinStore operations.
///
/// Each variant corresponds to a specific failure mode documented in SPEC.md
/// Section 4. The error type derives `Debug` and `Clone` for testability
/// (API-004 requires `Clone + PartialEq`).
///
/// # Chia comparison
///
/// Chia uses `chia.util.errors.Err` enum codes. dig-coinstore uses typed
/// variants with structured payloads for better error messages and matching.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CoinStoreError {
    // -- Genesis --
    /// `init_genesis()` called on a store that has already been initialized.
    /// SPEC: Section 4, GenesisAlreadyInitialized.
    #[error("genesis already initialized")]
    GenesisAlreadyInitialized,

    /// Operation attempted on a store that has not been initialized.
    /// Call `init_genesis()` before applying blocks or querying state.
    /// SPEC: Section 4, NotInitialized.
    #[error("coinstate not initialized (call init_genesis first)")]
    NotInitialized,

    // -- Storage --
    /// A storage backend operation failed (I/O, corruption, etc.).
    /// Wraps the low-level `StorageError` from the storage module.
    #[error("storage error: {0}")]
    StorageError(String),

    /// Serialization failure (bincode encode).
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Deserialization failure (bincode decode).
    #[error("deserialization error: {0}")]
    DeserializationError(String),
}

/// Convert storage-level errors into CoinStoreError.
impl From<StorageError> for CoinStoreError {
    fn from(err: StorageError) -> Self {
        CoinStoreError::StorageError(err.to_string())
    }
}
