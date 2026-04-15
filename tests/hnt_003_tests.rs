//! # HNT-003 Tests — Bidirectional Indices
//!
//! Verifies requirement **HNT-003**: the forward index (`CF_HINTS`: coin_id -> hints)
//! and reverse index (`CF_HINTS_BY_VALUE`: hint -> coin_ids) are both correctly
//! maintained by `add_hint()`.
//!
//! # Requirement: HNT-003
//! # Spec: docs/requirements/domains/hints/specs/HNT-003.md
//! # SPEC.md: §7.2 (column family key layout), §3.9 (Hint Query API)
//!
//! ## How these tests prove the requirement
//!
//! - **Forward index:** Hints for a coin_id are found via `get_hints_for_coin_ids`.
//! - **Reverse index:** Coin IDs for a hint are found via `get_coin_ids_by_hint`.
//! - **Both indices consistent after add_hint:** Forward and reverse agree.
//! - **Multiple hints per coin, multiple coins per hint:** Many-to-many relationship.

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::Bytes32;

/// **HNT-003 / Forward index:** Hints for a coin_id found via forward lookup.
///
/// **Proof:** Insert two hints for the same coin_id. Query with get_hints_for_coin_ids
/// returns both hints for that coin.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_003_forward_index_lookup() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0xAA; 32]);
    let hint_a = Bytes32::from([0xBB; 32]);
    let hint_b = Bytes32::from([0xCC; 32]);

    store.add_hint(&coin_id, hint_a.as_ref()).unwrap();
    store.add_hint(&coin_id, hint_b.as_ref()).unwrap();

    let map = store.get_hints_for_coin_ids(&[coin_id]).unwrap();
    assert!(map.contains_key(&coin_id), "Forward index must contain coin_id");
    let hints = map.get(&coin_id).unwrap();
    assert_eq!(hints.len(), 2, "Two hints must be returned for the coin");
    assert!(hints.contains(&hint_a), "Forward index must contain hint_a");
    assert!(hints.contains(&hint_b), "Forward index must contain hint_b");
}

/// **HNT-003 / Reverse index:** Coin IDs for a hint found via reverse lookup.
///
/// **Proof:** Insert the same hint for two different coin_ids. Query with
/// get_coin_ids_by_hint returns both coin_ids.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_003_reverse_index_lookup() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0xAA; 32]);
    let coin_b = Bytes32::from([0xBB; 32]);
    let hint = Bytes32::from([0xCC; 32]);

    store.add_hint(&coin_a, hint.as_ref()).unwrap();
    store.add_hint(&coin_b, hint.as_ref()).unwrap();

    let results = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert_eq!(results.len(), 2, "Reverse index must return both coin_ids");
    assert!(results.contains(&coin_a), "Reverse index must contain coin_a");
    assert!(results.contains(&coin_b), "Reverse index must contain coin_b");
}

/// **HNT-003 / Consistency:** Both indices agree after add_hint.
///
/// **Proof:** Insert a hint for a coin. Verify forward lookup finds the hint for
/// that coin, and reverse lookup finds that coin for the hint.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_003_indices_consistent() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x11; 32]);
    let hint = Bytes32::from([0x22; 32]);

    store.add_hint(&coin_id, hint.as_ref()).unwrap();

    // Forward: coin_id -> hints should include this hint.
    let map = store.get_hints_for_coin_ids(&[coin_id]).unwrap();
    let fwd_hints = map.get(&coin_id).unwrap();
    assert!(
        fwd_hints.contains(&hint),
        "Forward index must contain the hint"
    );

    // Reverse: hint -> coin_ids should include this coin_id.
    let rev_coins = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert!(
        rev_coins.contains(&coin_id),
        "Reverse index must contain the coin_id"
    );
}

/// **HNT-003 / Many-to-many:** Multiple hints per coin, multiple coins per hint.
///
/// **Proof:** Create a grid of 2 coins x 2 hints (plus one shared hint).
/// Verify forward index returns correct hints per coin and reverse index
/// returns correct coins per hint.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_003_many_to_many() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0xAA; 32]);
    let coin_b = Bytes32::from([0xBB; 32]);
    let hint_shared = Bytes32::from([0x11; 32]);
    let hint_only_a = Bytes32::from([0x22; 32]);
    let hint_only_b = Bytes32::from([0x33; 32]);

    // coin_a has hint_shared + hint_only_a
    store.add_hint(&coin_a, hint_shared.as_ref()).unwrap();
    store.add_hint(&coin_a, hint_only_a.as_ref()).unwrap();

    // coin_b has hint_shared + hint_only_b
    store.add_hint(&coin_b, hint_shared.as_ref()).unwrap();
    store.add_hint(&coin_b, hint_only_b.as_ref()).unwrap();

    // Forward: coin_a should have 2 hints.
    let map = store.get_hints_for_coin_ids(&[coin_a, coin_b]).unwrap();
    let hints_a = map.get(&coin_a).unwrap();
    assert_eq!(hints_a.len(), 2, "coin_a must have 2 hints");
    assert!(hints_a.contains(&hint_shared));
    assert!(hints_a.contains(&hint_only_a));

    let hints_b = map.get(&coin_b).unwrap();
    assert_eq!(hints_b.len(), 2, "coin_b must have 2 hints");
    assert!(hints_b.contains(&hint_shared));
    assert!(hints_b.contains(&hint_only_b));

    // Reverse: hint_shared should map to both coins.
    let shared_coins = store.get_coin_ids_by_hint(&hint_shared, 100).unwrap();
    assert_eq!(shared_coins.len(), 2, "Shared hint must map to 2 coins");
    assert!(shared_coins.contains(&coin_a));
    assert!(shared_coins.contains(&coin_b));

    // Reverse: hint_only_a should map to only coin_a.
    let only_a_coins = store.get_coin_ids_by_hint(&hint_only_a, 100).unwrap();
    assert_eq!(only_a_coins.len(), 1);
    assert_eq!(only_a_coins[0], coin_a);

    // Total: 4 forward-index entries.
    let count = store.count_hints().unwrap();
    assert_eq!(count, 4, "4 unique (coin_id, hint) pairs total");
}
