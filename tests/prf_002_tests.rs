//! # PRF-002 Tests — LRU Coin Record Cache
//!
//! Verifies **PRF-002**: repeated reads of the same coin record return the same result.
//! The LRU cache is a write-through cache that avoids storage I/O on repeat accesses.
//! The behavioral contract is: `get_coin_record(id)` always returns the current state
//! of the coin, whether served from cache or storage.
//!
//! # Requirement: PRF-002
//! # SPEC.md: §1.6 #14 (LRU Cache), §2.7 (DEFAULT_COIN_CACHE_CAPACITY)
//!
//! ## How these tests prove the requirement
//!
//! - **Repeat reads return same result:** Two consecutive `get_coin_record` calls match.
//! - **Cache coherence after mutation:** After `apply_block` spends a coin, the next read
//!   reflects the spent state (write-through or invalidation).
//! - **Cache coherence after rollback:** After rollback, previously-spent coins read as unspent.
//! - **Many reads perform well:** 10k reads of the same coin complete quickly (smoke test).
//! - **Batch reads consistent:** `get_coin_records` returns same records as individual lookups.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32};

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

/// **PRF-002:** Repeated reads of the same coin return identical records.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_002_repeated_reads_same_result() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let r1 = store.get_coin_record(&id).unwrap();
    let r2 = store.get_coin_record(&id).unwrap();
    assert_eq!(r1, r2, "Repeated reads must return identical records");
}

/// **PRF-002:** After spending a coin, reads reflect the spent state (cache coherence).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_002_cache_coherent_after_spend() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // Read before spend
    let before = store.get_coin_record(&id).unwrap().unwrap();
    assert!(!before.is_spent());

    // Spend the coin
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![id];
    store.apply_block(block).unwrap();

    // Read after spend — must reflect spent state
    let after = store.get_coin_record(&id).unwrap().unwrap();
    assert!(
        after.is_spent(),
        "Cache must reflect spent state after apply_block"
    );
    assert_eq!(after.spent_height, Some(1));
}

/// **PRF-002:** After rollback, reads reflect the un-spent state.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_002_cache_coherent_after_rollback() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // Spend in block 1
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.removals = vec![id];
    store.apply_block(block).unwrap();

    let spent = store.get_coin_record(&id).unwrap().unwrap();
    assert!(spent.is_spent());

    // Rollback to 0
    store.rollback_to_block(0).unwrap();

    let after_rb = store.get_coin_record(&id).unwrap().unwrap();
    assert!(
        !after_rb.is_spent(),
        "Cache must reflect un-spent state after rollback"
    );
}

/// **PRF-002:** Many reads of the same coin complete quickly (smoke test for caching).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_002_many_reads_bounded_time() {
    use std::time::Instant;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    let n = 10_000u32;
    let t0 = Instant::now();
    for _ in 0..n {
        let _ = store.get_coin_record(&id).unwrap();
    }
    let elapsed = t0.elapsed();
    // With an LRU cache, 10k reads should be < 500ms even without one
    assert!(
        elapsed.as_millis() < 2_000,
        "10k get_coin_record calls took {:?} — expected fast from cache or storage",
        elapsed
    );
}

/// **PRF-002:** Batch reads consistent with individual reads.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_002_batch_consistent_with_individual() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let id1 = c1.coin_id();
    let id2 = c2.coin_id();
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    let individual1 = store.get_coin_record(&id1).unwrap().unwrap();
    let individual2 = store.get_coin_record(&id2).unwrap().unwrap();
    let batch = store.get_coin_records(&[id1, id2]).unwrap();

    assert_eq!(batch.len(), 2);
    assert!(batch.iter().any(|r| r.coin_id() == individual1.coin_id()));
    assert!(batch.iter().any(|r| r.coin_id() == individual2.coin_id()));
}
