//! # STO-008 Tests — KV bincode + composite key encoding
//!
//! **Normative:** [`STO-008`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-008)
//! **Spec:** [`STO-008.md`](../../docs/requirements/domains/storage/specs/STO-008.md)
//! **Implementation:**
//! - [`dig_coinstore::storage::kv_bincode`](../../src/storage/kv_bincode.rs) — fixed-int + big-endian bincode for
//!   [`CoinRecord`] / [`CoinStoreSnapshot`] values (with legacy decode fallback).
//! - [`dig_coinstore::storage::schema`](../../src/storage/schema.rs) — composite key helpers (`coin_key`,
//!   `puzzle_hash_coin_key`, `height_coin_key`, hint keys, `merkle_node_key`, `metadata_key`, …).
//! - [`dig_coinstore::coin_store::CoinStore`](../../src/coin_store.rs) — snapshot save/load and FF coin rows route
//!   through `kv_bincode`; [`CoinStore::restore`](../../src/coin_store.rs) writes hint keys via `schema` helpers.
//!
//! ## Acceptance criteria → evidence
//!
//! | STO-008 acceptance row | How this file proves it |
//! |------------------------|-------------------------|
//! | `CoinRecord` bincode round-trip | [`vv_req_sto_008_coin_record_kv_roundtrip`], legacy path [`vv_req_sto_008_coin_record_legacy_default_bincode_still_loads`] |
//! | Snapshot round-trip (spec “StateSnapshot”; crate: [`CoinStoreSnapshot`]) | [`vv_req_sto_008_snapshot_kv_roundtrip`] |
//! | Fixed-width integers + BE | [`vv_req_sto_008_kv_options_differ_from_default_bincode_for_integers`] (scalar field ordering), [`vv_req_sto_008_encode_coin_record_is_deterministic`] |
//! | Fixed-width keys (except metadata) | [`vv_req_sto_008_key_helpers_fixed_widths`] |
//! | Height keys sort numerically under lexicographic order | [`vv_req_sto_008_height_snapshot_keys_sort_like_numeric`] |
//! | Puzzle-hash prefix = first 32 bytes | [`vv_req_sto_008_puzzle_hash_prefix_extraction`] + optional Rocks [`vv_req_sto_008_prefix_scan_matches_puzzle_hash`] |
//! | Encode/decode inverses | [`vv_req_sto_008_height_coin_key_roundtrip`], [`vv_req_sto_008_merkle_node_key_roundtrip`], [`vv_req_sto_008_hint_keys_roundtrip_layout`] |
//! | Metadata UTF-8 | [`vv_req_sto_008_metadata_key_utf8`] |

mod helpers;

use std::collections::HashMap;

use dig_coinstore::storage::kv_bincode;
use dig_coinstore::storage::schema;
use dig_coinstore::{CoinRecord, CoinStoreSnapshot};

/// **STO-008 / test plan `test_coin_record_roundtrip`:** normative KV bincode encodes every [`CoinRecord`] field.
///
/// **Proof:** [`kv_bincode::encode_coin_record`] then [`kv_bincode::decode_coin_record`] must yield `==` input.
/// This is the strict decoder used for “new bytes only”; storage also accepts legacy default bincode separately.
#[test]
fn vv_req_sto_008_coin_record_kv_roundtrip() {
    let coin = helpers::test_coin(9, 10, 42);
    let rec = CoinRecord::new(coin, 100, 1_700_000_000, true);
    let mut spent = rec.clone();
    spent.spend(101);
    for r in [rec, spent] {
        let bytes = kv_bincode::encode_coin_record(&r).expect("encode");
        let got = kv_bincode::decode_coin_record(&bytes).expect("decode");
        assert_eq!(got, r);
    }
}

/// **STO-008 backward compatibility:** rows written with pre-STO-008 `bincode::serialize` (library defaults) must
/// still deserialize through [`kv_bincode::decode_coin_record_storage`] (the path [`CoinStore`] uses before falling
/// back to the 97-byte genesis tuple).
///
/// **Proof:** serialize with default options, decode with storage helper → identical [`CoinRecord`].
#[test]
fn vv_req_sto_008_coin_record_legacy_default_bincode_still_loads() {
    let coin = helpers::test_coin(3, 4, 8);
    let rec = CoinRecord::new(coin, 7, 99, false);
    let legacy = bincode::serialize(&rec).expect("legacy default bincode");
    let got = kv_bincode::decode_coin_record_storage(&legacy).expect("storage decode must accept legacy");
    assert_eq!(got, rec);
}

