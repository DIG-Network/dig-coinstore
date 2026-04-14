//! CoinStore — the primary public API struct for dig-coinstore.
//!
//! Orchestrates block application, rollback, queries, hints, caching,
//! and Merkle tree operations. All public methods are defined here or
//! on `impl CoinStore` blocks in domain-specific modules.
//!
//! # Requirement: API-001
//! # Spec: docs/requirements/domains/crate_api/specs/API-001.md
