//! # STR-001 Tests — Cargo.toml Configuration
//!
//! Verifies requirement **STR-001**: Cargo.toml with dependencies, feature gates, and metadata.
//! Each test imports a different production dependency to prove it resolves, compiles, and functions.
//!
//! # Requirement: STR-001
//! # Spec: docs/requirements/domains/crate_structure/specs/STR-001.md
//! # SPEC.md: §1.2 (Crate Dependencies), §2.7 (Constants)
//!
//! ## How these tests prove the requirement
//!
//! Each test `use`s a type from a dependency listed in [SPEC.md §1.2](../../docs/resources/SPEC.md):
//! `chia-sha2`, `chia-protocol`, `chia-traits`, `dig-clvm`, `dig-constants`, `serde`, `bincode`,
//! `parking_lot`, `thiserror`, `tracing`, `lru`, `rayon`. If any dependency is missing or
//! version-conflicted, the test fails at **compile time**. The default-feature test checks
//! `cfg!(feature = "rocksdb-storage")` proving `[features] default` is wired.

mod helpers;

// ─────────────────────────────────────────────────────────────────────────────
// STR-001: Cargo.toml Configuration
// Requirement: docs/requirements/domains/crate_structure/specs/STR-001.md
// NORMATIVE: docs/requirements/domains/crate_structure/NORMATIVE.md#STR-001
// SPEC.md: §1.2 (Crate Dependencies)
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies STR-001: The default feature is `rocksdb-storage`.
///
/// When no explicit features are selected (i.e., the consumer uses default
/// features), `rocksdb-storage` MUST be active. This test proves it by
/// checking the `cfg` flag at compile time.
///
/// This is the most important feature gate test because it determines which
/// storage backend downstream consumers get by default.
#[test]
fn vv_req_str_001_default_feature_is_rocksdb() {
    // If the default feature includes rocksdb-storage, this cfg is true.
    // The test itself compiling and passing proves the feature is active.
    // We use a runtime variable to avoid clippy::assertions_on_constants.
    let rocksdb_enabled = cfg!(feature = "rocksdb-storage");
    assert!(
        rocksdb_enabled,
        "Default features must include rocksdb-storage"
    );
}

/// Verifies STR-001: chia-sha2 is available as a dependency.
///
/// The crate MUST depend on `chia-sha2` for SHA-256 hashing (used in Merkle
/// tree leaf/node computation). This test proves the dependency resolves
/// and the `Sha256` type is importable.
///
/// chia-sha2 wraps the `sha2` crate and is the same implementation used
/// by `Coin::coin_id()` in chia-protocol, ensuring hash consistency.
#[test]
fn vv_req_str_001_chia_sha2_available() {
    use chia_sha2::Sha256;

    // Compute a SHA-256 hash to prove the dependency is functional,
    // not just importable. chia-sha2 API: new() -> update(&[u8]) -> finalize().
    let mut hasher = Sha256::new();
    hasher.update(b"dig-coinstore");
    let hash = hasher.finalize();

    // SHA-256 always produces 32 bytes.
    assert_eq!(hash.len(), 32, "SHA-256 output must be 32 bytes");
}

/// Verifies STR-001: chia-protocol types are available.
///
/// The crate MUST depend on `chia-protocol` for core blockchain types:
/// `Coin`, `Bytes32`, `CoinState`. This test proves they are importable.
#[test]
fn vv_req_str_001_chia_protocol_types_available() {
    use chia_protocol::{Bytes32, Coin, CoinState};

    // Construct a Bytes32 from a fixed array — proves the type resolves.
    let _hash = Bytes32::from([0u8; 32]);

    // Construct a Coin — proves Coin is importable with its fields.
    let parent = Bytes32::from([1u8; 32]);
    let puzzle_hash = Bytes32::from([2u8; 32]);
    let coin = Coin::new(parent, puzzle_hash, 1000);
    assert_eq!(coin.amount, 1000);

    // Prove CoinState is importable (it wraps Coin + height info).
    let _cs = CoinState::new(coin, None, Some(42));
}

