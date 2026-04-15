//! # PRF-001 Tests — In-Memory Unspent Set (`HashSet<CoinId>`)
//!
//! Verifies **PRF-001**: the in-memory `HashSet<CoinId>` inside `CoinStore` provides O(1)
//! `is_unspent()` checks. The set is populated at genesis, via `restore()`, and on store
//! reopen (rebuild from disk scan). Incremental maintenance during `apply_block` and
//! `rollback` will be added by BLK-008+ and RBK-003+.
//!
//! # Requirement: PRF-001
//! # SPEC.md: §1.6 #13 (In-Memory Unspent Set), §2.7 (MATERIALIZATION_BATCH_SIZE)
//!
//! ## How these tests prove the requirement
//!
//! - **Genesis population:** `is_unspent` returns true for coins inserted at genesis.
//! - **Unknown coin:** `is_unspent` returns false for coins never inserted.
//! - **Reopen rebuild:** Closing and reopening the store rebuilds the set from disk.
//! - **Restore population:** `restore()` populates the set from snapshot data.
//! - **Spent coins excluded:** `restore()` with a spent coin does not include it in the set.
//! - **O(1) smoke:** Many lookups complete in bounded time.
//! - **Multiple genesis coins:** All genesis coins appear in the unspent set.

mod helpers;

use std::collections::HashMap;
use std::time::Instant;
use dig_coinstore::{
    coin_store::CoinStore, Bytes32, CoinId, CoinRecord, CoinStoreSnapshot,
};
use dig_coinstore::merkle::{merkle_leaf_hash, SparseMerkleTree};

/// Same 97-byte legacy layout used by CoinStore for non-ff_eligible rows.
fn legacy_storage_bytes(rec: &CoinRecord) -> Vec<u8> {
    let mut buf = Vec::with_capacity(97);
    buf.extend_from_slice(rec.coin.parent_coin_info.as_ref());
    buf.extend_from_slice(rec.coin.puzzle_hash.as_ref());
    buf.extend_from_slice(&rec.coin.amount.to_le_bytes());
    buf.extend_from_slice(&rec.confirmed_height.to_le_bytes());
    let spent_raw = rec.spent_height.unwrap_or(0);
    buf.extend_from_slice(&spent_raw.to_le_bytes());
    buf.push(if rec.coinbase { 1 } else { 0 });
    buf.extend_from_slice(&rec.timestamp.to_le_bytes());
    buf
}

fn smt_root_from_records(records: &[CoinRecord]) -> Bytes32 {
    let mut tree = SparseMerkleTree::new();
    let entries: Vec<(Bytes32, Bytes32)> = records
        .iter()
        .map(|r| (r.coin_id(), merkle_leaf_hash(&legacy_storage_bytes(r))))
        .collect();
    if !entries.is_empty() {
        tree.batch_insert(&entries).unwrap();
    }
    tree.root()
}

/// **PRF-001:** Genesis coins are present in the unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_genesis_populates_unspent_set() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let id1 = c1.coin_id();
    let id2 = c2.coin_id();
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    assert!(store.is_unspent(&id1));
    assert!(store.is_unspent(&id2));
}

/// **PRF-001:** Unknown coin IDs return false from is_unspent.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_unknown_coin_returns_false() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let random_id: CoinId = helpers::test_hash(0xFF);
    assert!(!store.is_unspent(&random_id));
}

/// **PRF-001:** Reopening the store rebuilds the unspent set from disk.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_reopen_rebuilds_unspent_set() {
    let dir = helpers::temp_dir();
    let path = dir.path().to_path_buf();
    let coin = helpers::test_coin(1, 2, 100);
    let id = coin.coin_id();

    {
        let mut store = CoinStore::new(&path).unwrap();
        store
            .init_genesis(vec![(coin, false)], 1_700_000_000)
            .unwrap();
        assert!(store.is_unspent(&id));
    }

    // Reopen
    let store2 = CoinStore::new(&path).unwrap();
    assert!(store2.is_unspent(&id), "Unspent set must survive reopen");
}

/// **PRF-001:** restore() populates the unspent set from snapshot coins.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_restore_populates_unspent_set() {
    let coin = helpers::test_coin(5, 6, 750);
    let rec = CoinRecord::new(coin, 0, 1_700_000_000, false);
    let id = rec.coin_id();
    let mut coins = HashMap::new();
    coins.insert(id, rec.clone());
    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::from([0u8; 32]),
        state_root: smt_root_from_records(std::slice::from_ref(&rec)),
        timestamp: 1_700_000_000,
        coins,
        hints: vec![],
        total_coins: 1,
        total_value: 750,
    };

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.restore(snap).unwrap();
    assert!(store.is_unspent(&id), "Restored unspent coin must be in set");
}

