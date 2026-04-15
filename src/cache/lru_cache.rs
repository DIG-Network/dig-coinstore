//! LRU coin record cache ([SPEC.md §1.6 #14](../../docs/resources/SPEC.md)).
//!
//! Write-through LRU cache of recently accessed [`CoinRecord`](crate::CoinRecord)s.
//! Capacity: `DEFAULT_COIN_CACHE_CAPACITY` (1M entries, ~200 MB)
//! ([SPEC.md §2.7](../../docs/resources/SPEC.md)). Cache hits avoid storage I/O entirely.
//!
//! - **Write-through** on `apply_block()` — callers never see stale data.
//! - **Full invalidation** on `rollback()` — rebuilt from storage on demand.
//!
//! Chia uses an unbounded `coin_record_cache: Dict[bytes32, CoinRecord]` with manual clearing.
//! dig-coinstore uses a bounded LRU (`lru` crate) for predictable memory usage.
//!
//! # Requirements: PRF-002
//! # Spec: docs/requirements/domains/performance/specs/PRF-002.md
//! # SPEC.md: §1.6 #14 (LRU Cache), §2.7 (DEFAULT_COIN_CACHE_CAPACITY)
