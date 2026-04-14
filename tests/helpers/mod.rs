//! Shared test utilities for dig-coinstore integration tests.
//!
//! Provides coin builder functions, temporary directory management,
//! genesis state initialization, and block builder helpers.
//!
//! All test files import this via `mod helpers;` at the top.
//!
//! # Requirement: STR-006
//! # Spec: docs/requirements/domains/crate_structure/specs/STR-006.md

use chia_protocol::{Bytes32, Coin};

/// Create a deterministic Bytes32 from a seed byte.
/// Useful for generating unique but reproducible coin IDs, puzzle hashes, etc.
#[allow(dead_code)]
pub fn test_hash(seed: u8) -> Bytes32 {
    Bytes32::from([seed; 32])
}

/// Create a test coin with deterministic fields.
///
/// `parent_seed`, `puzzle_seed`: single bytes expanded to 32-byte hashes.
/// `amount`: coin value in mojos.
#[allow(dead_code)]
pub fn test_coin(parent_seed: u8, puzzle_seed: u8, amount: u64) -> Coin {
    Coin::new(test_hash(parent_seed), test_hash(puzzle_seed), amount)
}
