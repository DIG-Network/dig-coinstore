//! # PRF-009 Tests — Performance Benchmark Targets
//!
//! Verifies **PRF-009**: the behavioral contracts underlying the 11 criterion benchmarks
//! from SPEC.md §13.12. These are smoke tests proving the operations execute correctly;
//! actual performance benchmarks will be implemented as criterion benches in `benches/`.
//!
//! # Requirement: PRF-009
//! # SPEC.md: §13.12 (Performance benchmark targets)
//!
//! ## How these tests prove the requirement
//!
//! Each test exercises one of the benchmark target operations end-to-end, verifying
//! correctness rather than throughput:
//!
//! - **apply_block:** Block with additions and removals succeeds.
//! - **get_coin_record:** Point lookup returns correct data.
//! - **is_unspent:** In-memory set check works.
//! - **get_coin_records_by_puzzle_hash:** Index query returns correct results.
//! - **snapshot/restore:** Round-trip works.
//! - **rollback:** State reverts correctly.
//! - **num_unspent:** Aggregate scan returns correct count.
//! - **stats:** Full aggregate computation works.
//! - **state_root:** Merkle root computation returns non-zero hash.

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

/// **PRF-009:** Benchmark target: apply_block with additions and removals.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_apply_block() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let new_coin = helpers::test_coin(10, 11, 999);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    block.removals = vec![id];
    let result = store.apply_block(block).unwrap();
    assert_eq!(result.coins_created, 3); // 2 coinbase + 1 addition
    assert_eq!(result.coins_spent, 1);
}

/// **PRF-009:** Benchmark target: get_coin_record point lookup.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_get_coin_record() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let rec = store.get_coin_record(&id).unwrap();
    assert!(rec.is_some());
    assert_eq!(rec.unwrap().coin.amount, 500);
}

/// **PRF-009:** Benchmark target: is_unspent check.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_is_unspent() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    assert!(store.is_unspent(&id));
    let missing = helpers::test_hash(0xFF);
    assert!(!store.is_unspent(&missing));
}

/// **PRF-009:** Benchmark target: puzzle hash query.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_puzzle_hash_query() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let puzzle_hash = helpers::test_hash(2);
    let coin = chia_protocol::Coin::new(helpers::test_hash(1), puzzle_hash, 100);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let results = store
        .get_coin_records_by_puzzle_hash(false, &puzzle_hash, 0, u64::MAX)
        .unwrap();
    assert_eq!(results.len(), 1);
}

/// **PRF-009:** Benchmark target: snapshot/restore round-trip.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_snapshot_restore() {
    let dir1 = helpers::temp_dir();
    let mut store1 = CoinStore::new(dir1.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store1
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let snap = store1.snapshot().unwrap();

    let dir2 = helpers::temp_dir();
    let mut store2 = CoinStore::new(dir2.path()).unwrap();
    store2.restore(snap).unwrap();
    assert_eq!(store2.height(), 0);
}

/// **PRF-009:** Benchmark target: rollback.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_rollback() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let b1 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store.apply_block(b1).unwrap();
    assert_eq!(store.height(), 1);

    store.rollback_to_block(0).unwrap();
    assert_eq!(store.height(), 0);
}

/// **PRF-009:** Benchmark target: num_unspent aggregate.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_num_unspent() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    assert_eq!(store.num_unspent().unwrap(), 2);
}

/// **PRF-009:** Benchmark target: stats() aggregate.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_stats() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let stats = store.stats();
    assert_eq!(stats.unspent_count, 1);
    assert_eq!(stats.total_unspent_value, 500);
}

/// **PRF-009:** Benchmark target: state_root computation.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_009_bench_state_root() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let root = store.state_root();
    assert_ne!(
        root,
        Bytes32::from([0u8; 32]),
        "State root with coins should be non-zero"
    );
}
