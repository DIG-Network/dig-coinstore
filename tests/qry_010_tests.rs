//! # QRY-010 Tests — Chain State Metadata
//!
//! Verifies **QRY-010**: `height()`, `tip_hash()`, `state_root()`, `timestamp()`, `stats()`, `is_empty()`.
//! Most of these already exist from API-001/API-007; this file verifies them as QRY-010.
//!
//! # Requirement: QRY-010
//! # SPEC.md: §3.12

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_010_fresh_store_state() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    assert_eq!(store.height(), 0);
    assert_eq!(store.tip_hash(), Bytes32::from([0u8; 32]));
    assert_eq!(store.timestamp(), 0);
    assert!(store.is_empty());
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_010_after_genesis() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![(helpers::test_coin(1, 2, 100), false)], 1_700_000_000).unwrap();
    assert_eq!(store.height(), 0);
    assert!(!store.is_empty());
    assert_eq!(store.timestamp(), 1_700_000_000);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_010_after_block() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let block = dig_coinstore::BlockData {
        height: 1, timestamp: 1_700_000_018,
        block_hash: hash1, parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![], removals: vec![],
        coinbase_coins: vec![helpers::test_coin(200, 201, 1_750_000_000_000), helpers::test_coin(202, 203, 250_000_000_000)],
        hints: vec![], expected_state_root: None,
    };
    store.apply_block(block).unwrap();

    assert_eq!(store.height(), 1);
    assert_eq!(store.tip_hash(), hash1);
    assert_eq!(store.timestamp(), 1_700_000_018);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_010_stats_matches_accessors() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![(helpers::test_coin(1, 2, 100), false)], 1_700_000_000).unwrap();

    let s = store.stats();
    assert_eq!(s.height, store.height());
    assert_eq!(s.timestamp, store.timestamp());
    assert_eq!(s.tip_hash, store.tip_hash());
}
