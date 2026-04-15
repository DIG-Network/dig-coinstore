//! # PRF-006 Tests — Snapshot-Based Fast Sync
//!
//! Verifies **PRF-006**: `snapshot()` and `restore()` provide a complete round-trip
//! for fast sync. A snapshot captures the full coinstate at a point in time; restoring
//! it on a fresh store reproduces the exact same state (height, tip_hash, state_root,
//! coins, stats).
//!
//! # Requirement: PRF-006
//! # SPEC.md: §3.14 (Snapshot/Restore), improvement #6 (fast sync)
//!
//! ## How these tests prove the requirement
//!
//! - **Round-trip:** `snapshot()` then `restore()` on a fresh store yields identical state.
//! - **Merkle root verification:** `restore()` recomputes the Merkle root and verifies it
//!   matches `snapshot.state_root` — tampered snapshots are rejected.
//! - **Stats parity:** After restore, `stats()` matches the original store's `stats()`.
//! - **Coin access:** After restore, individual `get_coin_record()` calls return the same data.

mod helpers;

use dig_coinstore::{
    coin_store::CoinStore, BlockData, Bytes32,
};

fn make_block(height: u64, parent_hash: Bytes32, block_hash: Bytes32) -> BlockData {
    let coinbase_coins = if height == 0 {
        vec![]
    } else {
        vec![
            helpers::test_coin(200 + height as u8, 201, 1_750_000_000_000),
            helpers::test_coin(202 + height as u8, 203, 250_000_000_000),
        ]
    };
    BlockData {
        height,
        timestamp: 1_700_000_000 + height * 18,
        block_hash,
        parent_hash,
        additions: vec![],
        removals: vec![],
        coinbase_coins,
        hints: vec![],
        expected_state_root: None,
    }
}

/// **PRF-006:** Full snapshot/restore round-trip preserves state.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_006_snapshot_restore_round_trip() {
    // Build a chain with some state
    let dir1 = helpers::temp_dir();
    let mut store1 = CoinStore::new(dir1.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    store1
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store1.apply_block(block).unwrap();

    // Take snapshot
    let snap = store1.snapshot().unwrap();
    let stats1 = store1.stats();

    // Restore on a fresh store
    let dir2 = helpers::temp_dir();
    let mut store2 = CoinStore::new(dir2.path()).unwrap();
    store2.restore(snap).unwrap();

    let stats2 = store2.stats();
    assert_eq!(stats1.height, stats2.height);
    assert_eq!(stats1.unspent_count, stats2.unspent_count);
    assert_eq!(stats1.spent_count, stats2.spent_count);
    assert_eq!(stats1.total_unspent_value, stats2.total_unspent_value);
    assert_eq!(stats1.state_root, stats2.state_root);
    assert_eq!(stats1.tip_hash, stats2.tip_hash);
}

/// **PRF-006:** Tampered state root in snapshot is rejected by restore.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_006_tampered_state_root_rejected() {
    let dir1 = helpers::temp_dir();
    let mut store1 = CoinStore::new(dir1.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store1
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let mut snap = store1.snapshot().unwrap();
    // Tamper with state root
    snap.state_root = helpers::filled_hash(0xFF);

    let dir2 = helpers::temp_dir();
    let mut store2 = CoinStore::new(dir2.path()).unwrap();
    let err = store2.restore(snap).unwrap_err();
    assert!(
        matches!(err, dig_coinstore::CoinStoreError::StateRootMismatch { .. }),
        "Tampered state root must be rejected: {:?}",
        err
    );
}

/// **PRF-006:** After restore, individual coin records are accessible.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_006_restored_coins_queryable() {
    let dir1 = helpers::temp_dir();
    let mut store1 = CoinStore::new(dir1.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let id1 = c1.coin_id();
    let id2 = c2.coin_id();
    store1
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    let snap = store1.snapshot().unwrap();

    let dir2 = helpers::temp_dir();
    let mut store2 = CoinStore::new(dir2.path()).unwrap();
    store2.restore(snap).unwrap();

    let rec1 = store2.get_coin_record(&id1).unwrap();
    assert!(rec1.is_some(), "Coin 1 must be queryable after restore");
    assert_eq!(rec1.unwrap().coin.amount, 100);

    let rec2 = store2.get_coin_record(&id2).unwrap();
    assert!(rec2.is_some(), "Coin 2 must be queryable after restore");
    assert_eq!(rec2.unwrap().coin.amount, 200);
}

/// **PRF-006:** Restore rebuilds the unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_006_restore_rebuilds_unspent_set() {
    let dir1 = helpers::temp_dir();
    let mut store1 = CoinStore::new(dir1.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store1
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let snap = store1.snapshot().unwrap();

    let dir2 = helpers::temp_dir();
    let mut store2 = CoinStore::new(dir2.path()).unwrap();
    store2.restore(snap).unwrap();

    assert!(store2.is_unspent(&id), "Unspent set must be rebuilt after restore");
}
