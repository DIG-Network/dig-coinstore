//! # QRY-008 Tests — Singleton Lineage Lookup
//!
//! Verifies **QRY-008**: `get_unspent_lineage_info_for_puzzle_hash()`.
//!
//! # Requirement: QRY-008
//! # SPEC.md: §3.10, §2.5

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_008_exactly_one_unspent() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 100);
    store.init_genesis(vec![(coin, false)], 1_700_000_000).unwrap();

    let ph = helpers::test_hash(2);
    let info = store.get_unspent_lineage_info_for_puzzle_hash(&ph).unwrap();
    assert!(info.is_some(), "Exactly 1 unspent coin → Some");
    let info = info.unwrap();
    assert_eq!(info.coin_id, coin.coin_id());
    assert_eq!(info.parent_id, coin.parent_coin_info);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_008_zero_unspent_returns_none() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let ph = helpers::test_hash(99);
    let info = store.get_unspent_lineage_info_for_puzzle_hash(&ph).unwrap();
    assert!(info.is_none(), "No coins → None");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_008_multiple_unspent_returns_none() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    // Two coins with same puzzle hash
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 2, 200);
    store.init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000).unwrap();

    let ph = helpers::test_hash(2);
    let info = store.get_unspent_lineage_info_for_puzzle_hash(&ph).unwrap();
    assert!(info.is_none(), "2 unspent coins → None (not a singleton)");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_008_spent_coin_not_counted() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 2, 200);
    store.init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000).unwrap();

    // Spend c1 → only c2 remains unspent for puzzle_hash(2)
    let block = dig_coinstore::BlockData {
        height: 1, timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1), parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![], removals: vec![c1.coin_id()],
        coinbase_coins: vec![helpers::test_coin(200, 201, 1_750_000_000_000), helpers::test_coin(202, 203, 250_000_000_000)],
        hints: vec![], expected_state_root: None,
    };
    store.apply_block(block).unwrap();

    let ph = helpers::test_hash(2);
    let info = store.get_unspent_lineage_info_for_puzzle_hash(&ph).unwrap();
    assert!(info.is_some(), "After spending c1, exactly 1 unspent → Some");
    assert_eq!(info.unwrap().coin_id, c2.coin_id());
}
