//! # API-007 Tests — [`CoinStoreStats`] and [`CoinStore::stats`]
//!
//! Verifies requirement **API-007**: the nine-field [`dig_coinstore::CoinStoreStats`] snapshot type and
//! infallible [`dig_coinstore::coin_store::CoinStore::stats`] entry point per
//! [NORMATIVE API-007](../../docs/requirements/domains/crate_api/NORMATIVE.md#API-007) and
//! [`API-007.md`](../../docs/requirements/domains/crate_api/specs/API-007.md).
//!
//! # How this proves the requirement
//!
//! - **Struct surface:** Literal construction plus per-field assertions fail to compile if any field is
//!   missing, private, or mis-typed (NORMATIVE MUST list).
//! - **Fresh store:** [`CoinStore::stats`] on an uninitialized store must expose zero counts and the
//!   same sentinel hashes as accessors / empty Merkle tree (API-007 Test Plan: “Fresh store stats”).
//! - **Genesis:** After [`CoinStore::init_genesis`] with **N** coins, `unspent_count == N`, `spent_count == 0`,
//!   `height == 0`, and `total_unspent_value` equals the sum of coin amounts — proves the scan over
//!   `coin_records` matches [`CoinStore::serialize_genesis_record`] encoding (until PRF-003 counters).
//! - **Persistence:** Re-opening the same directory and calling `stats()` again proves aggregates survive
//!   process restart (same invariant as API-001 reopen, now through the stats API).
//! - **Merkle parity:** `stats().state_root` matches [`CoinStore::state_root`] after genesis so
//!   [`SparseMerkleTree::root_observed`](dig_coinstore::merkle::SparseMerkleTree::root_observed) stays
//!   consistent with the mutable [`state_root`](dig_coinstore::coin_store::CoinStore::state_root) path.
//! - **Placeholder indices:** `hint_count` / `snapshot_count` stay `0` until HNT-* / PRF-008 — documented
//!   here so future work cannot silently regress defaults.
//! - **Ergonomics:** `Clone` + `Debug` coverage from API-007 Test Plan.
//!
//! **Deferred:** Stats after real `apply_block` / rollback / hints / snapshots follow BLK, RBK, HNT,
//! and PRF requirements; this file only covers what exists today.
//!
//! # Requirement: API-007
//! # Spec: docs/requirements/domains/crate_api/specs/API-007.md

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::merkle::{empty_hash, SMT_HEIGHT};
use dig_coinstore::{Bytes32, CoinStoreStats};

/// Sentinel used for `tip_hash` before BLK writes a real tip hash (matches genesis metadata in `coin_store`).
fn zero_hash() -> Bytes32 {
    Bytes32::from([0u8; 32])
}

/// Verifies API-007: all nine [`CoinStoreStats`] fields are public and accept the expected types.
///
/// **Proof:** Struct literal + field reads; removing or renaming a field breaks compilation.
#[test]
fn vv_req_api_007_stats_struct_all_fields_accessible() {
    let s = CoinStoreStats {
        height: 9,
        timestamp: 88,
        unspent_count: 100,
        spent_count: 5,
        total_unspent_value: 1_234_567,
        state_root: helpers::filled_hash(0x11),
        tip_hash: helpers::filled_hash(0x22),
        hint_count: 3,
        snapshot_count: 7,
    };
    assert_eq!(s.height, 9);
    assert_eq!(s.timestamp, 88);
    assert_eq!(s.unspent_count, 100);
    assert_eq!(s.spent_count, 5);
    assert_eq!(s.total_unspent_value, 1_234_567);
    assert_eq!(s.state_root, helpers::filled_hash(0x11));
    assert_eq!(s.tip_hash, helpers::filled_hash(0x22));
    assert_eq!(s.hint_count, 3);
    assert_eq!(s.snapshot_count, 7);
}

