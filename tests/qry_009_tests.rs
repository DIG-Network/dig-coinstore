//! # QRY-009 Tests — Aggregate Queries
//!
//! Verifies **QRY-009**: `num_unspent()`, `total_unspent_value()`, `aggregate_unspent_by_puzzle_hash()`, `num_total()`.
//!
//! # Requirement: QRY-009
//! # SPEC.md: §3.11

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (CoinStore, tempfile::TempDir) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    // Genesis with 2 coins: amounts 100 and 200
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 2, 200); // same puzzle hash as c1
    store.init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000).unwrap();

    // Block 1: spend c1, add c3 (different puzzle hash)
    let c3 = helpers::test_coin(10, 11, 500);
    let block = dig_coinstore::BlockData {
        height: 1, timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1), parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![CoinAddition::from_coin(c3, false)],
        removals: vec![c1.coin_id()],
        coinbase_coins: vec![helpers::test_coin(200, 201, 1_750_000_000_000), helpers::test_coin(202, 203, 250_000_000_000)],
        hints: vec![], expected_state_root: None,
    };
    store.apply_block(block).unwrap();
    (store, dir)
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_009_num_unspent() {
    let (store, _dir) = setup();
    // Unspent: c2 (200), c3 (500), 2 coinbase
    let count = store.num_unspent().unwrap();
    assert_eq!(count, 4, "c2 + c3 + 2 coinbase = 4 unspent");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_009_total_unspent_value() {
    let (store, _dir) = setup();
    let total = store.total_unspent_value().unwrap();
    // c2=200, c3=500, coinbase1=1_750_000_000_000, coinbase2=250_000_000_000
    let expected = 200u128 + 500 + 1_750_000_000_000 + 250_000_000_000;
    assert_eq!(total, expected);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_009_num_total() {
    let (store, _dir) = setup();
    // c1 (spent), c2, c3, 2 coinbase = 5 total
    let count = store.num_total().unwrap();
    assert_eq!(count, 5, "2 genesis + 1 addition + 2 coinbase = 5 total");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_009_aggregate_by_puzzle_hash() {
    let (store, _dir) = setup();
    let agg = store.aggregate_unspent_by_puzzle_hash().unwrap();
    // puzzle_hash(2) has c2 unspent (200), c1 was spent
    let ph2 = helpers::test_hash(2);
    assert!(agg.contains_key(&ph2), "Puzzle hash 2 should have unspent coins");
    let (amount, count) = agg[&ph2];
    assert_eq!(amount, 200, "Only c2 (200) is unspent for ph2");
    assert_eq!(count, 1);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_009_empty_store() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();
    assert_eq!(store.num_unspent().unwrap(), 0);
    assert_eq!(store.total_unspent_value().unwrap(), 0);
    assert_eq!(store.num_total().unwrap(), 0);
    assert!(store.aggregate_unspent_by_puzzle_hash().unwrap().is_empty());
}
