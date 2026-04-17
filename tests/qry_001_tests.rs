//! # QRY-001 Tests — Point Lookups by Coin ID
//!
//! Verifies **QRY-001**: `get_coin_record()` and `get_coin_records()` retrieve coins by ID.
//!
//! # Requirement: QRY-001
//! # SPEC.md: §3.4, Chia: coin_store.py:181-221
//!
//! ## How these tests prove the requirement
//!
//! - `get_coin_record` returns `Some` for existing coins and `None` for missing.
//! - `get_coin_records` returns records for found IDs and skips missing ones.
//! - Both return spent and unspent coins.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition};

/// Helper: create store, genesis with one coin, apply block 1 with additions and removals.
#[cfg(feature = "rocksdb-storage")]
fn setup_store_with_coins() -> (
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

    // Block 1: add a new coin and spend the genesis coin
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

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_001_get_existing_coin() {
    let (store, _dir, _genesis, new_coin) = setup_store_with_coins();
    let rec = store.get_coin_record(&new_coin.coin_id()).unwrap();
    assert!(rec.is_some(), "Existing coin must be found");
    assert_eq!(rec.unwrap().coin, new_coin);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_001_get_missing_coin() {
    let (store, _dir, _, _) = setup_store_with_coins();
    let fake_id = helpers::test_coin(99, 99, 99).coin_id();
    let rec = store.get_coin_record(&fake_id).unwrap();
    assert!(rec.is_none(), "Missing coin must return None");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_001_get_spent_coin() {
    let (store, _dir, genesis_coin, _) = setup_store_with_coins();
    // Genesis coin was spent in block 1 — should still be returned.
    let rec = store.get_coin_record(&genesis_coin.coin_id()).unwrap();
    assert!(rec.is_some(), "Spent coin must still be found");
    assert!(rec.unwrap().is_spent());
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_001_batch_mixed() {
    let (store, _dir, genesis_coin, new_coin) = setup_store_with_coins();
    let fake_id = helpers::test_coin(99, 99, 99).coin_id();
    let results = store
        .get_coin_records(&[genesis_coin.coin_id(), new_coin.coin_id(), fake_id])
        .unwrap();
    // 2 found, 1 missing (skipped)
    assert_eq!(results.len(), 2, "Should find 2 of 3 IDs");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_001_empty_batch() {
    let (store, _dir, _, _) = setup_store_with_coins();
    let results = store.get_coin_records(&[]).unwrap();
    assert!(results.is_empty());
}
