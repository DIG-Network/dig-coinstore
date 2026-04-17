//! # PRF-003 Tests — Materialized Aggregate Counters
//!
//! Verifies **PRF-003**: `stats()`, `num_unspent()`, and `total_unspent_value()` return
//! correct aggregate values that track coin creation and spending. Once materialized
//! counters are implemented (PRF-003), these will be O(1); currently they scan
//! `CF_COIN_RECORDS` but the behavioral contract is identical.
//!
//! # Requirement: PRF-003
//! # SPEC.md: §1.6 #18 (Materialized Counters), §3.11-§3.12
//!
//! ## How these tests prove the requirement
//!
//! - **Empty store:** Counters are all zero before genesis.
//! - **After genesis:** Counters match the genesis coin set.
//! - **After apply_block with additions:** Unspent count and value increase.
//! - **After apply_block with removals:** Unspent count decreases, spent count increases.
//! - **After rollback:** Counters revert to pre-block values.
//! - **Consistency:** `stats()` aggregates match `num_unspent()` and `total_unspent_value()`.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition};

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

/// **PRF-003:** Uninitialized store has all-zero counters.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_003_empty_store_zero_counters() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    let stats = store.stats();
    assert_eq!(stats.unspent_count, 0);
    assert_eq!(stats.spent_count, 0);
    assert_eq!(stats.total_unspent_value, 0);
}

/// **PRF-003:** After genesis, counters match inserted coins.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_003_genesis_counters() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    let stats = store.stats();
    assert_eq!(stats.unspent_count, 2);
    assert_eq!(stats.spent_count, 0);
    assert_eq!(stats.total_unspent_value, 300);

    let num = store.num_unspent().unwrap();
    assert_eq!(num, 2);
    let total = store.total_unspent_value().unwrap();
    assert_eq!(total, 300);
}

/// **PRF-003:** After apply_block with additions, unspent count increases.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_003_counters_after_additions() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let stats_before = store.stats();
    assert_eq!(stats_before.unspent_count, 0);

    let new_coin = helpers::test_coin(10, 11, 999);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    store.apply_block(block).unwrap();

    let stats_after = store.stats();
    // 2 coinbase + 1 addition = 3 new unspent coins
    assert_eq!(stats_after.unspent_count, 3);
    assert_eq!(stats_after.spent_count, 0);
}

/// **PRF-003:** After spending a coin, unspent count decreases and spent count increases.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_003_counters_after_spend() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    assert_eq!(store.stats().unspent_count, 1);
    assert_eq!(store.stats().spent_count, 0);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![id];
    store.apply_block(block).unwrap();

    let stats = store.stats();
    // Genesis coin now spent, 2 coinbase coins unspent
    assert_eq!(stats.spent_count, 1, "One coin should be spent");
    assert_eq!(
        stats.unspent_count, 2,
        "Two coinbase coins should be unspent"
    );
}

/// **PRF-003:** After rollback, counters revert.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_003_counters_after_rollback() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let stats_at_genesis = store.stats();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store.apply_block(block).unwrap();

    // After block 1: more coins (coinbase added)
    assert!(store.stats().unspent_count > stats_at_genesis.unspent_count);

    // Rollback to genesis
    store.rollback_to_block(0).unwrap();
    let stats_after_rb = store.stats();
    assert_eq!(
        stats_after_rb.unspent_count, stats_at_genesis.unspent_count,
        "Counters should revert after rollback"
    );
}

/// **PRF-003:** stats() consistency with num_unspent() and total_unspent_value().
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_003_consistency_stats_vs_queries() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let c3 = helpers::test_coin(5, 6, 300);
    store
        .init_genesis(vec![(c1, false), (c2, false), (c3, false)], 1_700_000_000)
        .unwrap();

    let stats = store.stats();
    let num = store.num_unspent().unwrap();
    let total = store.total_unspent_value().unwrap();

    assert_eq!(stats.unspent_count, num);
    assert_eq!(stats.total_unspent_value as u128, total);
}
