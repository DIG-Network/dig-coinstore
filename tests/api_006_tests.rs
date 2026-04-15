//! # API-006 Tests — [`ApplyBlockResult`] and [`RollbackResult`]
//!
//! Dedicated integration tests for requirement **API-006**: success return types for
//! [`dig_coinstore::coin_store::CoinStore::apply_block`] and rollback entry points, per
//! [`docs/resources/SPEC.md`] §§3.2–3.3.
//!
//! # Requirement: API-006
//! # Spec: docs/requirements/domains/crate_api/specs/API-006.md
//! # NORMATIVE: docs/requirements/domains/crate_api/NORMATIVE.md#API-006
//!
//! ## How these tests prove API-006
//!
//! - **Public field surface (NORMATIVE):** Struct literals that read every public field on
//!   [`ApplyBlockResult`] and [`RollbackResult`] must compile — any rename, privacy change, or missing
//!   field breaks these tests at compile time.
//! - **`modified_coins` shape:** Populating [`RollbackResult::modified_coins`] as
//!   `HashMap<CoinId, CoinRecord>` proves the NORMATIVE type alias wiring (`CoinId` = `Bytes32`) matches
//!   stored rows ([`CoinRecord`] from API-002).
//! - **Count bookkeeping:** A well-formed rollback result satisfies
//!   `coins_deleted + coins_unspent == modified_coins.len()` when each map entry represents exactly one
//!   audited mutation (API-006 acceptance + spec commentary on enriched counts).
//! - **Serde / bincode:** Round-trip encodes both result structs for future STO-008 / RPC envelopes;
//!   mirrors the API-005 pattern for [`BlockData`].
//! - **Method signatures:** Under `rocksdb-storage`, calling [`CoinStore::apply_block`],
//!   [`CoinStore::rollback_to_block`], and [`CoinStore::rollback_n_blocks`] with explicit
//!   `Result<ApplyBlockResult, _>` / `Result<RollbackResult, _>` annotations proves the public `CoinStore`
//!   API uses these success types (pipeline bodies land in BLK-001+ / RBK-001+; stubs return
//!   [`CoinStoreError::StorageError`] with recognizable prefixes so we do not conflate with validation
//!   errors later).
//!
//! **SocratiCode:** Not used here (no MCP). **Repomix / GitNexus:** run per `docs/prompt/start.md`
//! before changing production code.

mod helpers;

use std::collections::HashMap;

use dig_coinstore::{
    coin_store::CoinStore, ApplyBlockResult, BlockData, CoinId, CoinRecord, CoinStoreError,
    RollbackResult,
};

// ─────────────────────────────────────────────────────────────────────────────
// ApplyBlockResult — field access + serialization
// ─────────────────────────────────────────────────────────────────────────────

/// **Acceptance:** All four [`ApplyBlockResult`] fields are public and match NORMATIVE API-006.
///
/// **Proof:** Assign each field from distinct test values; assert round-trip equality after clone.
#[test]
fn vv_req_api_006_apply_block_result_all_fields_accessible() {
    let root = helpers::test_hash(0x71);
    let r = ApplyBlockResult {
        state_root: root,
        coins_created: 11,
        coins_spent: 4,
        height: 99,
    };
    assert_eq!(r.state_root, root);
    assert_eq!(r.coins_created, 11);
    assert_eq!(r.coins_spent, 4);
    assert_eq!(r.height, 99);
    assert_eq!(r, r.clone());
}

/// **Acceptance:** Bincode can encode/decode [`ApplyBlockResult`] (same trait set as other API structs).
#[test]
fn vv_req_api_006_apply_block_result_bincode_roundtrip() {
    let r = ApplyBlockResult {
        state_root: helpers::filled_hash(0xCE),
        coins_created: 3,
        coins_spent: 2,
        height: 42,
    };
    let bytes = bincode::serialize(&r).expect("serialize ApplyBlockResult");
    let back: ApplyBlockResult = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back, r);
}

// ─────────────────────────────────────────────────────────────────────────────
// RollbackResult — HashMap surface, counts, serialization
// ─────────────────────────────────────────────────────────────────────────────

/// **Acceptance:** All four [`RollbackResult`] fields are public; `modified_coins` is `HashMap<CoinId, CoinRecord>`.
///
/// **Proof:** Insert two distinct [`CoinRecord`] rows keyed by [`Coin::coin_id`](dig_coinstore::Coin::coin_id);
/// set `coins_deleted = 2`, `coins_unspent = 0`, `new_height = 5` — types must unify with `HashMap` + `usize` + `u64`.
#[test]
fn vv_req_api_006_rollback_result_all_fields_and_hashmap_type() {
    let coin_a = helpers::test_coin(1, 2, 100);
    let coin_b = helpers::test_coin(3, 4, 200);
    let id_a: CoinId = coin_a.coin_id();
    let id_b: CoinId = coin_b.coin_id();

    let rec_a = CoinRecord::new(coin_a, 10, 1_700_000_000, false);
    let rec_b = CoinRecord::new(coin_b, 11, 1_700_000_001, true);

    let mut modified = HashMap::new();
    modified.insert(id_a, rec_a);
    modified.insert(id_b, rec_b);

    let roll = RollbackResult {
        modified_coins: modified,
        coins_deleted: 2,
        coins_unspent: 0,
        new_height: 5,
    };

    assert_eq!(roll.modified_coins.len(), 2);
    assert_eq!(
        roll.coins_deleted + roll.coins_unspent,
        roll.modified_coins.len()
    );
    assert_eq!(roll.new_height, 5);
}

