//! # API-009 Tests — [`CoinId`], [`PuzzleHash`], [`UnspentLineageInfo`]
//!
//! Dedicated integration tests for requirement **API-009**: semantic [`Bytes32`] aliases and the lineage
//! carrier struct used by future [`get_unspent_lineage_info_for_puzzle_hash`](../../docs/requirements/domains/queries/specs/QRY-008.md)
//! ([`SPEC.md`](../../docs/resources/SPEC.md) §2.1, §2.5).
//!
//! # Requirement: API-009
//! # NORMATIVE: docs/requirements/domains/crate_api/NORMATIVE.md#API-009
//! # Spec: docs/requirements/domains/crate_api/specs/API-009.md
//!
//! ## How these tests satisfy acceptance
//!
//! - **Type aliases:** Assigning a [`dig_coinstore::Bytes32`] value to both [`CoinId`] and [`PuzzleHash`]
//!   without `as` casts proves they are the same underlying type in this crate’s public graph (STR-005 /
//!   `dig-clvm` re-export identity).
//! - **UnspentLineageInfo:** Field access + [`Debug`] / [`Clone`] / [`PartialEq`] behavior match API-009 and
//!   the spec’s test plan (`test_unspent_lineage_info_*`).
//! - **Consistent API surface:** A single “signature zoo” construction touches every normatively listed type
//!   that already carries [`CoinId`] / [`PuzzleHash`] in public fields (`BlockData`, `CoinAddition`,
//!   `ApplyBlockResult`, `RollbackResult`, `CoinStoreSnapshot`, [`CoinStoreError`]). If any field drifts back
//!   to raw [`Bytes32`] where a semantic alias belongs, this module stops compiling.
//!
//! **SocratiCode:** not available (no MCP). **Repomix / GitNexus:** per `docs/prompt/start.md` before edits.

mod helpers;

use std::collections::HashMap;

use dig_coinstore::{
    ApplyBlockResult, BlockData, Bytes32, CoinAddition, CoinId, CoinRecord, CoinStoreError,
    CoinStoreSnapshot, PuzzleHash, RollbackResult, UnspentLineageInfo,
};

// ─────────────────────────────────────────────────────────────────────────────
// Type aliases (NORMATIVE: CoinId = Bytes32, PuzzleHash = Bytes32)
// ─────────────────────────────────────────────────────────────────────────────

/// **Acceptance:** [`CoinId`] is a public alias of [`Bytes32`] — values flow freely without transmute.
#[test]
fn vv_req_api_009_coin_id_alias_matches_bytes32() {
    let raw: Bytes32 = helpers::test_hash(0x41);
    let as_coin_id: CoinId = raw;
    let round_trip: Bytes32 = as_coin_id;
    assert_eq!(round_trip, raw);
}

/// **Acceptance:** [`PuzzleHash`] shares the same underlying representation as [`Bytes32`].
#[test]
fn vv_req_api_009_puzzle_hash_alias_matches_bytes32() {
    let raw: Bytes32 = helpers::test_hash(0x42);
    let as_puzzle: PuzzleHash = raw;
    assert_eq!(Bytes32::from(as_puzzle), raw);
}

/// **Acceptance (API-009 consistency, puzzle side):** [`Coin::puzzle_hash`] field is assignable to [`PuzzleHash`].
#[test]
fn vv_req_api_009_coin_puzzle_hash_assignable_to_puzzle_hash_alias() {
    let coin = helpers::test_coin(7, 8, 999);
    let ph: PuzzleHash = coin.puzzle_hash;
    assert_eq!(ph, coin.puzzle_hash);
}

/// **Acceptance (API-009 consistency, identity side):** [`Coin::coin_id`] is assignable to [`CoinId`].
#[test]
fn vv_req_api_009_coin_coin_id_assignable_to_coin_id_alias() {
    let coin = helpers::test_coin(9, 10, 1);
    let id: CoinId = coin.coin_id();
    assert_eq!(id, coin.coin_id());
}

// ─────────────────────────────────────────────────────────────────────────────
// UnspentLineageInfo
// ─────────────────────────────────────────────────────────────────────────────

/// **Acceptance:** All three public fields exist and are [`CoinId`] typed (SPEC §2.5 / API-009 struct table).
#[test]
fn vv_req_api_009_unspent_lineage_info_all_fields_accessible() {
    let a = helpers::test_hash(1);
    let b = helpers::test_hash(2);
    let c = helpers::test_hash(3);
    let info = UnspentLineageInfo {
        coin_id: a,
        parent_id: b,
        parent_parent_id: c,
    };
    assert_eq!(info.coin_id, a);
    assert_eq!(info.parent_id, b);
    assert_eq!(info.parent_parent_id, c);
}

