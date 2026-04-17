//! # STO-003 Tests — LMDB backend: six named databases + MVCC semantics
//!
//! **Normative:** [`docs/requirements/domains/storage/NORMATIVE.md#STO-003`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-003)
//! **Spec:** [`STO-003.md`](../../docs/requirements/domains/storage/specs/STO-003.md)
//! **Implementation:** [`src/storage/lmdb.rs`](../../src/storage/lmdb.rs)
//!
//! ## What this file proves
//!
//! | STO-003 acceptance / test plan | Evidence in this module |
//! |--------------------------------|---------------------------|
//! | Six named DBs on open | [`vv_req_sto_003_open_creates_all_six_named_databases`] |
//! | `StorageBackend` (STO-001) | [`vv_req_sto_003_implements_storage_backend_trait_object`] |
//! | MVCC snapshot on long-lived read txn | [`vv_req_sto_003_mvcc_read_txn_sees_snapshot_while_writer_updates`] |
//! | Concurrent readers + writer | [`vv_req_sto_003_concurrent_readers_while_writer`] |
//! | `map_size` from config | [`vv_req_sto_003_map_size_configurable_at_open`] |
//! | Persist across reopen | [`vv_req_sto_003_reopen_preserves_logical_column_families`] |
//! | Logical CF isolation (multiplex) | [`vv_req_sto_003_logical_column_family_isolation_same_user_key`] |
//! | `prefix_scan` (LMDB range / prefix iterator) | [`vv_req_sto_003_prefix_scan_multiplexed_puzzle_hash_cf`] |
//! | `MDB_MAP_FULL` → [`StorageError::MapFull`](dig_coinstore::storage::StorageError::MapFull) | [`vv_req_sto_003_map_full_returns_storage_error_map_full`] |
//!
//! ## Tooling note (`docs/prompt/start.md`)
//!
//! **Repomix:** `npx repomix@latest src/storage -o .repomix/pack-storage.xml` before edits.
//! **GitNexus:** `npx gitnexus status` failed in this environment (npm error: `node.target` null).
//! **SocratiCode:** MCP `codebase_search` not available here; navigation used specs + `heed` docs.

#![cfg(feature = "lmdb-storage")]

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use dig_coinstore::config::CoinStoreConfig;
use dig_coinstore::storage::lmdb::{LmdbBackend, LMDB_DB_METADATA, LMDB_NAMED_DATABASES};
use dig_coinstore::storage::schema::{
    ALL_COLUMN_FAMILIES, CF_COIN_BY_PUZZLE_HASH, CF_COIN_RECORDS, CF_METADATA,
};
use dig_coinstore::storage::{StorageBackend, StorageError};
use heed::types::Bytes;
use heed::Database;

fn open_backend() -> (tempfile::TempDir, LmdbBackend) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CoinStoreConfig::default_with_path(dir.path());
    let backend = LmdbBackend::open(&cfg).unwrap();
    (dir, backend)
}

/// **STO-003 / test plan `test_lmdb_open_creates_dbs`:** After [`LmdbBackend::open`], every normative
/// database name exists on the environment. We prove this by opening each handle with
/// [`heed::Env::open_database`] inside a read transaction — LMDB returns `None` only when the name
/// was never created.
#[test]
fn vv_req_sto_003_open_creates_all_six_named_databases() {
    let (_dir, backend) = open_backend();
    assert_eq!(
        LMDB_NAMED_DATABASES.len(),
        6,
        "STO-003 normative: exactly six named databases"
    );
    let rtxn = backend.environment().read_txn().unwrap();
    for name in LMDB_NAMED_DATABASES {
        let db: Option<Database<Bytes, Bytes>> = backend
            .environment()
            .open_database(&rtxn, Some(name))
            .unwrap();
        assert!(
            db.is_some(),
            "named LMDB database {name:?} must exist after open"
        );
    }
}

