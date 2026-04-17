//! # PRF-004 Tests — Unspent-Only Puzzle Hash Index
//!
//! Verifies **PRF-004**: the `CF_UNSPENT_BY_PUZZLE_HASH` column family is maintained
//! during block application and rollback, providing an index of only unspent coins
//! by puzzle hash for efficient queries.
//!
//! # Requirement: PRF-004
//! # SPEC.md: §1.6 #15 (Unspent-only puzzle hash index)
//!
//! ## How these tests prove the requirement
//!
//! - **Genesis populates index:** Unspent coins at genesis appear in puzzle hash queries.
//! - **Spend removes from index:** After spending, `get_coin_records_by_puzzle_hash` with
//!   `include_spent=false` excludes the spent coin.
//! - **Rollback restores index:** After rollback, previously-spent coins reappear in
//!   unspent puzzle hash queries.
//! - **Multiple coins same puzzle hash:** Multiple unspent coins under the same puzzle
//!   hash are all returned.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition};

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

/// **PRF-004:** Genesis coins appear in unspent puzzle hash queries.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_004_genesis_populates_unspent_index() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let puzzle_hash = helpers::test_hash(2);
    let coin = chia_protocol::Coin::new(helpers::test_hash(1), puzzle_hash, 100);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let results = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].coin.amount, 100);
}

/// **PRF-004:** Spending a coin removes it from unspent puzzle hash queries.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_004_spend_removes_from_unspent_index() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let puzzle_hash = helpers::test_hash(2);
    let coin = chia_protocol::Coin::new(helpers::test_hash(1), puzzle_hash, 100);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // Verify present before spend
    let before = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(before.len(), 1);

    // Spend the coin
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![id];
    store.apply_block(block).unwrap();

    // include_spent=false should now return empty for this puzzle hash
    let after = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(
        after.len(),
        0,
        "Spent coin should not appear in unspent query"
    );

    // include_spent=true should still return it
    let all = store
        .get_coin_records_by_puzzle_hash(true, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(
        all.len(),
        1,
        "Spent coin should still be in all-inclusive query"
    );
}

/// **PRF-004:** Rollback restores coins to the unspent puzzle hash index.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_004_rollback_restores_unspent_index() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let puzzle_hash = helpers::test_hash(2);
    let coin = chia_protocol::Coin::new(helpers::test_hash(1), puzzle_hash, 100);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // Spend in block 1
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![id];
    store.apply_block(block).unwrap();

    // Confirm removed from unspent index
    let mid = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(mid.len(), 0);

    // Rollback to 0
    store.rollback_to_block(0).unwrap();

    // Should be back in unspent index
    let restored = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(
        restored.len(),
        1,
        "Coin should reappear in unspent index after rollback"
    );
}

/// **PRF-004:** Multiple unspent coins with the same puzzle hash are all returned.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_004_multiple_coins_same_puzzle_hash() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let puzzle_hash = helpers::test_hash(10);
    let c1 = chia_protocol::Coin::new(helpers::test_hash(1), puzzle_hash, 100);
    let c2 = chia_protocol::Coin::new(helpers::test_hash(2), puzzle_hash, 200);
    let c3 = chia_protocol::Coin::new(helpers::test_hash(3), puzzle_hash, 300);
    store
        .init_genesis(vec![(c1, false), (c2, false), (c3, false)], 1_700_000_000)
        .unwrap();

    let results = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(results.len(), 3, "All 3 unspent coins should be returned");

    let total: u64 = results.iter().map(|r| r.coin.amount).sum();
    assert_eq!(total, 600);
}

/// **PRF-004:** Additions from apply_block appear in the unspent puzzle hash index.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_004_additions_appear_in_unspent_index() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let puzzle_hash = helpers::test_hash(20);
    let new_coin = chia_protocol::Coin::new(helpers::test_hash(50), puzzle_hash, 999);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    store.apply_block(block).unwrap();

    let results = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].coin.amount, 999);
}
