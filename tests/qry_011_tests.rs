//! # QRY-011 Tests — Large Input Batching
//!
//! Verifies **QRY-011**: query methods handle large input slices correctly.
//! Since our KV-based approach processes items individually (no SQL parameter limits),
//! the chunking guarantee is inherent. These tests verify large inputs work.
//!
//! # Requirement: QRY-011
//! # SPEC.md: §2.7 (DEFAULT_LOOKUP_BATCH_SIZE)

mod helpers;

use dig_coinstore::{coin_store::CoinStore, Bytes32};

/// Large batch of coin IDs (100) doesn't crash or error.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_011_large_coin_ids_batch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // 100 random IDs, none exist → should get empty result, no crash.
    let ids: Vec<Bytes32> = (0..100u8).map(helpers::test_hash).collect();
    let results = store.get_coin_records(&ids).unwrap();
    assert!(results.is_empty());
}

/// Large batch of puzzle hashes doesn't crash.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_011_large_puzzle_hash_batch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let phs: Vec<Bytes32> = (0..100u8).map(helpers::test_hash).collect();
    let results = store
        .get_coin_records_by_puzzle_hashes(true, &phs, 0, u64::MAX)
        .unwrap();
    assert!(results.is_empty());
}

/// Large batch of names with filters doesn't crash.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_011_large_names_batch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let names: Vec<Bytes32> = (0..100u8).map(helpers::test_hash).collect();
    let results = store
        .get_coin_records_by_names(true, &names, 0, u64::MAX)
        .unwrap();
    assert!(results.is_empty());
}

/// Large batch of parent IDs doesn't crash.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_qry_011_large_parent_ids_batch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let parents: Vec<Bytes32> = (0..100u8).map(helpers::test_hash).collect();
    let results = store
        .get_coin_records_by_parent_ids(true, &parents, 0, u64::MAX)
        .unwrap();
    assert!(results.is_empty());
}
