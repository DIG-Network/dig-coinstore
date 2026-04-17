//! # HNT-004 Tests — Hint Queries
//!
//! Verifies requirement **HNT-004**: hint query methods on `CoinStore`.
//! - `get_coin_ids_by_hint(hint, max_items)` — single hint reverse lookup with limit
//! - `get_coin_ids_by_hints(hints, max_items)` — batch reverse lookup, deduplicated
//! - `get_hints_for_coin_ids(coin_ids)` — forward lookup returning `HashMap<CoinId, Vec<Bytes32>>`
//! - `count_hints()` — total forward-index entry count
//!
//! # Requirement: HNT-004
//! # Spec: docs/requirements/domains/hints/specs/HNT-004.md
//! # SPEC.md: §3.9 (Hint Query API)
//!
//! ## How these tests prove the requirement
//!
//! - **get_coin_ids_by_hint returns matching coins**
//! - **get_coin_ids_by_hint with max_items limit**
//! - **get_coin_ids_by_hints batch query with deduplication**
//! - **get_hints_for_coin_ids returns correct map**
//! - **count_hints returns correct count**
//! - **Empty store returns empty/zero**
//! - **Nonexistent hint returns empty**
//! - **get_hints_for_coin_ids with unknown coin_id returns empty map**

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::Bytes32;

/// **HNT-004:** get_coin_ids_by_hint returns matching coins.
///
/// **Proof:** Insert hints for 3 coins with the same hint, query by that hint,
/// and verify all 3 coin IDs are returned.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_get_coin_ids_by_hint_basic() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hint = Bytes32::from([0xAA; 32]);
    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let coin_c = Bytes32::from([0x03; 32]);

    store.add_hint(&coin_a, hint.as_ref()).unwrap();
    store.add_hint(&coin_b, hint.as_ref()).unwrap();
    store.add_hint(&coin_c, hint.as_ref()).unwrap();

    let results = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert_eq!(results.len(), 3, "All 3 coins must be returned");
    assert!(results.contains(&coin_a));
    assert!(results.contains(&coin_b));
    assert!(results.contains(&coin_c));
}

/// **HNT-004:** get_coin_ids_by_hint respects max_items limit.
///
/// **Proof:** Insert 5 coins with the same hint, query with max_items=2.
/// Only 2 results should be returned.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_get_coin_ids_by_hint_max_items() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hint = Bytes32::from([0xAA; 32]);
    for i in 1..=5u8 {
        let coin_id = Bytes32::from([i; 32]);
        store.add_hint(&coin_id, hint.as_ref()).unwrap();
    }

    let results = store.get_coin_ids_by_hint(&hint, 2).unwrap();
    assert_eq!(
        results.len(),
        2,
        "max_items=2 must limit results to 2, got {}",
        results.len()
    );
}

/// **HNT-004:** get_coin_ids_by_hints batch query with deduplication.
///
/// **Proof:** Insert coin_a with hint_x, coin_b with hint_y, and coin_a also with hint_y.
/// Query for [hint_x, hint_y]. coin_a should appear once (deduplicated), coin_b once.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_get_coin_ids_by_hints_batch_dedup() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let hint_x = Bytes32::from([0xAA; 32]);
    let hint_y = Bytes32::from([0xBB; 32]);

    // coin_a has both hint_x and hint_y.
    store.add_hint(&coin_a, hint_x.as_ref()).unwrap();
    store.add_hint(&coin_a, hint_y.as_ref()).unwrap();
    // coin_b has only hint_y.
    store.add_hint(&coin_b, hint_y.as_ref()).unwrap();

    let results = store.get_coin_ids_by_hints(&[hint_x, hint_y], 100).unwrap();
    assert_eq!(
        results.len(),
        2,
        "Deduplicated results should contain 2 unique coin IDs, got {}",
        results.len()
    );
    assert!(results.contains(&coin_a));
    assert!(results.contains(&coin_b));
}

