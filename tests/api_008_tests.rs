//! # API-008 Tests — [`CoinStoreSnapshot`]
//!
//! Dedicated tests for requirement **API-008**: the serializable checkpoint type used for fast sync,
//! backup, and restore ([`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) §3.14, improvement #6).
//!
//! # Requirement: API-008
//! # NORMATIVE: docs/requirements/domains/crate_api/NORMATIVE.md#API-008
//! # Spec: docs/requirements/domains/crate_api/specs/API-008.md
//!
//! ## Scope
//!
//! Covers **API-008** end-to-end: the [`CoinStoreSnapshot`] wire type (NORMATIVE eight fields + serde) and the
//! [`dig_coinstore::coin_store::CoinStore`] operations from [`API-008.md`](../../docs/requirements/domains/crate_api/specs/API-008.md)
//! — `snapshot`, `restore`, `save_snapshot`, `load_snapshot`, `load_latest_snapshot`, Merkle/state-root validation,
//! aggregate consistency (`stats` vs snapshot metadata), and `max_snapshots` pruning ([`SPEC.md`](../../docs/resources/SPEC.md) §3.14).
//!
//! ## How tests prove API-008
//!
//! - **Field surface:** Struct literals that assign every public field must compile — catches renames,
//!   missing fields, or privacy regressions against NORMATIVE.
//! - **`coins` / `hints` types:** `HashMap<CoinId, CoinRecord>` and `Vec<(CoinId, Bytes32)>` match
//!   [`BlockData`](dig_coinstore::BlockData) hint tuple conventions (same `CoinId` / [`Bytes32`] identity).
//! - **Serde / bincode:** Round-trip through `bincode` is the intended on-disk / RPC envelope (API-008
//!   implementation notes); success implies `CoinRecord` + upstream `Bytes32` serde compatibility.
//! - **Live store:** After genesis coins, `snapshot()` metadata and aggregates match `stats()`; `restore()` on a
//!   fresh directory reproduces Merkle root and stats; tampered `state_root` / `total_value` are rejected.
//! - **Persistence:** `save_snapshot` / `load_*` behavior and oldest-height pruning when `max_snapshots` is exceeded.
//!
//! **SocratiCode:** not used (no MCP). **Repomix / GitNexus:** per `docs/prompt/start.md` before editing sources.

mod helpers;

use std::collections::HashMap;

use dig_coinstore::{Bytes32, CoinId, CoinRecord, CoinStoreSnapshot};

/// **Acceptance (NORMATIVE):** All eight [`CoinStoreSnapshot`] fields are public and readable.
#[test]
fn vv_req_api_008_all_eight_fields_accessible() {
    let bh = helpers::test_hash(0x11);
    let sr = helpers::test_hash(0x22);
    let coin = helpers::test_coin(3, 4, 999);
    let cid: CoinId = coin.coin_id();
    let mut coins = HashMap::new();
    coins.insert(cid, CoinRecord::new(coin, 1, 1_700_000_000, false));
    let hints = vec![(cid, helpers::filled_hash(0x33))];

    let snap = CoinStoreSnapshot {
        height: 7,
        block_hash: bh,
        state_root: sr,
        timestamp: 1_700_000_123,
        coins,
        hints,
        total_coins: 1,
        total_value: 999,
    };

    assert_eq!(snap.height, 7);
    assert_eq!(snap.block_hash, bh);
    assert_eq!(snap.state_root, sr);
    assert_eq!(snap.timestamp, 1_700_000_123);
    assert_eq!(snap.coins.len(), 1);
    assert_eq!(snap.hints.len(), 1);
    assert_eq!(snap.total_coins, 1);
    assert_eq!(snap.total_value, 999);
}

/// **Acceptance:** `coins` is `HashMap<CoinId, CoinRecord>` — two distinct keys, iteration preserves inserts.
#[test]
fn vv_req_api_008_coins_hashmap_type() {
    let a = helpers::test_coin(1, 2, 10);
    let b = helpers::test_coin(3, 4, 20);
    let id_a = a.coin_id();
    let id_b = b.coin_id();
    let mut coins = HashMap::new();
    coins.insert(id_a, CoinRecord::new(a, 0, 0, false));
    coins.insert(id_b, CoinRecord::new(b, 0, 0, false));

    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::default(),
        state_root: Bytes32::default(),
        timestamp: 0,
        coins,
        hints: vec![],
        total_coins: 2,
        total_value: 30,
    };

    assert_eq!(snap.coins.len(), 2);
    assert_eq!(snap.coins[&id_a].coin.amount, 10);
    assert_eq!(snap.coins[&id_b].coin.amount, 20);
}

