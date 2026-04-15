//! # STO-002 Tests — RocksDB backend and twelve column families
//!
//! **Normative:** [`STO-002`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-002)
//! **Spec:** [`STO-002.md`](../../docs/requirements/domains/storage/specs/STO-002.md)
//! **Implementation:** [`src/storage/rocksdb.rs`](../../src/storage/rocksdb.rs)
//! **Schema names:** [`src/storage/schema.rs`](../../src/storage/schema.rs) (`ALL_COLUMN_FAMILIES`)
//!
//! ## How this proves STO-002
//!
//! | Spec / test plan | Mechanism |
//! |------------------|-----------|
//! | All 12 CFs created on open | After [`RocksDbBackend::open`], [`RocksDbBackend::list_column_families`] must report every name in [`dig_coinstore::storage::schema::ALL_COLUMN_FAMILIES`] (Rocks may also keep `default`, which we filter). |
//! | Data isolated between CFs | Same key bytes written to two CFs hold distinct values; reading the wrong CF yields `None`. |
//! | Reopen preserves data + CFs | Drop the backend handle, reopen with a fresh [`RocksDbBackend`], read back the same keys. |
//! | Implements [`StorageBackend`] (STO-001) | Coerce to `dyn StorageBackend` and exercise `put`/`get`. |
//! | Per-CF write buffers + compaction profiles | Verified **by construction** in `rocksdb.rs` (see module docs there); this file does not introspect Rocks internal metrics (reserved for future STO-006 property tests). |
//!
//! **Feature gate:** These tests require `rocksdb-storage` (default crate feature).

mod helpers;

#[cfg(feature = "rocksdb-storage")]
mod rocks_sto002 {
    use std::collections::HashSet;

    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::storage::rocksdb::RocksDbBackend;
    use dig_coinstore::storage::schema::ALL_COLUMN_FAMILIES;
    use dig_coinstore::storage::StorageBackend;

    fn open(path: &std::path::Path) -> RocksDbBackend {
        let cfg = CoinStoreConfig::default_with_path(path).with_backend(Engine::RocksDb);
        RocksDbBackend::open(&cfg).expect("RocksDB open")
    }

    /// **STO-002 / Test plan `test_rocksdb_open_creates_cfs`:** fresh directory → 12 logical CFs on disk.
    #[test]
    fn vv_req_sto_002_open_creates_all_twelve_column_families() {
        let dir = super::helpers::temp_dir();
        let path = dir.path();
        let cfg = CoinStoreConfig::default_with_path(path).with_backend(Engine::RocksDb);
        let _db = RocksDbBackend::open(&cfg).unwrap();
        drop(_db);

        let listed = RocksDbBackend::list_column_families(&cfg).unwrap();
        let names: HashSet<String> = listed.into_iter().filter(|n| n != "default").collect();
        for expected in ALL_COLUMN_FAMILIES {
            assert!(
                names.contains(*expected),
                "missing column family {expected}; got {names:?}"
            );
        }
        assert_eq!(
            names.len(),
            ALL_COLUMN_FAMILIES.len(),
            "unexpected extra CFs (excluding default): {names:?}"
        );
    }

    /// **STO-002 / Test plan `test_rocksdb_cf_isolation`:** logical stores do not alias.
    #[test]
    fn vv_req_sto_002_column_family_isolation_same_key_distinct_values() {
        let dir = super::helpers::temp_dir();
        let db = open(dir.path());
        let key = b"shared-key-bytes";
        db.put(
            dig_coinstore::storage::schema::CF_COIN_RECORDS,
            key,
            b"payload-a",
        )
        .unwrap();
        db.put(
            dig_coinstore::storage::schema::CF_METADATA,
            key,
            b"payload-b",
        )
        .unwrap();
        assert_eq!(
            db.get(dig_coinstore::storage::schema::CF_COIN_RECORDS, key)
                .unwrap()
                .as_deref(),
            Some(b"payload-a".as_ref())
        );
        assert_eq!(
            db.get(dig_coinstore::storage::schema::CF_METADATA, key)
                .unwrap()
                .as_deref(),
            Some(b"payload-b".as_ref())
        );
        assert_eq!(
            db.get(dig_coinstore::storage::schema::CF_HINTS, key)
                .unwrap(),
            None
        );
    }

    /// **STO-002 / Test plan `test_rocksdb_reopen`:** durability across process-equivalent reopen.
    #[test]
    fn vv_req_sto_002_reopen_preserves_column_families_and_rows() {
        let dir = super::helpers::temp_dir();
        let path = dir.path().to_path_buf();
        let cfg = || CoinStoreConfig::default_with_path(&path).with_backend(Engine::RocksDb);

        {
            let db = RocksDbBackend::open(&cfg()).unwrap();
            db.put(
                dig_coinstore::storage::schema::CF_METADATA,
                b"tip",
                b"height-7",
            )
            .unwrap();
        }

        let db2 = RocksDbBackend::open(&cfg()).unwrap();
        assert_eq!(
            db2.get(dig_coinstore::storage::schema::CF_METADATA, b"tip")
                .unwrap()
                .as_deref(),
            Some(b"height-7".as_ref())
        );

        let cfs: HashSet<_> = RocksDbBackend::list_column_families(&cfg())
            .unwrap()
            .into_iter()
            .filter(|n| n != "default")
            .collect();
        assert_eq!(cfs.len(), ALL_COLUMN_FAMILIES.len());
    }

    /// **STO-002 / Test plan `test_rocksdb_implements_trait`:** `RocksDbBackend` is usable as `dyn StorageBackend`.
    #[test]
    fn vv_req_sto_002_implements_storage_backend_trait_object() {
        let dir = super::helpers::temp_dir();
        let db = open(dir.path());
        let store: &dyn StorageBackend = &db;
        store
            .put(
                dig_coinstore::storage::schema::CF_MERKLE_NODES,
                b"path",
                b"hash",
            )
            .unwrap();
        assert_eq!(
            store
                .get(dig_coinstore::storage::schema::CF_MERKLE_NODES, b"path")
                .unwrap()
                .as_deref(),
            Some(b"hash".as_ref())
        );
    }

    /// **STO-002 acceptance:** every CF accepts at least one round-trip write (proves handle wiring).
    #[test]
    fn vv_req_sto_002_put_get_roundtrip_each_column_family() {
        let dir = super::helpers::temp_dir();
        let db = open(dir.path());
        for (i, cf) in ALL_COLUMN_FAMILIES.iter().enumerate() {
            let key = format!("sto002|{i}").into_bytes();
            let val = format!("v-{i}").into_bytes();
            db.put(cf, &key, &val).unwrap();
            assert_eq!(db.get(cf, &key).unwrap().as_deref(), Some(val.as_slice()));
        }
    }
}
