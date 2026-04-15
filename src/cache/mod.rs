//! In-memory caching layer for dig-coinstore.
//!
//! Three complementary caches that accelerate the hot path
//! ([SPEC.md §1.6 #13,14,18](../../docs/resources/SPEC.md)):
//!
//! - [`unspent_set`]: `HashSet<CoinId>` for O(1) "is unspent?" checks (§1.6 #13)
//! - [`lru_cache`]: LRU `CoinRecord` cache for repeat point lookups (§1.6 #14)
//! - [`counters`]: Materialized aggregate counters — unspent_count, total_value (§1.6 #18)
//!
//! All caches are maintained incrementally during block application and
//! rollback, and populated from storage on startup.
//!
//! # Requirements: PRF-001, PRF-002, PRF-003
//! # Spec: docs/requirements/domains/performance/specs/
//! # SPEC.md: §1.6 #13,14,18 (Performance Improvements Over Chia)

pub mod counters;
pub mod lru_cache;
pub mod unspent_set;
