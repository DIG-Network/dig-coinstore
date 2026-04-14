//! # STR-002 Tests — Module Hierarchy

mod helpers;

// ─────────────────────────────────────────────────────────────────────────────
// STR-002: Module Hierarchy
// Requirement: docs/requirements/domains/crate_structure/specs/STR-002.md
// NORMATIVE: docs/requirements/domains/crate_structure/NORMATIVE.md#STR-002
// SPEC.md: Sections 1, 7
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies STR-002: The crate compiles with all modules declared.
///
/// If any module file is missing or has a syntax error, this test will fail
/// at compilation time. The fact that this test compiles and runs proves
/// all `mod` declarations in `src/lib.rs` resolve to existing files with
/// valid Rust syntax.
///
/// This is a compile-time verification: if ANY module is missing, `cargo test`
/// will fail before reaching this function.
#[test]
fn vv_req_str_002_crate_compiles_with_all_modules() {
    // The crate root (dig_coinstore) declares all modules.
    // If any file is missing, this entire test binary fails to compile.
    // Simply referencing the crate in a test proves all modules resolved.
    #[allow(unused_imports)]
    use dig_coinstore as _;
}

/// Verifies STR-002: All 12 top-level modules are declared as `pub mod` in lib.rs.
///
/// Tests that each module is accessible from external crate code (integration
/// test = separate crate). This proves the `pub mod` declarations exist.
#[test]
fn vv_req_str_002_all_top_level_modules_accessible() {
    // Each of these module paths must resolve. If any is missing from lib.rs,
    // this fails at compile time with "could not find `X` in `dig_coinstore`".
    //
    // The 12 modules are: coin_store, config, error, types, block_apply,
    // rollback, queries, hints, archive, storage, merkle, cache.
    macro_rules! assert_module_exists {
        ($($mod_path:path),+ $(,)?) => {
            $(
                // Referencing the module path in a type position proves it exists.
                let _: &str = module_path!();
                {
                    #[allow(unused_imports)]
                    use $mod_path as _;
                }
            )+
        };
    }

    assert_module_exists!(
        dig_coinstore::coin_store,
        dig_coinstore::config,
        dig_coinstore::error,
        dig_coinstore::types,
        dig_coinstore::block_apply,
        dig_coinstore::rollback,
        dig_coinstore::queries,
        dig_coinstore::hints,
        dig_coinstore::archive,
        dig_coinstore::storage,
        dig_coinstore::merkle,
        dig_coinstore::cache,
    );
}

/// Verifies STR-002: Storage submodules exist and are feature-gated.
///
/// `storage::schema` is always available. `storage::rocksdb` is available
/// when `rocksdb-storage` feature is enabled. `storage::lmdb` is available
/// when `lmdb-storage` feature is enabled.
#[test]
fn vv_req_str_002_storage_submodules() {
    // schema is always available (not feature-gated)
    #[allow(unused_imports)]
    use dig_coinstore::storage::schema as _;

    // rocksdb submodule available with default features
    #[cfg(feature = "rocksdb-storage")]
    {
        #[allow(unused_imports)]
        use dig_coinstore::storage::rocksdb as _;
    }
}

/// Verifies STR-002: Merkle submodules exist.
///
/// The `merkle` module MUST contain `proof` and `persistent` submodules.
#[test]
fn vv_req_str_002_merkle_submodules() {
    #[allow(unused_imports)]
    use dig_coinstore::merkle::persistent as _;
    #[allow(unused_imports)]
    use dig_coinstore::merkle::proof as _;
}

/// Verifies STR-002: Cache submodules exist.
///
/// The `cache` module MUST contain `unspent_set`, `lru_cache`, and `counters`.
#[test]
fn vv_req_str_002_cache_submodules() {
    #[allow(unused_imports)]
    use dig_coinstore::cache::counters as _;
    #[allow(unused_imports)]
    use dig_coinstore::cache::lru_cache as _;
    #[allow(unused_imports)]
    use dig_coinstore::cache::unspent_set as _;
}