/// **Acceptance:** `hints` is `Vec<(CoinId, Bytes32)>` — same pair shape as [`BlockData::hints`](dig_coinstore::BlockData::hints).
#[test]
fn vv_req_api_008_hints_vec_tuple_type() {
    let cid = helpers::test_coin(9, 8, 1).coin_id();
    let h = helpers::test_hash(0x44);
    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::default(),
        state_root: Bytes32::default(),
        timestamp: 0,
        coins: HashMap::new(),
        hints: vec![(cid, h)],
        total_coins: 0,
        total_value: 0,
    };
    assert_eq!(snap.hints[0].0, cid);
    assert_eq!(snap.hints[0].1, h);
}

/// **Serde:** Empty snapshot round-trips through bincode (genesis / pre-bootstrap case in API-008.md).
#[test]
fn vv_req_api_008_bincode_roundtrip_empty_snapshot() {
    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::default(),
        state_root: Bytes32::default(),
        timestamp: 0,
        coins: HashMap::new(),
        hints: vec![],
        total_coins: 0,
        total_value: 0,
    };
    let bytes = bincode::serialize(&snap).expect("serialize empty snapshot");
    let back: CoinStoreSnapshot = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back, snap);
}

/// **Serde:** Populated snapshot with spent + unspent rows and hints survives bincode round-trip.
#[test]
fn vv_req_api_008_bincode_roundtrip_with_coins_and_hints() {
    let unspent = helpers::test_coin(1, 2, 100);
    let spent_coin = helpers::test_coin(5, 6, 200);
    let id_u = unspent.coin_id();
    let id_s = spent_coin.coin_id();

    let mut rec_s = CoinRecord::new(spent_coin, 2, 1_700_000_000, false);
    rec_s.spend(5);

    let mut coins = HashMap::new();
    coins.insert(id_u, CoinRecord::new(unspent, 2, 1_700_000_000, false));
    coins.insert(id_s, rec_s);

    let hints = vec![
        (id_u, helpers::filled_hash(0x01)),
        (id_s, helpers::filled_hash(0x02)),
    ];

    let snap = CoinStoreSnapshot {
        height: 10,
        block_hash: helpers::test_hash(0xAA),
        state_root: helpers::test_hash(0xBB),
        timestamp: 1_700_000_999,
        coins,
        hints,
        total_coins: 2,
        total_value: 100,
    };

    let bytes = bincode::serialize(&snap).expect("serialize populated snapshot");
    let back: CoinStoreSnapshot = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back, snap);
    assert!(back.coins[&id_s].spent_height.is_some());
    assert!(back.coins[&id_u].spent_height.is_none());
}

/// **Field semantics (API-008 field table):** `total_coins == coins.len()` and `total_value` sums **unspent** amounts.
///
/// **Proof:** The struct does not enforce this invariant automatically; `CoinStore::snapshot` fills both fields and
/// `CoinStore::restore` validates them before touching disk.
#[test]
fn vv_req_api_008_total_coins_and_total_value_conventions() {
    let c1 = helpers::test_coin(1, 1, 50);
    let c2 = helpers::test_coin(2, 2, 150);
    let id1 = c1.coin_id();
    let id2 = c2.coin_id();
    let mut r2 = CoinRecord::new(c2, 1, 0, false);
    r2.spend(2);

    let mut coins = HashMap::new();
    coins.insert(id1, CoinRecord::new(c1, 1, 0, false));
    coins.insert(id2, r2);

    let total_value: u64 = coins
        .values()
        .filter(|r| r.spent_height.is_none())
        .map(|r| r.coin.amount)
        .sum();

    let snap = CoinStoreSnapshot {
        height: 3,
        block_hash: Bytes32::default(),
        state_root: Bytes32::default(),
        timestamp: 0,
        coins,
        hints: vec![],
        total_coins: 2,
        total_value,
    };

    assert_eq!(snap.total_coins, snap.coins.len() as u64);
    assert_eq!(snap.total_value, 50, "only unspent c1 contributes");
}

