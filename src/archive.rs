//! Tiered spent coin archival for dig-coinstore.
//!
//! Manages the hot/archive/prune tiers for spent coin records. Coins spent
//! beyond the rollback window ([SPEC.md §2.7 `DEFAULT_ROLLBACK_WINDOW`](../../docs/resources/SPEC.md))
//! are migrated from the hot tier (full indexing) to the archive tier (coin ID
//! only) as a background operation.
//!
//! # Three-tier model ([SPEC.md §1.6 #12](../../docs/resources/SPEC.md))
//!
//! | Tier | CF | Indexed by | Rollback? |
//! |------|---|-----------|-----------|
//! | **Hot** | [`CF_COIN_RECORDS`](crate::storage::schema::CF_COIN_RECORDS) + all secondary CFs | puzzle hash, parent, height | Yes |
//! | **Archive** | [`CF_ARCHIVE_COIN_RECORDS`](crate::storage::schema::CF_ARCHIVE_COIN_RECORDS) | coin ID only | No |
//! | **Prune** | Deleted | None | No |
//!
//! Chia does not tier spent coins — all historical records remain in SQLite indefinitely.
//! dig-coinstore's tiered model keeps the hot index small while retaining historical access by ID.
//!
//! # Requirements: PRF-005
//! # Spec: docs/requirements/domains/performance/specs/PRF-005.md
//! # SPEC.md: §1.6 #12 (Tiered Archival), §2.7 (DEFAULT_ROLLBACK_WINDOW)
