//! # CON-004 Tests — Parallel Removal Validation via In-Memory Unspent Set
//!
//! Verifies **CON-004**: removal validation in `apply_block` Phase 1 checks each coin
//! exists and is unspent before any mutations. Large removal batches complete correctly.
//! The in-memory unspent set (PRF-001) will accelerate this validation; currently
//! Phase 1 reads from storage.
//!
//! # Requirement: CON-004
//! # SPEC.md: §1.6 #20 (parallel removal validation), §1.6 #13 (unspent set)
//!
//! ## How these tests prove the requirement
//!
//! - **Large removal batch:** 50 coins created at genesis, all spent in one block.
//! - **Invalid removal detection:** Nonexistent coin ID in removals rejects the block.
//! - **Already-spent detection:** Spending a coin that was spent in a previous block fails.
//! - **Removal validation is pre-mutation:** Invalid block leaves state unchanged.
//! - **Additions and removals in same block:** Both succeed atomically.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition, CoinStoreError};

/// Build a minimal valid block at the given height.
fn make_block(height: u64, parent_hash: Bytes32, block_hash: Bytes32) -> BlockData {
    let coinbase_coins = if height == 0 {
        vec![]
    } else {
        vec![
            helpers::test_coin(200 + height as u8, 201, 1_750_000_000_000),
            helpers::test_coin(202 + height as u8, 203, 250_000_000_000),
        ]
    };
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

/// **CON-004:** Large removal batch — spend 50 coins in one block.
///
/// **Proof:** Create 50 coins at genesis, spend all 50 in block 1. All 50 must be marked
/// spent. This exercises the removal validation path at scale.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_004_large_removal_batch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    let mut genesis_coins = Vec::new();
    let mut removal_ids = Vec::new();
    for i in 0..50u8 {
        let coin = helpers::test_coin(i, 100, 1000 + i as u64);
        removal_ids.push(coin.coin_id());
        genesis_coins.push((coin, false));
    }
    store.init_genesis(genesis_coins, 1_700_000_000).unwrap();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xC1));
    block.removals = removal_ids.clone();
    let result = store.apply_block(block).unwrap();
    assert_eq!(result.coins_spent, 50, "All 50 coins should be spent");

    // Verify all are now marked as spent in storage
    for id in &removal_ids {
        let rec = store.get_coin_record(id).unwrap().unwrap();
        assert!(rec.is_spent(), "Coin should be spent after removal");
        assert_eq!(rec.spent_height, Some(1));
    }
}

/// **CON-004:** Removal of nonexistent coin detected at scale.
///
/// **Proof:** 49 valid removals + 1 nonexistent coin ID -- the entire block is rejected.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_004_invalid_removal_rejects_entire_block() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    let mut genesis_coins = Vec::new();
    let mut removal_ids = Vec::new();
    for i in 0..49u8 {
        let coin = helpers::test_coin(i, 100, 1000);
        removal_ids.push(coin.coin_id());
        genesis_coins.push((coin, false));
    }
    store.init_genesis(genesis_coins, 1_700_000_000).unwrap();

    // Add one fake coin ID
    let fake_id = helpers::test_coin(99, 99, 99).coin_id();
    removal_ids.push(fake_id);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xC3));
    block.removals = removal_ids;
    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::CoinNotFound(_)),
        "Block with invalid removal must fail: {:?}",
        err
    );

    // No state change -- height still 0
    assert_eq!(store.height(), 0, "Failed block must not change height");
}

/// **CON-004:** Already-spent coin in a subsequent block is detected.
///
/// **Proof:** Spend coin in block 1; trying to spend it again in block 2 returns DoubleSpend.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_004_double_spend_across_blocks() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // Block 1: spend the coin
    let hash1 = helpers::test_hash(0xB1);
    let mut b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    b1.removals = vec![id];
    store.apply_block(b1).unwrap();

    // Block 2: try to spend the same coin again
    let hash2 = helpers::test_hash(0xB2);
    let mut b2 = make_block(2, hash1, hash2);
    b2.removals = vec![id];
    let err = store.apply_block(b2).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::DoubleSpend(_)),
        "Double spend must be detected: {:?}",
        err
    );
}

/// **CON-004:** Additions and removals in the same block both succeed atomically.
///
/// **Proof:** Spend one genesis coin and add a new one in the same block. Both operations
/// are reflected in storage after the block.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_004_additions_and_removals_same_block() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let old_coin = helpers::test_coin(1, 2, 500);
    let old_id = old_coin.coin_id();
    store
        .init_genesis(vec![(old_coin, false)], 1_700_000_000)
        .unwrap();

    let new_coin = helpers::test_coin(10, 11, 999);
    let new_id = new_coin.coin_id();
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xC5));
    block.removals = vec![old_id];
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    let result = store.apply_block(block).unwrap();
    assert_eq!(result.coins_spent, 1);
    assert_eq!(result.coins_created, 3); // 2 coinbase + 1 addition

    // Old coin is spent
    let old_rec = store.get_coin_record(&old_id).unwrap().unwrap();
    assert!(old_rec.is_spent(), "Old coin should be spent");

    // New coin exists and is unspent
    let new_rec = store.get_coin_record(&new_id).unwrap().unwrap();
    assert!(!new_rec.is_spent(), "New coin should be unspent");
}

/// **CON-004:** Validation prevents mutation on invalid block.
///
/// **Proof:** A block with both valid additions and an invalid removal fails, and no
/// state changes occur -- the additions are NOT persisted either.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_004_validation_prevents_partial_mutation() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let new_coin = helpers::test_coin(50, 51, 999);
    let fake_removal = helpers::test_coin(99, 99, 99).coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xC6));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    block.removals = vec![fake_removal];

    let err = store.apply_block(block).unwrap_err();
    assert!(matches!(err, CoinStoreError::CoinNotFound(_)));

    // No state change
    assert_eq!(store.height(), 0);
    let rec = store.get_coin_record(&new_coin.coin_id()).unwrap();
    assert!(
        rec.is_none(),
        "Failed block should not persist any additions"
    );
}