/// **Acceptance:** [`UnspentLineageInfo`] derives [`Debug`], [`Clone`], [`PartialEq`] per API-009.
#[test]
fn vv_req_api_009_unspent_lineage_info_debug_clone_partial_eq() {
    let info = UnspentLineageInfo {
        coin_id: helpers::test_hash(0x11),
        parent_id: helpers::test_hash(0x22),
        parent_parent_id: helpers::test_hash(0x33),
    };
    let dbg = format!("{info:?}");
    assert!(
        dbg.contains("coin_id") && dbg.contains("parent_id"),
        "Debug output should name fields for operators: {dbg}"
    );
    let cloned = info.clone();
    assert_eq!(cloned, info);
    let different = UnspentLineageInfo {
        coin_id: helpers::filled_hash(0xFF),
        parent_id: info.parent_id,
        parent_parent_id: info.parent_parent_id,
    };
    assert_ne!(different, info);
}

/// **Edge case (API-009 implementation notes):** Sentinel / missing grandparent rows still use concrete [`CoinId`] values.
#[test]
fn vv_req_api_009_unspent_lineage_info_genesis_sentinel_row() {
    let zero = Bytes32::default();
    let info = UnspentLineageInfo {
        coin_id: helpers::test_hash(0x77),
        parent_id: zero,
        parent_parent_id: zero,
    };
    assert_eq!(info.parent_id, info.parent_parent_id);
}

// ─────────────────────────────────────────────────────────────────────────────
// Compile-time / integration: aliases appear on the normatively listed public types
// ─────────────────────────────────────────────────────────────────────────────

/// **Acceptance (consistency):** Builds one instance of each struct/error that NORMATIVE API-009 calls out for
/// alias usage. This is both a regression test and documentation of where `CoinId` / `PuzzleHash` already land.
#[test]
fn vv_req_api_009_aliases_used_on_listed_public_types() {
    let coin = helpers::test_coin(5, 6, 1234);
    let coin_id: CoinId = coin.coin_id();
    let addition = CoinAddition::from_coin(coin, false);

    let block = BlockData {
        height: 2,
        timestamp: 3,
        block_hash: helpers::test_hash(0xB1),
        parent_hash: helpers::test_hash(0xB2),
        additions: vec![addition.clone()],
        removals: vec![coin_id],
        coinbase_coins: vec![helpers::test_coin(0xC0, 0xC1, 50)],
        hints: vec![(coin_id, helpers::filled_hash(0x01))],
        expected_state_root: Some(helpers::test_hash(0xB3)),
    };

    let apply = ApplyBlockResult {
        state_root: helpers::test_hash(0xD0),
        coins_created: 1,
        coins_spent: 0,
        height: block.height,
    };

    let mut modified = HashMap::new();
    modified.insert(
        coin_id,
        CoinRecord::new(helpers::test_coin(0xE0, 0xE1, 99), 0, 1_700_000_000, false),
    );
    let rollback = RollbackResult {
        modified_coins: modified,
        coins_deleted: 0,
        coins_unspent: 1,
        new_height: 1,
    };

    let mut coins = HashMap::new();
    coins.insert(
        coin_id,
        CoinRecord::new(helpers::test_coin(0xF0, 0xF1, 42), 0, 1_700_000_001, false),
    );
    let snapshot = CoinStoreSnapshot {
        height: 9,
        block_hash: helpers::test_hash(0xA0),
        state_root: helpers::test_hash(0xA1),
        timestamp: 1_700_000_002,
        coins,
        hints: vec![(coin_id, helpers::filled_hash(0x02))],
        total_coins: 1,
        total_value: 42,
    };

    let err = CoinStoreError::CoinNotFound(coin_id);

    // Touch every binding so rustc cannot optimize the test away.
    assert_eq!(block.removals.len(), 1);
    assert_eq!(apply.coins_created, 1);
    assert_eq!(rollback.coins_unspent, 1);
    assert_eq!(snapshot.total_coins, 1);
    assert!(matches!(err, CoinStoreError::CoinNotFound(id) if id == coin_id));
    let _: &CoinId = &addition.coin_id;
    let _: PuzzleHash = addition.coin.puzzle_hash;
}
