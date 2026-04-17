//! # QRY-002 Tests — Puzzle Hash Queries
//!
//! Verifies **QRY-002**: `get_coin_records_by_puzzle_hash()` and batch variant.
//!
//! # Requirement: QRY-002
//! # SPEC.md: §3.5, Chia: coin_store.py:257-307

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32};

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (CoinStore, tempfile::TempDir, Bytes32) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    // Genesis with 2 coins sharing puzzle hash seed=2
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 2, 200);
    let puzzle_hash = helpers::test_hash(2);
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    // Block 1: spend c1
    let block = dig_coinstore::BlockData {
        height: 1,
        timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1),
        parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![],
        removals: vec![c1.coin_id()],
        coinbase_coins: vec![
            helpers::test_coin(200, 201, 1_750_000_000_000),
            helpers::test_coin(202, 203, 250_000_000_000),
        ],
        hints: vec![],
        expected_state_root: None,
    };
    store.apply_block(block).unwrap();
    (store, dir, puzzle_hash)
}

/// Both spent and unspent coins returned when include_spent=true.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_002_include_spent_true() {
    let (store, _dir, ph) = setup();
    let results = store
        .get_coin_records_by_puzzle_hash(true, &ph, 0, u64::MAX)
        .unwrap();
    assert_eq!(
        results.len(),
        2,
        "Both spent + unspent with include_spent=true"
    );
}

/// Only unspent coins returned when include_spent=false.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_002_include_spent_false() {
    let (store, _dir, ph) = setup();
    let results = store
        .get_coin_records_by_puzzle_hash(false, &ph, 0, u64::MAX)
        .unwrap();
    assert_eq!(results.len(), 1, "Only unspent with include_spent=false");
    assert!(!results[0].is_spent());
}

/// Height range filters correctly.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_002_height_range() {
    let (store, _dir, ph) = setup();
    // Both genesis coins confirmed at height 0 — querying height 1..MAX returns none.
    let results = store
        .get_coin_records_by_puzzle_hash(true, &ph, 1, u64::MAX)
        .unwrap();
    assert_eq!(results.len(), 0, "No coins confirmed at height >= 1");
}

/// Non-matching puzzle hash returns empty.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_002_no_match() {
    let (store, _dir, _ph) = setup();
    let other = helpers::test_hash(99);
    let results = store
        .get_coin_records_by_puzzle_hash(true, &other, 0, u64::MAX)
        .unwrap();
    assert!(results.is_empty());
}

/// Batch version with multiple puzzle hashes.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_002_batch() {
    let (store, _dir, ph) = setup();
    let other = helpers::test_hash(201); // coinbase puzzle hash
    let results = store
        .get_coin_records_by_puzzle_hashes(true, &[ph, other], 0, u64::MAX)
        .unwrap();
    assert!(
        results.len() >= 2,
        "Should find coins from both puzzle hashes"
    );
}
