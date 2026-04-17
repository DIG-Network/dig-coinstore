//! # CON-001 Tests — CoinStore is Send + Sync
//!
//! Verifies **CON-001**: `CoinStore` implements `Send + Sync` for safe concurrent access.
//!
//! # Requirement: CON-001
//! # SPEC.md: §1.6 #19 (MVCC), §1.6 #20 (parallel validation)

mod helpers;

/// **CON-001:** CoinStore is Send + Sync.
///
/// **Proof:** If CoinStore does not implement Send + Sync, this function
/// signature fails at compile time. This is a zero-cost compile-time assertion.
#[test]
fn vv_req_con_001_coin_store_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<dig_coinstore::coin_store::CoinStore>();
}

#[test]
fn vv_req_con_001_coin_store_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<dig_coinstore::coin_store::CoinStore>();
}

/// **CON-001:** CoinStore can be shared across threads via Arc.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_con_001_arc_shareable() {
    use dig_coinstore::coin_store::CoinStore;
    use std::sync::Arc;

    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    let shared = Arc::new(store);
    let _clone = Arc::clone(&shared);
    // If CoinStore is not Send+Sync, Arc<CoinStore> cannot be sent to threads.
}
