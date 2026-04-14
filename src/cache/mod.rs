//! In-memory caching layer for dig-coinstore.
//!
//! Three complementary caches that accelerate the hot path:
//! - [`unspent_set`]: `HashSet<CoinId>` for O(1) "is unspent?" checks
//! - [`lru_cache`]: LRU `CoinRecord` cache for repeat point lookups
//! - [`counters`]: Materialized aggregate counters (unspent_count, total_value)
//!
//! All caches are maintained incrementally during block application and
//! rollback, and populated from storage on startup.
//!
//! # Requirements: PRF-001, PRF-002, PRF-003
//! # Spec: docs/requirements/domains/performance/specs/

pub mod counters;
pub mod lru_cache;
pub mod unspent_set;