/// **Acceptance:** For consistent rollback snapshots, deleted + unspent tallies cover every map entry.
///
/// **Proof:** Synthetic mix (`coins_deleted = 1`, `coins_unspent = 2`, three map entries) documents the
/// invariant tests in RBK domains will preserve once [`CoinStore::rollback_to_block`] is fully implemented.
#[test]
fn vv_req_api_006_rollback_result_count_invariant_matches_map_len() {
    let c1 = helpers::test_coin(5, 6, 1);
    let c2 = helpers::test_coin(7, 8, 2);
    let c3 = helpers::test_coin(9, 10, 3);

    let mut m = HashMap::new();
    m.insert(c1.coin_id(), CoinRecord::new(c1, 3, 0, false));
    m.insert(c2.coin_id(), CoinRecord::new(c2, 4, 0, false));
    m.insert(c3.coin_id(), CoinRecord::new(c3, 4, 0, false));

    let roll = RollbackResult {
        modified_coins: m,
        coins_deleted: 1,
        coins_unspent: 2,
        new_height: 2,
    };
    assert_eq!(
        roll.coins_deleted + roll.coins_unspent,
        roll.modified_coins.len(),
        "NORMATIVE API-006 / API-006.md: enriched counts should align with modified_coins cardinality"
    );
}

#[test]
fn vv_req_api_006_rollback_result_bincode_roundtrip() {
    let coin = helpers::test_coin(0xAA, 0xBB, 999);
    let id = coin.coin_id();
    let mut m = HashMap::new();
    m.insert(id, CoinRecord::new(coin, 20, 1234567890, false));

    let roll = RollbackResult {
        modified_coins: m,
        coins_deleted: 1,
        coins_unspent: 0,
        new_height: 19,
    };

    let bytes = bincode::serialize(&roll).expect("serialize RollbackResult");
    let back: RollbackResult = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back, roll);
}

// ─────────────────────────────────────────────────────────────────────────────
// CoinStore — success types on public methods (stubs return Err until BLK/RBK)
// ─────────────────────────────────────────────────────────────────────────────

fn minimal_block_at(height: u64) -> BlockData {
    BlockData {
        height,
        timestamp: 0,
        block_hash: helpers::test_hash(0xC1),
        parent_hash: helpers::test_hash(0xC2),
        additions: vec![],
        removals: vec![],
        coinbase_coins: vec![],
        hints: vec![],
        expected_state_root: None,
    }
}

/// **Contract:** Stubs reject block work until genesis so operators never “apply” into a half-born DB.
///
/// **Proof:** [`CoinStoreError::NotInitialized`] is the error arm of the same `Result<ApplyBlockResult, _>`
/// surface (API-006 + API-001 interplay).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_006_apply_block_not_initialized_without_genesis() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let out: Result<ApplyBlockResult, CoinStoreError> = store.apply_block(minimal_block_at(1));
    assert!(matches!(out, Err(CoinStoreError::NotInitialized)));
}

/// **Acceptance / BLK-001 precursor:** [`CoinStore::apply_block`] returns `Result<ApplyBlockResult, CoinStoreError>`.
///
/// **Proof:** Type annotation on the `let` binding forces the success arm to be [`ApplyBlockResult`].
/// [`CoinStore::apply_block`] returns [`CoinStoreError::NotInitialized`] until [`CoinStore::init_genesis`]
/// runs (API-001); after empty genesis, the BLK stub returns [`CoinStoreError::StorageError`] with an
/// `apply_block:` prefix until BLK-001+ implements the pipeline.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_006_coin_store_apply_block_result_type() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();
    let block = minimal_block_at(7);

    let out: Result<ApplyBlockResult, CoinStoreError> = store.apply_block(block);
    let err = out.expect_err("apply_block is stub until BLK-001+");
    match err {
        CoinStoreError::StorageError(msg) => {
            assert!(
                msg.contains("apply_block:"),
                "stub error should stay prefixed for test stability: {msg}"
            );
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

/// **Acceptance / RBK-001 precursor:** [`CoinStore::rollback_to_block`] returns `Result<RollbackResult, CoinStoreError>`.
/// Requires genesis (same `NotInitialized` gate as `apply_block`).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_006_coin_store_rollback_to_block_result_type() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Height is 0 after empty genesis; target 0 is not strictly above tip, so the RBK stub path
    // still returns `StorageError`. (Target > tip is [`CoinStoreError::RollbackAboveTip`] — API-010,
    // `tests/api_010_tests.rs`.)
    let out: Result<RollbackResult, CoinStoreError> = store.rollback_to_block(0);
    let err = out.expect_err("rollback_to_block is stub until RBK-001+");
    match err {
        CoinStoreError::StorageError(msg) => {
            assert!(msg.contains("rollback_to_block:"), "got: {msg}");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

/// **Acceptance:** [`CoinStore::rollback_n_blocks`] shares the same [`RollbackResult`] success type (RBK-005).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_006_coin_store_rollback_n_blocks_result_type() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let out: Result<RollbackResult, CoinStoreError> = store.rollback_n_blocks(2);
    let err = out.expect_err("rollback_n_blocks is stub until RBK-005+");
    match err {
        CoinStoreError::StorageError(msg) => {
            assert!(msg.contains("rollback_n_blocks:"), "got: {msg}");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}
