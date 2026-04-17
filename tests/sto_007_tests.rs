//! # STO-007 Tests — Cargo feature gates for storage backends
//!
//! **Normative:** [`STO-007`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-007)
//! **Spec:** [`STO-007.md`](../../docs/requirements/domains/storage/specs/STO-007.md)
//! **Implementation:** [`src/lib.rs`](../../src/lib.rs) (`compile_error!` when no backend feature),
//! [`src/storage/mod.rs`](../../src/storage/mod.rs) (`open_storage_backend`, conditional `rocksdb` / `lmdb` modules),
//! [`Cargo.toml`](../../Cargo.toml) `[features]` (`default`, `rocksdb-storage`, `lmdb-storage`, `full-storage`).
//!
//! ## What this requirement enforces
//!
//! - **Default:** `rocksdb-storage` is on by default so `cargo build` pulls RocksDB only.
//! - **Isolation:** LMDB-only and Rocks-only builds compile only the matching native stack; `full-storage` compiles both.
//! - **Hard guard:** `cargo check --no-default-features` (no explicit storage feature) must **fail at compile time**
//!   with a message that names the Cargo features (proved here via a nested `cargo check` subprocess).
//! - **Factory:** [`dig_coinstore::open_storage_backend`] / [`dig_coinstore::storage::open_storage_backend`] must open
//!   the engine implied by [`dig_coinstore::config::StorageBackend`] when that engine's feature is enabled.
//!
//! ## How passing tests map to acceptance criteria
//!
//! | STO-007 acceptance row | Evidence in this file |
//! |------------------------|------------------------|
//! | Default is RocksDB | [`vv_req_sto_007_default_features_include_rocksdb`] (`cfg!(feature = "rocksdb-storage")` under default `cargo test`) |
//! | `compile_error!` for zero backends | [`vv_req_sto_007_no_default_features_emits_compile_error`] nested `cargo check --no-default-features` |
//! | Runtime factory | [`vv_req_sto_007_open_storage_backend_rocksdb_smoke`] / [`vv_req_sto_007_open_storage_backend_lmdb_smoke`] + full matrix test |
//! | `full-storage` both engines | [`vv_req_sto_007_full_storage_opens_each_engine`] (requires both feature flags) |
//!
//! **Note:** Link-level isolation (“no LMDB .lib when Rocks-only”) is proven by **feature wiring** in `Cargo.toml`
//! (`rocksdb` / `heed` are `optional = true` and only activated by their features). A linker inventory is intentionally
//! out of scope for portable integration tests; CI can add `cargo tree` / platform-specific checks if needed.

use std::process::Command;

/// **STO-007 / default features:** the package default includes `rocksdb-storage` per `Cargo.toml` `[features].default`.
///
/// This does not prove Cargo.toml was never edited incorrectly — it proves the **built test binary** was compiled with
/// the RocksDB backend feature enabled, which is the contract for “default consumer gets RocksDB”.
#[test]
#[allow(clippy::assertions_on_constants)]
fn vv_req_sto_007_default_features_include_rocksdb() {
    // Runtime assert on cfg! — this evaluates to a constant at compile time
    // but fails the test binary if feature gate drift breaks default config.
    assert!(
        cfg!(feature = "rocksdb-storage"),
        "run `cargo test` without `--no-default-features`; default features must include rocksdb-storage"
    );
}

/// **STO-007 / compile-time validation:** with **no** storage features, the crate root `compile_error!` must fire.
///
/// We spawn a nested `cargo check` from the crate manifest directory so the compiler surfaces the same diagnostic
/// a human sees when they pass `--no-default-features` without adding `lmdb-storage` or `rocksdb-storage`.
#[test]
fn vv_req_sto_007_no_default_features_emits_compile_error() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("cargo")
        .current_dir(manifest_dir)
        .args(["check", "--no-default-features"])
        .output()
        .expect("spawn cargo check for STO-007 compile_error probe");

    assert!(
        !output.status.success(),
        "expected `cargo check --no-default-features` to fail; stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rocksdb-storage") && stderr.contains("lmdb-storage"),
        "compile_error message should name both backend features so operators know what to enable; stderr:\n{stderr}"
    );
}

#[cfg(feature = "rocksdb-storage")]
mod sto007_rocksdb {
    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::open_storage_backend;
    use dig_coinstore::storage::schema;

    /// **STO-007 / factory + Rocks path:** `open_storage_backend(RocksDb, …)` returns a live [`StorageBackend`].
    #[test]
    fn vv_req_sto_007_open_storage_backend_rocksdb_smoke() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(Engine::RocksDb);
        let db = open_storage_backend(Engine::RocksDb, &cfg).expect("open rocks");
        let key = b"sto007-rocksdb-smoke";
        let val = b"ok";
        db.put(schema::CF_METADATA, key, val).expect("put");
        assert_eq!(
            db.get(schema::CF_METADATA, key).expect("get").as_deref(),
            Some(val.as_slice())
        );
    }
}

#[cfg(feature = "lmdb-storage")]
mod sto007_lmdb {
    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::open_storage_backend;
    use dig_coinstore::storage::schema;

    /// **STO-007 / factory + LMDB path:** `open_storage_backend(Lmdb, …)` returns a live [`StorageBackend`].
    #[test]
    fn vv_req_sto_007_open_storage_backend_lmdb_smoke() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(Engine::Lmdb);
        let db = open_storage_backend(Engine::Lmdb, &cfg).expect("open lmdb");
        let key = b"sto007-lmdb-smoke";
        let val = b"ok";
        db.put(schema::CF_METADATA, key, val).expect("put");
        assert_eq!(
            db.get(schema::CF_METADATA, key).expect("get").as_deref(),
            Some(val.as_slice())
        );
    }
}

/// **STO-007 / `full-storage`:** both engines compile together; each can open an independent directory.
///
/// This is the matrix row “`cargo build --features full-storage` compiles both backends” plus runtime selection via
/// the shared factory (same code path [`CoinStore`] uses internally).
#[cfg(all(feature = "rocksdb-storage", feature = "lmdb-storage"))]
mod sto007_full {
    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::open_storage_backend;
    use dig_coinstore::storage::schema;

    #[test]
    fn vv_req_sto_007_full_storage_opens_each_engine() {
        let rocks_dir = tempfile::tempdir().expect("tempdir rocks");
        let rocks_cfg =
            CoinStoreConfig::default_with_path(rocks_dir.path()).with_backend(Engine::RocksDb);
        let rocks = open_storage_backend(Engine::RocksDb, &rocks_cfg).expect("rocks");
        rocks
            .put(schema::CF_METADATA, b"which", b"rocks")
            .expect("rocks put");

        let lmdb_dir = tempfile::tempdir().expect("tempdir lmdb");
        let lmdb_cfg =
            CoinStoreConfig::default_with_path(lmdb_dir.path()).with_backend(Engine::Lmdb);
        let lmdb = open_storage_backend(Engine::Lmdb, &lmdb_cfg).expect("lmdb");
        lmdb.put(schema::CF_METADATA, b"which", b"lmdb")
            .expect("lmdb put");

        assert_eq!(
            rocks
                .get(schema::CF_METADATA, b"which")
                .expect("r")
                .as_deref(),
            Some(b"rocks".as_slice())
        );
        assert_eq!(
            lmdb.get(schema::CF_METADATA, b"which")
                .expect("l")
                .as_deref(),
            Some(b"lmdb".as_slice())
        );
    }
}
