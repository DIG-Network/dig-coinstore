//! # STR-005 Tests — Re-export Strategy

mod helpers;

// ─────────────────────────────────────────────────────────────────────────────
// STR-005: Re-export Strategy
// Requirement: docs/requirements/domains/crate_structure/specs/STR-005.md
// NORMATIVE: docs/requirements/domains/crate_structure/NORMATIVE.md#STR-005
// SPEC.md: Sections 1, 10
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies STR-005: `Coin` is re-exported from the crate root.
///
/// Consumers MUST be able to use `dig_coinstore::Coin` without depending
/// on dig-clvm or chia-protocol directly.
#[test]
fn vv_req_str_005_coin_reexport() {
    // Prove Coin is importable from the crate root.
    let parent = dig_coinstore::Bytes32::from([1u8; 32]);
    let puzzle_hash = dig_coinstore::Bytes32::from([2u8; 32]);
    let coin = dig_coinstore::Coin::new(parent, puzzle_hash, 1000);
    assert_eq!(coin.amount, 1000);
}

/// Verifies STR-005: `Bytes32` is re-exported from the crate root.
#[test]
fn vv_req_str_005_bytes32_reexport() {
    let hash = dig_coinstore::Bytes32::from([0xABu8; 32]);
    assert_eq!(hash.as_ref().len(), 32);
}

/// Verifies STR-005: `CoinState` is re-exported from the crate root.
#[test]
fn vv_req_str_005_coinstate_reexport() {
    let coin = dig_coinstore::Coin::new(
        dig_coinstore::Bytes32::from([1u8; 32]),
        dig_coinstore::Bytes32::from([2u8; 32]),
        500,
    );
    let cs = dig_coinstore::CoinState::new(coin, None, Some(42));
    assert_eq!(cs.spent_height, None);
    assert_eq!(cs.created_height, Some(42));
}

/// Verifies STR-005: `CoinStateFilters` is re-exported from the crate root.
///
/// This type is from chia-protocol directly (not in dig-clvm), used by
/// batch_coin_states_by_puzzle_hashes() (QRY-007).
#[test]
fn vv_req_str_005_coinstatefilters_reexport() {
    // Prove CoinStateFilters is importable and constructible.
    let _ = std::any::type_name::<dig_coinstore::CoinStateFilters>();
}

/// Verifies STR-005: `dig_coinstore::Coin` IS the same type as `dig_clvm::Coin`.
///
/// If they were different types, assigning one to the other would fail at
/// compile time. This proves the re-export chain is correct.
#[test]
fn vv_req_str_005_type_identity_coin() {
    let coin: dig_coinstore::Coin = dig_clvm::Coin::new(
        dig_clvm::Bytes32::from([0u8; 32]),
        dig_clvm::Bytes32::from([0u8; 32]),
        0,
    );
    // This assignment proves the types are identical.
    let _: dig_clvm::Coin = coin;
}

/// Verifies STR-005: `dig_coinstore::Bytes32` IS the same type as `dig_clvm::Bytes32`.
#[test]
fn vv_req_str_005_type_identity_bytes32() {
    let hash: dig_coinstore::Bytes32 = dig_clvm::Bytes32::from([0u8; 32]);
    let _: dig_clvm::Bytes32 = hash;
}

/// Verifies STR-005: `dig_coinstore::CoinStateFilters` IS the same type
/// as `chia_protocol::CoinStateFilters`.
#[test]
fn vv_req_str_005_type_identity_coinstatefilters() {
    // Both must be the same concrete type — assignment proves it.
    fn _check(f: dig_coinstore::CoinStateFilters) -> chia_protocol::CoinStateFilters {
        f
    }
}
