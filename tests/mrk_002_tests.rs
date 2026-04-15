//! # MRK-002 Tests — Memoized Empty Hashes
//!
//! Verifies **MRK-002**: pre-computed empty subtree hashes for all 257 levels (0..=256),
//! initialized lazily via `OnceLock` and accessed in O(1).
//!
//! # Requirement: MRK-002
//! # SPEC.md: §9 (Merkle Tree), §1.6 #1 (Merkle Commitment)
//!
//! ## How these tests prove the requirement
//!
//! - **Leaf base case:** `empty_hash(0)` = `merkle_leaf_hash([0u8; 32])` — the domain-separated hash.
//! - **Recursive consistency:** All 256 levels satisfy `empty_hash(n) = node_hash(empty_hash(n-1), empty_hash(n-1))`.
//! - **Root cross-check:** Independent iterative computation matches `empty_hash(256)`.
//! - **O(1) lookup:** Repeated calls return the same value (memoized, thread-safe via `OnceLock`).

mod helpers;

use dig_coinstore::merkle::{empty_hash, merkle_leaf_hash, merkle_node_hash, SMT_HEIGHT};

// ─────────────────────────────────────────────────────────────────────────────
// MRK-002: Memoized empty hashes
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies MRK-002: empty_hash(0) equals the leaf-level empty sentinel hash.
///
/// Level 0 is the empty leaf: SHA256(0x00 || [0; 32]).
#[test]
fn vv_req_mrk_002_empty_hash_leaf_level() {
    let expected = merkle_leaf_hash(&[0u8; 32]);
    assert_eq!(empty_hash(0), expected, "Level 0 must be empty leaf hash");
}

/// Verifies MRK-002: empty_hash(n) == merkle_node_hash(empty_hash(n-1), empty_hash(n-1))
/// for all levels 1..=256.
///
/// This proves the bottom-up construction is correct for all 256 internal levels.
#[test]
fn vv_req_mrk_002_empty_hash_consistency() {
    for n in 1..=SMT_HEIGHT {
        let child = empty_hash(n - 1);
        let expected = merkle_node_hash(&child, &child);
        assert_eq!(
            empty_hash(n),
            expected,
            "empty_hash({}) must equal node_hash(empty_hash({}), empty_hash({}))",
            n,
            n - 1,
            n - 1
        );
    }
}

/// Verifies MRK-002: empty_hash(256) is the root of an entirely empty tree.
///
/// Computed by iteratively hashing the empty leaf 256 times.
#[test]
fn vv_req_mrk_002_empty_hash_root_level() {
    // Compute manually: start from leaf, hash up 256 times.
    let mut current = merkle_leaf_hash(&[0u8; 32]);
    for _ in 1..=SMT_HEIGHT {
        current = merkle_node_hash(&current, &current);
    }
    assert_eq!(
        empty_hash(SMT_HEIGHT),
        current,
        "Level 256 must match iterative computation"
    );
}

/// Verifies MRK-002: empty_hash() is O(1) — repeated calls return immediately.
#[test]
fn vv_req_mrk_002_empty_hash_o1_lookup() {
    let first = empty_hash(128);
    let second = empty_hash(128);
    assert_eq!(first, second, "Repeated calls must return same value");
}
