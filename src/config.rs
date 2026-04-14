//! Configuration types and constants for dig-coinstore.
//!
//! Contains [`CoinStoreConfig`] with builder pattern and all tunable parameters,
//! plus named constants referenced by the SPEC (Section 2.7).
//!
//! # Requirement: API-003
//! # Spec: docs/requirements/domains/crate_api/specs/API-003.md
//! # SPEC.md: Section 2.6, 2.7

use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// Constants (SPEC Section 2.7)
// ─────────────────────────────────────────────────────────────────────────────

/// Default number of snapshots to retain before pruning.
pub const DEFAULT_MAX_SNAPSHOTS: usize = 10;

/// Maximum query results per batch. Matches Chia's default `max_items=50000`.
pub const DEFAULT_MAX_QUERY_RESULTS: usize = 50_000;

// ─────────────────────────────────────────────────────────────────────────────
// CoinStoreConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for a CoinStore instance.
///
/// All fields have sensible defaults. Use `default_with_path()` for the
/// common case of "defaults + custom storage path".
///
/// # Requirement: API-003
/// # Spec: docs/requirements/domains/crate_api/specs/API-003.md
#[derive(Debug, Clone)]
pub struct CoinStoreConfig {
    /// Filesystem path for the storage directory.
    /// Created automatically if it doesn't exist.
    pub storage_path: PathBuf,

    /// Maximum number of state snapshots to retain.
    /// Default: `DEFAULT_MAX_SNAPSHOTS` (10).
    pub max_snapshots: usize,

    /// Maximum results per batch query.
    /// Default: `DEFAULT_MAX_QUERY_RESULTS` (50,000).
    pub max_query_results: usize,
}

impl CoinStoreConfig {
    /// Create a config with defaults and a custom storage path.
    ///
    /// This is the most common constructor — equivalent to what `CoinStore::new(path)`
    /// uses internally.
    pub fn default_with_path(path: impl AsRef<Path>) -> Self {
        Self {
            storage_path: path.as_ref().to_path_buf(),
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            max_query_results: DEFAULT_MAX_QUERY_RESULTS,
        }
    }

    /// Builder: set custom max_snapshots.
    pub fn with_max_snapshots(mut self, max: usize) -> Self {
        self.max_snapshots = max;
        self
    }

    /// Builder: set custom max_query_results.
    pub fn with_max_query_results(mut self, max: usize) -> Self {
        self.max_query_results = max;
        self
    }
}
