//! # BLK-004 Tests — Reward Coin Count Assertion
//!
//! Verifies requirement **BLK-004**: `apply_block()` MUST reject a non-genesis block
//! whose `coinbase_coins.len()` is less than `MIN_REWARD_COINS_PER_BLOCK` (2).
//! Returns `CoinStoreError::InvalidRewardCoinCount { expected, got }` on violation.
//!
//! # Requirement: BLK-004
//! # Spec: docs/requirements/domains/block_application/specs/BLK-004.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-004
//! # SPEC.md: §1.5 #11 (reward coin validation), §2.7 MIN_REWARD_COINS_PER_BLOCK = 2
//!
//! ## How these tests prove the requirement
//!
//! - **2 coinbase coins succeeds:** The minimum valid count (farmer + pool reward)
//!   applies without error — proves the boundary is `>=` not `>`.
//! - **0 coinbase coins fails:** Empty coinbase_coins returns `InvalidRewardCoinCount`.
//! - **1 coinbase coin fails:** Sub-minimum count (only farmer reward) is rejected.
//! - **3 coinbase coins succeeds:** Above-minimum count is accepted — proves no upper
//!   bound restriction beyond the minimum.
//! - **5 coinbase coins succeeds:** Confirms that generous reward counts are valid.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinStoreError};

// ─────────────────────────────────────────────────────────────────────────────
// Block builder helper
// ─────────────────────────────────────────────────────────────────────────────

/// Build a block at the given height with a custom coinbase coin list.
/// Does NOT auto-populate coinbase coins — caller provides them explicitly.
fn make_block_with_coinbase(
    height: u64,
    parent_hash: Bytes32,
    block_hash: Bytes32,
    coinbase_coins: Vec<chia_protocol::Coin>,
) -> BlockData {
    BlockData {
        height,
        timestamp: 1_700_000_000 + height * 18,
        block_hash,
        parent_hash,
        additions: vec![],
        removals: vec![],
        coinbase_coins,
        hints: vec![],
        expected_state_root: None,
    }
}

/// Create N distinct coinbase coins for testing reward count boundaries.
fn make_coinbase_coins(count: u8, height: u64) -> Vec<chia_protocol::Coin> {
    (0..count)
        .map(|i| {
            helpers::test_coin(
                200 + height as u8 + i * 2,
                201 + i * 2,
                1_000_000_000_000 + i as u64 * 100,
            )
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// BLK-004: Reward coin count assertion
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-004 / 2 coinbase coins:** The minimum valid count succeeds.
///
/// **Proof:** A block with exactly 2 coinbase coins (farmer + pool reward) at height 1
/// applies without error. This confirms the boundary condition `>= 2` is satisfied.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_004_two_coinbase_succeeds() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coinbase = make_coinbase_coins(2, 1);
    let block = make_block_with_coinbase(
        1,
        Bytes32::from([0u8; 32]),
        helpers::test_hash(0xB1),
        coinbase,
    );
    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "2 coinbase coins (minimum) must succeed: {:?}",
        result.err()
    );
    assert_eq!(store.height(), 1);
}

/// **BLK-004 / 0 coinbase coins:** Empty coinbase returns `InvalidRewardCoinCount`.
///
/// **Proof:** A non-genesis block (height 1) with zero coinbase coins fails with
/// `InvalidRewardCoinCount { expected: ">= 2", got: 0 }`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_004_zero_coinbase_fails() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block_with_coinbase(
        1,
        Bytes32::from([0u8; 32]),
        helpers::test_hash(0xB1),
        vec![],
    );
    let err = store.apply_block(block).unwrap_err();
    match err {
        CoinStoreError::InvalidRewardCoinCount { ref expected, got } => {
            assert!(
                expected.contains("2"),
                "Expected message should reference the minimum of 2, got: {}",
                expected
            );
            assert_eq!(got, 0, "Got field must be 0 for empty coinbase");
        }
        other => panic!("Expected InvalidRewardCoinCount, got: {:?}", other),
    }
}

/// **BLK-004 / 1 coinbase coin:** Sub-minimum count returns `InvalidRewardCoinCount`.
///
/// **Proof:** A block with only 1 coinbase coin (missing pool reward) fails.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_004_one_coinbase_fails() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coinbase = make_coinbase_coins(1, 1);
    let block = make_block_with_coinbase(
        1,
        Bytes32::from([0u8; 32]),
        helpers::test_hash(0xB1),
        coinbase,
    );
    let err = store.apply_block(block).unwrap_err();
    match err {
        CoinStoreError::InvalidRewardCoinCount { ref expected, got } => {
            assert!(
                expected.contains("2"),
                "Expected message should reference the minimum of 2, got: {}",
                expected
            );
            assert_eq!(got, 1, "Got field must be 1 for single coinbase");
        }
        other => panic!("Expected InvalidRewardCoinCount, got: {:?}", other),
    }
}

/// **BLK-004 / 3 coinbase coins:** Above-minimum count succeeds.
///
/// **Proof:** A block with 3 coinbase coins applies without error.
/// Confirms there is no upper-bound restriction — only a minimum.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_004_three_coinbase_succeeds() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coinbase = make_coinbase_coins(3, 1);
    let block = make_block_with_coinbase(
        1,
        Bytes32::from([0u8; 32]),
        helpers::test_hash(0xB1),
        coinbase,
    );
    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "3 coinbase coins must succeed: {:?}",
        result.err()
    );
}

/// **BLK-004 / 5 coinbase coins:** Generous reward count succeeds.
///
/// **Proof:** A block with 5 coinbase coins applies without error.
/// Confirms generous reward counts are not capped.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_004_five_coinbase_succeeds() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coinbase = make_coinbase_coins(5, 1);
    let block = make_block_with_coinbase(
        1,
        Bytes32::from([0u8; 32]),
        helpers::test_hash(0xB1),
        coinbase,
    );
    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "5 coinbase coins must succeed: {:?}",
        result.err()
    );
}
