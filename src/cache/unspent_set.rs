//! In-memory unspent coin ID set ([SPEC.md §1.6 #13](../../docs/resources/SPEC.md)).
//!
//! `HashSet<CoinId>` providing O(1) "is this coin unspent?" checks during BLK-005
//! removal validation. At ~40 bytes per coin, 10M unspent coins consume ~400 MB.
//!
//! Maintained incrementally: insert on creation, remove on spend,
//! re-insert on rollback un-spend. Populated from
//! [`CF_UNSPENT_BY_PUZZLE_HASH`](crate::storage::schema::CF_UNSPENT_BY_PUZZLE_HASH)
//! on startup in `MATERIALIZATION_BATCH_SIZE` chunks ([SPEC.md §2.7](../../docs/resources/SPEC.md)).
//!
//! Chia's `CoinStore` has no in-memory unspent set — each check hits SQLite
//! ([SPEC.md §1.4](../../docs/resources/SPEC.md), `coin_store.py:96-103`).
//!
//! # Requirements: PRF-001
//! # Spec: docs/requirements/domains/performance/specs/PRF-001.md
//! # SPEC.md: §1.6 #13 (In-Memory Unspent Set), §2.7 (MATERIALIZATION_BATCH_SIZE)
