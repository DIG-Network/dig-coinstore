//! Shared test utilities for dig-coinstore integration tests.
//!
//! Provides:
//! - **Coin builders**: Create `Coin` instances with deterministic, reproducible fields
//! - **Hash utilities**: Generate deterministic `Bytes32` values from seeds
//! - **Temporary directory management**: Wrappers around `tempfile::TempDir`
//! - **Block builder helpers**: Construct `BlockData` payloads for `apply_block()` tests
//!
//! All test files import this module via `mod helpers;` at the top of each
//! integration test file. This leverages Cargo's module resolution for the
//! `tests/` directory where `tests/helpers/mod.rs` is treated as a shared
//! module, not a standalone test binary.
//!
//! # Requirement: STR-006
//! # Spec: docs/requirements/domains/crate_structure/specs/STR-006.md

use chia_protocol::{Bytes32, Coin, CoinState};
use chia_sha2::Sha256;

// ─────────────────────────────────────────────────────────────────────────────
// Hash utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Create a deterministic `Bytes32` from a single seed byte.
///
/// Useful for generating unique but reproducible coin IDs, puzzle hashes,
/// block hashes, etc. Each seed produces a distinct hash.
///
/// Note: This is NOT the same as `Bytes32::from([seed; 32])` — it produces
/// a proper SHA-256 hash of the seed byte, which has better distribution
/// for testing key-space-dependent behavior (like Merkle tree bit paths).
#[allow(dead_code)]
pub fn test_hash(seed: u8) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update([seed]);
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Bytes32::from(bytes)
}

/// Create a deterministic `Bytes32` from a string label.
///
/// Useful for named test values: `test_hash_str("genesis_puzzle")`.
/// Same label always produces the same hash.
#[allow(dead_code)]
pub fn test_hash_str(label: &str) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Bytes32::from(bytes)
}

/// Create a simple `Bytes32` filled with a single byte value.
///
/// Less realistic than `test_hash` but useful when you need predictable
/// byte patterns for visual debugging (e.g., `[0xAA; 32]`).
#[allow(dead_code)]
pub fn filled_hash(byte: u8) -> Bytes32 {
    Bytes32::from([byte; 32])
}

// ─────────────────────────────────────────────────────────────────────────────
// Coin builders
// ─────────────────────────────────────────────────────────────────────────────

/// Create a test coin with deterministic fields derived from seed bytes.
///
/// `parent_seed`: seed for the parent coin info hash.
/// `puzzle_seed`: seed for the puzzle hash.
/// `amount`: coin value in mojos.
///
/// The coin ID is deterministic: same seeds + amount always produce the
/// same `Coin::coin_id()`.
#[allow(dead_code)]
pub fn test_coin(parent_seed: u8, puzzle_seed: u8, amount: u64) -> Coin {
    Coin::new(test_hash(parent_seed), test_hash(puzzle_seed), amount)
}

/// Create N test coins with sequential seeds and the same puzzle hash.
///
/// Useful for populating a coinstate with many coins for the same "address"
/// (puzzle hash). Returns `(coins, puzzle_hash)`.
#[allow(dead_code)]
pub fn test_coins_same_puzzle(count: u8, puzzle_seed: u8, amount: u64) -> (Vec<Coin>, Bytes32) {
    let puzzle_hash = test_hash(puzzle_seed);
    let coins: Vec<Coin> = (0..count)
        .map(|i| Coin::new(test_hash(i), puzzle_hash, amount))
        .collect();
    (coins, puzzle_hash)
}

/// Compute the coin ID of a coin using `Coin::coin_id()`.
///
/// This is the canonical identity: `sha256(parent_coin_info || puzzle_hash || amount)`.
/// Wrapper for convenience in test assertions.
#[allow(dead_code)]
pub fn coin_id(coin: &Coin) -> Bytes32 {
    coin.coin_id()
}

// ─────────────────────────────────────────────────────────────────────────────
// CoinState builders
// ─────────────────────────────────────────────────────────────────────────────

/// Create a CoinState for an unspent coin created at `created_height`.
#[allow(dead_code)]
pub fn unspent_coin_state(coin: Coin, created_height: u32) -> CoinState {
    CoinState::new(coin, None, Some(created_height))
}