/// **PRF-001:** restore() with a spent coin excludes it from the unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_restore_excludes_spent_coins() {
    let coin = helpers::test_coin(9, 8, 123);
    let mut rec = CoinRecord::new(coin, 0, 1_700_000_000, false);
    rec.spend(5);
    let id = rec.coin_id();
    let mut coins = HashMap::new();
    coins.insert(id, rec.clone());
    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::from([0u8; 32]),
        state_root: smt_root_from_records(std::slice::from_ref(&rec)),
        timestamp: 1_700_000_000,
        coins,
        hints: vec![],
        total_coins: 1,
        total_value: 0, // spent, so no unspent value
    };

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.restore(snap).unwrap();
    assert!(!store.is_unspent(&id), "Spent coin must not be in unspent set");
}

/// **PRF-001:** O(1) smoke test — many lookups complete in bounded time.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_many_lookups_bounded_time() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(7, 7, 1);
    let id = coin.coin_id();
    store.init_genesis(vec![(coin, false)], 1).unwrap();

    let n = 50_000u32;
    let t0 = Instant::now();
    for _ in 0..n {
        assert!(store.is_unspent(&id));
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed.as_millis() < 200,
        "50k is_unspent calls took {:?} (expected HashSet O(1) aggregate)",
        elapsed
    );
}

/// **PRF-001:** Multiple genesis coins all appear in the unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_multiple_genesis_coins() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    let mut coins = Vec::new();
    let mut ids = Vec::new();
    for i in 0..20u8 {
        let coin = helpers::test_coin(i, 50, 100 + i as u64);
        ids.push(coin.coin_id());
        coins.push((coin, false));
    }
    store.init_genesis(coins, 1_700_000_000).unwrap();

    for id in &ids {
        assert!(store.is_unspent(id), "All 20 genesis coins should be unspent");
    }
}

/// **PRF-001:** apply_block adds new coins to unspent set and removes spent coins.
///
/// This is the critical incremental maintenance test. After apply_block:
/// - Newly created coins (coinbase + additions) MUST be in the unspent set.
/// - Spent coins MUST be removed from the unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_apply_block_maintains_unspent_set() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    let genesis_id = genesis_coin.coin_id();
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    assert!(store.is_unspent(&genesis_id), "Genesis coin must be unspent");

    // Apply block 1: add a new coin, spend the genesis coin.
    let new_coin = helpers::test_coin(10, 11, 500);
    let new_id = new_coin.coin_id();
    let cb1 = helpers::test_coin(200, 201, 1_750_000_000_000);
    let cb2 = helpers::test_coin(202, 203, 250_000_000_000);
    let cb1_id = cb1.coin_id();
    let cb2_id = cb2.coin_id();

    let block = dig_coinstore::BlockData {
        height: 1,
        timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1),
        parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![dig_coinstore::CoinAddition::from_coin(new_coin, false)],
        removals: vec![genesis_id],
        coinbase_coins: vec![cb1, cb2],
        hints: vec![],
        expected_state_root: None,
    };
    store.apply_block(block).unwrap();

    // Genesis coin was spent — must NOT be in unspent set.
    assert!(
        !store.is_unspent(&genesis_id),
        "Spent genesis coin must be removed from unspent set after apply_block"
    );

    // New transaction coin must be in unspent set.
    assert!(
        store.is_unspent(&new_id),
        "New addition must be in unspent set after apply_block"
    );

    // Coinbase coins must be in unspent set.
    assert!(
        store.is_unspent(&cb1_id),
        "Coinbase coin 1 must be in unspent set after apply_block"
    );
    assert!(
        store.is_unspent(&cb2_id),
        "Coinbase coin 2 must be in unspent set after apply_block"
    );
}

/// **PRF-001:** rollback_to_block maintains unspent set correctly.
///
/// After rollback:
/// - Coins deleted (confirmed after target) MUST be removed from unspent set.
/// - Coins un-spent (spent after target) MUST be re-added to unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_001_rollback_maintains_unspent_set() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    let genesis_id = genesis_coin.coin_id();
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    // Apply block 1: spend genesis, add new coin.
    let new_coin = helpers::test_coin(10, 11, 500);
    let new_id = new_coin.coin_id();
    let block = dig_coinstore::BlockData {
        height: 1,
        timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1),
        parent_hash: Bytes32::from([0u8; 32]),
        additions: vec![dig_coinstore::CoinAddition::from_coin(new_coin, false)],
        removals: vec![genesis_id],
        coinbase_coins: vec![
            helpers::test_coin(200, 201, 1_750_000_000_000),
            helpers::test_coin(202, 203, 250_000_000_000),
        ],
        hints: vec![],
        expected_state_root: None,
    };
    store.apply_block(block).unwrap();

    // After block 1: genesis spent (not in set), new_coin in set.
    assert!(!store.is_unspent(&genesis_id));
    assert!(store.is_unspent(&new_id));

    // Rollback to height 0: new_coin deleted, genesis un-spent.
    store.rollback_to_block(0).unwrap();

    // Genesis coin should be back in the unspent set (un-spent).
    assert!(
        store.is_unspent(&genesis_id),
        "Un-spent genesis coin must be back in unspent set after rollback"
    );

    // New coin from block 1 should be gone (deleted).
    assert!(
        !store.is_unspent(&new_id),
        "Deleted coin must be removed from unspent set after rollback"
    );
}
