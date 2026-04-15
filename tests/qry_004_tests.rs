//! # QRY-004 Tests — Parent ID Queries
//!
//! Verifies **QRY-004**: `get_coin_records_by_parent_ids()`.
//!
//! # Requirement: QRY-004
//! # SPEC.md: §3.7, Chia: coin_store.py:380-406

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (CoinStore, tempfile::TempDir) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    // Genesis with coin whose parent is test_hash(1)
    let c1 = helpers::test_coin(1, 2, 100);
    store.init_genesis(vec![(c1, false)], 1_700_000_000).unwrap();
    (store, dir)
}

/// Find coins by parent_coin_info.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_004_by_parent_id() {
    let (store, _dir) = setup();
    let parent = helpers::test_hash(1); // parent of genesis coin
    let results = store.get_coin_records_by_parent_ids(true, &[parent], 0, u64::MAX).unwrap();
    assert_eq!(results.len(), 1, "Genesis coin has parent test_hash(1)");
}

/// No match for unknown parent.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_004_no_match() {
    let (store, _dir) = setup();
    let unknown = helpers::test_hash(99);
    let results = store.get_coin_records_by_parent_ids(true, &[unknown], 0, u64::MAX).unwrap();
    assert!(results.is_empty());
}

/// include_spent=false filters out spent coins.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_004_include_spent_false() {
    let (mut store, _dir) = setup();
    let genesis_coin = helpers::test_coin(1, 2, 100);
    // Spend genesis coin in block 1
    let block = dig_coinstore::BlockData {
        height: 1, timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1), parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![], removals: vec![genesis_coin.coin_id()],
        coinbase_coins: vec![helpers::test_coin(200, 201, 1_750_000_000_000), helpers::test_coin(202, 203, 250_000_000_000)],
        hints: vec![], expected_state_root: None,
    };
    store.apply_block(block).unwrap();

    let parent = helpers::test_hash(1);
    let results = store.get_coin_records_by_parent_ids(false, &[parent], 0, u64::MAX).unwrap();
    assert!(results.is_empty(), "Spent coin filtered out");
}

/// Multiple parents batch.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_004_multiple_parents() {
    let (store, _dir) = setup();
    let parent1 = helpers::test_hash(1);
    let parent2 = helpers::test_hash(99); // no match
    let results = store.get_coin_records_by_parent_ids(true, &[parent1, parent2], 0, u64::MAX).unwrap();
    assert_eq!(results.len(), 1);
}
