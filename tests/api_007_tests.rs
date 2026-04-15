//! # API-007 Tests — [`CoinStoreStats`] and [`CoinStore::stats`]
//!
//! Dedicated integration tests for requirement **API-007**: the nine-field statistics snapshot returned
//! from [`dig_coinstore::coin_store::CoinStore::stats`], per
//! [`docs/requirements/domains/crate_api/NORMATIVE.md`](../../docs/requirements/domains/crate_api/NORMATIVE.md#API-007)
//! and [`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) §3.12.
//!
//! # Requirement: API-007
//! # Spec: docs/requirements/domains/crate_api/specs/API-007.md
//!
//! ## How these tests prove API-007
//!
//! - **Struct surface (NORMATIVE):** A struct literal touching every public field must compile; renaming
//!   or privatizing a field breaks compilation immediately.
//! - **Crate re-export:** Imports use [`CoinStoreStats`] from the **crate root** (`dig_coinstore::CoinStoreStats`)
//!   alongside [`dig_coinstore::coin_store::CoinStore`], matching STR-005 / API export expectations.
//! - **Fresh store:** [`API-007.md` test plan](docs/requirements/domains/crate_api/specs/API-007.md#verification)
//!   expects zeros and stable empty-tree `state_root` before genesis.
//! - **Post-genesis counts:** After [`CoinStore::init_genesis`] with N coins, `unspent_count == N`,
//!   `spent_count == 0`, `height == 0`, and `total_unspent_value` equals the sum of inserted amounts —
//!   this proves [`CoinStore::stats`] reads the same rows [`init_genesis`](CoinStore::init_genesis) wrote
//!   (including the legacy 97-byte layout until STO-008 standardizes bincode [`CoinRecord`] everywhere).
//! - **Accessor parity:** `stats().height`, `tip_hash`, `timestamp`, and `state_root` match the existing
//!   accessors / Merkle snapshot so callers see one coherent view of the tip.
//! - **Serde:** Bincode round-trip on [`CoinStoreStats`] supports future RPC / persistence paths without
//!   a second schema definition.
//!
//! **SocratiCode:** Not available in this environment (no MCP). **Repomix / GitNexus:** run per
//! `docs/prompt/start.md` before editing production sources.

mod helpers;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::merkle::{empty_hash, SMT_HEIGHT};
use dig_coinstore::{Bytes32, CoinStoreStats};

/// **Acceptance:** All nine [`CoinStoreStats`] fields are public and assignable (NORMATIVE API-007).
///
/// **Proof:** Field-by-field construction and equality — any drift from the normative type table fails here.
#[test]
fn vv_req_api_007_coin_store_stats_struct_all_fields_accessible() {
    let s = CoinStoreStats {
        height: 9,
        timestamp: 88,
        unspent_count: 10,
        spent_count: 2,
        total_unspent_value: 1_234,
        state_root: Bytes32::from([0xAB; 32]),
        tip_hash: Bytes32::from([0xCD; 32]),
        hint_count: 3,
        snapshot_count: 4,
    };
    assert_eq!(s.height, 9);
    assert_eq!(s.timestamp, 88);
    assert_eq!(s.unspent_count, 10);
    assert_eq!(s.spent_count, 2);
    assert_eq!(s.total_unspent_value, 1_234);
    assert_eq!(s.state_root, Bytes32::from([0xAB; 32]));
    assert_eq!(s.tip_hash, Bytes32::from([0xCD; 32]));
    assert_eq!(s.hint_count, 3);
    assert_eq!(s.snapshot_count, 4);
}

/// **Test plan:** Fresh store before genesis — all aggregates zero, tip hashes zeroed, root is empty SMT.
///
/// **Proof:** Matches API-007 verification table (“Fresh store stats”) and documents `stats` does not
/// require initialization to be called safely.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_007_stats_fresh_uninitialized_all_zeros() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    let s = store.stats();
    assert_eq!(s.height, 0);
    assert_eq!(s.timestamp, 0);
    assert_eq!(s.unspent_count, 0);
    assert_eq!(s.spent_count, 0);
    assert_eq!(s.total_unspent_value, 0);
    assert_eq!(s.tip_hash, Bytes32::from([0u8; 32]));
    assert_eq!(s.hint_count, 0);
    assert_eq!(s.snapshot_count, 0);
    assert_eq!(
        s.state_root,
        empty_hash(SMT_HEIGHT),
        "empty Merkle tree root (MRK-002)"
    );
}

/// **Test plan:** Empty genesis — store initialized but zero coins; counts remain zero; height still 0.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_007_stats_after_empty_genesis() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();
    let s = store.stats();
    assert_eq!(s.unspent_count, 0);
    assert_eq!(s.spent_count, 0);
    assert_eq!(s.total_unspent_value, 0);
    assert_eq!(s.height, 0);
    assert_eq!(s.timestamp, store.timestamp());
}

/// **Test plan:** `init_genesis` with three coins — `unspent_count == 3`, `spent_count == 0`, value sum exact.
///
/// **Proof:** Validates decode path for genesis bytes + Merkle alignment with [`CoinStore::init_genesis`].
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_007_stats_after_genesis_three_coins() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let c3 = helpers::test_coin(5, 6, 300);
    store
        .init_genesis(vec![(c1, false), (c2, true), (c3, false)], 1_700_000_001)
        .unwrap();

    let s = store.stats();
    assert_eq!(s.unspent_count, 3, "three live rows in coin_records");
    assert_eq!(s.spent_count, 0);
    assert_eq!(s.total_unspent_value, 600, "100 + 200 + 300 mojos");
    assert_eq!(s.height, 0);
    assert_eq!(s.timestamp, 1_700_000_001);
}

/// **Proof:** [`CoinStore::stats`] mirrors scalar accessors and [`CoinStore::state_root`] for the same open store.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_007_stats_matches_accessors_and_state_root() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store
        .init_genesis(vec![(helpers::test_coin(9, 8, 42), false)], 1_700_000_002)
        .unwrap();

    let s = store.stats();
    assert_eq!(s.height, store.height());
    assert_eq!(s.timestamp, store.timestamp());
    assert_eq!(s.tip_hash, store.tip_hash());
    assert_eq!(s.state_root, store.state_root());
}

/// **Test plan:** Clone + Debug + bincode round-trip (API-007 implementation notes).
#[test]
fn vv_req_api_007_coin_store_stats_clone_debug_bincode() {
    let s = CoinStoreStats {
        height: 1,
        timestamp: 2,
        unspent_count: 3,
        spent_count: 4,
        total_unspent_value: 5,
        state_root: Bytes32::from([1u8; 32]),
        tip_hash: Bytes32::from([2u8; 32]),
        hint_count: 6,
        snapshot_count: 7,
    };
    let c = s.clone();
    assert_eq!(c, s);
    let dbg = format!("{s:?}");
    assert!(!dbg.is_empty(), "Debug must be non-empty for ops logging");

    let bytes = bincode::serialize(&s).expect("serialize CoinStoreStats");
    let back: CoinStoreStats = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back, s);
}
