//! Query method implementations for dig-coinstore.
//!
//! All coin state query methods on [`CoinStore`]: by ID, puzzle hash, height,
//! parent, hint, batch pagination, singleton lineage, and aggregates.
//!
//! # Requirements: QRY-001 through QRY-011
//! # Spec: docs/requirements/domains/queries/specs/
//! # SPEC.md: Section 3 (Public API, query methods 3.4-3.14)
//! # Chia reference: coin_store.py, coin_store_protocol.py
