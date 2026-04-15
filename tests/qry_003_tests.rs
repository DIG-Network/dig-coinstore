//! # QRY-003 Tests — Height Queries
//!
//! Verifies **QRY-003**: `get_coins_added_at_height()` and `get_coins_removed_at_height()`.
//!
//! # Requirement: QRY-003
//! # SPEC.md: §3.6, §1.5 #12 (height 0 removals empty), Chia: coin_store.py:223-254

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (CoinStore, tempfile::TempDir, chia_protocol::Coin) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    store.init_genesis(vec![(genesis_coin, false)], 1_700_000_000).unwrap();

    // Block 1: spend genesis, add new coin
    let new_coin = helpers::test_coin(10, 11, 500);
    let block = dig_coinstore::BlockData {
        height: 1, timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1), parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![CoinAddition::from_coin(new_coin, false)],
        removals: vec![genesis_coin.coin_id()],
        coinbase_coins: vec![helpers::test_coin(200, 201, 1_750_000_000_000), helpers::test_coin(202, 203, 250_000_000_000)],
        hints: vec![], expected_state_root: None,
    };
    store.apply_block(block).unwrap();
    (store, dir, genesis_coin)
}

/// Genesis coins appear at height 0.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_003_added_at_genesis() {
    let (store, _dir, _) = setup();
    let coins = store.get_coins_added_at_height(0).unwrap();
    assert_eq!(coins.len(), 1, "1 genesis coin at height 0");
}

/// Block 1 additions appear at height 1 (tx addition + 2 coinbase).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_003_added_at_height_1() {
    let (store, _dir, _) = setup();
    let coins = store.get_coins_added_at_height(1).unwrap();
    assert_eq!(coins.len(), 3, "1 tx addition + 2 coinbase at height 1");
}

/// Coins removed at height 1 (genesis coin spent).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_003_removed_at_height_1() {
    let (store, _dir, genesis_coin) = setup();
    let coins = store.get_coins_removed_at_height(1).unwrap();
    assert_eq!(coins.len(), 1, "1 coin spent at height 1");
    assert_eq!(coins[0].coin, genesis_coin);
}

/// Height 0 removals always returns empty (SPEC.md §1.5 #12).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_003_removed_at_height_0_empty() {
    let (store, _dir, _) = setup();
    let coins = store.get_coins_removed_at_height(0).unwrap();
    assert!(coins.is_empty(), "Height 0 removals must be empty per §1.5 #12");
}

/// Non-existent height returns empty.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_003_no_coins_at_height_999() {
    let (store, _dir, _) = setup();
    assert!(store.get_coins_added_at_height(999).unwrap().is_empty());
    assert!(store.get_coins_removed_at_height(999).unwrap().is_empty());
}
