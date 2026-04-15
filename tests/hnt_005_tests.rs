//! # HNT-005 Tests -- Rollback Hint Cleanup
//!
//! Verifies requirement **HNT-005**: `remove_hints_for_coins()` correctly removes
//! all hint entries from both the forward (`CF_HINTS`) and reverse (`CF_HINTS_BY_VALUE`)
//! indices for the given coin IDs. This is used during rollback (RBK-002) to prevent
//! orphaned hint entries.
//!
//! # Requirement: HNT-005
//! # Spec: docs/requirements/domains/hints/specs/HNT-005.md
//!
//! ## How these tests prove the requirement
//!
//! - **Remove hints for a coin -- both indices cleaned**
//! - **After removal, get_coin_ids_by_hint returns empty for that hint**
//! - **After removal, get_hints_for_coin_ids returns empty for that coin**
//! - **Shared hint preserved for non-deleted coin**
//! - **Coin with no hints -- noop, returns 0**
//! - **Multi-hint coin -- all hints removed**
//! - **Count decremented correctly**
//! - **Empty coin_ids slice -- noop**

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::Bytes32;

/// **HNT-005:** Remove hints for a coin -- both forward and reverse indices cleaned.
///
/// **Proof:** Insert a hint for a coin, remove it via remove_hints_for_coins,
/// and verify both indices are empty and the return count is 1.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_remove_hints_both_indices_cleaned() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_id, hint.as_ref()).unwrap();
    assert_eq!(store.count_hints().unwrap(), 1);

    let removed = store.remove_hints_for_coins(&[coin_id]).unwrap();
    assert_eq!(removed, 1, "Must report 1 pair removed");
    assert_eq!(store.count_hints().unwrap(), 0, "Forward index must be empty");

    // Reverse index must also be empty.
    let reverse = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert!(reverse.is_empty(), "Reverse index must be empty after removal");
}

/// **HNT-005:** After removal, get_coin_ids_by_hint returns empty for that hint.
///
/// **Proof:** Insert a single (coin, hint) pair, remove the coin's hints, then
/// query the reverse index by that hint.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_reverse_empty_after_removal() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_id, hint.as_ref()).unwrap();
    store.remove_hints_for_coins(&[coin_id]).unwrap();

    let results = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert!(
        results.is_empty(),
        "get_coin_ids_by_hint must return empty after removal"
    );
}

/// **HNT-005:** After removal, get_hints_for_coin_ids returns empty for that coin.
///
/// **Proof:** Insert a hint for a coin, remove the coin's hints, then query
/// the forward index for that coin.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_forward_empty_after_removal() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_id, hint.as_ref()).unwrap();
    store.remove_hints_for_coins(&[coin_id]).unwrap();

    let map = store.get_hints_for_coin_ids(&[coin_id]).unwrap();
    assert!(
        map.is_empty(),
        "get_hints_for_coin_ids must return empty map after removal"
    );
}

/// **HNT-005:** Shared hint preserved for non-deleted coin.
///
/// **Proof:** Two coins share the same hint. Remove hints for only one coin.
/// The other coin's hint entry must survive in both indices.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_shared_hint_preserved_for_other_coin() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let shared_hint = Bytes32::from([0xAA; 32]);

    store.add_hint(&coin_a, shared_hint.as_ref()).unwrap();
    store.add_hint(&coin_b, shared_hint.as_ref()).unwrap();
    assert_eq!(store.count_hints().unwrap(), 2);

    // Remove only coin_a's hints.
    let removed = store.remove_hints_for_coins(&[coin_a]).unwrap();
    assert_eq!(removed, 1, "Only coin_a's hint pair removed");
    assert_eq!(store.count_hints().unwrap(), 1, "One hint pair remains");

    // coin_b's hint must still be in the reverse index.
    let reverse = store.get_coin_ids_by_hint(&shared_hint, 100).unwrap();
    assert_eq!(reverse.len(), 1, "Shared hint must still map to coin_b");
    assert_eq!(reverse[0], coin_b);

    // coin_b's forward index must still work.
    let map = store.get_hints_for_coin_ids(&[coin_b]).unwrap();
    assert!(map.contains_key(&coin_b));
    assert_eq!(map.get(&coin_b).unwrap().len(), 1);
}