/// Verifies STR-001: chia-traits Streamable trait is available.
///
/// The crate MUST depend on `chia-traits` for the `Streamable` trait,
/// which provides canonical Chia wire-format serialization. This is used
/// for CoinState serialization in sync protocol responses.
#[test]
fn vv_req_str_001_chia_traits_available() {
    // Prove the Streamable trait is importable.
    use chia_protocol::Bytes32;
    use chia_traits::Streamable;

    // Bytes32 implements Streamable — round-trip to prove it works.
    let original = Bytes32::from([42u8; 32]);
    let bytes = original.to_bytes();
    let restored = Bytes32::from_bytes(&bytes).unwrap();
    assert_eq!(original, restored);
}

/// Verifies STR-001: dig-clvm dependency resolves.
///
/// The crate MUST depend on `dig-clvm` as the single integration point
/// for Chia ecosystem types. This test proves the dependency resolves.
#[test]
fn vv_req_str_001_dig_clvm_available() {
    // dig-clvm re-exports Chia types; prove it's linked.
    // Just importing a known re-exported type is sufficient.
    let _ = std::any::type_name::<dig_clvm::SpendBundle>();
}

/// Verifies STR-001: dig-constants dependency resolves.
///
/// The crate MUST depend on `dig-constants` for network configuration
/// (genesis challenge, puzzle hashes, etc.).
#[test]
fn vv_req_str_001_dig_constants_available() {
    // Prove the crate links by referencing a known export.
    let _ = std::any::type_name::<dig_constants::NetworkConstants>();
}

/// Verifies STR-001: serde derive macros work.
///
/// The crate MUST depend on `serde` with the `derive` feature enabled.
/// This test proves derive macros compile correctly.
#[test]
fn vv_req_str_001_serde_derive_works() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestStruct {
        value: u64,
    }

    let original = TestStruct { value: 42 };
    let encoded = bincode::serialize(&original).unwrap();
    let decoded: TestStruct = bincode::deserialize(&encoded).unwrap();
    assert_eq!(original, decoded);
}

/// Verifies STR-001: parking_lot RwLock is available.
///
/// The crate MUST depend on `parking_lot` for fast RwLock/Mutex.
/// This is critical for the concurrency model (CON-002).
#[test]
fn vv_req_str_001_parking_lot_available() {
    let lock = parking_lot::RwLock::new(42u64);
    let read = lock.read();
    assert_eq!(*read, 42);
}

/// Verifies STR-001: thiserror derive macro works.
///
/// The crate MUST depend on `thiserror` for error enum derivation.
#[test]
fn vv_req_str_001_thiserror_available() {
    #[derive(Debug, thiserror::Error)]
    enum TestError {
        #[error("test error: {0}")]
        Test(String),
    }

    let err = TestError::Test("hello".into());
    assert_eq!(format!("{}", err), "test error: hello");
}

/// Verifies STR-001: tracing macros work.
///
/// The crate MUST depend on `tracing` for structured logging.
/// Used for performance warnings during block application (BLK-010).
#[test]
fn vv_req_str_001_tracing_available() {
    // Just prove the macros compile — no subscriber needed for this test.
    tracing::info!("STR-001 tracing test");
    tracing::warn!("STR-001 tracing warning test");
}

/// Verifies STR-001: bincode serialization works.
///
/// The crate MUST depend on `bincode` for compact binary serialization
/// of internal storage values (CoinRecord, snapshots).
#[test]
fn vv_req_str_001_bincode_available() {
    let value: u64 = 123456789;
    let encoded = bincode::serialize(&value).unwrap();
    let decoded: u64 = bincode::deserialize(&encoded).unwrap();
    assert_eq!(value, decoded);
}

/// Verifies STR-001: lru cache crate is available.
///
/// The crate MUST depend on `lru` for the CoinRecord LRU cache (PRF-002).
#[test]
fn vv_req_str_001_lru_available() {
    use lru::LruCache;
    use std::num::NonZeroUsize;

    let mut cache: LruCache<u64, String> = LruCache::new(NonZeroUsize::new(10).unwrap());
    cache.put(1, "one".to_string());
    assert_eq!(cache.get(&1), Some(&"one".to_string()));
}

/// Verifies STR-001: rayon parallel iteration works.
///
/// The crate MUST depend on `rayon` for parallel removal validation (CON-004).
#[test]
fn vv_req_str_001_rayon_available() {
    use rayon::prelude::*;

    let sum: u64 = (0..1000u64).into_par_iter().sum();
    assert_eq!(sum, 999 * 1000 / 2);
}
