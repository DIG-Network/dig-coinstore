//! # HNT-001 Tests — Hint Validation
//!
//! Verifies requirement **HNT-001**: empty hints (0 bytes) are silently skipped, hints exceeding
//! `MAX_HINT_LENGTH` (32 bytes) are rejected with `HintTooLong`, and valid hints (1-32 bytes)
//! are accepted for storage.
//!
//! # Requirement: HNT-001
//! # Spec: docs/requirements/domains/hints/specs/HNT-001.md
//! # SPEC.md: §1.5 #13 (Hint length validation), §2.7 (MAX_HINT_LENGTH = 32)
//!
//! ## How these tests prove the requirement
//!
//! - **Empty hint → Skip:** `validate_hint(&[])` returns `HintAction::Skip`, no error.
//! - **33-byte hint → Error:** `validate_hint(&[0u8; 33])` returns `Err(HintTooLong { length: 33, max: 32 })`.
//! - **32-byte hint → Store:** `validate_hint(&[0xAB; 32])` returns `Ok(HintAction::Store)`.
//! - **Short hints (1-31 bytes) → Store:** All valid lengths accepted.
//! - **Constant:** `MAX_HINT_LENGTH == 32` is a public constant.
//!
//! ## Chia reference
//!
//! [`hint_management.py:44-48`](https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/hint_management.py#L44)
//! — zero-length skip and max-length check.

mod helpers;

use dig_coinstore::hints::{validate_hint, HintAction, MAX_HINT_LENGTH};

/// **HNT-001:** `MAX_HINT_LENGTH` constant is 32.
///
/// **Proof:** If the constant changes, this assertion fails immediately.
/// SPEC.md §2.7 defines this as 32 bytes.
#[test]
fn vv_req_hnt_001_max_hint_length_constant() {
    assert_eq!(
        MAX_HINT_LENGTH, 32,
        "MAX_HINT_LENGTH must be 32 per SPEC.md §2.7"
    );
}

/// **HNT-001:** Empty hint (0 bytes) returns `HintAction::Skip`.
///
/// **Proof:** Empty hints carry no information — they must be silently discarded,
/// not stored, and not cause an error. Chia: `hint_management.py:44` skips empty hints.
#[test]
fn vv_req_hnt_001_empty_hint_skipped() {
    let result = validate_hint(&[]);
    assert!(result.is_ok(), "Empty hint must not error");
    assert_eq!(
        result.unwrap(),
        HintAction::Skip,
        "Empty hint must return Skip"
    );
}

/// **HNT-001:** 33-byte hint returns `HintTooLong { length: 33, max: 32 }`.
///
/// **Proof:** Hints exceeding MAX_HINT_LENGTH are malformed and must be rejected.
/// Chia: `hint_management.py:48` asserts `len(hint) <= 32`.
#[test]
fn vv_req_hnt_001_hint_too_long_33() {
    let hint = vec![0u8; 33];
    let result = validate_hint(&hint);
    assert!(result.is_err(), "33-byte hint must error");
    match result.unwrap_err() {
        dig_coinstore::hints::HintError::HintTooLong { length, max } => {
            assert_eq!(length, 33);
            assert_eq!(max, 32);
        }
    }
}

/// **HNT-001:** 64-byte hint also returns `HintTooLong`.
///
/// **Proof:** Any length > 32 must be rejected, not just 33.
#[test]
fn vv_req_hnt_001_hint_too_long_64() {
    let hint = vec![0u8; 64];
    let result = validate_hint(&hint);
    assert!(result.is_err());
    match result.unwrap_err() {
        dig_coinstore::hints::HintError::HintTooLong { length, max } => {
            assert_eq!(length, 64);
            assert_eq!(max, 32);
        }
    }
}

/// **HNT-001:** 32-byte hint returns `HintAction::Store`.
///
/// **Proof:** Exactly MAX_HINT_LENGTH bytes is valid and eligible for puzzle-hash subscription.
#[test]
fn vv_req_hnt_001_hint_32_bytes_valid() {
    let hint = [0xABu8; 32];
    let result = validate_hint(&hint);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), HintAction::Store);
}

/// **HNT-001:** 1-byte hint returns `HintAction::Store`.
///
/// **Proof:** Short hints (1-31 bytes) are valid for storage, though only 32-byte hints
/// participate in puzzle-hash subscription matching.
#[test]
fn vv_req_hnt_001_hint_1_byte_valid() {
    let result = validate_hint(&[0x42]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), HintAction::Store);
}

/// **HNT-001:** 31-byte hint returns `HintAction::Store`.
///
/// **Proof:** Boundary case — one less than MAX_HINT_LENGTH is valid.
#[test]
fn vv_req_hnt_001_hint_31_bytes_valid() {
    let hint = vec![0xCDu8; 31];
    let result = validate_hint(&hint);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), HintAction::Store);
}

/// **HNT-001:** 16-byte hint returns `HintAction::Store`.
///
/// **Proof:** Mid-range valid hint length.
#[test]
fn vv_req_hnt_001_hint_16_bytes_valid() {
    let hint = vec![0xEFu8; 16];
    let result = validate_hint(&hint);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), HintAction::Store);
}
