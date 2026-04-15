//! # MRK-002 Tests — Memoized empty subtree hashes (`OnceLock<[Bytes32; 257]>`)
//!
//! **Normative:** [`MRK-002`](../../docs/requirements/domains/merkle/NORMATIVE.md#MRK-002)
//! **Spec:** [`MRK-002.md`](../../docs/requirements/domains/merkle/specs/MRK-002.md) (data structure, behavior §, test plan)
//! **Implementation:** [`dig_coinstore::merkle::empty_hash`](../../src/merkle/mod.rs) — 257 precomputed digests, lazy init, O(1) lookup.
//!
//! ## What MRK-002 mandates (and how we prove it)
//!
//! | MRK-002 rule | Evidence |
//! |--------------|----------|
//! | `OnceLock<[Bytes32; 257]>` static, single bottom-up init | Production type in `src/merkle/mod.rs`; tests assume identical outputs vs naive recompute. |
//! | `empty_hash(level)` is O(1) index | [`vv_req_mrk_002_empty_hash_o1_lookup`], [`vv_req_mrk_002_concurrent_reads_match_serial`] |
//! | Leaf index 0 = sentinel leaf hash | [`vv_req_mrk_002_empty_hash_leaf_level`] |
//! | Index 256 = full empty 256-level tree root | [`vv_req_mrk_002_empty_hash_root_level`] |
//! | Each level `n>0` = `node(child, child)` | [`vv_req_mrk_002_empty_hash_consistency`] |
//! | Table matches naive recursion (no memo at query time in naive path) | [`vv_req_mrk_002_empty_hash_matches_recursive`] |
//! | Thread-safe lazy init (`std::sync::OnceLock`) | [`vv_req_mrk_002_concurrent_reads_match_serial`] — concurrent readers after barrier all agree on every level. |
//! | Out-of-range `level` rejected | [`vv_req_mrk_002_empty_hash_panics_above_smt_height`] |
//!
//! **SocratiCode:** not wired for this crate; discovery used Repomix + `gitnexus impact empty_hash`.
//! **GitNexus:** `empty_hash` has broad callers (CRITICAL blast radius); this change preserves digest values, only the backing static type.

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{empty_hash, merkle_leaf_hash, merkle_node_hash, SMT_HEIGHT};
use std::sync::{Arc, Barrier};
use std::thread;

// ─────────────────────────────────────────────────────────────────────────────
// MRK-002: Memoized empty hashes
// Requirement: docs/requirements/domains/merkle/specs/MRK-002.md
// ─────────────────────────────────────────────────────────────────────────────

/// Naive reference: recompute empty subtree at `level` by recursion (no global memo).
///
/// **Indexing contract (matches production):** `level == 0` is the empty leaf; `level == n`
/// hashes two copies of `level - 1`. **Expensive** — O(level) hashes — used only to cross-check
/// the precomputed table, never in hot paths.
fn naive_empty_subtree(level: usize) -> Bytes32 {
    assert!(level <= SMT_HEIGHT);
    if level == 0 {
        merkle_leaf_hash(&[0u8; 32])
    } else {
        let child = naive_empty_subtree(level - 1);
        merkle_node_hash(&child, &child)
    }
}

/// **MRK-002 / test plan `test_empty_hash_leaf_level`:** `empty_hash(0)` is the domain-separated
/// hash of the all-zero empty leaf sentinel (`0x00 || sentinel` per [`merkle_leaf_hash`]).
#[test]
fn vv_req_mrk_002_empty_hash_leaf_level() {
    let expected = merkle_leaf_hash(&[0u8; 32]);
    assert_eq!(empty_hash(0), expected, "Level 0 must be empty leaf hash");
}

/// **MRK-002 / `test_empty_hash_consistency`:** For every `n` in `1..=256`, `empty_hash(n)`
/// equals `merkle_node_hash(empty_hash(n-1), empty_hash(n-1))`.
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

/// **MRK-002 / `test_empty_hash_root_level`:** `empty_hash(SMT_HEIGHT)` matches an independent
/// iterative fold (same recurrence, no shared static).
#[test]
fn vv_req_mrk_002_empty_hash_root_level() {
    let mut current = merkle_leaf_hash(&[0u8; 32]);
    for _ in 1..=SMT_HEIGHT {
        current = merkle_node_hash(&current, &current);
    }
    assert_eq!(
        empty_hash(SMT_HEIGHT),
        current,
        "Level {} must match iterative computation",
        SMT_HEIGHT
    );
}

/// **MRK-002 / `test_empty_hash_o1_lookup`:** Repeated reads at the same level are stable
/// (memoized table; no per-call recompute of the 257-entry chain).
#[test]
fn vv_req_mrk_002_empty_hash_o1_lookup() {
    let first = empty_hash(128);
    let second = empty_hash(128);
    assert_eq!(first, second, "Repeated calls must return same value");
}

/// **MRK-002 / `test_empty_hash_matches_recursive`:** Every `empty_hash(k)` matches
/// [`naive_empty_subtree`]`(k)` — proves the static table equals the mathematical definition
/// without relying on internal implementation details beyond the public API.
#[test]
fn vv_req_mrk_002_empty_hash_matches_recursive() {
    for k in 0..=SMT_HEIGHT {
        assert_eq!(
            empty_hash(k),
            naive_empty_subtree(k),
            "empty_hash({}) must match naive recursion",
            k
        );
    }
}

/// **MRK-002 / `test_once_lock_thread_safety` (adapted):** Many threads read the table after a
/// barrier so they contend on the same levels concurrently. **Proof obligation:** no torn reads,
/// all digests match the serial baseline. Rust's [`std::sync::OnceLock`] additionally guarantees
/// a single successful initialization of the static (see `get_or_init` docs); we cannot observe
/// "init count" without test-only hooks, so this test stresses **concurrent read coherence**.
#[test]
fn vv_req_mrk_002_concurrent_reads_match_serial() {
    const THREADS: usize = 16;
    let barrier = Arc::new(Barrier::new(THREADS));
    let mut handles = Vec::with_capacity(THREADS);

    for tid in 0..THREADS {
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            // All workers meet here, then hammer `empty_hash` in parallel.
            b.wait();
            for level in 0..=SMT_HEIGHT {
                let got = empty_hash(level);
                let want = naive_empty_subtree(level);
                assert_eq!(
                    got, want,
                    "thread {} level {}: concurrent read must match naive reference",
                    tid, level
                );
            }
        }));
    }

    for h in handles {
        h.join().expect("worker must not panic");
    }
}

/// **MRK-002 implementation notes:** `empty_hash` panics when `level > SMT_HEIGHT` (spec: out of range).
#[test]
#[should_panic(expected = "exceeds SMT_HEIGHT")]
fn vv_req_mrk_002_empty_hash_panics_above_smt_height() {
    let _ = empty_hash(SMT_HEIGHT + 1);
}
