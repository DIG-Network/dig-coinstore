//! Error types for dig-coinstore.
//!
//! [`CoinStoreError`] is the single failure type for fallible [`crate::coin_store::CoinStore`]
//! operations (construction, genesis, block application, rollback, queries, snapshots). Variants
//! mirror [`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) Section 4 and the normative list
//! in [`docs/requirements/domains/crate_api/NORMATIVE.md`](../../docs/requirements/domains/crate_api/NORMATIVE.md#API-004).
//!
//! # Design: `String` for I/O and serde
//!
//! Storage backends (`heed`, `rocksdb`) and `bincode` expose rich error types that are not always
//! `Clone`/`PartialEq`. We stringify at the boundary so [`CoinStoreError`] can satisfy API-004
//! (`Clone + PartialEq`) and still embed human-readable diagnostics in logs.
//!
//! # Bincode encode vs decode
//!
//! `bincode::Error` is unified; we map **encode** failures through [`From`] to
//! [`CoinStoreError::SerializationError`] and **decode** failures through
//! [`CoinStoreError::from_bincode_deserialize`] so callers can classify without losing the message.
//!
//! # Requirement: API-004
//! # Spec: docs/requirements/domains/crate_api/specs/API-004.md
//! # SPEC.md: Section 4 (Error Types)

use chia_protocol::Bytes32;

use crate::storage::StorageError;
use crate::types::CoinId;

/// Errors returned by [`crate::coin_store::CoinStore`] and related APIs.
///
/// Each variant is a typed, matchable failure mode. Structured fields carry the data needed for
/// tests and for operator-facing messages via `thiserror`’s [`Display`](std::fmt::Display).
///
/// **Variant count:** 15 normative API-004 variants **plus** [`CoinStoreError::RollbackAboveTip`] (API-010).
///
/// # Requirement: API-004
/// # Spec: docs/requirements/domains/crate_api/specs/API-004.md
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CoinStoreError {
    // -- Chain continuity (apply_block validation; BLK domain) --
    /// Next block height does not equal `current_height + 1`.
    #[error("block height {got} does not follow current height {expected}")]
    HeightMismatch { expected: u64, got: u64 },

    /// Block’s parent hash does not match the stored chain tip hash.
    #[error("parent hash mismatch: expected {expected:?}, got {got:?}")]
    ParentHashMismatch { expected: Bytes32, got: Bytes32 },

    /// Optional state-root check failed after applying block effects.
    #[error("state root mismatch: expected {expected:?}, computed {computed:?}")]
    StateRootMismatch {
        expected: Bytes32,
        computed: Bytes32,
    },

    // -- Coin existence --
    /// Removal referenced a coin ID not present in the store.
    #[error("coin not found: {0:?}")]
    CoinNotFound(CoinId),

    /// Addition introduced a coin ID that already exists.
    #[error("coin already exists: {0:?}")]
    CoinAlreadyExists(CoinId),

    // -- Spend validity --
    /// Spend of a coin that is already marked spent (Chia coin_store double-spend defense).
    #[error("double spend: coin {0:?} already spent")]
    DoubleSpend(CoinId),

    /// Number of spend updates did not match removals (Chia `coin_store.py` strict count check).
    #[error("spend count mismatch: expected {expected} updates, got {actual}")]
    SpendCountMismatch { expected: usize, actual: usize },

    // -- Block structure --
    /// Reward coin count invalid for height (genesis vs non-genesis rules).
    #[error("invalid reward coin count: expected {expected}, got {got}")]
    InvalidRewardCoinCount { expected: String, got: usize },

    /// Hint length exceeds the limit (SPEC §2.7 `MAX_HINT_LENGTH`, typically 32 bytes).
    #[error("hint too long: {length} bytes exceeds maximum {max}")]
    HintTooLong { length: usize, max: usize },

    // -- Genesis --
    /// [`crate::coin_store::CoinStore::init_genesis`] called when already initialized.
    #[error("genesis already initialized")]
    GenesisAlreadyInitialized,

    /// Operation requires genesis but [`init_genesis`](crate::coin_store::CoinStore::init_genesis) has not run.
    #[error("coinstate not initialized (call init_genesis first)")]
    NotInitialized,

    // -- Rollback (API-010; RBK domain) --
    /// Rollback target height is strictly above the current chain tip (invalid target).
    ///
    /// **`target` as `i64`:** Accepts signed inputs from callers (including negative placeholders per RBK-001)
    /// while `current` stays `u64` like [`crate::coin_store::CoinStore::height`].
    ///
    /// **Not an error:** `target == current` is a legal no-op once RBK is implemented; only `target > current`
    /// triggers this variant ([`API-010`](../../docs/requirements/domains/crate_api/specs/API-010.md) § RollbackAboveTip Trigger).
    #[error("cannot rollback: target height {target} above current height {current}")]
    RollbackAboveTip { target: i64, current: u64 },

    // -- Query (QRY-007 batching) --
    /// Too many puzzle hashes in one batch request (SQL parameter parity / memory bounds).
    #[error("puzzle hash batch size {size} exceeds maximum {max}")]
    PuzzleHashBatchTooLarge { size: usize, max: usize },

    // -- Storage / serde (string payloads for Clone + PartialEq) --
    /// LMDB/RocksDB or other backend I/O failure.
    #[error("storage error: {0}")]
    StorageError(String),

    /// Bincode encode or other serialization failure (see [`From`] for `bincode::Error`).
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Bincode decode failure — use [`CoinStoreError::from_bincode_deserialize`].
    #[error("deserialization error: {0}")]
    DeserializationError(String),
}

impl CoinStoreError {
    /// Wrap a `bincode` decode error as [`CoinStoreError::DeserializationError`].
    ///
    /// `bincode` uses one `Error` type for both directions; call this on **deserialize** paths so
    /// metrics and logs can distinguish decode failures from encode failures.
    pub fn from_bincode_deserialize(err: bincode::Error) -> Self {
        CoinStoreError::DeserializationError(err.to_string())
    }
}

impl From<StorageError> for CoinStoreError {
    fn from(err: StorageError) -> Self {
        CoinStoreError::StorageError(err.to_string())
    }
}

impl From<bincode::Error> for CoinStoreError {
    fn from(err: bincode::Error) -> Self {
        CoinStoreError::SerializationError(err.to_string())
    }
}

#[cfg(feature = "lmdb-storage")]
impl From<heed::Error> for CoinStoreError {
    fn from(err: heed::Error) -> Self {
        CoinStoreError::StorageError(err.to_string())
    }
}

#[cfg(feature = "rocksdb-storage")]
impl From<rocksdb::Error> for CoinStoreError {
    fn from(err: rocksdb::Error) -> Self {
        CoinStoreError::StorageError(err.to_string())
    }
}