/// **STO-008 / test plan `test_snapshot_roundtrip`:** [`CoinStoreSnapshot`] is the crate’s snapshot type (API-008);
/// STO-008 spec table still calls it “StateSnapshot” — same serde shape, KV bincode options here.
#[test]
fn vv_req_sto_008_snapshot_kv_roundtrip() {
    let coin = helpers::test_coin(1, 2, 3);
    let id = coin.coin_id();
    let mut coins = HashMap::new();
    let row = CoinRecord::new(coin, 0, 1, false);
    coins.insert(id, row.clone());
    let snap = CoinStoreSnapshot {
        height: 5,
        block_hash: helpers::test_hash(11),
        state_root: helpers::test_hash(22),
        timestamp: 1234,
        coins,
        hints: vec![(id, helpers::test_hash(33))],
        total_coins: 1,
        total_value: 3,
    };
    let bytes = kv_bincode::encode_coin_store_snapshot(&snap).expect("encode snap");
    let got = kv_bincode::decode_coin_store_snapshot_storage(&bytes).expect("decode snap");
    assert_eq!(got, snap);
}

/// **STO-008 / test plan `test_height_key_sort_order`:** [`schema::snapshot_key`] is the 8-byte BE height index for
/// `state_snapshots`; lexicographic `cmp` on key bytes must track numeric `height` order (genesis through large tips).
#[test]
fn vv_req_sto_008_height_snapshot_keys_sort_like_numeric() {
    let heights = [
        0u64,
        1,
        255,
        256,
        65_535,
        u32::MAX as u64,
        u64::MAX,
    ];
    let keys: Vec<[u8; 8]> = heights.iter().copied().map(schema::snapshot_key).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "BE u64 prefix keys must sort like integers");
}

/// **STO-008:** first 32 bytes of [`schema::puzzle_hash_coin_key`] equal the puzzle hash (prefix-scan contract with
/// STO-004 prefix bloom on puzzle-hash CFs).
#[test]
fn vv_req_sto_008_puzzle_hash_prefix_extraction() {
    let ph = helpers::test_hash(7);
    let cid = helpers::test_hash(8);
    let key = schema::puzzle_hash_coin_key(&ph, &cid);
    assert_eq!(key.len(), 64);
    let extracted = schema::puzzle_hash_from_key(&key);
    assert_eq!(extracted, ph);
    assert_eq!(&key[..32], ph.as_ref());
}

/// **STO-008 / test plan `test_key_roundtrip_height`:** [`schema::height_coin_key`] + [`schema::height_from_key`].
#[test]
fn vv_req_sto_008_height_coin_key_roundtrip() {
    let h = 9_223_372_036_854_775_000u64;
    let cid = helpers::test_hash(5);
    let key = schema::height_coin_key(h, &cid);
    assert_eq!(key.len(), 40);
    assert_eq!(schema::height_from_key(&key), h);
    let (h2, id2) = schema::height_coin_from_key(&key);
    assert_eq!(h2, h);
    assert_eq!(id2, cid);
}

/// **STO-008 / test plan `test_bincode_deterministic`:** same value → identical bytes (required for stable Merkle
/// leaves over serialized coin rows).
#[test]
fn vv_req_sto_008_encode_coin_record_is_deterministic() {
    let rec = CoinRecord::new(helpers::test_coin(4, 5, 6), 3, 0, true);
    let a = kv_bincode::encode_coin_record(&rec).unwrap();
    let b = kv_bincode::encode_coin_record(&rec).unwrap();
    assert_eq!(a, b);
}

/// **STO-008:** fixed-int + BE options change scalar encoding vs default bincode for the same struct shape, so we
/// are not silently still on “serde default” for persisted KV rows.
///
/// **Proof:** craft a record with `confirmed_height = 1` (fits in one byte as varint in default bincode, fixed 8 in
/// STO-008 path); serialized lengths differ.
#[test]
fn vv_req_sto_008_kv_options_differ_from_default_bincode_for_integers() {
    let rec = CoinRecord::new(helpers::test_coin(1, 1, 1), 1, 0, true);
    let kv = kv_bincode::encode_coin_record(&rec).unwrap();
    let legacy = bincode::serialize(&rec).unwrap();
    assert_ne!(
        kv, legacy,
        "STO-008 fixint+BE encoding must not match legacy default bincode for this fixture"
    );
}