/// Create a CoinState for a coin created at `created_height` and spent at `spent_height`.
#[allow(dead_code)]
pub fn spent_coin_state(coin: Coin, created_height: u32, spent_height: u32) -> CoinState {
    CoinState::new(coin, Some(spent_height), Some(created_height))
}

// ─────────────────────────────────────────────────────────────────────────────
// Temporary directory management
// ─────────────────────────────────────────────────────────────────────────────

/// Create a temporary directory for storage backend tests.
///
/// Returns a `tempfile::TempDir` that auto-deletes on drop.
/// The directory is created in the system temp location.
///
/// # Usage
///
/// ```ignore
/// let dir = helpers::temp_dir();
/// let backend = RocksDbBackend::open(dir.path()).unwrap();
/// // ... use backend ...
/// // dir auto-deleted when it goes out of scope
/// ```
#[allow(dead_code)]
pub fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory for test")
}

// ─────────────────────────────────────────────────────────────────────────────
// Block builder helpers
// ─────────────────────────────────────────────────────────────────────────────
// These will be fleshed out when BlockData is defined (API-005, Phase 1).
// For now, they provide the foundational patterns that BlockData builders
// will use.

/// Parameters for building a test block.
///
/// This is a pre-BlockData helper struct. Once API-005 defines `BlockData`,
/// this will be converted to produce `BlockData` instances directly.
#[allow(dead_code)]
pub struct TestBlockParams {
    /// Block height.
    pub height: u64,
    /// Block timestamp (unix seconds).
    pub timestamp: u64,
    /// Block hash (derived from height if not specified).
    pub block_hash: Bytes32,
    /// Parent block hash.
    pub parent_hash: Bytes32,
    /// Coins to create in this block.
    pub additions: Vec<Coin>,
    /// Coin IDs to spend in this block.
    pub removals: Vec<Bytes32>,
    /// Coinbase reward coins.
    pub coinbase_coins: Vec<Coin>,
    /// Hints: (coin_id, hint_hash) pairs.
    pub hints: Vec<(Bytes32, Bytes32)>,
}

impl TestBlockParams {
    /// Create a minimal block at the given height.
    ///
    /// Parent hash is derived from `height - 1` seed. Block hash is derived
    /// from `height` seed. No additions, removals, or hints.
    /// Two coinbase coins are included (farmer + pool reward) for height > 0.
    #[allow(dead_code)]
    pub fn at_height(height: u64) -> Self {
        let block_hash = test_hash_str(&format!("block_{}", height));
        let parent_hash = if height == 0 {
            Bytes32::from([0u8; 32]) // Genesis parent is zero hash
        } else {
            test_hash_str(&format!("block_{}", height - 1))
        };

        // Coinbase: farmer reward + pool reward (required for height > 0)
        let coinbase_coins = if height == 0 {
            vec![]
        } else {
            let farmer_reward = Coin::new(
                test_hash_str(&format!("farmer_parent_{}", height)),
                test_hash_str("farmer_puzzle"),
                1_750_000_000_000, // 1.75 XCH equivalent
            );
            let pool_reward = Coin::new(
                test_hash_str(&format!("pool_parent_{}", height)),
                test_hash_str("pool_puzzle"),
                250_000_000_000, // 0.25 XCH equivalent
            );
            vec![farmer_reward, pool_reward]
        };

        Self {
            height,
            timestamp: 1_700_000_000 + height * 18, // ~18s per block
            block_hash,
            parent_hash,
            additions: vec![],
            removals: vec![],
            coinbase_coins,
            hints: vec![],
        }
    }

    /// Add coins to be created in this block.
    #[allow(dead_code)]
    pub fn with_additions(mut self, coins: Vec<Coin>) -> Self {
        self.additions = coins;
        self
    }

    /// Add coin IDs to be spent in this block.
    #[allow(dead_code)]
    pub fn with_removals(mut self, coin_ids: Vec<Bytes32>) -> Self {
        self.removals = coin_ids;
        self
    }

    /// Add hints for coins created in this block.
    #[allow(dead_code)]
    pub fn with_hints(mut self, hints: Vec<(Bytes32, Bytes32)>) -> Self {
        self.hints = hints;
        self
    }
}
