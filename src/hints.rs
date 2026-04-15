//! Hint store for puzzle hash hints on coins.
//!
//! Manages the bidirectional hint index: forward (coin_id → hints) and
//! reverse (hint → coin_ids). Supports hint validation, idempotent insertion,
//! variable-length keys, and rollback cleanup.
//!
//! # What are hints? ([SPEC.md §3.9](../../docs/resources/SPEC.md))
//!
//! In the Chia coinset model, hints are optional 32-byte values emitted by `CREATE_COIN`
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
//! - Hints > 32 bytes → [`CoinStoreError::HintTooLong`](crate::CoinStoreError::HintTooLong)
//! - Empty hints → silently skipped
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