/// **HNT-004:** get_hints_for_coin_ids returns correct map.
///
/// **Proof:** Insert 2 hints for coin_a, 1 hint for coin_b. Query both.
/// Verify the returned map has correct entries.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_get_hints_for_coin_ids_map() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let hint_1 = Bytes32::from([0xAA; 32]);
    let hint_2 = Bytes32::from([0xBB; 32]);
    let hint_3 = Bytes32::from([0xCC; 32]);

    store.add_hint(&coin_a, hint_1.as_ref()).unwrap();
    store.add_hint(&coin_a, hint_2.as_ref()).unwrap();
    store.add_hint(&coin_b, hint_3.as_ref()).unwrap();

    let map = store.get_hints_for_coin_ids(&[coin_a, coin_b]).unwrap();

    assert!(map.contains_key(&coin_a));
    let hints_a = map.get(&coin_a).unwrap();
    assert_eq!(hints_a.len(), 2, "coin_a must have 2 hints");
    assert!(hints_a.contains(&hint_1));
    assert!(hints_a.contains(&hint_2));

    assert!(map.contains_key(&coin_b));
    let hints_b = map.get(&coin_b).unwrap();
    assert_eq!(hints_b.len(), 1, "coin_b must have 1 hint");
    assert_eq!(hints_b[0], hint_3);
}

/// **HNT-004:** count_hints returns correct count.
///
/// **Proof:** Insert 3 unique (coin_id, hint) pairs. count_hints must return 3.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_count_hints_correct() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    assert_eq!(store.count_hints().unwrap(), 0, "Empty store has 0 hints");

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let hint_1 = Bytes32::from([0xAA; 32]);
    let hint_2 = Bytes32::from([0xBB; 32]);

    store.add_hint(&coin_a, hint_1.as_ref()).unwrap();
    store.add_hint(&coin_a, hint_2.as_ref()).unwrap();
    store.add_hint(&coin_b, hint_1.as_ref()).unwrap();

    assert_eq!(
        store.count_hints().unwrap(),
        3,
        "3 unique (coin_id, hint) pairs must yield count 3"
    );
}

/// **HNT-004:** Empty store returns empty results and zero count.
///
/// **Proof:** On a freshly initialized store with no hints, all query methods
/// return empty/zero.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_empty_store_queries() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let any_hint = Bytes32::from([0xFF; 32]);
    let any_coin = Bytes32::from([0xEE; 32]);

    assert_eq!(store.count_hints().unwrap(), 0);
    assert!(store
        .get_coin_ids_by_hint(&any_hint, 100)
        .unwrap()
        .is_empty());
    assert!(store
        .get_coin_ids_by_hints(&[any_hint], 100)
        .unwrap()
        .is_empty());
    assert!(store
        .get_hints_for_coin_ids(&[any_coin])
        .unwrap()
        .is_empty());
}

/// **HNT-004:** get_coin_ids_by_hint for nonexistent hint returns empty.
///
/// **Proof:** Insert a hint for one coin, then query for a completely different hint.
/// The result must be empty.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_nonexistent_hint_returns_empty() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let stored_hint = Bytes32::from([0xAA; 32]);
    let missing_hint = Bytes32::from([0xFF; 32]);

    store.add_hint(&coin_id, stored_hint.as_ref()).unwrap();

    let results = store.get_coin_ids_by_hint(&missing_hint, 100).unwrap();
    assert!(
        results.is_empty(),
        "Query for nonexistent hint must return empty"
    );
}

/// **HNT-004:** get_hints_for_coin_ids with unknown coin_id returns empty map.
///
/// **Proof:** Insert a hint for coin_a, then query for coin_b (which has no hints).
/// The map should not contain coin_b.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_004_unknown_coin_returns_empty_map() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_a, hint.as_ref()).unwrap();

    let map = store.get_hints_for_coin_ids(&[coin_b]).unwrap();
    assert!(
        map.is_empty(),
        "Map must be empty for coin_id with no hints"
    );
}
