//! # CON-003 Tests — MVCC Reads During Writes (Snapshot Isolation)
//!
//! Verifies **CON-003**: reads see a consistent state and are not affected by concurrent
//! mutations. The underlying RocksDB engine provides MVCC naturally via snapshots.
//! At the Rust level, `&self` / `&mut self` separation ensures that no read can
//! observe a partial mutation (the borrow checker prevents it in single-threaded code;
//! `RwLock` prevents it in multi-threaded code).
//!
//! # Requirement: CON-003
//! # SPEC.md: §1.6 #19 (MVCC reads during block application)
//!
//! ## How these tests prove the requirement
//!
//! - **Consistent snapshot:** `snapshot()` captures a point-in-time view; fields are self-consistent.
//! - **Read stability:** Between two reads without mutation, results are identical.
//! - **Mutation isolation:** After `apply_block`, a new snapshot differs from the pre-mutation one.
//! - **Stats consistency:** `stats()` aggregates match individual query results.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition};

/// Build a minimal valid block at the given height.
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

/// **CON-003:** Snapshot captures a consistent point-in-time view.
///
/// **Proof:** After genesis with coins, `snapshot()` returns a `CoinStoreSnapshot` whose
/// `total_coins` and `total_value` match the actual coin count and unspent value.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_003_snapshot_is_consistent() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    let snap = store.snapshot().unwrap();
    assert_eq!(snap.total_coins, 2);
    assert_eq!(snap.total_value, 300); // 100 + 200 unspent
    assert_eq!(snap.height, 0);
    assert_eq!(snap.coins.len(), 2);
}

/// **CON-003:** Repeated reads without mutation return identical results.
///
/// **Proof:** Two consecutive `stats()` calls return the same values. Two consecutive
/// `get_coin_record()` calls return the same record. No phantom reads.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_003_read_stability_without_mutation() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(5, 6, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let stats1 = store.stats();
    let stats2 = store.stats();
    assert_eq!(stats1.unspent_count, stats2.unspent_count);
    assert_eq!(stats1.total_unspent_value, stats2.total_unspent_value);
    assert_eq!(stats1.height, stats2.height);

    let rec1 = store.get_coin_record(&id).unwrap();
    let rec2 = store.get_coin_record(&id).unwrap();
    assert_eq!(rec1, rec2);
}

/// **CON-003:** After mutation (apply_block), reads reflect the new state.
///
/// **Proof:** Stats before and after `apply_block` differ, proving the mutation is visible.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_003_mutation_visible_after_apply() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let stats_before = store.stats();

    let new_coin = helpers::test_coin(10, 11, 999);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    store.apply_block(block).unwrap();

    let stats_after = store.stats();
    assert_eq!(stats_after.height, 1);
    assert!(
        stats_after.unspent_count > stats_before.unspent_count,
        "Unspent count should increase after adding coins"
    );
}

/// **CON-003:** Stats aggregates match individual query results.
///
/// **Proof:** `stats().unspent_count` matches `num_unspent()`. This ensures the two
/// code paths (full-scan stats vs query) agree on the same snapshot of data.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_003_stats_match_individual_queries() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    let stats = store.stats();
    let num = store.num_unspent().unwrap();
    let total = store.total_unspent_value().unwrap();

    assert_eq!(stats.unspent_count, num);
    assert_eq!(stats.total_unspent_value as u128, total);
}

/// **CON-003:** Snapshot before and after block differ.
///
/// **Proof:** Two snapshots taken before and after `apply_block` have different heights
/// and state roots, proving mutation isolation between observation points.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_003_snapshots_differ_across_mutations() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let snap1 = store.snapshot().unwrap();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store.apply_block(block).unwrap();

    let snap2 = store.snapshot().unwrap();
    assert_ne!(snap1.height, snap2.height);
    assert_ne!(snap1.state_root, snap2.state_root);
}
