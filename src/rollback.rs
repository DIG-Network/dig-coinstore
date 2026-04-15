//! Rollback pipeline for chain reorganization recovery.
//!
//! Implements `rollback_to_block()` and `rollback_n_blocks()` on [`crate::coin_store::CoinStore`]:
//! coin deletion, un-spending, FF-eligible recomputation, hint cleanup,
//! Merkle rebuild, and atomic state revert.
//!
//! # Algorithm ([SPEC.md §3.3](../../docs/resources/SPEC.md), [§1.3 #6](../../docs/resources/SPEC.md))
//!
//! Given `target_height`:
//!
//! 1. **Coin deletion (RBK-002):** Scan [`CF_COIN_BY_CONFIRMED_HEIGHT`](crate::storage::schema::CF_COIN_BY_CONFIRMED_HEIGHT)
//!    for heights `> target_height`. Delete the coin record, all secondary index entries, and the
//!    Merkle leaf.
//! 2. **Un-spending (RBK-003):** Scan [`CF_COIN_BY_SPENT_HEIGHT`](crate::storage::schema::CF_COIN_BY_SPENT_HEIGHT)
//!    for heights `> target_height`. Clear `spent_height`, re-add to unspent puzzle hash index,
//!    update the Merkle leaf hash.
//! 3. **FF-eligible recomputation (RBK-004, [SPEC.md §1.5 #4](../../docs/resources/SPEC.md)):** For un-spent coins,
//!    re-evaluate `ff_eligible` by checking whether the parent coin still exists.
//! 4. **Hint cleanup (RBK-005 / HNT-005):** Remove hint index entries for deleted coins.
//! 5. **Merkle rebuild (RBK-006, [SPEC.md §1.6 #7](../../docs/resources/SPEC.md)):** Single `root()` recomputation.
//! 6. **Atomic commit (RBK-007, [SPEC.md §1.6 #17](../../docs/resources/SPEC.md)):** Single [`WriteBatch`](crate::storage::WriteBatch).
//!
//! # Return type ([SPEC.md §1.6 #11](../../docs/resources/SPEC.md))
//!
//! [`RollbackResult`](crate::RollbackResult) includes the `modified_coins` map (matching Chia's
//! return from [`coin_store.py:rollback_to_block`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561))
//! plus enriched `coins_deleted` / `coins_unspent` counts.
//!
//! # Chia reference ([SPEC.md §1.4](../../docs/resources/SPEC.md))
//!
//! - [`coin_store.py:561-624`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L561)
//! - dig-coinstore replaces full-table scan with height-indexed CF scans.
//!
//! # Requirements: RBK-001 through RBK-007
//! # Spec: docs/requirements/domains/rollback/specs/
//! # SPEC.md: §3.3 (Rollback API), §1.5 #4 (FF recomputation), §1.6 #11 (enriched result)