/// Verifies API-007 Test Plan: `stats()` on a **new**, uninitialized store returns zeros / sentinel hashes.
///
/// **Proof:** No genesis ⇒ no coin rows scanned; Merkle tree is the empty SMT from `with_config`.
#[test]
fn vv_req_api_007_stats_fresh_store_defaults() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    let s = store.stats();

    assert_eq!(s.height, 0);
    assert_eq!(s.timestamp, 0);
    assert_eq!(s.unspent_count, 0);
    assert_eq!(s.spent_count, 0);
    assert_eq!(s.total_unspent_value, 0);
    assert_eq!(s.tip_hash, zero_hash());
    assert_eq!(s.hint_count, 0);
    assert_eq!(s.snapshot_count, 0);
    // Empty sparse Merkle root (MRK-002) — same as `SparseMerkleTree::new()` before any inserts.
    assert_eq!(s.state_root, empty_hash(SMT_HEIGHT));
}

/// Verifies API-007 acceptance: after genesis with **N** coins, counts and value match expectations.
#[test]
fn vv_req_api_007_stats_after_genesis_three_coins() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 1, 100);
    let c2 = helpers::test_coin(2, 2, 200);
    let c3 = helpers::test_coin(3, 3, 300);
    let ts = 1_700_000_123u64;
    store
        .init_genesis(vec![(c1, false), (c2, true), (c3, false)], ts)
        .expect("genesis");

    let s = store.stats();
    assert_eq!(s.height, 0, "genesis tip height");
    assert_eq!(s.timestamp, ts);
    assert_eq!(s.unspent_count, 3, "API-007: N genesis coins => N unspent");
    assert_eq!(s.spent_count, 0);
    assert_eq!(s.total_unspent_value, 600, "100 + 200 + 300 mojos");
    assert_eq!(s.tip_hash, zero_hash());
    assert_eq!(s.hint_count, 0);
    assert_eq!(s.snapshot_count, 0);
    assert_eq!(s.state_root, store.state_root(), "root_observed vs root()");
}

/// Verifies API-007: aggregates survive reopen (disk-backed truth source).
#[test]
fn vv_req_api_007_stats_survives_reopen() {
    let dir = helpers::temp_dir();
    let path = dir.path().to_path_buf();
    let c1 = helpers::test_coin(10, 11, 50);
    let c2 = helpers::test_coin(12, 13, 70);
    {
        let mut store = CoinStore::new(&path).unwrap();
        store
            .init_genesis(vec![(c1, false), (c2, false)], 999)
            .unwrap();
        let s = store.stats();
        assert_eq!(s.unspent_count, 2);
        assert_eq!(s.total_unspent_value, 120);
    }
    let mut store2 = CoinStore::new(&path).unwrap();
    let s2 = store2.stats();
    assert_eq!(s2.unspent_count, 2);
    assert_eq!(s2.total_unspent_value, 120);
    assert_eq!(s2.state_root, store2.state_root());
}

/// Verifies API-007 Test Plan: `Clone` preserves all field values.
#[test]
fn vv_req_api_007_stats_clone_round_trip() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store
        .init_genesis(vec![(helpers::test_coin(4, 5, 10), false)], 1)
        .unwrap();
    let a = store.stats();
    let b = a.clone();
    assert_eq!(a, b);
}

/// Verifies API-007 Test Plan: `Debug` output is non-empty (operators / logs can stringify stats).
#[test]
fn vv_req_api_007_stats_debug_non_empty() {
    let s = CoinStoreStats {
        height: 1,
        timestamp: 2,
        unspent_count: 3,
        spent_count: 4,
        total_unspent_value: 5,
        state_root: zero_hash(),
        tip_hash: zero_hash(),
        hint_count: 0,
        snapshot_count: 0,
    };
    let dbg = format!("{s:?}");
    assert!(
        dbg.contains("CoinStoreStats") && dbg.len() > 20,
        "unexpected Debug: {dbg}"
    );
}
