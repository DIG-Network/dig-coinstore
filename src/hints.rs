//! Hint store for puzzle hash hints on coins.
//!
//! Manages the bidirectional hint index: forward (coin_id → hints) and
//! reverse (hint → coin_ids). Supports hint validation, idempotent insertion,
//! variable-length keys, and rollback cleanup.
//!
//! # What are hints? ([SPEC.md §3.9](../../docs/resources/SPEC.md))
//!
//! In the Chia coinset model, hints are optional byte sequences emitted by `CREATE_COIN`
//! conditions that signal which wallet/puzzle hash a coin is "intended for." The hint store
//! indexes these so wallet subscriptions can find relevant coins in O(1).
//!
//! # Storage layout ([SPEC.md §7.2](../../docs/resources/SPEC.md))
//!
//! | Direction | CF | Key | Value |
//! |-----------|---|-----|-------|
//! | Forward | [`CF_HINTS`](crate::storage::schema::CF_HINTS) | `coin_id \|\| hint` | empty |
//! | Reverse | [`CF_HINTS_BY_VALUE`](crate::storage::schema::CF_HINTS_BY_VALUE) | `hint \|\| coin_id` | empty |
//!
//! # Validation rules ([SPEC.md §1.5 #13](../../docs/resources/SPEC.md), [§2.7 `MAX_HINT_LENGTH`](../../docs/resources/SPEC.md))
//!
//! - Hints > 32 bytes → [`HintError::HintTooLong`]
//! - Empty hints (0 bytes) → silently skipped ([`HintAction::Skip`])
//! - Valid hints (1-32 bytes) → accepted for storage ([`HintAction::Store`])
//! - Only 32-byte hints participate in puzzle-hash subscription matching
//! - Insertion is **idempotent** ([SPEC.md §1.5 #14](../../docs/resources/SPEC.md)): re-insert = no-op
//!
//! # Chia reference ([SPEC.md §1.4](../../docs/resources/SPEC.md))
//!
//! - [`hint_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/hint_store.py)
//! - [`hint_management.py:44-48`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py#L44)
//!
//! # Requirements: HNT-001 through HNT-006
//! # Spec: docs/requirements/domains/hints/specs/
//! # SPEC.md: §3.9 (Hint Query API), §1.5 #13,14 (Adopted Chia Behaviors)

// ─────────────────────────────────────────────────────────────────────────────
// HNT-001: Hint validation constants, types, and function
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum allowed hint length in bytes ([SPEC.md §2.7](../../docs/resources/SPEC.md)).
///
/// Matches Chia's `hint_management.py:44` assertion `assert len(hint) <= 32`.
/// Only 32-byte hints are eligible for puzzle-hash subscription matching
/// in [`batch_coin_states_by_puzzle_hashes`](crate::coin_store::CoinStore) (QRY-007).
pub const MAX_HINT_LENGTH: usize = 32;

/// Result of hint validation ([HNT-001](../../docs/requirements/domains/hints/specs/HNT-001.md)).
///
/// Determines whether a hint should be stored or silently discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintAction {
    /// The hint is 0 bytes — silently ignore, do not store.
    /// Chia: `hint_management.py:44` skips zero-length hints.
    Skip,
    /// The hint is valid (1-32 bytes) — proceed with storage.
    /// Only 32-byte hints participate in puzzle-hash subscription matching.
    Store,
}

/// Errors from hint validation and hint store operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HintError {
    /// Hint exceeds [`MAX_HINT_LENGTH`] bytes.
    /// Chia: `hint_management.py:48` asserts `len(hint) <= 32`.
    #[error("hint too long: {length} bytes exceeds maximum {max}")]
    HintTooLong {
        /// Actual hint length in bytes.
        length: usize,
        /// Maximum allowed length ([`MAX_HINT_LENGTH`]).
        max: usize,
    },
}

/// Validate a hint byte slice per [HNT-001](../../docs/requirements/domains/hints/specs/HNT-001.md).
///
/// # Rules ([SPEC.md §1.5 #13](../../docs/resources/SPEC.md))
///
/// 1. Empty hint (0 bytes) → `Ok(HintAction::Skip)` — silently discard.
/// 2. Hint > [`MAX_HINT_LENGTH`] (32 bytes) → `Err(HintError::HintTooLong)`.
/// 3. Hint 1-32 bytes → `Ok(HintAction::Store)` — valid for storage.
///
/// # Chia reference
///
/// [`hint_management.py:44-48`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/hint_management.py#L44):
/// ```python
/// if len(hint) == 0:
///     continue
/// assert len(hint) <= 32
/// ```
pub fn validate_hint(hint: &[u8]) -> Result<HintAction, HintError> {
    if hint.is_empty() {
        return Ok(HintAction::Skip);
    }
    if hint.len() > MAX_HINT_LENGTH {
        return Err(HintError::HintTooLong {
            length: hint.len(),
            max: MAX_HINT_LENGTH,
        });
    }
    Ok(HintAction::Store)
}
