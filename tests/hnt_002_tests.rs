//! # HNT-002 Tests — Idempotent Hint Insertion
//!
//! Verifies requirement **HNT-002**: duplicate `(coin_id, hint)` pairs are silently
//! ignored via insert-or-ignore semantics. `add_hint()` validates via HNT-001, checks
//! for existing forward key, and writes both forward and reverse indices only when new.
//!
//! # Requirement: HNT-002
//! # Spec: docs/requirements/domains/hints/specs/HNT-002.md
//! # SPEC.md: §1.5 #14 (Idempotent hint insertion)
//!
//! ## How these tests prove the requirement
//!
//! - **Double insert returns Ok both times:** `add_hint` succeeds on first and second call.
//! - **After double insert, query returns hint exactly once:** No duplicate entries.
//! - **Same coin_id with different hints creates distinct entries:** Each (coin_id, hint) pair is independent.
//! - **Different coin_ids with same hint creates distinct entries:** Reverse index tracks all coins per hint.
//! - **No panic on duplicate insertion:** Stress test with repeated inserts.
//! - **Empty hint silently skipped via add_hint:** Validates HNT-001 integration in add_hint path.

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::Bytes32;

/// **HNT-002:** Double insert of the same (coin_id, hint) pair returns Ok both times.
///
/// **Proof:** Idempotent insertion means re-inserting an existing hint is a no-op,
/// not an error. Both calls must return Ok(()).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_002_double_insert_returns_ok() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0xAA; 32]);
    let hint = [0xBB_u8; 32];

    let result1 = store.add_hint(&coin_id, &hint);
    assert!(result1.is_ok(), "First insert must succeed");

    let result2 = store.add_hint(&coin_id, &hint);
    assert!(
        result2.is_ok(),
        "Second (duplicate) insert must also succeed"
    );
}

/// **HNT-002:** After double insert, the hint appears exactly once in query results.
///
/// **Proof:** Insert the same (coin_id, hint) twice, then query by hint. The coin_id
/// must appear exactly once, proving no duplicate rows were created.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_002_double_insert_query_returns_once() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0xAA; 32]);
    let hint = Bytes32::from([0xBB; 32]);

    store.add_hint(&coin_id, hint.as_ref()).unwrap();
    store.add_hint(&coin_id, hint.as_ref()).unwrap();

    let results = store.get_coin_ids_by_hint(&hint, 100).unwrap();
    assert_eq!(
        results.len(),
        1,
        "Duplicate insert must not create extra entries; expected 1 result, got {}",
        results.len()
    );
    assert_eq!(results[0], coin_id);
}

/// **HNT-002:** Same coin_id with different hints creates distinct entries.
///
/// **Proof:** Insert two different hints for the same coin_id. Both should be stored
/// as separate entries in the forward index.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_002_same_coin_different_hints_distinct() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0xAA; 32]);
    let hint_a = [0xBB_u8; 32];
    let hint_b = [0xCC_u8; 32];

    store.add_hint(&coin_id, &hint_a).unwrap();
    store.add_hint(&coin_id, &hint_b).unwrap();

    let count = store.count_hints().unwrap();
    assert_eq!(
        count, 2,
        "Two different hints for the same coin must create 2 forward-index entries"
    );
}

/// **HNT-002:** Different coin_ids with the same hint creates distinct entries.
///
/// **Proof:** Insert the same hint for two different coin_ids. Both should be stored
/// as separate entries in both indices.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_002_different_coins_same_hint_distinct() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = Bytes32::from([0xAA; 32]);
    let coin_b = Bytes32::from([0xBB; 32]);
    let hint = [0xCC_u8; 32];

    store.add_hint(&coin_a, &hint).unwrap();
    store.add_hint(&coin_b, &hint).unwrap();

    let count = store.count_hints().unwrap();
    assert_eq!(
        count, 2,
        "Same hint for two different coins must create 2 forward-index entries"
    );
}

/// **HNT-002:** No panic on repeated duplicate insertion.
///
/// **Proof:** Insert the same (coin_id, hint) 10 times — all must succeed without panic.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_002_no_panic_on_repeated_duplicates() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x11; 32]);
    let hint = [0x22_u8; 32];

    for _ in 0..10 {
        let result = store.add_hint(&coin_id, &hint);
        assert!(
            result.is_ok(),
            "Repeated duplicate insert must not panic or error"
        );
    }

    let count = store.count_hints().unwrap();
    assert_eq!(
        count, 1,
        "Only one entry despite 10 inserts of the same pair"
    );
}

/// **HNT-002:** Empty hint is silently skipped via add_hint (HNT-001 integration).
///
/// **Proof:** Calling add_hint with an empty hint returns Ok but stores nothing.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_hnt_002_empty_hint_skipped_via_add_hint() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_id = Bytes32::from([0x33; 32]);
    let result = store.add_hint(&coin_id, &[]);
    assert!(result.is_ok(), "Empty hint must be silently skipped");

    let count = store.count_hints().unwrap();
    assert_eq!(count, 0, "Empty hint must not create any entries");
}
