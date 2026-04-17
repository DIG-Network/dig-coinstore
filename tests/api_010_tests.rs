//! # API-010 Tests — `RollbackAboveTip` and [`CoinStore::is_unspent`]
//!
//! Dedicated tests for requirement **API-010** per
//! [`docs/requirements/domains/crate_api/NORMATIVE.md`](../../docs/requirements/domains/crate_api/NORMATIVE.md#API-010)
//! and [`docs/requirements/domains/crate_api/specs/API-010.md`](../../docs/requirements/domains/crate_api/specs/API-010.md).
//!
//! ## How these tests prove API-010
//!
//! - **`RollbackAboveTip` surface:** Constructing the variant and matching `Display` output proves the
//!   `thiserror` template matches the normative string so operators and logs stay grep-friendly.
//! - **Rollback trigger:** [`CoinStore::rollback_to_block`] with `target_height > height()` must return
//!   `Err(RollbackAboveTip { target, current })` **before** the RBK “not implemented” stub — proving the
//!   ordering contract in API-010 § RollbackAboveTip Trigger.
//! - **`is_unspent` truth:** After [`CoinStore::init_genesis`], inserted genesis coins are present in the
//!   in-memory set (PRF-001 seed) and return `true` without any storage read.
//! - **`is_unspent` false (spent):** A [`CoinStore::restore`] from a snapshot whose only row is spent must
//!   leave the unspent set empty for that ID — proving the set tracks **unspent** rows, not mere existence.
//! - **`is_unspent` false (missing):** Random [`CoinId`] never inserted must return `false` without panicking.
//! - **Reopen parity:** Closing and reopening the same directory rebuilds the set from `coin_records`, so
//!   `is_unspent` stays consistent with disk (same predicate as [`CoinStore::stats`] unspent counts).
//! - **O(1) smoke:** Many consecutive lookups complete in bounded wall time — not a formal benchmark, but
//!   catches accidental regression to per-call storage scans.
//!
//! **Backing set:** [`PRF-001.md`](../../performance/specs/PRF-001.md) (in-memory `HashSet<CoinId>`); **consumer
//! context:** [`CON-004.md`](../../concurrency/specs/CON-004.md) (parallel validation expects cheap `is_unspent`).

mod helpers;

use std::collections::HashMap;
use std::time::Instant;

use dig_coinstore::coin_store::CoinStore;
use dig_coinstore::error::CoinStoreError;
use dig_coinstore::merkle::{merkle_leaf_hash, SparseMerkleTree};
use dig_coinstore::types::CoinRecord;
use dig_coinstore::{Bytes32, CoinId, CoinStoreSnapshot};

/// Same 97-byte legacy layout as production `CoinStore` uses for non-`ff_eligible` rows (see `serialize_legacy_coin_record`).
///
/// **Test-only duplicate:** kept local so this integration test file stays self-contained; if the on-disk
/// layout changes, update this helper and the spent-snapshot Merkle expectation together.
fn legacy_storage_bytes(rec: &CoinRecord) -> Vec<u8> {
    let mut buf = Vec::with_capacity(97);
    buf.extend_from_slice(rec.coin.parent_coin_info.as_ref());
    buf.extend_from_slice(rec.coin.puzzle_hash.as_ref());
    buf.extend_from_slice(&rec.coin.amount.to_le_bytes());
    buf.extend_from_slice(&rec.confirmed_height.to_le_bytes());
    let spent_raw = rec.spent_height.unwrap_or(0);
    buf.extend_from_slice(&spent_raw.to_le_bytes());
    buf.push(if rec.coinbase { 1 } else { 0 });
    buf.extend_from_slice(&rec.timestamp.to_le_bytes());
    buf
}

fn smt_root_from_legacy_records(records: &[CoinRecord]) -> Bytes32 {
    let mut tree = SparseMerkleTree::new();
    let entries: Vec<(Bytes32, Bytes32)> = records
        .iter()
        .map(|r| (r.coin_id(), merkle_leaf_hash(&legacy_storage_bytes(r))))
        .collect();
    if !entries.is_empty() {
        tree.batch_insert(&entries).unwrap();
    }
    tree.root()
}

