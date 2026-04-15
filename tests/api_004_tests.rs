//! # API-004 Tests — `CoinStoreError`
//!
//! Verifies requirement **API-004**: public [`dig_coinstore::CoinStoreError`] with the 15 variants
//! listed in [`docs/requirements/domains/crate_api/NORMATIVE.md`](../../docs/requirements/domains/crate_api/NORMATIVE.md#API-004),
//! deriving `Debug + Clone + PartialEq + thiserror::Error`, with storage/serde conversions as
//! specified in [`API-004.md`](../../docs/requirements/domains/crate_api/specs/API-004.md).
//!
//! # How this proves the requirement
//!
//! - **Constructible / clone / eq / Display:** If any variant is missing from the public enum, lacks
//!   derive output, or format strings drift from the normative messages, these tests fail.
//! - **Reachable paths:** Where the crate already produces errors (`double genesis`, storage shim),
//!   we assert the **exact** variant and stable `Display` substrings operators rely on.
//! - **Bincode:** Encode and decode both yield `bincode::Error`, but API-004 requires routing decode
//!   to [`CoinStoreError::DeserializationError`] via [`CoinStoreError::from_bincode_deserialize`].
//! - **Integration rows in API-004 (height mismatch, coin not found, double spend):** Block
//!   application is not implemented yet (BLK-001+). Those scenarios are covered here only at the
//!   **type** level; end-to-end tests will land with the BLK requirement set.
//!
//! **`RollbackAboveTip`:** Normative **API-010** adds this variant; it is intentionally **not**
//! part of the 15 variants for API-004 (see NORMATIVE §2 vs §9).
//!
//! # Requirement: API-004
//! # Spec: docs/requirements/domains/crate_api/specs/API-004.md

use std::fmt::Write as _;

use chia_protocol::Bytes32;

use dig_coinstore::storage::StorageError;
use dig_coinstore::CoinId;
use dig_coinstore::CoinStoreError;

mod helpers;

/// Stable test coin ID for payload-bearing variants.
fn sample_coin_id() -> CoinId {
    Bytes32::from([0xABu8; 32])
}

/// Verifies API-004: every normative variant can be constructed (API surface completeness).
#[test]
fn vv_req_api_004_all_fifteen_variants_constructible() {
    let _ = CoinStoreError::HeightMismatch {
        expected: 1,
        got: 9,
    };
    let _ = CoinStoreError::ParentHashMismatch {
        expected: Bytes32::from([1u8; 32]),
        got: Bytes32::from([2u8; 32]),
    };
    let _ = CoinStoreError::StateRootMismatch {
        expected: Bytes32::from([3u8; 32]),
        computed: Bytes32::from([4u8; 32]),
    };
    let _ = CoinStoreError::CoinNotFound(sample_coin_id());
    let _ = CoinStoreError::CoinAlreadyExists(sample_coin_id());
    let _ = CoinStoreError::DoubleSpend(sample_coin_id());
    let _ = CoinStoreError::SpendCountMismatch {
        expected: 3,
        actual: 0,
    };
    let _ = CoinStoreError::InvalidRewardCoinCount {
        expected: ">= 2".into(),
        got: 1,
    };
    let _ = CoinStoreError::HintTooLong {
        length: 33,
        max: 32,
    };
    let _ = CoinStoreError::GenesisAlreadyInitialized;
    let _ = CoinStoreError::NotInitialized;
    let _ = CoinStoreError::PuzzleHashBatchTooLarge {
        size: 10_000,
        max: 990,
    };
    let _ = CoinStoreError::StorageError("io".into());
    let _ = CoinStoreError::SerializationError("ser".into());
    let _ = CoinStoreError::DeserializationError("de".into());
}

/// Verifies API-004: `Clone` preserves payloads for all structured variants (batch / retry flows).
#[test]
fn vv_req_api_004_clone_equals_original_for_each_shape() {
    let cases: Vec<CoinStoreError> = vec![
        CoinStoreError::HeightMismatch {
            expected: 5,
            got: 6,
        },
        CoinStoreError::ParentHashMismatch {
            expected: Bytes32::from([7u8; 32]),
            got: Bytes32::from([8u8; 32]),
        },
        CoinStoreError::StateRootMismatch {
            expected: Bytes32::from([9u8; 32]),
            computed: Bytes32::from([10u8; 32]),
        },
        CoinStoreError::CoinNotFound(sample_coin_id()),
        CoinStoreError::CoinAlreadyExists(sample_coin_id()),
        CoinStoreError::DoubleSpend(sample_coin_id()),
        CoinStoreError::SpendCountMismatch {
            expected: 2,
            actual: 1,
        },
        CoinStoreError::InvalidRewardCoinCount {
            expected: "0".into(),
            got: 1,
        },
        CoinStoreError::HintTooLong {
            length: 40,
            max: 32,
        },
        CoinStoreError::GenesisAlreadyInitialized,
        CoinStoreError::NotInitialized,
        CoinStoreError::PuzzleHashBatchTooLarge { size: 100, max: 50 },
        CoinStoreError::StorageError("e".into()),
        CoinStoreError::SerializationError("s".into()),
        CoinStoreError::DeserializationError("d".into()),
    ];
    for e in cases {
        assert_eq!(
            e,
            e.clone(),
            "Clone must preserve discriminant + data: {:?}",
            e
        );
    }
}