/// **Ergonomics:** `Debug` + `Clone` + `PartialEq` (same derive bundle as other API domain structs).
#[test]
fn vv_req_api_008_clone_eq_debug() {
    let snap = CoinStoreSnapshot {
        height: 1,
        block_hash: Bytes32::from([7u8; 32]),
        state_root: Bytes32::from([8u8; 32]),
        timestamp: 2,
        coins: HashMap::new(),
        hints: vec![],
        total_coins: 0,
        total_value: 0,
    };
    assert_eq!(snap.clone(), snap);
    let d = format!("{snap:?}");
    assert!(!d.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// CoinStore integration (RocksDB) — `snapshot` / `restore` / retention APIs
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "rocksdb-storage")]
use dig_coinstore::coin_store::CoinStore;
#[cfg(feature = "rocksdb-storage")]
use dig_coinstore::config::CoinStoreConfig;
#[cfg(feature = "rocksdb-storage")]
use dig_coinstore::error::CoinStoreError;
#[cfg(feature = "rocksdb-storage")]
use dig_coinstore::merkle::{empty_hash, merkle_leaf_hash, SparseMerkleTree, SMT_HEIGHT};

/// Legacy 97-byte `coin_records` layout — must match `CoinStore`’s private encoder for Merkle predictions.
#[cfg(feature = "rocksdb-storage")]
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

#[cfg(feature = "rocksdb-storage")]
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

/// **`snapshot()` before genesis:** [`CoinStoreError::NotInitialized`].
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_snapshot_not_initialized() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    assert_eq!(
        store.snapshot().unwrap_err(),
        CoinStoreError::NotInitialized
    );
}

/// **`save_snapshot()` before genesis:** same guard as `snapshot()`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_save_snapshot_not_initialized() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    assert_eq!(
        store.save_snapshot().unwrap_err(),
        CoinStoreError::NotInitialized
    );
}

/// **`snapshot()` vs `stats()`** after three-coin genesis (API-007 alignment).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_snapshot_matches_stats_three_coins() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let c3 = helpers::test_coin(5, 6, 300);
    store
        .init_genesis(vec![(c1, false), (c2, true), (c3, false)], 1_700_000_010)
        .unwrap();
    let stats = store.stats();
    let snap = store.snapshot().unwrap();
    assert_eq!(snap.height, stats.height);
    assert_eq!(snap.timestamp, stats.timestamp);
    assert_eq!(snap.state_root, stats.state_root);
    assert_eq!(snap.block_hash, stats.tip_hash);
    assert_eq!(snap.total_coins, 3);
    assert_eq!(snap.total_value, stats.total_unspent_value);
}

/// **`restore` round-trip** to a fresh database directory.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_restore_roundtrip_new_directory() {
    let dir_a = helpers::temp_dir();
    let mut a = CoinStore::new(dir_a.path()).unwrap();
    a.init_genesis(
        vec![(helpers::test_coin(10, 20, 1_000), false)],
        1_700_000_020,
    )
    .unwrap();
    let snap = a.snapshot().unwrap();

    let dir_b = helpers::temp_dir();
    let mut b = CoinStore::new(dir_b.path()).unwrap();
    b.restore(snap.clone()).unwrap();
    assert_eq!(b.snapshot().unwrap(), snap);
}

/// **`stats()` after `restore()`** matches snapshot tip + aggregates.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_stats_after_restore_match_snapshot() {
    let dir_a = helpers::temp_dir();
    let mut a = CoinStore::new(dir_a.path()).unwrap();
    a.init_genesis(vec![], 1_700_000_030).unwrap();
    let snap = a.snapshot().unwrap();

    let dir_b = helpers::temp_dir();
    let mut b = CoinStore::new(dir_b.path()).unwrap();
    b.restore(snap.clone()).unwrap();
    let st = b.stats();
    assert_eq!(st.height, snap.height);
    assert_eq!(st.timestamp, snap.timestamp);
    assert_eq!(st.state_root, snap.state_root);
    assert_eq!(st.tip_hash, snap.block_hash);
    assert_eq!(st.unspent_count + st.spent_count, snap.total_coins);
    assert_eq!(st.total_unspent_value, snap.total_value);
}

