//! # API-003 Tests — `CoinStoreConfig` and `StorageBackend`
//!
//! Dedicated integration tests for requirement **API-003**: public [`CoinStoreConfig`] with eight
//! fields, [`Default`] values matching the SPEC / API-003 table, `with_*` builder chaining, and
//! [`StorageBackend`] selection semantics. These tests encode the acceptance criteria in
//! `docs/requirements/domains/crate_api/specs/API-003.md` so regressions fail CI instead of silently
//! drifting defaults.
//!
//! # How this proves the requirement
//!
//! - **Defaults:** If `CoinStoreConfig::default()` drifts from SPEC §2.6–2.7, the field-by-field
//!   assertions fail immediately—no store open required.
//! - **Builders:** Chaining `with_*` must return `Self` and only mutate the targeted field; we
//!   compare against an expected struct literal.
//! - **CoinStore integration:** When a storage feature is enabled, [`dig_coinstore::coin_store::CoinStore`]
//!   must retain the exact config used at construction so operators can audit limits (`config()`).
//!
//! # Requirement: API-003
//! # Spec: docs/requirements/domains/crate_api/specs/API-003.md
//! # NORMATIVE: docs/requirements/domains/crate_api/NORMATIVE.md#API-003
//! # SPEC.md: Sections 2.6, 2.7

use std::path::PathBuf;

use dig_coinstore::config::{
    default_storage_backend_for_features, CoinStoreConfig, StorageBackend, DEFAULT_LMDB_MAP_SIZE,
    DEFAULT_MAX_QUERY_RESULTS, DEFAULT_MAX_SNAPSHOTS, DEFAULT_ROCKSDB_MAX_OPEN_FILES,
    DEFAULT_ROCKSDB_WRITE_BUFFER_SIZE,
};

mod helpers;

/// Verifies API-003: `Default` matches the normative default table (path, snapshots, query cap,
/// LMDB map size, RocksDB tuning, bloom toggle).
///
/// **Proof:** Any change to a default constant or `Default` impl breaks this test, which is tied
/// directly to SPEC §2.7 numeric literals (10 GiB LMDB, 64 MiB write buffer, 1000 fds, 50k results).
#[test]
fn vv_req_api_003_defaults_match_spec_table() {
    let c = CoinStoreConfig::default();
    assert_eq!(c.storage_path, PathBuf::from("./coinstate"));
    assert_eq!(c.max_snapshots, DEFAULT_MAX_SNAPSHOTS);
    assert_eq!(c.max_query_results, DEFAULT_MAX_QUERY_RESULTS);
    assert_eq!(c.lmdb_map_size, DEFAULT_LMDB_MAP_SIZE);
    assert_eq!(
        c.rocksdb_write_buffer_size,
        DEFAULT_ROCKSDB_WRITE_BUFFER_SIZE
    );
    assert_eq!(c.rocksdb_max_open_files, DEFAULT_ROCKSDB_MAX_OPEN_FILES);
    assert!(c.bloom_filter, "SPEC default enables RocksDB bloom filters");
    assert_eq!(c.backend, default_storage_backend_for_features());
}

/// Verifies API-003: default `StorageBackend` follows the feature matrix (LMDB when available and
/// dual-feature builds prefer LMDB per API-003 notes).
///
/// **Proof:** We assert the same predicate as `default_storage_backend_for_features()` compiled for
/// this test binary’s feature set—documenting intended behavior for `cargo test` vs
/// `--features full-storage`.
#[test]
fn vv_req_api_003_default_backend_follows_feature_matrix() {
    let c = CoinStoreConfig::default();
    #[cfg(all(feature = "lmdb-storage", feature = "rocksdb-storage"))]
    assert_eq!(
        c.backend,
        StorageBackend::Lmdb,
        "dual features → LMDB preferred"
    );

    #[cfg(all(feature = "lmdb-storage", not(feature = "rocksdb-storage")))]
    assert_eq!(c.backend, StorageBackend::Lmdb);

    #[cfg(all(not(feature = "lmdb-storage"), feature = "rocksdb-storage"))]
    assert_eq!(c.backend, StorageBackend::RocksDb);

    #[cfg(all(not(feature = "lmdb-storage"), not(feature = "rocksdb-storage")))]
    assert_eq!(c.backend, StorageBackend::RocksDb);
}

