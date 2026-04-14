//! # dig-coinstore
//!
//! Persistent global coin state database for the DIG Network L2 blockchain.
//!
//! This crate manages the authoritative database of all spent and unspent coins
//! across the entire blockchain using the coinset model (UTXO-like). It accepts
//! validated blocks as input, applies their state transitions, and provides a
//! rich query API for coin lookups by ID, puzzle hash, hint, parent, and height.
//!
//! ## Crate boundary
//!
//! - **Input:** Pre-validated `BlockData` (additions, removals, coinbase, hints)
//! - **Output:** `CoinRecord`s, `CoinState`s, state roots, Merkle proofs
//! - **Not in scope:** CLVM execution, block production, networking, consensus
//!
//! ## Storage backends
//!
//! Storage is feature-gated:
//! - `rocksdb-storage` (default) — RocksDB with bloom filters and column families
//! - `lmdb-storage` — LMDB with memory-mapped I/O
//! - `full-storage` — Both backends enabled
//!
//! ## Spec reference
//!
//! Master specification: `docs/resources/SPEC.md`
//! Requirements: `docs/requirements/IMPLEMENTATION_ORDER.md`
//!
//! See also:
//! - Chia CoinStore: `chia/full_node/coin_store.py`
//! - Chia HintStore: `chia/full_node/hint_store.py`

// Placeholder module declarations — these will be populated as requirements
// are implemented in later phases. Each module maps to a domain in the
// requirements structure (docs/requirements/domains/).
//
// Phase 0 (STR-001) establishes Cargo.toml; subsequent STR requirements
// will flesh out these modules.
