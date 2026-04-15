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

// ─────────────────────────────────────────────────────────────────────────────
// HNT-002: Idempotent hint insertion
// HNT-004: Hint query methods
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};

use chia_protocol::Bytes32;

use crate::coin_store::CoinStore;
use crate::error::CoinStoreError;
use crate::storage::schema;
use crate::types::CoinId;

impl CoinStore {
    /// Insert a hint for a coin, idempotently.
    ///
    /// Writes both the forward index (`CF_HINTS`: `coin_id || hint`) and
    /// reverse index (`CF_HINTS_BY_VALUE`: `hint || coin_id`) with empty values.
    ///
    /// # Idempotency ([SPEC.md §1.5 #14](../../docs/resources/SPEC.md))
    ///
    /// If the forward key already exists, returns `Ok(())` without writing.
    /// This matches Chia's insert-or-ignore semantics.
    ///
    /// # Validation
    ///
    /// Calls [`validate_hint`] first:
    /// - Empty hint → `Ok(())` (silently skipped).
    /// - Hint > [`MAX_HINT_LENGTH`] → `Err(CoinStoreError::HintTooLong)`.
    ///
    /// # Requirement: HNT-002
    /// # Spec: docs/requirements/domains/hints/specs/HNT-002.md
    pub fn add_hint(&self, coin_id: &CoinId, hint: &[u8]) -> Result<(), CoinStoreError> {
        // HNT-001: validate hint length.
        match validate_hint(hint)? {
            HintAction::Skip => return Ok(()),
            HintAction::Store => {}
        }

        // Build the forward key: coin_id || hint (up to 64 bytes).
        let mut fwd_key = Vec::with_capacity(32 + hint.len());
        fwd_key.extend_from_slice(coin_id.as_ref());
        fwd_key.extend_from_slice(hint);

        // Idempotency check: if forward key already exists, nothing to do.
        if self.backend.get(schema::CF_HINTS, &fwd_key)?.is_some() {
            return Ok(());
        }

        // Build the reverse key: hint || coin_id (up to 64 bytes).
        let mut rev_key = Vec::with_capacity(hint.len() + 32);
        rev_key.extend_from_slice(hint);
        rev_key.extend_from_slice(coin_id.as_ref());

        // Write both indices (standalone, not batch).
        self.backend.put(schema::CF_HINTS, &fwd_key, &[])?;
        self.backend
            .put(schema::CF_HINTS_BY_VALUE, &rev_key, &[])?;

        Ok(())
    }

    /// Look up all coin IDs associated with a given 32-byte hint.
    ///
    /// Performs a prefix scan on `CF_HINTS_BY_VALUE` using `hint` as the prefix,
    /// extracts the trailing 32-byte coin ID from each key, and returns up to
    /// `max_items` results.
    ///
    /// # Requirement: HNT-004
    /// # Spec: docs/requirements/domains/hints/specs/HNT-004.md
    pub fn get_coin_ids_by_hint(
        &self,
        hint: &Bytes32,
        max_items: usize,
    ) -> Result<Vec<CoinId>, CoinStoreError> {
        let entries = self
            .backend
            .prefix_scan(schema::CF_HINTS_BY_VALUE, hint.as_ref())?;

        let mut result = Vec::with_capacity(entries.len().min(max_items));
        for (key, _value) in entries {
            if result.len() >= max_items {
                break;
            }
            // Key is `hint (32 bytes) || coin_id (32 bytes)`.
            if key.len() >= 64 {
                let mut coin_bytes = [0u8; 32];
                coin_bytes.copy_from_slice(&key[32..64]);
                result.push(CoinId::from(coin_bytes));
            }
        }
        Ok(result)
    }

    /// Batch query: look up coin IDs for multiple hints, deduplicated.
    ///
    /// Iterates over each hint, aggregates results, deduplicates, and limits
    /// to `max_items` total.
    ///
    /// # Requirement: HNT-004
    /// # Spec: docs/requirements/domains/hints/specs/HNT-004.md
    pub fn get_coin_ids_by_hints(
        &self,
        hints: &[Bytes32],
        max_items: usize,
    ) -> Result<Vec<CoinId>, CoinStoreError> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for hint in hints {
            let coin_ids = self.get_coin_ids_by_hint(hint, max_items)?;
            for cid in coin_ids {
                if result.len() >= max_items {
                    return Ok(result);
                }
                if seen.insert(cid) {
                    result.push(cid);
                }
            }
        }
        Ok(result)
    }

    /// Reverse lookup: for each coin ID, return all associated hints.
    ///
    /// Performs a prefix scan on `CF_HINTS` for each coin ID, extracts the
    /// trailing 32 bytes as the hint value.
    ///
    /// # Requirement: HNT-004
    /// # Spec: docs/requirements/domains/hints/specs/HNT-004.md
    pub fn get_hints_for_coin_ids(
        &self,
        coin_ids: &[CoinId],
    ) -> Result<HashMap<CoinId, Vec<Bytes32>>, CoinStoreError> {
        let mut result = HashMap::new();

        for coin_id in coin_ids {
            let entries = self
                .backend
                .prefix_scan(schema::CF_HINTS, coin_id.as_ref())?;

            let mut hints_for_coin = Vec::new();
            for (key, _value) in entries {
                // Key is `coin_id (32 bytes) || hint (32 bytes)`.
                if key.len() >= 64 {
                    let mut hint_bytes = [0u8; 32];
                    hint_bytes.copy_from_slice(&key[32..64]);
                    hints_for_coin.push(Bytes32::from(hint_bytes));
                }
            }
            if !hints_for_coin.is_empty() {
                result.insert(*coin_id, hints_for_coin);
            }
        }
        Ok(result)
    }

    /// Count the total number of hint entries in the forward index (`CF_HINTS`).
    ///
    /// # Requirement: HNT-004
    /// # Spec: docs/requirements/domains/hints/specs/HNT-004.md
    pub fn count_hints(&self) -> Result<u64, CoinStoreError> {
        let entries = self.backend.prefix_scan(schema::CF_HINTS, &[])?;
        Ok(entries.len() as u64)
    }
}
