//! # PRF-008 Tests — Snapshot Persistence (save, load, prune)
//!
//! Verifies **PRF-008**: `save_snapshot()`, `load_snapshot()`, and `load_latest_snapshot()`
//! persist snapshots to `CF_STATE_SNAPSHOTS` and can retrieve them by height. Older
//! snapshots are pruned when `max_snapshots` is exceeded.
//!
//! # Requirement: PRF-008
//! # SPEC.md: §3.14 (Snapshot persistence), API-003 (max_snapshots config)
//!
//! ## How these tests prove the requirement
//!
//! - **Save and load:** `save_snapshot` persists the current state; `load_snapshot(height)` retrieves it.
//! - **Load latest:** `load_latest_snapshot()` returns the most recent saved snapshot.
//! - **Round-trip fidelity:** Loaded snapshot has the same height, coins, and state root.
//! - **Missing height:** `load_snapshot` for a non-saved height returns `None`.
//! - **Pruning:** When multiple snapshots exceed `max_snapshots`, oldest are pruned.

mod helpers;

use dig_coinstore::{
    coin_store::CoinStore, BlockData, Bytes32, CoinStoreConfig,
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

/// **PRF-008:** save_snapshot then load_snapshot round-trip.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_008_save_and_load_by_height() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    store.save_snapshot().unwrap();

    let loaded = store.load_snapshot(0).unwrap();
    assert!(loaded.is_some(), "Saved snapshot should be loadable");
    let snap = loaded.unwrap();
    assert_eq!(snap.height, 0);
    assert_eq!(snap.coins.len(), 1);
    assert_eq!(snap.total_coins, 1);
    assert_eq!(snap.total_value, 500);
}

/// **PRF-008:** load_latest_snapshot returns the most recent.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_008_load_latest_snapshot() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Save at height 0
    store.save_snapshot().unwrap();

    // Apply block 1
    let b1 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store.apply_block(b1).unwrap();
    store.save_snapshot().unwrap();

    let latest = store.load_latest_snapshot().unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().height, 1, "Latest snapshot should be at height 1");
}

/// **PRF-008:** load_snapshot for non-saved height returns None.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_008_missing_height_returns_none() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let loaded = store.load_snapshot(999).unwrap();
    assert!(loaded.is_none(), "Non-saved height should return None");
}

/// **PRF-008:** State root fidelity across save/load.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_008_state_root_fidelity() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let snap_before = store.snapshot().unwrap();
    store.save_snapshot().unwrap();
    let snap_loaded = store.load_snapshot(0).unwrap().unwrap();

    assert_eq!(
        snap_before.state_root, snap_loaded.state_root,
        "Loaded snapshot state_root must match original"
    );
}

/// **PRF-008:** Pruning removes oldest snapshots when max_snapshots is exceeded.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_008_pruning_oldest_snapshots() {
    let dir = helpers::temp_dir();
    let mut config = CoinStoreConfig::default_with_path(dir.path());
    config.max_snapshots = 2; // Keep at most 2

    let mut store = CoinStore::with_config(config).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Save snapshot at height 0
    store.save_snapshot().unwrap();

    // Apply and save at height 1
    let b1 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store.apply_block(b1).unwrap();
    store.save_snapshot().unwrap();

    // Apply and save at height 2
    let b2 = make_block(2, helpers::test_hash(0xB1), helpers::test_hash(0xB2));
    store.apply_block(b2).unwrap();
    store.save_snapshot().unwrap();

    // Height 0 should be pruned (only 2 kept: heights 1 and 2)
    let snap0 = store.load_snapshot(0).unwrap();
    assert!(snap0.is_none(), "Height 0 snapshot should be pruned");

    let snap1 = store.load_snapshot(1).unwrap();
    assert!(snap1.is_some(), "Height 1 snapshot should still exist");

    let snap2 = store.load_snapshot(2).unwrap();
    assert!(snap2.is_some(), "Height 2 snapshot should still exist");
}
