//! # API-002 Tests — `CoinRecord` and `ChiaCoinRecord`
//!
//! Dedicated test file for requirement **API-002**: native coin row + Chia interop conversions.
//!
//! # Requirement: API-002
//! # Spec: docs/requirements/domains/crate_api/specs/API-002.md
//! # NORMATIVE: docs/requirements/domains/crate_api/NORMATIVE.md#API-002
//! # SPEC.md: Section 2.2
//!
//! [`ChiaCoinRecord`](dig_coinstore::ChiaCoinRecord) mirrors the streamable layout documented at
//! <https://docs.rs/chia-protocol/latest/chia_protocol/struct.CoinRecord.html> while this repo stays
//! on `chia-protocol` 0.26 via `dig-clvm` (see module docs on `src/types.rs`).

mod helpers;

use dig_coinstore::{ChiaCoinRecord, CoinId, CoinRecord, CoinState};

// ─────────────────────────────────────────────────────────────────────────────
// Field accessibility & constructors
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies API-002 / Test Plan: all six [`CoinRecord`] fields are public and readable.
///
/// **Proof:** If any field were missing or private, this struct literal would not compile.
/// Exercising each field in assertions ties them to expected test values from helpers.
#[test]
fn vv_req_api_002_all_fields_accessible() {
    let coin = helpers::test_coin(1, 2, 3);
    let r = CoinRecord {
        coin,
        confirmed_height: 10,
        spent_height: Some(20),
        coinbase: true,
        timestamp: 1_700_000_001,
        ff_eligible: true,
    };
    assert_eq!(r.confirmed_height, 10);
    assert_eq!(r.spent_height, Some(20));
    assert!(r.coinbase);
    assert_eq!(r.timestamp, 1_700_000_001);
    assert!(r.ff_eligible);
    assert_eq!(r.coin, coin);
}

/// Verifies API-002: [`CoinRecord::new`] produces an **unspent** row with `ff_eligible == false`.
///
/// **Proof:** [`CoinRecord::is_spent`] is defined as `spent_height.is_some()` (spec); new coins must
/// report `false`. FF flag defaults false until `apply_block` sets it (API-005 linkage).
#[test]
fn vv_req_api_002_new_creates_unspent_ff_false() {
    let coin = helpers::test_coin(7, 8, 9);
    let r = CoinRecord::new(coin, 100, 1_234, false);
    assert!(!r.is_spent(), "new() must leave spent_height unset");
    assert_eq!(r.spent_height, None);
    assert!(!r.ff_eligible);
    assert_eq!(r.confirmed_height, 100);
    assert_eq!(r.timestamp, 1_234);
}

/// Verifies API-002: [`CoinRecord::spend`] records height and flips [`CoinRecord::is_spent`].
#[test]
fn vv_req_api_002_spend_marks_spent() {
    let mut r = CoinRecord::new(helpers::test_coin(1, 1, 1), 1, 0, false);
    r.spend(500);
    assert_eq!(r.spent_height, Some(500));
    assert!(r.is_spent());
}

/// Verifies API-002: [`CoinRecord::coin_id`] matches [`Coin::coin_id`] on the same [`Coin`].
///
/// **Proof:** Requirement mandates delegating ID computation to Chia’s implementation (no custom hash).
#[test]
fn vv_req_api_002_coin_id_matches_coin() {
    let coin = helpers::test_coin(11, 22, 33);
    let r = CoinRecord::new(coin, 5, 0, false);
    assert_eq!(r.coin_id(), coin.coin_id());
    let _: CoinId = r.coin_id();
}

/// Verifies API-002: [`CoinRecord::to_coin_state`] maps heights into [`CoinState`] options.
///
/// **Proof:** For height 42 and spend 100, the sync view must show `created_height == Some(42)`,
/// `spent_height == Some(100)`, and the same [`Coin`].
#[test]
fn vv_req_api_002_to_coin_state_maps_heights() {
    let coin = helpers::test_coin(3, 3, 3);
    let mut r = CoinRecord::new(coin, 42, 0, false);
    r.spend(100);
    let cs: CoinState = r.to_coin_state();
    assert_eq!(cs.coin, coin);
    assert_eq!(cs.created_height, Some(42));
    assert_eq!(cs.spent_height, Some(100));
}

/// Verifies API-002 / Test Plan: [`Clone`] duplicates all fields.
#[test]
fn vv_req_api_002_clone_round_trip_fields() {
    let r1 = CoinRecord::new(helpers::test_coin(9, 9, 9), 9, 9, true);
    let r2 = r1.clone();
    assert_eq!(r1, r2);
}

