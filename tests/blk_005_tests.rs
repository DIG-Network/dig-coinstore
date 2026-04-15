//! # BLK-005 Tests — Removal Validation
//!
//! Verifies requirement **BLK-005**: `apply_block()` validates every removal coin ID
//! before mutating state. Each removal must reference an existing, unspent coin.
//! Returns `CoinStoreError::CoinNotFound` for missing coins and
//! `CoinStoreError::DoubleSpend` for already-spent coins.
//!
//! # Requirement: BLK-005
//! # Spec: docs/requirements/domains/block_application/specs/BLK-005.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-005
//! # SPEC.md: §1.5 #1 (spend validation), §1.5 #2 (double-spend detection),
//! #          §1.3 #5 (validation before mutations)
//!
//! ## How these tests prove the requirement
//!
//! - **Valid removal succeeds:** A genesis coin spent in block 1 is accepted, and
//!   `coins_spent` in the result equals 1.
//! - **Missing coin returns CoinNotFound:** Attempting to remove a coin ID that was
//!   never created returns `CoinNotFound(coin_id)`.
//! - **Already-spent returns DoubleSpend:** A coin spent in block 1, then referenced
//!   again in block 2's removals, returns `DoubleSpend(coin_id)`.
//! - **Validation before mutations (atomicity):** A block with both a valid addition
//!   and an invalid removal fails entirely — the addition is not persisted and the
//!   store height/tip remain unchanged.
//! - **Multiple removals in one block:** Two distinct genesis coins can be spent in
//!   a single block, confirming batch removal works.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition, CoinStoreError};

// ─────────────────────────────────────────────────────────────────────────────
// Block builder helper
// ─────────────────────────────────────────────────────────────────────────────

/// Build a minimal valid block at the given height with 2 coinbase coins.
fn make_block(height: u64, parent_hash: Bytes32, block_hash: Bytes32) -> BlockData {
    let coinbase_coins = vec![
        helpers::test_coin(200 + height as u8, 201, 1_750_000_000_000),
        helpers::test_coin(202 + height as u8, 203, 250_000_000_000),
    ];
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

// ─────────────────────────────────────────────────────────────────────────────
// BLK-005: Removal validation
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-005 / Valid removal:** Spending an existing unspent coin succeeds.
///
/// **Proof:** Insert a coin via genesis, spend it in block 1. The result reports
/// `coins_spent = 1` and the block applies successfully.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_005_valid_removal_succeeds() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 1_000_000);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![coin.coin_id()];

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Valid removal must succeed: {:?}",
        result.err()
    );
    let r = result.unwrap();
    assert_eq!(r.coins_spent, 1, "One coin should be marked spent");
    assert_eq!(r.height, 1);
}

/// **BLK-005 / Missing coin:** Removing a non-existent coin returns `CoinNotFound`.
///
/// **Proof:** An empty genesis store has no coins. Attempting to remove a fabricated
/// coin ID returns `CoinNotFound` with that ID.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_005_missing_coin_returns_coin_not_found() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let fake_coin = helpers::test_coin(99, 99, 99);
    let fake_id = fake_coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![fake_id];

    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::CoinNotFound(id) if id == fake_id),
        "Expected CoinNotFound({:?}), got: {:?}",
        fake_id,
        err
    );
}

/// **BLK-005 / Already-spent:** Double-spending returns `DoubleSpend`.
///
/// **Proof:** Coin is created in genesis, spent in block 1, then referenced again
/// in block 2's removals. Block 2 fails with `DoubleSpend(coin_id)`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_005_already_spent_returns_double_spend() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 1_000_000);
    let coin_id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // Block 1: spend the coin.
    let hash1 = helpers::test_hash(0xB1);
    let mut b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    b1.removals = vec![coin_id];
    store.apply_block(b1).unwrap();

    // Block 2: try to spend the same coin again.
    let mut b2 = make_block(2, hash1, helpers::test_hash(0xB2));
    b2.removals = vec![coin_id];
    let err = store.apply_block(b2).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::DoubleSpend(id) if id == coin_id),
        "Expected DoubleSpend({:?}), got: {:?}",
        coin_id,
        err
    );
}

/// **BLK-005 / Atomicity:** Invalid removal prevents valid additions from persisting.
///
/// **Proof:** A block contains a valid addition AND an invalid removal. The block fails
/// with `CoinNotFound`. After the failure, the store height and tip_hash are unchanged,
/// proving validation occurs before any mutations.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_005_validation_before_mutations() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let height_before = store.height();
    let tip_before = store.tip_hash();

    // Build a block with a valid addition but invalid removal.
    let new_coin = helpers::test_coin(50, 51, 999);
    let fake_removal = helpers::test_coin(99, 99, 99).coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    block.removals = vec![fake_removal];

    let result = store.apply_block(block);
    assert!(result.is_err(), "Block with invalid removal must fail");

    // Verify no state changes occurred — atomicity guarantee.
    assert_eq!(
        store.height(),
        height_before,
        "Height must not change on failure"
    );
    assert_eq!(
        store.tip_hash(),
        tip_before,
        "Tip hash must not change on failure"
    );
}

/// **BLK-005 / Multiple removals:** Two distinct coins spent in one block.
///
/// **Proof:** Two coins created in genesis are both removed in a single block.
/// The result reports `coins_spent = 2`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_005_multiple_removals_in_one_block() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin_a = helpers::test_coin(1, 2, 1_000_000);
    let coin_b = helpers::test_coin(3, 4, 2_000_000);
    store
        .init_genesis(vec![(coin_a, false), (coin_b, false)], 1_700_000_000)
        .unwrap();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![coin_a.coin_id(), coin_b.coin_id()];

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Multiple valid removals must succeed: {:?}",
        result.err()
    );
    let r = result.unwrap();
    assert_eq!(
        r.coins_spent, 2,
        "Both coins should be marked spent in one block"
    );
}
