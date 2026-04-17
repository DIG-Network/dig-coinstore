//! # QRY-005 Tests — Name Queries (by IDs with filters)
//!
//! Verifies **QRY-005**: `get_coin_records_by_names()` with include_spent and height range.
//!
//! # Requirement: QRY-005
//! # SPEC.md: §3.4, Chia: coin_store.py:309-335

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (
    CoinStore,
    tempfile::TempDir,
    chia_protocol::Coin,
    chia_protocol::Coin,
) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    let new_coin = helpers::test_coin(10, 11, 500);
    let block = dig_coinstore::BlockData {
        height: 1,
        timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1),
        parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![CoinAddition::from_coin(new_coin, false)],
        removals: vec![genesis_coin.coin_id()],
        coinbase_coins: vec![
            helpers::test_coin(200, 201, 1_750_000_000_000),
            helpers::test_coin(202, 203, 250_000_000_000),
        ],
        hints: vec![],
        expected_state_root: None,
    };
    store.apply_block(block).unwrap();
    (store, dir, genesis_coin, new_coin)
}

/// include_spent=true returns both spent and unspent.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_005_include_spent_true() {
    let (store, _dir, genesis_coin, new_coin) = setup();
    let results = store
        .get_coin_records_by_names(
            true,
            &[genesis_coin.coin_id(), new_coin.coin_id()],
            0,
            u64::MAX,
        )
        .unwrap();
    assert_eq!(results.len(), 2);
}

/// include_spent=false filters out the spent genesis coin.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_005_include_spent_false() {
    let (store, _dir, genesis_coin, new_coin) = setup();
    let results = store
        .get_coin_records_by_names(
            false,
            &[genesis_coin.coin_id(), new_coin.coin_id()],
            0,
            u64::MAX,
        )
        .unwrap();
    assert_eq!(results.len(), 1, "Only unspent new_coin returned");
    assert_eq!(results[0].coin, new_coin);
}

/// Height range filter.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_005_height_range() {
    let (store, _dir, genesis_coin, new_coin) = setup();
    // Only height 1 coins (new_coin confirmed at 1, genesis at 0)
    let results = store
        .get_coin_records_by_names(true, &[genesis_coin.coin_id(), new_coin.coin_id()], 1, 1)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].coin, new_coin);
}

/// Empty names returns empty.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_005_empty_names() {
    let (store, _dir, _, _) = setup();
    let results = store
        .get_coin_records_by_names(true, &[], 0, u64::MAX)
        .unwrap();
    assert!(results.is_empty());
}