/// **HNT-005:** Coin with no hints -- noop, returns 0.
///
/// **Proof:** Call remove_hints_for_coins on a coin_id that has no hints.
/// Must return 0 removed and not error.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_coin_with_no_hints_noop() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);

    let removed = store.remove_hints_for_coins(&[coin_id]).unwrap();
    assert_eq!(removed, 0, "Coin with no hints must return 0 removed");
}

/// **HNT-005:** Multi-hint coin -- all hints removed.
///
/// **Proof:** Insert 3 different hints for the same coin. Remove all via
/// remove_hints_for_coins. All 3 must be removed from both indices.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_multi_hint_coin_all_removed() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x01; 32]);
    let hint_a = Bytes32::from([0xAA; 32]);
    let hint_b = Bytes32::from([0xBB; 32]);
    let hint_c = Bytes32::from([0xCC; 32]);

    store.add_hint(&coin_id, hint_a.as_ref()).unwrap();
    store.add_hint(&coin_id, hint_b.as_ref()).unwrap();
    store.add_hint(&coin_id, hint_c.as_ref()).unwrap();
    assert_eq!(store.count_hints().unwrap(), 3);

    let removed = store.remove_hints_for_coins(&[coin_id]).unwrap();
    assert_eq!(removed, 3, "All 3 hint pairs must be removed");
    assert_eq!(store.count_hints().unwrap(), 0, "No hints remaining");

    // All reverse lookups must be empty.
    assert!(store.get_coin_ids_by_hint(&hint_a, 100).unwrap().is_empty());
    assert!(store.get_coin_ids_by_hint(&hint_b, 100).unwrap().is_empty());
    assert!(store.get_coin_ids_by_hint(&hint_c, 100).unwrap().is_empty());

    // Forward lookup must be empty.
    let map = store.get_hints_for_coin_ids(&[coin_id]).unwrap();
    assert!(map.is_empty());
}

/// **HNT-005:** Count decremented correctly after removal.
///
/// **Proof:** Insert hints for 2 coins (3 total pairs), remove one coin's hints,
/// and verify count reflects the remaining pairs.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_count_decremented_correctly() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0x01; 32]);
    let coin_b = Bytes32::from([0x02; 32]);
    let hint_1 = Bytes32::from([0xAA; 32]);
    let hint_2 = Bytes32::from([0xBB; 32]);

    // coin_a: 2 hints, coin_b: 1 hint => 3 total
    store.add_hint(&coin_a, hint_1.as_ref()).unwrap();
    store.add_hint(&coin_a, hint_2.as_ref()).unwrap();
    store.add_hint(&coin_b, hint_1.as_ref()).unwrap();
    assert_eq!(store.count_hints().unwrap(), 3);

    // Remove coin_a's hints (2 pairs).
    let removed = store.remove_hints_for_coins(&[coin_a]).unwrap();
    assert_eq!(removed, 2, "coin_a had 2 hint pairs");
    assert_eq!(
        store.count_hints().unwrap(),
        1,
        "Only coin_b's 1 hint pair remains"
    );
}

/// **HNT-005:** Empty coin_ids slice -- noop, returns 0.
///
/// **Proof:** Call remove_hints_for_coins with an empty slice.
/// Must return 0 and not error.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_005_empty_coin_ids_noop() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Add a hint so we can verify nothing is disturbed.
    let coin_id = Bytes32::from([0x01; 32]);
    let hint = Bytes32::from([0xAA; 32]);
    store.add_hint(&coin_id, hint.as_ref()).unwrap();

    let removed = store.remove_hints_for_coins(&[]).unwrap();
    assert_eq!(removed, 0, "Empty slice must return 0");
    assert_eq!(
        store.count_hints().unwrap(),
        1,
        "Existing hints must be undisturbed"
    );
}
