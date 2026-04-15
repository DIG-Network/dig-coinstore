//! # QRY-006 Tests — Lightweight CoinState Queries
//!
//! Verifies **QRY-006**: `get_coin_states_by_ids()` and `get_coin_states_by_puzzle_hashes()`.
//!
//! # Requirement: QRY-006
//! # SPEC.md: §3.8, §3.5

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32, CoinAddition, CoinState};

#[cfg(feature = "rocksdb-storage")]
fn setup() -> (CoinStore, tempfile::TempDir, chia_protocol::Coin, chia_protocol::Coin) {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    store.init_genesis(vec![(genesis_coin, false)], 1_700_000_000).unwrap();

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
    (store, dir, genesis_coin, new_coin)
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_006_coin_states_by_ids() {
    let (store, _dir, genesis_coin, new_coin) = setup();
    let states = store.get_coin_states_by_ids(true, &[genesis_coin.coin_id(), new_coin.coin_id()], 0, u64::MAX, 100).unwrap();
    assert_eq!(states.len(), 2);
    // Verify CoinState structure
    for cs in &states {
        assert!(cs.created_height.is_some());
    }
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_006_coin_states_by_ids_exclude_spent() {
    let (store, _dir, genesis_coin, new_coin) = setup();
    let states = store.get_coin_states_by_ids(false, &[genesis_coin.coin_id(), new_coin.coin_id()], 0, u64::MAX, 100).unwrap();
    assert_eq!(states.len(), 1, "Only unspent new_coin");
    assert_eq!(states[0].coin, new_coin);
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_006_coin_states_by_ids_max_items() {
    let (store, _dir, genesis_coin, new_coin) = setup();
    let states = store.get_coin_states_by_ids(true, &[genesis_coin.coin_id(), new_coin.coin_id()], 0, u64::MAX, 1).unwrap();
    assert_eq!(states.len(), 1, "max_items=1 caps result");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_006_coin_states_by_puzzle_hashes() {
    let (store, _dir, _genesis, new_coin) = setup();
    let ph = helpers::test_hash(11); // new_coin puzzle hash
    let states = store.get_coin_states_by_puzzle_hashes(true, &[ph], 0, 100).unwrap();
    assert!(!states.is_empty(), "Should find new_coin by puzzle hash");
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_006_spent_coin_state_has_spent_height() {
    let (store, _dir, genesis_coin, _) = setup();
    let states = store.get_coin_states_by_ids(true, &[genesis_coin.coin_id()], 0, u64::MAX, 100).unwrap();
    assert_eq!(states.len(), 1);
    assert!(states[0].spent_height.is_some(), "Spent coin must have spent_height");
    assert_eq!(states[0].spent_height, Some(1));
}