/// **STO-003:** Higher layers use `dyn StorageBackend`; LMDB must be object-safe and functional.
#[test]
fn vv_req_sto_003_implements_storage_backend_trait_object() {
    let (_dir, backend) = open_backend();
    let store: &dyn StorageBackend = &backend;
    store.put(CF_METADATA, b"dyn", b"x").unwrap();
    assert_eq!(
        store.get(CF_METADATA, b"dyn").unwrap().as_deref(),
        Some(b"x".as_slice())
    );
}

/// **STO-003 § MVCC / test plan `test_lmdb_mvcc_snapshot`:** A read transaction opened before a write
/// continues to return the pre-write value until the read transaction is dropped — classic LMDB MVCC.
///
/// We use [`LmdbBackend::environment`] + raw `heed` APIs (dev-dependency) to hold an [`RoTxn`](heed::RoTxn)
/// while calling [`StorageBackend::put`] on the same logical key through the backend (separate write txn).
#[test]
fn vv_req_sto_003_mvcc_read_txn_sees_snapshot_while_writer_updates() {
    let (_dir, backend) = open_backend();
    backend.put(CF_METADATA, b"mvcc_k", b"v1").unwrap();

    let rtxn = backend.environment().read_txn().unwrap();
    let meta: Database<Bytes, Bytes> = backend
        .environment()
        .open_database(&rtxn, Some(LMDB_DB_METADATA))
        .unwrap()
        .expect("metadata db");

    assert_eq!(
        meta.get(&rtxn, b"mvcc_k").unwrap(),
        Some(b"v1".as_slice()),
        "baseline read in snapshot"
    );

    backend.put(CF_METADATA, b"mvcc_k", b"v2").unwrap();

    assert_eq!(
        meta.get(&rtxn, b"mvcc_k").unwrap(),
        Some(b"v1".as_slice()),
        "read txn must still see v1 (snapshot) after concurrent logical update"
    );

    drop(rtxn);

    assert_eq!(
        backend.get(CF_METADATA, b"mvcc_k").unwrap().as_deref(),
        Some(b"v2".as_slice()),
        "after dropping read txn, new snapshot observes v2"
    );
}

/// **STO-003 § MVCC / test plan `test_lmdb_concurrent_readers`:** Many threads issue `get` while another
/// thread mutates — LMDB allows read txns to proceed without blocking the writer (writer serializes).
#[test]
fn vv_req_sto_003_concurrent_readers_while_writer() {
    let backend = Arc::new(open_backend().1);
    backend.put(CF_METADATA, b"ctr", b"0").unwrap();

    let writer = {
        let b = Arc::clone(&backend);
        thread::spawn(move || {
            for i in 1u8..=50 {
                thread::sleep(Duration::from_millis(1));
                b.put(CF_METADATA, b"ctr", &[i]).unwrap();
            }
        })
    };

    let mut readers = vec![];
    for _ in 0..8 {
        let b = Arc::clone(&backend);
        readers.push(thread::spawn(move || {
            for _ in 0..200 {
                let _ = b.get(CF_METADATA, b"ctr").unwrap();
            }
        }));
    }

    for r in readers {
        r.join().expect("reader panicked");
    }
    writer.join().expect("writer panicked");
    assert!(backend.get(CF_METADATA, b"ctr").unwrap().is_some());
}

/// **STO-003 § Environment Configuration:** `map_size` MUST follow [`CoinStoreConfig::lmdb_map_size`].
/// We pick a non-default value and assert open succeeds (capacity reservation is validated at `mdb_env_open`).
#[test]
fn vv_req_sto_003_map_size_configurable_at_open() {
    let dir = tempfile::tempdir().unwrap();
    let custom = 5 * 1024 * 1024;
    let cfg = CoinStoreConfig::default_with_path(dir.path()).with_lmdb_map_size(custom);
    assert_eq!(cfg.lmdb_map_size, custom);
    let _backend = LmdbBackend::open(&cfg).unwrap();
}