/// **STO-008 / test plan `test_key_fixed_width`:** lengths from the key/value table in STO-008 / `schema.rs`.
#[test]
fn vv_req_sto_008_key_helpers_fixed_widths() {
    let id = helpers::test_hash(1);
    assert_eq!(schema::coin_key(&id).len(), 32);
    let ph = helpers::test_hash(2);
    assert_eq!(schema::puzzle_hash_coin_key(&ph, &id).len(), 64);
    assert_eq!(schema::parent_coin_key(&ph, &id).len(), 64);
    assert_eq!(schema::height_coin_key(1, &id).len(), 40);
    assert_eq!(schema::snapshot_key(2).len(), 8);
    let hint = helpers::test_hash(3);
    assert_eq!(schema::coin_hint_key(&id, &hint).len(), 64);
    assert_eq!(schema::hint_coin_key(&hint, &id).len(), 64);
    let path = helpers::test_hash(4);
    assert_eq!(schema::merkle_node_key(17, &path).len(), 33);
}

/// **STO-008 / MRK-003 alignment:** merkle internal node keys are `level || path` (33 bytes).
#[test]
fn vv_req_sto_008_merkle_node_key_roundtrip() {
    let path = helpers::filled_hash(0xAB);
    let key = schema::merkle_node_key(9, &path);
    let (lvl, p2) = schema::merkle_node_from_key(&key).expect("roundtrip");
    assert_eq!(lvl, 9);
    assert_eq!(p2, path);
}

/// **STO-008:** [`schema::metadata_key`] is the only variable-width helper — UTF-8 bytes match `str::as_bytes`.
#[test]
fn vv_req_sto_008_metadata_key_utf8() {
    let name = "chain_height";
    assert_eq!(schema::metadata_key(name), name.as_bytes());
}

/// **STO-008:** hint forward/reverse keys are pure concatenations (HNT-003 groundwork).
#[test]
fn vv_req_sto_008_hint_keys_roundtrip_layout() {
    let cid = helpers::test_hash(0x11);
    let hint = helpers::test_hash(0x22);
    let fwd = schema::coin_hint_key(&cid, &hint);
    assert_eq!(&fwd[..32], cid.as_ref());
    assert_eq!(&fwd[32..], hint.as_ref());
    let rev = schema::hint_coin_key(&hint, &cid);
    assert_eq!(&rev[..32], hint.as_ref());
    assert_eq!(&rev[32..], cid.as_ref());
}

/// **STO-008 + STO-002:** `prefix_scan` on [`schema::CF_COIN_BY_PUZZLE_HASH`] with the first 32 bytes of the composite
/// key returns every `(puzzle_hash, coin_id)` entry sharing that puzzle hash.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_sto_008_prefix_scan_matches_puzzle_hash() {
    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::storage::rocksdb::RocksDbBackend;
    use dig_coinstore::storage::StorageBackend;

    let dir = helpers::temp_dir();
    let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(Engine::RocksDb);
    let db = RocksDbBackend::open(&cfg).expect("open");
    let ph = helpers::test_hash(0xC0);
    let c1 = helpers::test_hash(0x01);
    let c2 = helpers::test_hash(0x02);
    let k1 = schema::puzzle_hash_coin_key(&ph, &c1);
    let k2 = schema::puzzle_hash_coin_key(&ph, &c2);
    db.put(schema::CF_COIN_BY_PUZZLE_HASH, k1.as_slice(), &[])
        .unwrap();
    db.put(schema::CF_COIN_BY_PUZZLE_HASH, k2.as_slice(), &[])
        .unwrap();
    let prefix = ph.as_ref();
    let hits = db
        .prefix_scan(schema::CF_COIN_BY_PUZZLE_HASH, prefix)
        .expect("prefix_scan");
    assert_eq!(hits.len(), 2);
    let mut seen: Vec<Vec<u8>> = hits.into_iter().map(|(k, _)| k).collect();
    seen.sort();
    let mut expect = vec![k1.to_vec(), k2.to_vec()];
    expect.sort();
    assert_eq!(seen, expect);
}
