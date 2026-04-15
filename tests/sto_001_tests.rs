//! # STO-001 Tests — [`dig_coinstore::storage::StorageBackend`] contract
//!
//! **Normative:** [`docs/requirements/domains/storage/NORMATIVE.md`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-001)
//! **Spec:** [`STO-001.md`](../../docs/requirements/domains/storage/specs/STO-001.md)
//! **Crate layout:** [`src/storage/mod.rs`](../../src/storage/mod.rs) (trait + [`WriteBatch`](dig_coinstore::storage::WriteBatch) + [`StorageError`](dig_coinstore::storage::StorageError))
//!
//! ## Relationship to STR-003
//!
//! [`tests/str_003_tests.rs`](str_003_tests.rs) proves the storage **module** exists and wires Rocks/LMDB smoke paths.
//! *This file* is the **dedicated STO-001** acceptance battery: the seven trait methods, `Send + Sync`, `None` vs error
//! on missing `get`, idempotent `delete`, empty [`WriteBatch`](dig_coinstore::storage::WriteBatch), prefix scans, and
//! **both** backends where compiled ([`STO-002`](../../docs/requirements/domains/storage/specs/STO-002.md),
//! [`STO-003`](../../docs/requirements/domains/storage/specs/STO-003.md)).
//!
//! ## How this proves STO-001
//!
//! | Spec / acceptance | Test(s) |
//! |-------------------|---------|
//! | Trait `Send + Sync` + object safety | [`vv_req_sto_001_trait_send_sync_bounds_compile`], [`rocks_tests::vv_req_sto_001_trait_send_sync_and_dyn_coercion`] |
//! | `get` / `put` round-trip | `rocks_tests::vv_req_sto_001_get_put_roundtrip` / `lmdb_tests::vv_req_sto_001_lmdb_get_put_roundtrip` |
//! | `get` → `None` when missing | `vv_req_sto_001_get_missing_returns_none` / `vv_req_sto_001_lmdb_get_missing_returns_none` |
//! | `delete` idempotent | `vv_req_sto_001_delete_missing_key_no_error` / `vv_req_sto_001_lmdb_delete_missing_key_no_error` |
//! | `prefix_scan` (five keys) | `vv_req_sto_001_prefix_scan_five_keys` / `vv_req_sto_001_lmdb_prefix_scan_five_keys` |
//! | `batch_write` (three puts + empty) | Rocks + LMDB `*_batch_write_three_puts_visible` + `*_batch_write_empty_is_noop` |
//! | Unknown CF | `vv_req_sto_001_unknown_column_family_errors` / `vv_req_sto_001_lmdb_unknown_column_family_errors` |
//! | `flush` / `compact` | `vv_req_sto_001_flush_and_compact_callable` / `vv_req_sto_001_lmdb_flush_and_compact_callable` |
//!
//! **GitNexus / SocratiCode:** not confirmed in this environment. **Repomix:** `npx repomix@latest src/storage -o .repomix/pack-storage.xml` per `docs/prompt/start.md` before authoring these tests.

// ─────────────────────────────────────────────────────────────────────────────
// Shared compile-time proof (no I/O; builds whenever the trait is in the graph)
// ─────────────────────────────────────────────────────────────────────────────

use dig_coinstore::storage::WriteBatch;

/// **STO-001 / CON-001:** The storage surface used across threads must be `Send + Sync`.
#[test]
fn vv_req_sto_001_trait_send_sync_bounds_compile() {
    use dig_coinstore::storage::StorageBackend;
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WriteBatch>();
    assert_send_sync::<dig_coinstore::storage::WriteOp>();
    fn assert_backend<T: StorageBackend + Send + Sync>() {}
    #[cfg(feature = "rocksdb-storage")]
    assert_backend::<dig_coinstore::storage::rocksdb::RocksDbBackend>();
    #[cfg(feature = "lmdb-storage")]
    assert_backend::<dig_coinstore::storage::lmdb::LmdbBackend>();
}

// ─────────────────────────────────────────────────────────────────────────────
// RocksDB (`STO-002`) — default feature `rocksdb-storage`
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "rocksdb-storage")]
mod rocks_tests {
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::rocksdb::RocksDbBackend;
    use dig_coinstore::storage::schema;
    use dig_coinstore::storage::{StorageBackend, StorageError, WriteBatch};