/// **STO-003:** Close by dropping the backend handle, reopen the same directory, read prior rows.
#[test]
fn vv_req_sto_003_reopen_preserves_logical_column_families() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    {
        let cfg = CoinStoreConfig::default_with_path(&path);
        let b = LmdbBackend::open(&cfg).unwrap();
        b.put(CF_COIN_RECORDS, b"k-reopen", b"payload").unwrap();
    }
    let cfg = CoinStoreConfig::default_with_path(&path);
    let b2 = LmdbBackend::open(&cfg).unwrap();
    assert_eq!(
        b2.get(CF_COIN_RECORDS, b"k-reopen").unwrap().as_deref(),
        Some(b"payload".as_slice())
    );
}

/// **STO-003 / isolation:** `coin_records` and `metadata` multiplex different physical DBs (or tags).
/// Same user key bytes MUST map to independent values — this would be impossible if both logical CFs
/// shared a flat namespace without tagging.
#[test]
fn vv_req_sto_003_logical_column_family_isolation_same_user_key() {
    let (_dir, backend) = open_backend();
    let key = [0x77u8; 32];
    backend.put(CF_COIN_RECORDS, &key, b"coin-side").unwrap();
    backend.put(CF_METADATA, &key, b"meta-side").unwrap();
    assert_eq!(
        backend.get(CF_COIN_RECORDS, &key).unwrap().as_deref(),
        Some(b"coin-side".as_slice())
    );
    assert_eq!(
        backend.get(CF_METADATA, &key).unwrap().as_deref(),
        Some(b"meta-side".as_slice())
    );
}

/// **STO-003 / prefix_scan:** Puzzle-hash composite keys live in the multiplexed `coins_by_ph` DB.
/// Prefix scan must return only keys under the puzzle-hash prefix, with **decoded** user keys (no tag byte).
#[test]
fn vv_req_sto_003_prefix_scan_multiplexed_puzzle_hash_cf() {
    let (_dir, backend) = open_backend();
    let ph = [0xAAu8; 32];
    for i in 0u8..4 {
        let mut key = ph.to_vec();
        key.extend_from_slice(&[i; 32]);
        backend.put(CF_COIN_BY_PUZZLE_HASH, &key, &[i]).unwrap();
    }
    let rows = backend
        .prefix_scan(CF_COIN_BY_PUZZLE_HASH, ph.as_slice())
        .unwrap();
    assert_eq!(rows.len(), 4);
    for (k, _v) in &rows {
        assert_eq!(k.len(), 64);
        assert!(k.starts_with(ph.as_slice()));
    }
}

/// **STO-003 / test plan `test_lmdb_map_full_error`:** With a tiny `map_size`, repeated large writes
/// eventually exhaust the map; LMDB returns `MDB_MAP_FULL` which we surface as [`StorageError::MapFull`].
#[test]
fn vv_req_sto_003_map_full_returns_storage_error_map_full() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CoinStoreConfig::default_with_path(dir.path()).with_lmdb_map_size(96 * 1024);
    let backend = LmdbBackend::open(&cfg).unwrap();
    let chunk = vec![0xCCu8; 4096];
    let mut saw_map_full = false;
    for i in 0u64..10_000 {
        let key = i.to_be_bytes();
        match backend.put(CF_METADATA, &key, &chunk) {
            Ok(()) => {}
            Err(StorageError::MapFull) => {
                saw_map_full = true;
                break;
            }
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }
    assert!(
        saw_map_full,
        "expected MapFull with 96KiB map and 4KiB rows (platform-dependent; increase iterations if flaky)"
    );
}

/// **Routing completeness:** Every logical name in [`ALL_COLUMN_FAMILIES`] must accept at least one put/get
/// round-trip through the six-database layout (STO-003 mapping + multiplex tags in `lmdb.rs`).
#[test]
fn vv_req_sto_003_put_get_roundtrip_each_logical_column_family() {
    let (_dir, backend) = open_backend();
    for (i, cf) in ALL_COLUMN_FAMILIES.iter().enumerate() {
        let mut key = vec![0x11u8, 0x22u8];
        key.extend_from_slice(&i.to_be_bytes());
        let val = format!("v-{i}").into_bytes();
        backend.put(cf, &key, &val).unwrap();
        let got = backend.get(cf, &key).unwrap();
        assert_eq!(got.as_deref(), Some(val.as_slice()), "cf={cf}");
    }
}