/// **Tampered `state_root`** → [`CoinStoreError::StateRootMismatch`].
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_restore_rejects_state_root_tamper() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store
        .init_genesis(vec![(helpers::test_coin(7, 8, 50), false)], 1_700_000_040)
        .unwrap();
    let mut snap = store.snapshot().unwrap();
    snap.state_root = Bytes32::from([0xEE; 32]);

    let dir_b = helpers::temp_dir();
    let mut other = CoinStore::new(dir_b.path()).unwrap();
    let err = other.restore(snap).unwrap_err();
    assert!(matches!(err, CoinStoreError::StateRootMismatch { .. }));
}

/// **Empty snapshot** restores without prior `init_genesis` on the target store.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_restore_empty_without_prior_genesis() {
    let empty_root = empty_hash(SMT_HEIGHT);
    let snap = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::from([0u8; 32]),
        state_root: empty_root,
        timestamp: 1_700_000_050,
        coins: HashMap::new(),
        hints: vec![],
        total_coins: 0,
        total_value: 0,
    };
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.restore(snap.clone()).unwrap();
    assert!(store.is_initialized());
    assert_eq!(store.snapshot().unwrap(), snap);
}

/// **Spent legacy row** round-trips through `restore` when `state_root` matches legacy Merkle leaves.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_restore_spent_legacy_record() {
    let coin = helpers::test_coin(11, 12, 777);
    let mut rec = CoinRecord::new(coin, 0, 1_700_000_060, false);
    rec.spend(9);
    let mut coins = HashMap::new();
    let id = rec.coin_id();
    coins.insert(id, rec.clone());
    let state_root = smt_root_from_legacy_records(std::slice::from_ref(&rec));
    let snap = CoinStoreSnapshot {
        height: 4,
        block_hash: helpers::test_hash(4),
        state_root,
        timestamp: 1_700_000_061,
        coins,
        hints: vec![],
        total_coins: 1,
        total_value: 0,
    };
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.restore(snap.clone()).unwrap();
    assert_eq!(store.snapshot().unwrap(), snap);
    let st = store.stats();
    assert_eq!(st.spent_count, 1);
    assert_eq!(st.unspent_count, 0);
}

/// **`max_snapshots` pruning** keeps only the newest retained height.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_save_snapshot_prunes_when_max_one() {
    let dir = helpers::temp_dir();
    let cfg = CoinStoreConfig::default_with_path(dir.path()).with_max_snapshots(1);
    let mut store = CoinStore::with_config(cfg).unwrap();
    store.init_genesis(vec![], 1).unwrap();
    store.save_snapshot().unwrap();
    assert_eq!(store.available_snapshot_heights(), vec![0]);

    let snap_h1 = CoinStoreSnapshot {
        height: 1,
        block_hash: helpers::test_hash(1),
        state_root: empty_hash(SMT_HEIGHT),
        timestamp: 2,
        coins: HashMap::new(),
        hints: vec![],
        total_coins: 0,
        total_value: 0,
    };
    store.restore(snap_h1).unwrap();
    store.save_snapshot().unwrap();

    assert_eq!(store.available_snapshot_heights(), vec![1]);
    assert!(store.load_snapshot(0).unwrap().is_none());
    assert!(store.load_snapshot(1).unwrap().is_some());
    assert!(store.load_latest_snapshot().unwrap().is_some());
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_load_snapshot_missing_returns_none() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    assert!(store.load_snapshot(999).unwrap().is_none());
}

#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_load_latest_empty_returns_none() {
    let dir = helpers::temp_dir();
    let store = CoinStore::new(dir.path()).unwrap();
    assert!(store.load_latest_snapshot().unwrap().is_none());
}

/// **`total_value` mismatch** rejected before destructive clear.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_008_store_restore_rejects_total_value_mismatch() {
    let coin = helpers::test_coin(21, 22, 100);
    let rec = CoinRecord::new(coin, 0, 1, false);
    let id = rec.coin_id();
    let root = smt_root_from_legacy_records(std::slice::from_ref(&rec));
    let mut coins = HashMap::new();
    coins.insert(id, rec);
    let bad = CoinStoreSnapshot {
        height: 0,
        block_hash: Bytes32::from([0u8; 32]),
        state_root: root,
        timestamp: 1,
        coins,
        hints: vec![],
        total_coins: 1,
        total_value: 999,
    };
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let err = store.restore(bad).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::StorageError(ref s) if s.contains("total_value")),
        "unexpected: {err:?}"
    );
}