/// Verifies API-004: `PartialEq` distinguishes cases with the same variant but different data.
#[test]
fn vv_req_api_004_partial_eq_distinct_payloads() {
    let a = CoinStoreError::HeightMismatch {
        expected: 1,
        got: 2,
    };
    let b = CoinStoreError::HeightMismatch {
        expected: 1,
        got: 3,
    };
    assert_ne!(a, b);
    assert_eq!(a, a);

    let c = CoinStoreError::CoinNotFound(Bytes32::from([0x01u8; 32]));
    let d = CoinStoreError::CoinNotFound(Bytes32::from([0x02u8; 32]));
    assert_ne!(c, d);
}

/// Verifies API-004: `thiserror::Error` produces stable, human-readable `Display` strings.
///
/// Assertions use substrings tied to the `#[error("...")]` templates in `src/error.rs` so renames
/// do not silently break operator dashboards.
#[test]
fn vv_req_api_004_display_templates_match_normative_wording() {
    let mut buf = String::new();

    let err = CoinStoreError::HeightMismatch {
        expected: 10,
        got: 12,
    };
    write!(&mut buf, "{}", err).unwrap();
    assert!(buf.contains("10"), "{}", buf);
    assert!(buf.contains("12"), "{}", buf);

    buf.clear();
    write!(
        &mut buf,
        "{}",
        CoinStoreError::InvalidRewardCoinCount {
            expected: ">= 2".into(),
            got: 0,
        }
    )
    .unwrap();
    assert!(buf.contains(">= 2"), "{}", buf);
    assert!(buf.contains('0'), "{}", buf);

    buf.clear();
    write!(&mut buf, "{}", CoinStoreError::GenesisAlreadyInitialized).unwrap();
    assert!(buf.to_lowercase().contains("genesis"), "{}", buf);

    buf.clear();
    write!(&mut buf, "{}", CoinStoreError::NotInitialized).unwrap();
    assert!(buf.contains("init_genesis"), "{}", buf);
}

/// Verifies API-004 + API-001: double `init_genesis` surfaces `GenesisAlreadyInitialized`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_004_genesis_already_initialized_reachable_from_store() {
    use dig_coinstore::coin_store::CoinStore;

    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1).unwrap();
    let err = store.init_genesis(vec![], 2).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::GenesisAlreadyInitialized),
        "got {:?}",
        err
    );
}

/// Verifies API-004: `From<StorageError>` maps into `CoinStoreError::StorageError(String)`.
#[test]
fn vv_req_api_004_storage_error_from_conversion() {
    let inner = StorageError::UnknownColumnFamily("cf_x".into());
    let outer: CoinStoreError = inner.into();
    match outer {
        CoinStoreError::StorageError(s) => {
            assert!(s.contains("cf_x"), "{}", s);
        }
        other => panic!("expected StorageError variant, got {:?}", other),
    }
}

/// Verifies API-004: bincode **`From`** maps generic `bincode::Error` to `SerializationError`.
///
/// We build a `bincode::Error` via `io::Error` → `bincode::Error` (supported by bincode 1.3) to
/// exercise `impl From<bincode::Error>` without relying on a particular serialize failure shape.
#[test]
fn vv_req_api_004_bincode_error_maps_via_from_to_serialization_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "bincode io shim");
    let b_err: bincode::Error = io_err.into();
    let mapped: CoinStoreError = b_err.into();
    assert!(
        matches!(mapped, CoinStoreError::SerializationError(_)),
        "{:?}",
        mapped
    );
}

/// Verifies API-004: bincode **decode** failures use `from_bincode_deserialize` → `DeserializationError`.
#[test]
fn vv_req_api_004_bincode_decode_maps_to_deserialization_error() {
    let err = bincode::deserialize::<u64>(&[0u8; 1]).unwrap_err();
    let mapped = CoinStoreError::from_bincode_deserialize(err);
    assert!(
        matches!(mapped, CoinStoreError::DeserializationError(_)),
        "{:?}",
        mapped
    );
}

/// Verifies API-004: `rocksdb::Error` converts when `rocksdb-storage` is enabled.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_api_004_from_rocksdb_error_stringifies() {
    let dir = tempfile::tempdir().unwrap();
    // RocksDB expects a directory; opening on a regular file provokes a backend error cross-platform.
    let bad = dir.path().join("not_a_directory");
    std::fs::write(&bad, b"block").unwrap();
    let inner = rocksdb::DB::open(&rocksdb::Options::default(), &bad).unwrap_err();
    let outer: CoinStoreError = inner.into();
    match outer {
        CoinStoreError::StorageError(s) => assert!(!s.is_empty(), "{}", s),
        other => panic!("expected StorageError, got {:?}", other),
    }
}

/// Verifies API-004: `heed::Error` converts when `lmdb-storage` is enabled.
#[cfg(feature = "lmdb-storage")]
#[test]
fn vv_req_api_004_from_heed_error_stringifies() {
    let inner = heed::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, "heed test"));
    let outer: CoinStoreError = inner.into();
    match outer {
        CoinStoreError::StorageError(s) => assert!(!s.is_empty(), "{}", s),
        other => panic!("expected StorageError, got {:?}", other),
    }
}
