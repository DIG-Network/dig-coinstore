//! # HNT-006 Tests -- Variable-Length Hint Keys
//!
//! Verifies requirement **HNT-006**: the hint store correctly handles hints of
//! varying lengths (1-32 bytes). The key encoding uses raw concatenation, so
//! `get_coin_ids_by_hint_bytes()` must filter by exact key length to avoid
//! prefix collisions between hints of different lengths.
//!
//! # Requirement: HNT-006
//! # Spec: docs/requirements/domains/hints/specs/HNT-006.md
//!
//! ## How these tests prove the requirement
//!
//! - **32-byte hint stored and queried correctly (baseline)**
//! - **Short hint (16 bytes) stored via add_hint and queried via get_coin_ids_by_hint_bytes**
//! - **Short hints don't collide with 32-byte hints sharing the same prefix**
//! - **Two different-length hints for same coin both stored**
//! - **Only 32-byte hints match via get_coin_ids_by_hint (Bytes32 API)**

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::Bytes32;

/// **HNT-006:** 32-byte hint stored and queried correctly (baseline).
///
/// **Proof:** Insert a standard 32-byte hint, query via both the Bytes32 API
/// and the variable-length API. Both must return the same coin.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_006_32_byte_hint_baseline() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_id, hint.as_ref()).unwrap();

    // Bytes32 API
    let results_typed = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert_eq!(results_typed.len(), 1);
    assert_eq!(results_typed[0], coin_id);

    // Variable-length API
    let results_bytes = store
        .get_coin_ids_by_hint_bytes(hint.as_ref(), 100)
        .unwrap();
    assert_eq!(results_bytes.len(), 1);
    assert_eq!(results_bytes[0], coin_id);
}

/// **HNT-006:** Short hint (16 bytes) stored and queried via variable-length API.
///
/// **Proof:** Insert a 16-byte hint via add_hint (which accepts &[u8]),
/// query via get_coin_ids_by_hint_bytes with the same 16-byte slice.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_006_short_hint_stored_and_queried() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let short_hint: [u8; 16] = [0xBB; 16];

    store.add_hint(&coin_id, &short_hint).unwrap();
    assert_eq!(store.count_hints().unwrap(), 1, "Short hint must be stored");

    let results = store
        .get_coin_ids_by_hint_bytes(&short_hint, 100)
        .unwrap();
    assert_eq!(results.len(), 1, "Short hint must be found via bytes API");
    assert_eq!(results[0], coin_id);
}

/// **HNT-006:** Short hints don't collide with 32-byte hints sharing the same prefix.
///
/// **Proof:** Insert a 16-byte hint `[0xAA; 16]` for coin_a and a 32-byte hint
/// `[0xAA; 32]` (which starts with the same 16 bytes) for coin_b. Querying
/// the 16-byte hint must return only coin_a; querying the 32-byte hint must
/// return only coin_b.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_006_no_prefix_collision() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);

    let short_hint: [u8; 16] = [0xAA; 16];
    let long_hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_a, &short_hint).unwrap();
    store.add_hint(&coin_b, long_hint.as_ref()).unwrap();
    assert_eq!(store.count_hints().unwrap(), 2);

    // Query for the 16-byte hint: only coin_a.
    let short_results = store
        .get_coin_ids_by_hint_bytes(&short_hint, 100)
        .unwrap();
    assert_eq!(
        short_results.len(),
        1,
        "16-byte hint query must not match 32-byte hint"
    );
    assert_eq!(short_results[0], coin_a);

    // Query for the 32-byte hint: only coin_b.
    let long_results = store
        .get_coin_ids_by_hint_bytes(long_hint.as_ref(), 100)
        .unwrap();
    assert_eq!(
        long_results.len(),
        1,
        "32-byte hint query must not match 16-byte hint"
    );
    assert_eq!(long_results[0], coin_b);
}

/// **HNT-006:** Two different-length hints for the same coin both stored.
///
/// **Proof:** Insert a 16-byte hint and a 32-byte hint for the same coin_id.
/// Both must be stored as separate entries in the forward index, and both
/// must be queryable independently.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_006_two_lengths_same_coin() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let short_hint: [u8; 16] = [0xCC; 16];
    let long_hint = Bytes32::from([0xDD; 32]);

    store.add_hint(&coin_id, &short_hint).unwrap();
    store.add_hint(&coin_id, long_hint.as_ref()).unwrap();

    assert_eq!(
        store.count_hints().unwrap(),
        2,
        "Two different-length hints must create 2 forward entries"
    );

    // Query each independently.
    let short_results = store
        .get_coin_ids_by_hint_bytes(&short_hint, 100)
        .unwrap();
    assert_eq!(short_results.len(), 1);
    assert_eq!(short_results[0], coin_id);

    let long_results = store
        .get_coin_ids_by_hint_bytes(long_hint.as_ref(), 100)
        .unwrap();
    assert_eq!(long_results.len(), 1);
    assert_eq!(long_results[0], coin_id);
}

/// **HNT-006:** Only 32-byte hints match via get_coin_ids_by_hint (Bytes32 API).
///
/// **Proof:** Insert a 16-byte hint and a 32-byte hint for different coins.
/// The Bytes32 API (get_coin_ids_by_hint) must only return coins associated
/// with the 32-byte hint, never the 16-byte one.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_006_bytes32_api_only_matches_32_byte_hints() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_short = Bytes32::from([0x01; 32]);
    let coin_long = Bytes32::from([0x02; 32]);

    // 16-byte hint that is a prefix of the 32-byte hint value.
    let short_hint: [u8; 16] = [0xEE; 16];
    let long_hint = Bytes32::from([0xEE; 32]);

    store.add_hint(&coin_short, &short_hint).unwrap();
    store.add_hint(&coin_long, long_hint.as_ref()).unwrap();

    // Bytes32 API with the 32-byte [0xEE; 32] hint.
    let results = store.get_coin_ids_by_hint(&long_hint, 100).unwrap();

    // Must contain coin_long (32-byte hint match).
    assert!(
        results.contains(&coin_long),
        "Bytes32 API must return coin with matching 32-byte hint"
    );

    // Must NOT contain coin_short (16-byte hint is shorter, key is only 48 bytes,
    // so the get_coin_ids_by_hint filter `key.len() >= 64` excludes it).
    assert!(
        !results.contains(&coin_short),
        "Bytes32 API must not match coins with shorter hints"
    );
}
