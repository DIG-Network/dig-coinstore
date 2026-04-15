//! # API-005 Tests — `BlockData` and `CoinAddition`
//!
//! Verifies requirement **API-005**: public [`dig_coinstore::BlockData`] with nine fields and
//! [`dig_coinstore::CoinAddition`] with three fields, matching
//! [`NORMATIVE.md#API-005`](../../docs/requirements/domains/crate_api/NORMATIVE.md#API-005) and
//! [`API-005.md`](../../docs/requirements/domains/crate_api/specs/API-005.md).
//!
//! # How this proves the requirement
//!
//! - **Field coverage:** Each test constructs values with **struct literals** referencing every public
//!   field. If a field were missing, renamed, or not `pub`, these tests would fail to compile.
//! - **Type shapes:** Assertions pin `additions: Vec<CoinAddition>`, `removals: Vec<CoinId>`,
//!   `hints: Vec<(CoinId, Bytes32)>`, `expected_state_root: Option<Bytes32>`, and `coinbase_coins:
//!   Vec<Coin>` so accidental type drift is caught at compile time and at runtime where exercised.
//! - **Serde / bincode:** Optional roots and nested `Vec`s must round-trip through `bincode` for
//!   STO-008-style persistence; we mirror the API-002 pattern for [`dig_coinstore::CoinRecord`].
//! - **Scope boundary:** Mismatch between [`CoinAddition::coin_id`] and [`Coin::coin_id()`] is
//!   allowed at the type level (caller/pipeline responsibility per API-005 test plan); we document
//!   that with one explicit construction test until BLK-* enforces validation.
//!
//! Integration scenarios from API-005 (duplicate additions, same-block add+remove, `apply_block`
//! state-root checks) belong with **BLK-*** once `CoinStore::apply_block` exists; they are out of
//! scope for this type-only requirement.
//!
//! # Requirement: API-005
//! # Spec: docs/requirements/domains/crate_api/specs/API-005.md

mod helpers;

use dig_coinstore::{BlockData, Bytes32, CoinAddition, CoinId};

// ─────────────────────────────────────────────────────────────────────────────
// BlockData — nine public fields
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies API-005 / VERIFICATION table: every [`BlockData`] field is public and readable.
///
/// **Proof:** This builds a fully populated block payload and reads each field back. Missing or
/// private fields make the literal or accessors fail at compile time; wrong values fail assertions.
#[test]
fn vv_req_api_005_block_data_all_nine_fields_accessible() {
    let coin = helpers::test_coin(9, 8, 7);
    let coin_id: CoinId = coin.coin_id();
    let addition = CoinAddition {
        coin_id,
        coin,
        same_as_parent: true,
    };
    let removal_id: CoinId = helpers::test_coin(3, 4, 5).coin_id();
    let hint_target: CoinId = helpers::test_coin(1, 2, 3).coin_id();
    let hint_payload = helpers::filled_hash(0xEE);

    let block_hash = helpers::filled_hash(0x11);
    let parent_hash = helpers::filled_hash(0x22);
    let expected_root = Some(helpers::filled_hash(0x33));

    let block = BlockData {
        height: 42,
        timestamp: 1_700_000_042,
        block_hash,
        parent_hash,
        additions: vec![addition.clone()],
        removals: vec![removal_id],
        coinbase_coins: vec![helpers::test_coin(5, 6, 100)],
        hints: vec![(hint_target, hint_payload)],
        expected_state_root: expected_root,
    };

    assert_eq!(block.height, 42);
    assert_eq!(block.timestamp, 1_700_000_042);
    assert_eq!(block.block_hash, block_hash);
    assert_eq!(block.parent_hash, parent_hash);
    assert_eq!(block.additions.len(), 1);
    assert_eq!(block.additions[0], addition);
    assert_eq!(block.removals, vec![removal_id]);
    assert_eq!(block.coinbase_coins.len(), 1);
    assert_eq!(block.hints.len(), 1);
    assert_eq!(block.hints[0].0, hint_target);
    assert_eq!(block.hints[0].1, hint_payload);
    assert_eq!(block.expected_state_root, expected_root);
}

/// Verifies API-005 Test Plan: [`BlockData`] may carry empty `hints` and [`None`] state root.
///
/// **Proof:** Early pipeline stages still need a well-typed value without optional verification;
/// this matches BLK-009 “no check when [`Option::None`]”.
#[test]
fn vv_req_api_005_block_data_empty_hints_and_none_state_root() {
    let block = BlockData {
        height: 1,
        timestamp: 100,
        block_hash: helpers::filled_hash(0x01),
        parent_hash: helpers::filled_hash(0x02),
        additions: vec![],
        removals: vec![],
        coinbase_coins: vec![],
        hints: vec![],
        expected_state_root: None,
    };
    assert!(block.hints.is_empty());
    assert!(block.expected_state_root.is_none());
}

