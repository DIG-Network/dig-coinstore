//! Materialized aggregate counters ([SPEC.md §1.6 #18](../../docs/resources/SPEC.md)).
//!
//! Running totals of `unspent_count`, `spent_count`, and `total_value` updated atomically
//! in the same [`WriteBatch`](crate::storage::WriteBatch) as block application
//! ([SPEC.md §1.6 #17](../../docs/resources/SPEC.md)). Stored in
//! [`CF_METADATA`](crate::storage::schema::CF_METADATA) for persistence across restarts.
//!
//! Provides O(1) reads for `num_unspent()`, `total_unspent_value()`, and
//! [`stats()`](crate::coin_store::CoinStore::stats) ([SPEC.md §3.11-§3.12](../../docs/resources/SPEC.md))
//! instead of full-table scans.
//!
//! Until PRF-003 lands, [`CoinStore::stats`](crate::coin_store::CoinStore::stats) derives these
//! by scanning [`CF_COIN_RECORDS`](crate::storage::schema::CF_COIN_RECORDS). Chia computes via
//! SQL `COUNT(*)` / `SUM()` — also O(N) ([SPEC.md §1.4](../../docs/resources/SPEC.md), `coin_store.py:96-103`).
//!
//! # Requirements: PRF-003
//! # Spec: docs/requirements/domains/performance/specs/PRF-003.md
//! # SPEC.md: §1.6 #18 (Materialized Counters), §2.7 (MATERIALIZATION_BATCH_SIZE)
