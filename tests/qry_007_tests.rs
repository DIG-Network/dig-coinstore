//! # QRY-007 Tests — Batch Coin State Pagination
//!
//! Verifies **QRY-007**: `batch_coin_states_by_puzzle_hashes()` with `CoinStateFilters`.
//!
//! # Requirement: QRY-007
//! # SPEC.md: §3.5, §1.5 #5-9

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinStateFilters};

fn make_filters(
    include_spent: bool,
    include_unspent: bool,
    include_hinted: bool,
    min_amount: u64,
) -> CoinStateFilters {
    CoinStateFilters::new(include_spent, include_unspent, include_hinted, min_amount)
}

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (CoinStore, tempfile::TempDir, Bytes32) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 2, 200);
    let puzzle_hash = helpers::test_hash(2);
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

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

/// Basic query returns results.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_007_basic() {
    let (store, _dir, ph) = setup();
    let filters = make_filters(true, true, false, 0);
    let (states, next) = store
        .batch_coin_states_by_puzzle_hashes(&[ph], 0, filters, 100)
        .unwrap();
    assert_eq!(states.len(), 2, "Both coins for puzzle hash");
    assert!(next.is_none(), "All fit within max_items");
}

/// include_spent=false filters spent coins.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_007_exclude_spent() {
    let (store, _dir, ph) = setup();
    let filters = make_filters(false, true, false, 0);
    let (states, _) = store
        .batch_coin_states_by_puzzle_hashes(&[ph], 0, filters, 100)
        .unwrap();
    assert_eq!(states.len(), 1, "Only unspent c2");
}

/// min_amount filters out small coins.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_007_min_amount() {
    let (store, _dir, ph) = setup();
    let filters = make_filters(true, true, false, 150);
    let (states, _) = store
        .batch_coin_states_by_puzzle_hashes(&[ph], 0, filters, 100)
        .unwrap();
    // c1=100 (< 150, filtered), c2=200 (>= 150, kept)
    assert_eq!(states.len(), 1, "Only c2 >= 150");
}

/// Batch size limit enforced.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_007_batch_too_large() {
    let (store, _dir, _ph) = setup();
    let many_phs: Vec<Bytes32> = (0..991u16)
        .map(|i| {
            let mut b = [0u8; 32];
            b[0..2].copy_from_slice(&i.to_le_bytes());
            Bytes32::from(b)
        })
        .collect();
    let filters = make_filters(true, true, false, 0);
    let result = store.batch_coin_states_by_puzzle_hashes(&many_phs, 0, filters, 100);
    assert!(matches!(
        result,
        Err(dig_coinstore::CoinStoreError::PuzzleHashBatchTooLarge { .. })
    ));
}

/// Pagination: max_items=1 triggers next_height.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_007_pagination() {
    let (store, _dir, ph) = setup();
    let filters = make_filters(true, true, false, 0);
    let (states, next) = store
        .batch_coin_states_by_puzzle_hashes(&[ph], 0, filters, 1)
        .unwrap();
    // With block boundary preservation, results may be truncated differently.
    // But next should be Some(...) since there are more items.
    assert!(states.len() <= 1);
    // next_height should be set since we have 2 items but max_items=1
    assert!(next.is_some(), "Should have more pages");
}