/// Verifies API-005: [`Some`] `expected_state_root` is storable for BLK-009 optional verification.
#[test]
fn vv_req_api_005_block_data_expected_state_root_some() {
    let root = helpers::filled_hash(0xC0);
    let block = BlockData {
        height: 2,
        timestamp: 200,
        block_hash: helpers::filled_hash(0x03),
        parent_hash: helpers::filled_hash(0x04),
        additions: vec![],
        removals: vec![],
        coinbase_coins: vec![],
        hints: vec![],
        expected_state_root: Some(root),
    };
    assert_eq!(block.expected_state_root, Some(root));
}

// ─────────────────────────────────────────────────────────────────────────────
// CoinAddition — three public fields
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies API-005: [`CoinAddition`] exposes `coin_id`, `coin`, and `same_as_parent`.
///
/// **Proof:** Field access from a literal; [`Coin::coin_id()`] is used so IDs follow the canonical
/// definition (STR-005 / “no custom coin ID math”).
#[test]
fn vv_req_api_005_coin_addition_all_three_fields_accessible() {
    let coin = helpers::test_coin(7, 8, 99);
    let id = coin.coin_id();
    let row = CoinAddition {
        coin_id: id,
        coin,
        same_as_parent: false,
    };
    assert_eq!(row.coin_id, id);
    assert_eq!(row.coin.coin_id(), id);
    assert!(!row.same_as_parent);
}

/// Verifies API-005 test-plan note: [`CoinAddition`] does not auto-correct a wrong `coin_id`.
///
/// **Proof:** Malicious or buggy callers could set `coin_id` to a digest that does not match
/// `coin.coin_id()`. The struct permits that; BLK-* is responsible for rejecting inconsistent rows.
/// This documents behavior until `apply_block` validation lands.
#[test]
fn vv_req_api_005_coin_id_mismatch_still_constructible_until_blk_validation() {
    let coin = helpers::test_coin(1, 2, 3);
    let wrong_id = helpers::filled_hash(0xDE);
    assert_ne!(wrong_id, coin.coin_id());
    let row = CoinAddition {
        coin_id: wrong_id,
        coin,
        same_as_parent: false,
    };
    assert_eq!(row.coin_id, wrong_id);
}

// ─────────────────────────────────────────────────────────────────────────────
// Serde — bincode round-trip (STO-008 precursor)
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies API-005: `bincode` can persist a representative [`BlockData`] value.
///
/// **Proof:** Same pattern as API-002 [`CoinRecord`] tests — if a field lacks compatible
/// `Serialize`/`Deserialize`, or tuple encoding for `hints` breaks, round-trip fails.
#[test]
fn vv_req_api_005_serde_bincode_roundtrip_block_data() {
    let coin = helpers::test_coin(10, 11, 12);
    let original = BlockData {
        height: 99,
        timestamp: 12345,
        block_hash: helpers::filled_hash(0xA1),
        parent_hash: helpers::filled_hash(0xA2),
        additions: vec![CoinAddition {
            coin_id: coin.coin_id(),
            coin,
            same_as_parent: true,
        }],
        removals: vec![helpers::test_coin(20, 21, 22).coin_id()],
        coinbase_coins: vec![helpers::test_coin(30, 31, 500)],
        hints: vec![(
            helpers::test_coin(40, 41, 1).coin_id(),
            Bytes32::from([7u8; 32]),
        )],
        expected_state_root: Some(helpers::filled_hash(0xB0)),
    };

    let bytes = bincode::serialize(&original).expect("bincode serialize BlockData");
    let back: BlockData = bincode::deserialize(&bytes).expect("bincode deserialize BlockData");
    assert_eq!(back, original);
}

/// Verifies API-005: [`CoinAddition`] alone round-trips through bincode.
#[test]
fn vv_req_api_005_serde_bincode_roundtrip_coin_addition() {
    let coin = helpers::test_coin(50, 51, 52);
    let original = CoinAddition {
        coin_id: coin.coin_id(),
        coin,
        same_as_parent: false,
    };
    let bytes = bincode::serialize(&original).expect("bincode serialize CoinAddition");
    let back: CoinAddition =
        bincode::deserialize(&bytes).expect("bincode deserialize CoinAddition");
    assert_eq!(back, original);
}
