//! Domain types for dig-coinstore.
//!
//! Core data structures: [`CoinRecord`], [`BlockData`], [`CoinAddition`],
//! [`ApplyBlockResult`], [`RollbackResult`], [`CoinStoreStats`],
//! [`CoinStoreSnapshot`], [`UnspentLineageInfo`].
//!
//! Also defines type aliases: `CoinId = Bytes32`, `PuzzleHash = Bytes32`.
//!
//! # Requirements: API-002, API-005, API-006, API-007, API-008, API-009
//! # Spec: docs/requirements/domains/crate_api/specs/
