//! # API-001 Tests — CoinStore Constructor Verification
//!
//! Dedicated test file for requirement API-001: CoinStore constructors.
//! Verifies `new()`, `with_config()`, and `init_genesis()` behavior.
//!
//! # Requirement: API-001
//! # Spec: docs/requirements/domains/crate_api/specs/API-001.md
//! # NORMATIVE: docs/requirements/domains/crate_api/NORMATIVE.md#API-001
//! # SPEC.md: Section 3.1

mod helpers;

/// Verifies API-001: `CoinStore::new()` accepts a path and returns Ok.
///
/// The constructor MUST create the storage directory if it doesn't exist,
/// initialize internal data structures, and return a functional CoinStore.
///
/// This is the most basic "does it work" test for the entire crate.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_new_returns_ok() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path());
    assert!(
        store.is_ok(),
        "CoinStore::new() must return Ok for valid path"
    );
}

/// Verifies API-001: `CoinStore::with_config()` accepts a CoinStoreConfig.
///
/// The constructor MUST respect all configuration values provided.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_with_config_returns_ok() {
    use dig_coinstore::coin_store::CoinStore;
    use dig_coinstore::config::CoinStoreConfig;

    let dir = helpers::temp_dir();
    let config = CoinStoreConfig::default_with_path(dir.path());
    let store = CoinStore::with_config(config);
    assert!(store.is_ok(), "CoinStore::with_config() must return Ok");
}

/// Verifies API-001: A freshly constructed CoinStore is empty.
///
/// Before `init_genesis()`, `is_empty()` MUST return true and
/// `height()` MUST return 0.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_empty_before_genesis() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    assert!(store.is_empty(), "Store must be empty before genesis");
    assert_eq!(store.height(), 0, "Height must be 0 before genesis");
}

/// Verifies API-001: `init_genesis()` bootstraps the chain with coins.
///
/// After calling `init_genesis()` with coins, `height()` MUST return 0,
/// the returned state root MUST be non-zero (coins exist), and the store
/// MUST no longer be empty.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_init_genesis_with_coins() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    // Create two genesis coins.
    let coin1 = helpers::test_coin(1, 10, 1_000_000);
    let coin2 = helpers::test_coin(2, 20, 2_000_000);
    let initial_coins = vec![(coin1, false), (coin2, true)]; // second is coinbase

    let result = store.init_genesis(initial_coins, 1_700_000_000);
    assert!(result.is_ok(), "init_genesis must succeed");

    let state_root = result.unwrap();
    assert_ne!(
        state_root,
        dig_coinstore::Bytes32::from([0u8; 32]),
        "Genesis state root must be non-zero when coins exist"
    );
    assert_eq!(store.height(), 0, "Height must be 0 after genesis");
}

/// Verifies API-001: `init_genesis()` with empty coins is valid.
///
/// An empty genesis (no initial coins) is a legitimate bootstrap.
/// The state root should be the empty Merkle tree root.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_init_genesis_empty_coins() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    let result = store.init_genesis(vec![], 1_700_000_000);
    assert!(result.is_ok(), "Empty genesis must succeed");
    assert_eq!(store.height(), 0);
}

/// Verifies API-001: Double `init_genesis()` returns GenesisAlreadyInitialized.
///
/// The genesis step MUST be idempotent-safe: calling it twice MUST fail
/// with a clear error rather than silently overwriting state.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_double_genesis_rejected() {
    use dig_coinstore::coin_store::CoinStore;
    use dig_coinstore::error::CoinStoreError;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    // First genesis: ok.
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Second genesis: must fail.
    let result = store.init_genesis(vec![], 1_700_000_001);
    assert!(
        matches!(result, Err(CoinStoreError::GenesisAlreadyInitialized)),
        "Double genesis must return GenesisAlreadyInitialized, got: {:?}",
        result
    );
}

/// Verifies API-001: `new()` re-opens an existing store without data loss.
///
/// Opening the same path twice MUST preserve the previously initialized state.
/// This tests restart recovery.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_001_reopen_existing_store() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();

    // First open: init genesis.
    {
        let mut store = CoinStore::new(dir.path()).unwrap();
        let coin = helpers::test_coin(1, 10, 1_000);
        store
            .init_genesis(vec![(coin, false)], 1_700_000_000)
            .unwrap();
    }
    // CoinStore dropped, DB closed.

    // Second open: state must persist.
    {
        let store = CoinStore::new(dir.path()).unwrap();
        assert!(!store.is_empty(), "Re-opened store must not be empty");
        assert_eq!(store.height(), 0, "Re-opened store must preserve height");
    }
}
