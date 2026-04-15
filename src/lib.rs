//! # dig-coinstore
//!
//! Persistent global coin state database for the DIG Network L2 blockchain.
//!
//! This crate manages the authoritative database of all spent and unspent coins
//! across the entire blockchain using the coinset model (UTXO-like). It accepts
//! validated blocks as input, applies their state transitions, and provides a
//! rich query API for coin lookups by ID, puzzle hash, hint, parent, and height.
//!
//! ## Crate boundary
//!
//! - **Input:** Pre-validated `BlockData` (additions, removals, coinbase, hints)
//! - **Output:** `CoinRecord`s, `CoinState`s, state roots, Merkle proofs
//! - **Not in scope:** CLVM execution, block production, networking, consensus
//!
//! ## Storage backends
//!
//! Storage is feature-gated:
//! - `rocksdb-storage` (default) — RocksDB with bloom filters and column families
//! - `lmdb-storage` — LMDB with memory-mapped I/O
//! - `full-storage` — Both backends enabled
//!
//! ## Module organization
//!
//! | Module | Responsibility | Requirement domain |
//! |--------|---------------|--------------------|
//! | [`coin_store`] | Primary public API struct | API-001 |
//! | [`config`] | Configuration types and constants | API-003 |
//! | [`error`] | Error enum | API-004 |
//! | [`types`] | Domain types (CoinRecord, BlockData, etc.) | API-002, API-005, API-006, API-007, API-008..009 |
//! | [`block_apply`] | Block application pipeline | BLK-001..014 |
//! | [`rollback`] | Rollback / reorg recovery | RBK-001..007 |
//! | [`queries`] | Coin state queries | QRY-001..011 |
//! | [`hints`] | Hint store | HNT-001..006 |
//! | [`storage`] | Backend trait + implementations | STO-001..008 |
//! | [`merkle`] | Sparse Merkle tree + proofs | MRK-001..006 |
//! | [`cache`] | In-memory caching (unspent set, LRU, counters) | PRF-001..003 |
//! | [`archive`] | Tiered spent coin archival | PRF-005 |
//!
//! ## Spec reference
//!
//! Master specification: `docs/resources/SPEC.md`
//! Requirements: `docs/requirements/IMPLEMENTATION_ORDER.md`
//!
//! See also:
//! - Chia CoinStore: `chia/full_node/coin_store.py`
//! - Chia HintStore: `chia/full_node/hint_store.py`

// ─────────────────────────────────────────────────────────────────────────────
// Re-exports: Chia ecosystem types (STR-005)
// ─────────────────────────────────────────────────────────────────────────────
// These re-exports ensure consumers of dig-coinstore can access core Chia types
// without adding direct dependencies on chia-protocol or dig-clvm.
//
// The dependency chain is:
//   chia-protocol  →  dig-clvm  →  dig-coinstore  →  consumers
//       (defines)    (re-exports)   (re-exports)
//
// Requirement: STR-005
// Spec: docs/requirements/domains/crate_structure/specs/STR-005.md

/// The fundamental coin type in the Chia coinset model.
/// `CoinId = sha256(parent_coin_info || puzzle_hash || amount)`.
/// Re-exported from `dig-clvm` (which re-exports from `chia-protocol`).
pub use dig_clvm::Coin;

/// A 32-byte hash used for coin IDs, puzzle hashes, block hashes, state roots.
/// Re-exported from `dig-clvm` (which re-exports from `chia-protocol`).
pub use dig_clvm::Bytes32;

/// Lightweight coin state for the sync protocol: coin + created_height + spent_height.
/// Re-exported from `dig-clvm` (which re-exports from `chia-protocol`).
pub use dig_clvm::CoinState;

/// Filters for batch coin state queries (include_spent, include_unspent, include_hinted, min_amount).
/// Used by `batch_coin_states_by_puzzle_hashes()` (QRY-007).
/// Not re-exported by dig-clvm, so imported directly from chia-protocol.
pub use chia_protocol::CoinStateFilters;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level modules
// ─────────────────────────────────────────────────────────────────────────────
// Each module corresponds to one or more requirement domains in
// docs/requirements/domains/. The module hierarchy is defined by STR-002.

/// CoinStore struct — primary public API orchestration.
/// See: docs/requirements/domains/crate_api/specs/API-001.md
pub mod coin_store;

/// Configuration types and constants.
/// See: docs/requirements/domains/crate_api/specs/API-003.md
pub mod config;

// API-003: expose config surface at crate root (see also `storage::StorageBackend` trait).
pub use config::{
    default_storage_backend_for_features, CoinStoreConfig, StorageBackend,
    BLOOM_FILTER_BITS_PER_KEY, DEFAULT_LMDB_MAP_SIZE, DEFAULT_MAX_QUERY_RESULTS,
    DEFAULT_MAX_SNAPSHOTS, DEFAULT_ROCKSDB_MAX_OPEN_FILES, DEFAULT_ROCKSDB_WRITE_BUFFER_SIZE,
};

/// Error types for all coinstore operations.
/// See: docs/requirements/domains/crate_api/specs/API-004.md
pub mod error;

pub use error::CoinStoreError;

/// Domain types: CoinRecord, BlockData, CoinAddition, result structs, type aliases.
/// See: docs/requirements/domains/crate_api/specs/API-002.md
pub mod types;

// Wire-shaped coin row for interop (see `types` module doc: mirrors upstream `CoinRecord` until
// `chia-protocol` in the `dig-clvm` graph exposes it; then replace with `pub use chia_protocol::CoinRecord as ChiaCoinRecord`).
pub use types::{
    ApplyBlockResult, BlockData, ChiaCoinRecord, CoinAddition, CoinId, CoinRecord, CoinStoreStats,
    PuzzleHash, RollbackResult,
};

/// Block application pipeline (Phase 1 validation + Phase 2 mutation).
/// See: docs/requirements/domains/block_application/specs/
pub mod block_apply;

/// Rollback pipeline for chain reorganization recovery.
/// See: docs/requirements/domains/rollback/specs/
pub mod rollback;

/// All query method implementations on CoinStore.
/// See: docs/requirements/domains/queries/specs/
pub mod queries;

/// Hint store for puzzle hash hints on coins.
/// See: docs/requirements/domains/hints/specs/
pub mod hints;

/// Tiered spent coin archival (hot/archive/prune).
/// See: docs/requirements/domains/performance/specs/PRF-005.md
pub mod archive;

// ─────────────────────────────────────────────────────────────────────────────
// Subdirectory modules
// ─────────────────────────────────────────────────────────────────────────────

/// Storage backend abstraction (trait + RocksDB/LMDB implementations).
/// Backend modules are feature-gated: `rocksdb-storage`, `lmdb-storage`.
/// See: docs/requirements/domains/storage/specs/
pub mod storage;

/// Sparse Merkle tree for state root computation and proofs.
/// See: docs/requirements/domains/merkle/specs/
pub mod merkle;

/// In-memory caching: unspent set, LRU cache, materialized counters.
/// See: docs/requirements/domains/performance/specs/PRF-001..003.md
pub mod cache;