    fn open_backend() -> (tempfile::TempDir, RocksDbBackend) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CoinStoreConfig::default_with_path(dir.path());
        let backend = RocksDbBackend::open(&cfg).unwrap();
        (dir, backend)
    }

    /// **Dyn coercion:** higher layers hold `Box<dyn StorageBackend>`; this must compile and run.
    #[test]
    fn vv_req_sto_001_trait_send_sync_and_dyn_coercion() {
        let (_dir, backend) = open_backend();
        let db: &dyn StorageBackend = &backend;
        db.put(schema::CF_METADATA, b"dyn_key", b"dyn_val").unwrap();
        assert_eq!(
            db.get(schema::CF_METADATA, b"dyn_key").unwrap().as_deref(),
            Some(b"dyn_val".as_slice())
        );
    }

    #[test]
    fn vv_req_sto_001_get_put_roundtrip() {
        let (_dir, backend) = open_backend();
        backend
            .put(schema::CF_METADATA, b"sto001_k", b"sto001_v")
            .unwrap();
        let v = backend.get(schema::CF_METADATA, b"sto001_k").unwrap();
        assert_eq!(v.as_deref(), Some(b"sto001_v".as_slice()));
    }

    #[test]
    fn vv_req_sto_001_get_missing_returns_none() {
        let (_dir, backend) = open_backend();
        assert_eq!(
            backend.get(schema::CF_METADATA, b"nope_nope").unwrap(),
            None
        );
    }

    #[test]
    fn vv_req_sto_001_delete_missing_key_no_error() {
        let (_dir, backend) = open_backend();
        backend
            .delete(schema::CF_METADATA, b"never_written")
            .unwrap();
    }

    #[test]
    fn vv_req_sto_001_prefix_scan_five_keys() {
        let (_dir, backend) = open_backend();
        let prefix = b"sto001/px/";
        for i in 0u8..5 {
            let mut key = prefix.to_vec();
            key.push(i);
            backend.put(schema::CF_METADATA, &key, &[i]).unwrap();
        }
        let rows = backend.prefix_scan(schema::CF_METADATA, prefix).unwrap();
        assert_eq!(rows.len(), 5, "expected 5 keys under prefix {prefix:?}");
    }

    #[test]
    fn vv_req_sto_001_batch_write_three_puts_visible() {
        let (_dir, backend) = open_backend();
        let mut batch = WriteBatch::new();
        batch.put(schema::CF_METADATA, b"b1", b"v1");
        batch.put(schema::CF_METADATA, b"b2", b"v2");
        batch.put(schema::CF_METADATA, b"b3", b"v3");
        backend.batch_write(batch).unwrap();
        assert_eq!(
            backend.get(schema::CF_METADATA, b"b1").unwrap().as_deref(),
            Some(b"v1".as_slice())
        );
        assert_eq!(
            backend.get(schema::CF_METADATA, b"b2").unwrap().as_deref(),
            Some(b"v2".as_slice())
        );
        assert_eq!(
            backend.get(schema::CF_METADATA, b"b3").unwrap().as_deref(),
            Some(b"v3".as_slice())
        );
    }

    #[test]
    fn vv_req_sto_001_batch_write_empty_is_noop() {
        let (_dir, backend) = open_backend();
        let batch = WriteBatch::default();
        assert!(batch.is_empty());
        backend.batch_write(batch).unwrap();
    }

    #[test]
    fn vv_req_sto_001_unknown_column_family_errors() {
        let (_dir, backend) = open_backend();
        let err = backend.get("not_a_real_cf", b"k").unwrap_err();
        assert!(matches!(err, StorageError::UnknownColumnFamily(_)));
    }

    #[test]
    fn vv_req_sto_001_flush_and_compact_callable() {
        let (_dir, backend) = open_backend();
        backend.flush().unwrap();
        backend.compact(schema::CF_METADATA).unwrap();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LMDB (`STO-003`) — feature `lmdb-storage` (see `Cargo.toml`; run with `--features lmdb-storage`)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "lmdb-storage")]
mod lmdb_tests {
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::lmdb::LmdbBackend;
    use dig_coinstore::storage::schema;
    use dig_coinstore::storage::{StorageBackend, StorageError, WriteBatch};

    fn open_backend() -> (tempfile::TempDir, LmdbBackend) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CoinStoreConfig::default_with_path(dir.path());
        let backend = LmdbBackend::open(&cfg).unwrap();
        (dir, backend)
    }

    #[test]
    fn vv_req_sto_001_lmdb_get_put_roundtrip() {
        let (_dir, backend) = open_backend();
        backend
            .put(schema::CF_METADATA, b"lmdb_k", b"lmdb_v")
            .unwrap();
        let v = backend.get(schema::CF_METADATA, b"lmdb_k").unwrap();
        assert_eq!(v.as_deref(), Some(b"lmdb_v".as_slice()));
    }

    #[test]
    fn vv_req_sto_001_lmdb_get_missing_returns_none() {
        let (_dir, backend) = open_backend();
        assert_eq!(backend.get(schema::CF_METADATA, b"missing").unwrap(), None);
    }

    #[test]
    fn vv_req_sto_001_lmdb_delete_missing_key_no_error() {
        let (_dir, backend) = open_backend();
        backend.delete(schema::CF_METADATA, b"ghost").unwrap();
    }

    #[test]
    fn vv_req_sto_001_lmdb_prefix_scan_five_keys() {
        let (_dir, backend) = open_backend();
        let prefix = b"lmdb/sto001/";
        for i in 0u8..5 {
            let mut key = prefix.to_vec();
            key.push(i);
            backend.put(schema::CF_METADATA, &key, &[i, 1, 2]).unwrap();
        }
        let rows = backend.prefix_scan(schema::CF_METADATA, prefix).unwrap();
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn vv_req_sto_001_lmdb_batch_write_three_puts_visible() {
        let (_dir, backend) = open_backend();
        let mut batch = WriteBatch::new();
        batch.put(schema::CF_METADATA, b"x1", b"a");
        batch.put(schema::CF_METADATA, b"x2", b"b");
        batch.put(schema::CF_METADATA, b"x3", b"c");
        backend.batch_write(batch).unwrap();
        assert_eq!(
            backend.get(schema::CF_METADATA, b"x1").unwrap().as_deref(),
            Some(b"a".as_slice())
        );
    }

    #[test]
    fn vv_req_sto_001_lmdb_unknown_column_family_errors() {
        let (_dir, backend) = open_backend();
        let err = backend.get("cf_does_not_exist", b"k").unwrap_err();
        assert!(matches!(err, StorageError::UnknownColumnFamily(_)));
    }

    #[test]
    fn vv_req_sto_001_lmdb_flush_and_compact_callable() {
        let (_dir, backend) = open_backend();
        backend.flush().unwrap();
        backend.compact(schema::CF_METADATA).unwrap();
    }
}
