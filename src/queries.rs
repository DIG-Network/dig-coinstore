//! Query method implementations for dig-coinstore.
//!
//! All coin state query methods on [`crate::coin_store::CoinStore`]: by ID, puzzle hash, height,
//! parent, hint, batch pagination, singleton lineage, and aggregates.
//!
//! # Query method index ([SPEC.md §3.4-§3.11](../../docs/resources/SPEC.md))
//!
//! | Method | SPEC | CF scanned | Chia ref |
//! |--------|------|-----------|----------|
//! | `get_coin_record(id)` | §3.4 | [`CF_COIN_RECORDS`](crate::storage::schema::CF_COIN_RECORDS) | `coin_store.py:181` |
//! | `get_coin_records_by_puzzle_hash(ph)` | §3.5 | [`CF_COIN_BY_PUZZLE_HASH`](crate::storage::schema::CF_COIN_BY_PUZZLE_HASH) | `coin_store.py:257` |
//! | `get_coins_added_at_height(h)` | §3.6 | [`CF_COIN_BY_CONFIRMED_HEIGHT`](crate::storage::schema::CF_COIN_BY_CONFIRMED_HEIGHT) | `coin_store.py:223` |
//! | `get_coins_removed_at_height(h)` | §3.6 | [`CF_COIN_BY_SPENT_HEIGHT`](crate::storage::schema::CF_COIN_BY_SPENT_HEIGHT) | `coin_store.py:238` |
//! | `get_coin_records_by_parent_ids(ids)` | §3.7 | [`CF_COIN_BY_PARENT`](crate::storage::schema::CF_COIN_BY_PARENT) | `coin_store.py:380` |
//! | `batch_coin_states_by_puzzle_hashes(...)` | §3.5 | `CF_COIN_BY_PUZZLE_HASH` + [`CoinStateFilters`](crate::CoinStateFilters) | `coin_store.py:446` |
//! | `get_unspent_lineage_info_for_puzzle_hash(ph)` | §3.10 | [`CF_UNSPENT_BY_PUZZLE_HASH`](crate::storage::schema::CF_UNSPENT_BY_PUZZLE_HASH) | `coin_store.py:651` |
//! | `num_unspent()`, `total_unspent_value()` | §3.11 | materialized counters or scan | `coin_store.py:96` |
//! | `height()`, `tip_hash()`, `state_root()`, `stats()` | §3.12 | in-memory accessors | — |
//!
//! # Adopted Chia behaviors ([SPEC.md §1.5](../../docs/resources/SPEC.md))
//!
//! - **Block boundary pagination** (§1.5 #5): `batch_coin_states_by_puzzle_hashes` never splits blocks.
//! - **Deduplication** (§1.5 #6): dict-keyed by coin_id across direct + hinted results.
//! - **Deterministic sort** (§1.5 #7): `MAX(confirmed_height, spent_height) ASC`.
//! - **Batch size limit** (§1.5 #8): `MAX_PUZZLE_HASH_BATCH_SIZE` ([SPEC.md §2.7](../../docs/resources/SPEC.md)).
//! - **`min_amount` filter** (§1.5 #9): dust coin exclusion.
//! - **Large input batching** (§1.5 #10): chunk by `DEFAULT_LOOKUP_BATCH_SIZE` ([SPEC.md §2.7](../../docs/resources/SPEC.md)).
//!
//! # Requirements: QRY-001 through QRY-011
//! # Spec: docs/requirements/domains/queries/specs/
//! # SPEC.md: §3.4-§3.12 (Query APIs), §1.5 #5-10 (Adopted Chia Behaviors)