/// Verifies API-002 / Test Plan: `bincode` serialize + deserialize round-trips the struct.
///
/// **Why bincode:** STO-008 will persist [`CoinRecord`] in KV values; this proves derives are valid
/// for the storage codec already declared in Cargo.toml.
#[test]
fn vv_req_api_002_serde_bincode_roundtrip() {
    let original = CoinRecord::new(helpers::test_coin(4, 5, 6), 77, 88, false);
    let bytes = bincode::serialize(&original).expect("bincode serialize");
    let back: CoinRecord = bincode::deserialize(&bytes).expect("bincode deserialize");
    assert_eq!(original, back);
}

/// Verifies API-002 / Test Plan: calling [`CoinRecord::spend`] again overwrites `spent_height` (no struct-level guard).
#[test]
fn vv_req_api_002_double_spend_overwrites_height() {
    let mut r = CoinRecord::new(helpers::test_coin(1, 2, 3), 1, 0, false);
    r.spend(10);
    r.spend(99);
    assert_eq!(r.spent_height, Some(99));
}

/// Verifies API-002 / Test Plan: `confirmed_height == u32::MAX` casts cleanly to [`CoinState`].
///
/// **Proof:** Confirms we do not silently corrupt the boundary when widening/narrowing; `debug_assert!`
/// in `to_coin_state` would trip in debug if truncation occurred on `confirmed_height` itself.
#[test]
fn vv_req_api_002_to_coin_state_u32_boundary() {
    let h = u64::from(u32::MAX);
    let r = CoinRecord::new(helpers::test_coin(1, 1, 1), h, 0, false);
    let cs = r.to_coin_state();
    assert_eq!(cs.created_height, Some(u32::MAX));
}

// ─────────────────────────────────────────────────────────────────────────────
// Chia interop (`ChiaCoinRecord`)
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies API-002: `spent_block_index == 0` maps to [`None`] `spent_height`, `ff_eligible` reset.
#[test]
fn vv_req_api_002_from_chia_unspent() {
    let coin = helpers::test_coin(5, 5, 5);
    let chia = ChiaCoinRecord::new(coin, 300, 0, false, 1_111);
    let r = CoinRecord::from_chia_coin_record(chia);
    assert_eq!(r.spent_height, None);
    assert!(!r.is_spent());
    assert!(!r.ff_eligible);
    assert_eq!(r.confirmed_height, 300);
    assert_eq!(r.timestamp, 1_111);
    assert_eq!(r.coin, coin);
}

/// Verifies API-002: `spent_block_index > 0` maps to [`Some`] spend height as `u64`.
#[test]
fn vv_req_api_002_from_chia_spent() {
    let coin = helpers::test_coin(6, 6, 6);
    let chia = ChiaCoinRecord::new(coin, 1, 500, false, 2_222);
    let r = CoinRecord::from_chia_coin_record(chia);
    assert_eq!(r.spent_height, Some(500));
    assert!(r.is_spent());
}

/// Verifies API-002: [`CoinRecord::to_chia_coin_record`] narrows heights and maps `None` spend to `0`.
#[test]
fn vv_req_api_002_to_chia_coin_record() {
    let coin = helpers::test_coin(7, 7, 7);
    let mut r = CoinRecord::new(coin, 50, 0, true);
    r.spend(60);
    let chia = r.to_chia_coin_record();
    assert_eq!(chia.confirmed_block_index, 50);
    assert_eq!(chia.spent_block_index, 60);
    assert!(chia.coinbase);
    assert_eq!(chia.coin, coin);
}

/// Verifies API-002: `from_chia(to_chia(x))` preserves all fields except `ff_eligible` (wire has no bit).
#[test]
fn vv_req_api_002_chia_roundtrip_resets_ff_eligible() {
    let coin = helpers::test_coin(8, 8, 8);
    let mut native = CoinRecord::new(coin, 1000, 0, false);
    native.ff_eligible = true;
    native.spend(1001);
    let roundtrip = CoinRecord::from_chia_coin_record(native.to_chia_coin_record());
    assert_eq!(roundtrip.coin, native.coin);
    assert_eq!(roundtrip.confirmed_height, native.confirmed_height);
    assert_eq!(roundtrip.spent_height, native.spent_height);
    assert_eq!(roundtrip.coinbase, native.coinbase);
    assert_eq!(roundtrip.timestamp, native.timestamp);
    assert!(
        !roundtrip.ff_eligible,
        "ff_eligible is not carried on Chia row; must default false after roundtrip"
    );
}
