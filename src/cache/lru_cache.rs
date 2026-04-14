//! LRU coin record cache.
//!
//! Write-through LRU cache of recently accessed `CoinRecord`s.
//! Capacity: `DEFAULT_COIN_CACHE_CAPACITY` (1M entries, ~200 MB).
//! Cache hits avoid storage I/O entirely.
//!
//! Write-through on `apply_block()`, full invalidation on `rollback()`.
//! Not persisted — rebuilt from storage on demand.
//!
//! # Requirements: PRF-002
//! # Spec: docs/requirements/domains/performance/specs/PRF-002.md
//! # SPEC.md: Section 13.3 (LRU Coin Record Cache)
