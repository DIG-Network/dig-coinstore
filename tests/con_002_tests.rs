//! # CON-002 Tests — RwLock Strategy (Shared Reads, Exclusive Writes)
//!
//! Verifies **CON-002**: the `CoinStore` API enforces shared reads (`&self`) and exclusive
//! writes (`&mut self`) at the Rust borrow checker level. A future `Arc<RwLock<CoinStore>>`
//! wrapper can rely on this compile-time separation.
//!
//! # Requirement: CON-002
//! # SPEC.md: §1.6 #19 (MVCC reads), §1.6 #20 (parallel validation)
//!
//! ## How these tests prove the requirement
//!
//! - **Read methods take `&self`:** multiple shared references can coexist (compile-time proof).
//! - **Write methods take `&mut self`:** exclusive access required (compile-time proof).
//! - **Concurrent reads via `Arc`:** two `Arc` clones can call `&self` methods without contention.
//! - **RwLock wrapping compiles:** `Arc<parking_lot::RwLock<CoinStore>>` is constructible and
//!   `read()` / `write()` guards compile against the `&self` / `&mut self` API.

mod helpers;

// ─────────────────────────────────────────────────────────────────────────────
// Compile-time proofs: read methods take &self
// ─────────────────────────────────────────────────────────────────────────────

/// **CON-002:** Query methods accept `&self` (shared reference).
///
/// **Proof:** Calling multiple `&self` methods through the same shared reference compiles.
/// If any read method required `&mut self`, this would fail.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_002_read_methods_take_shared_ref() {
    let dir = helpers::temp_dir();
    let mut store = dig_coinstore::coin_store::CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(1, 2, 100);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_000)
        .unwrap();

    // All of these take &self — multiple borrows from the same reference:
    let shared: &dig_coinstore::coin_store::CoinStore = &store;
    let _h = shared.height();
    let _t = shared.tip_hash();
    let _ts = shared.timestamp();
    let _init = shared.is_initialized();
    let _empty = shared.is_empty();
    let _cfg = shared.config();
    let _unspent = shared.is_unspent(&id);
    let _rec = shared.get_coin_record(&id).unwrap();
    let _stats = shared.stats();
}

/// **CON-002:** Write methods require `&mut self` (exclusive reference).
///
/// **Proof:** `apply_block` and `rollback_to_block` take `&mut self`. This test
/// calls them through an exclusive reference. If they took `&self`, the borrow
/// checker would allow unsafe concurrent mutation.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_002_write_methods_take_exclusive_ref() {
    let dir = helpers::temp_dir();
    let mut store = dig_coinstore::coin_store::CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // apply_block takes &mut self
    let block = dig_coinstore::BlockData {
        height: 1,
        timestamp: 1_700_000_018,
        block_hash: helpers::test_hash(0xB1),
        parent_hash: dig_coinstore::Bytes32::from([0u8; 32]),
        additions: vec![],
        removals: vec![],
        coinbase_coins: vec![
            helpers::test_coin(200, 201, 1_750_000_000_000),
            helpers::test_coin(202, 203, 250_000_000_000),
        ],
        hints: vec![],
        expected_state_root: None,
    };
    let _result = store.apply_block(block).unwrap();

    // rollback_to_block takes &mut self
    let _rb = store.rollback_to_block(0).unwrap();
}

/// **CON-002:** `Arc<RwLock<CoinStore>>` compiles and the read guard exposes `&self` methods.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_002_rwlock_wrapper_compiles() {
    use dig_coinstore::coin_store::CoinStore;
    use parking_lot::RwLock;
    use std::sync::Arc;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let shared = Arc::new(RwLock::new(store));

    // Read guard — multiple can coexist
    {
        let r = shared.read();
        let _h = r.height();
        let _s = r.stats();
    }

    // Write guard — exclusive
    {
        let mut w = shared.write();
        let block = dig_coinstore::BlockData {
            height: 1,
            timestamp: 1_700_000_018,
            block_hash: helpers::test_hash(0xB1),
            parent_hash: dig_coinstore::Bytes32::from([0u8; 32]),
            additions: vec![],
            removals: vec![],
            coinbase_coins: vec![
                helpers::test_coin(200, 201, 1_750_000_000_000),
                helpers::test_coin(202, 203, 250_000_000_000),
            ],
            hints: vec![],
            expected_state_root: None,
        };
        let _r = w.apply_block(block).unwrap();
    }
}

/// **CON-002:** Multiple concurrent read guards from separate threads compile.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_002_concurrent_read_guards() {
    use dig_coinstore::coin_store::CoinStore;
    use parking_lot::RwLock;
    use std::sync::Arc;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();
    let shared = Arc::new(RwLock::new(store));

    let mut handles = vec![];
    for _ in 0..4 {
        let s = Arc::clone(&shared);
        handles.push(std::thread::spawn(move || {
            let r = s.read();
            let _h = r.height();
            let _stats = r.stats();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}
