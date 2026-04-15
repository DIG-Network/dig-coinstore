//! Block application pipeline for dig-coinstore.
//!
//! Implements the `apply_block()` method on [`crate::coin_store::CoinStore`]: Phase 1 validation
//! (height, parent hash, reward coins, removals, additions, hints) followed by
//! Phase 2 atomic mutation (coin insertion, spend marking, hint storage,
//! Merkle update, chain tip commit).
//!
//! # Two-phase design ([SPEC.md §3.2](../../docs/resources/SPEC.md), [§1.5 #5](../../docs/resources/SPEC.md))
//!
//! The pipeline deliberately separates **validation** from **mutation** so that an invalid block
//! can never leave the store in a partially-updated state ([SPEC.md §1.3 #5](../../docs/resources/SPEC.md)):
//!
//! ## Phase 1 — Validation (no writes)
//!
//! | Step | Check | Error | Req |
//! |------|-------|-------|-----|
//! | BLK-002 | `block.height == self.height + 1` | [`HeightMismatch`](crate::CoinStoreError::HeightMismatch) | BLK-002 |
//! | BLK-003 | `block.parent_hash == self.tip_hash` | [`ParentHashMismatch`](crate::CoinStoreError::ParentHashMismatch) | BLK-003 |
//! | BLK-004 | genesis: 0 rewards; non-genesis: ≥ 2 ([SPEC.md §1.5 #11](../../docs/resources/SPEC.md)) | [`InvalidRewardCoinCount`](crate::CoinStoreError::InvalidRewardCoinCount) | BLK-004 |
//! | BLK-005 | each removal exists + unspent ([SPEC.md §1.5 #1,2](../../docs/resources/SPEC.md)) | [`CoinNotFound`](crate::CoinStoreError::CoinNotFound) / [`DoubleSpend`](crate::CoinStoreError::DoubleSpend) | BLK-005 |
//! | BLK-006 | each addition not already present | [`CoinAlreadyExists`](crate::CoinStoreError::CoinAlreadyExists) | BLK-006 |
//! | BLK-011 | hint length ≤ 32, skip empty ([SPEC.md §1.5 #13](../../docs/resources/SPEC.md)) | [`HintTooLong`](crate::CoinStoreError::HintTooLong) | BLK-011 |
//!
//! ## Phase 2 — Mutation (atomic [`WriteBatch`](crate::storage::WriteBatch), [SPEC.md §1.6 #17](../../docs/resources/SPEC.md))
//!
//! | Step | Effect | Req |
//! |------|--------|-----|
//! | BLK-007 | Insert coinbase + addition [`CoinRecord`](crate::CoinRecord) rows; set `ff_eligible` per `same_as_parent` ([SPEC.md §1.5 #3](../../docs/resources/SPEC.md)) | BLK-007 |
//! | BLK-008 | Mark removals as spent; strict count assertion ([SPEC.md §1.5 #1](../../docs/resources/SPEC.md)) | BLK-008 |
//! | BLK-012 | Persist `(coin_id, hint)` pairs into forward + reverse hint indices | BLK-012 |
//! | BLK-013 | Batch update sparse Merkle tree; single `root()` recomputation ([SPEC.md §1.6 #7](../../docs/resources/SPEC.md)) | BLK-013 |
//! | BLK-014 | Swap `height`, `tip_hash`, `timestamp` in metadata CF | BLK-014 |
//!
//! ## Performance target (BLK-010, [SPEC.md §1.5 #15](../../docs/resources/SPEC.md), [§2.7 `BLOCK_APPLY_WARN_SECONDS`](../../docs/resources/SPEC.md))
//!
//! Blocks taking > 10 seconds emit a `tracing::warn!` with timing and coin counts.
//!
//! # Chia reference ([SPEC.md §1.4](../../docs/resources/SPEC.md))
//!
//! The pipeline mirrors `coin_store.py:new_block()` ([`coin_store.py:105-178`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L105)).
//! dig-coinstore replaces SQL with [`WriteBatch`](crate::storage::WriteBatch) and adds Merkle
//! commitment ([SPEC.md §1.6 #1](../../docs/resources/SPEC.md)) and hint validation.
//!
//! # Requirements: BLK-001 through BLK-014
//! # Spec: docs/requirements/domains/block_application/specs/
//! # SPEC.md: §3.2 (Block Application API), §1.5 (Adopted Chia Behaviors)