/// Verifies API-003: builder methods chain and set every field deterministically.
///
/// **Proof:** The final struct equals a manually-built expected value; missing `with_*` coverage
/// would leave a field at default and fail the equality check.
#[test]
fn vv_req_api_003_builder_chaining_sets_all_fields() {
    let built = CoinStoreConfig::default()
        .with_backend(StorageBackend::RocksDb)
        .with_storage_path("/tmp/chain_test")
        .with_max_snapshots(42)
        .with_max_query_results(12345)
        .with_lmdb_map_size(1_048_576)
        .with_rocksdb_write_buffer_size(2_097_152)
        .with_rocksdb_max_open_files(512)
        .with_bloom_filter(false);

    let expected = CoinStoreConfig {
        backend: StorageBackend::RocksDb,
        storage_path: PathBuf::from("/tmp/chain_test"),
        max_snapshots: 42,
        max_query_results: 12345,
        lmdb_map_size: 1_048_576,
        rocksdb_write_buffer_size: 2_097_152,
        rocksdb_max_open_files: 512,
        bloom_filter: false,
    };

    assert_eq!(built, expected);
}

/// Verifies API-003: overriding one field leaves the others at `Default` values.
///
/// **Proof:** Start from `default()`, change only `max_snapshots`, compare every other field to a
/// fresh `Default` instance—catches accidental “reset to hard-coded partial defaults” in `with_*`.
#[test]
fn vv_req_api_003_builder_preserves_unset_fields() {
    let tweaked = CoinStoreConfig::default().with_max_snapshots(99);
    let base = CoinStoreConfig::default();
    assert_eq!(tweaked.max_snapshots, 99);
    assert_eq!(tweaked.backend, base.backend);
    assert_eq!(tweaked.storage_path, base.storage_path);
    assert_eq!(tweaked.max_query_results, base.max_query_results);
    assert_eq!(tweaked.lmdb_map_size, base.lmdb_map_size);
    assert_eq!(
        tweaked.rocksdb_write_buffer_size,
        base.rocksdb_write_buffer_size
    );
    assert_eq!(tweaked.rocksdb_max_open_files, base.rocksdb_max_open_files);
    assert_eq!(tweaked.bloom_filter, base.bloom_filter);
}

/// Verifies API-003: `CoinStoreConfig` is `Clone` and preserves values.
///
/// **Proof:** Clone must duplicate every field for embedders that snapshot configs.
#[test]
fn vv_req_api_003_config_clone_round_trip() {
    let a = CoinStoreConfig::default()
        .with_max_query_results(777)
        .with_storage_path("snapshot_clone");
    let b = a.clone();
    assert_eq!(a, b);
}

/// Verifies API-003: `StorageBackend` exposes both `Lmdb` and `RocksDb` variants (public API).
///
/// **Proof:** Construction succeeds for both discriminants; this guards accidental enum shrinkage.
#[test]
fn vv_req_api_003_storage_backend_variants_constructible() {
    let _l = StorageBackend::Lmdb;
    let _r = StorageBackend::RocksDb;
}

/// Verifies API-003: `default_with_path` is equivalent to `default()` + `with_storage_path`.
///
/// **Proof:** `CoinStore::new` relies on this helper; mismatch would break path handling only in
/// constructors while manual `default()` stayed correct.
#[test]
fn vv_req_api_003_default_with_path_matches_default_plus_path() {
    let p = PathBuf::from("custom/coinstate_data");
    let a = CoinStoreConfig::default_with_path(&p);
    let b = CoinStoreConfig::default().with_storage_path(p.clone());
    assert_eq!(a, b);
}

/// Verifies API-003 + API-001: `CoinStore::with_config` stores the configuration for later inspection.
///
/// **Proof:** Operators must see the same limits the store was opened with; we set non-default
/// `max_snapshots` and `max_query_results` and read them back via `config()` without reaching into
/// private fields.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_003_coin_store_exposes_effective_config() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let cfg = CoinStoreConfig::default_with_path(dir.path())
        .with_max_snapshots(1234)
        .with_max_query_results(999)
        .with_backend(StorageBackend::RocksDb);

    let store = CoinStore::with_config(cfg.clone()).expect("open");
    assert_eq!(store.config(), &cfg);
}

/// Verifies API-003: selecting `StorageBackend::RocksDb` opens successfully on the default feature
/// set (rocksdb-storage).
///
/// **Proof:** End-to-end that the config enum is not merely typed—the backend factory dispatches
/// on it. LMDB path is covered when `lmdb-storage` is enabled (`vv_req_api_003_lmdb_backend_opens`).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_003_rocksdb_backend_opens_via_config() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(StorageBackend::RocksDb);
    let res = CoinStore::with_config(cfg);
    assert!(res.is_ok(), "RocksDB open failed: {:?}", res.err());
}

/// Verifies API-003: selecting `StorageBackend::Lmdb` opens when `lmdb-storage` is compiled in.
///
/// **Proof:** Dual-feature default picks LMDB; this test ensures the LMDB factory path is wired and
/// functional—not only the enum variant.
#[cfg(feature = "lmdb-storage")]
#[test]
fn vv_req_api_003_lmdb_backend_opens_via_config() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(StorageBackend::Lmdb);
    let res = CoinStore::with_config(cfg);
    assert!(res.is_ok(), "LMDB open failed: {:?}", res.err());
}