/// **Test plan `test_rollback_above_tip_variant`:** variant constructible; `Display` contains both heights.
#[test]
fn vv_req_api_010_rollback_above_tip_variant_constructible_and_display() {
    let err = CoinStoreError::RollbackAboveTip {
        target: 100,
        current: 50,
    };
    assert_eq!(err, err.clone());
    let msg = err.to_string();
    assert!(
        msg.contains("100") && msg.contains("50"),
        "Display should include target and current: {msg}"
    );
    assert!(
        msg.to_lowercase().contains("rollback"),
        "Display should mention rollback: {msg}"
    );
}

/// **Test plan `test_rollback_above_tip_triggered`:** `rollback_to_block` when target > tip height.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_rollback_above_tip_triggered_when_target_gt_height() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1).unwrap();
    assert_eq!(store.height(), 0);

    let err = store.rollback_to_block(100).unwrap_err();
    assert_eq!(
        err,
        CoinStoreError::RollbackAboveTip {
            target: 100,
            current: 0
        }
    );
}

/// **`target == current` is not `RollbackAboveTip`** — it's a no-op (RBK-001).
///
/// After RBK-001 implementation, rolling back to the current height returns Ok with empty result.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_rollback_equal_height_is_noop() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1).unwrap();
    let result = store.rollback_to_block(0).unwrap();
    assert_eq!(
        result.new_height, 0,
        "Rolling back to current height is a no-op"
    );
    assert_eq!(result.coins_deleted, 0);
    assert_eq!(result.coins_unspent, 0);
}

/// **Test plan `test_is_unspent_true`:** genesis coin IDs are tracked as unspent.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_is_unspent_true_after_genesis() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(3, 4, 500);
    let id = coin.coin_id();
    store
        .init_genesis(vec![(coin, false)], 1_700_000_100)
        .unwrap();
    assert!(store.is_unspent(&id));
}

/// **Test plan `test_is_unspent_false_spent`:** restore snapshot with only a spent row → not in unspent set.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_is_unspent_false_for_spent_row_after_restore() {
    let coin = helpers::test_coin(9, 8, 123);
    let mut rec = CoinRecord::new(coin, 0, 1, false);
    rec.spend(5);
    let id = rec.coin_id();
    let mut coins = HashMap::new();
    coins.insert(id, rec.clone());
    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::from([0u8; 32]),
        state_root: smt_root_from_legacy_records(std::slice::from_ref(&rec)),
        timestamp: 2,
        coins,
        hints: vec![],
        total_coins: 1,
        total_value: 0,
    };

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.restore(snap).unwrap();
    assert!(!store.is_unspent(&id));
}

/// **Test plan `test_is_unspent_false_missing`:** unknown ID → `false`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_is_unspent_false_for_random_id() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1).unwrap();
    let random: CoinId = helpers::test_hash(0xEE);
    assert!(!store.is_unspent(&random));
}

/// **Reopen:** `with_config` rebuilds unspent set from disk scan (PRF-001 startup parity).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_is_unspent_survives_store_reopen() {
    let dir = helpers::temp_dir();
    let path = dir.path().to_path_buf();
    let coin = helpers::test_coin(1, 2, 50);
    let id = coin.coin_id();

    {
        let mut store = CoinStore::new(&path).unwrap();
        store
            .init_genesis(vec![(coin, false)], 1_700_000_200)
            .unwrap();
        assert!(store.is_unspent(&id));
    }

    let store2 = CoinStore::new(&path).unwrap();
    assert!(store2.is_unspent(&id));
}

/// **Test plan `test_is_unspent_o1` (smoke):** many lookups stay fast (no per-call `prefix_scan`).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_010_is_unspent_many_lookups_bounded_time() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let coin = helpers::test_coin(7, 7, 1);
    let id = coin.coin_id();
    store.init_genesis(vec![(coin, false)], 1).unwrap();

    let n = 50_000u32;
    let t0 = Instant::now();
    for _ in 0..n {
        assert!(store.is_unspent(&id));
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed.as_millis() < 200,
        "50k is_unspent calls took {:?} (expected HashSet O(1) aggregate)",
        elapsed
    );
}
