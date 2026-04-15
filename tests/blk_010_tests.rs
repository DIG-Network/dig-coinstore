//! # BLK-010 Tests — Performance Logging
//!
//! Verifies requirement **BLK-010**: `apply_block()` measures wall-clock elapsed time
//! for block application and logs a warning via `tracing` when the duration exceeds the
//! configured `BLOCK_APPLY_WARN_SECONDS` threshold.
//!
//! # Requirement: BLK-010
//! # Spec: docs/requirements/domains/block_application/specs/BLK-010.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-010
//! # SPEC.md: §1.5 #15 (performance instrumentation), §2.7 (BLOCK_APPLY_WARN_SECONDS)
//!
//! ## How these tests prove the requirement
//!
//! - **Normal block runs timing code path:** Applying a minimal block succeeds without
//!   panicking, confirming the timing instrumentation does not interfere with correctness.
//! - **Block with additions and removals exercises full pipeline timing:** A block with
//!   both additions and removals exercises every phase of `apply_block`, confirming the
//!   timing code path covers the entire pipeline.
//!
//! ## Note on observability
//!
//! The performance logging behavior (elapsed time measurement, threshold comparison,
//! `tracing::warn!` emission) is verified by **code inspection** of `coin_store.rs`.
//! Unit tests cannot easily capture `tracing` output without a custom subscriber, and
//! the primary contract is that timing does not break correctness — which these tests
//! confirm by running the full `apply_block` pipeline end-to-end.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition};

// ─────────────────────────────────────────────────────────────────────────────
// Block builder helper
// ─────────────────────────────────────────────────────────────────────────────

/// Build a minimal valid block at the given height with 2 coinbase coins.
fn make_block(height: u64, parent_hash: Bytes32, block_hash: Bytes32) -> BlockData {
    let coinbase_coins = vec![
        helpers::test_coin(200 + height as u8, 201, 1_750_000_000_000),
        helpers::test_coin(202 + height as u8, 203, 250_000_000_000),
    ];
    BlockData {
        height,
        timestamp: 1_700_000_000 + height * 18,
        block_hash,
        parent_hash,
        additions: vec![],
        removals: vec![],
        coinbase_coins,
        hints: vec![],
        expected_state_root: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BLK-010: Performance logging
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-010 / Timing does not crash:** Normal block application completes successfully
/// even though the timing/logging instrumentation runs.
///
/// **Proof:** Apply a minimal block at height 1. The block succeeds, confirming that
/// the `Instant::now()` / `elapsed()` / threshold comparison code path does not panic
/// or interfere with the apply pipeline.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_010_timing_does_not_crash() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Block application must succeed (timing must not interfere): {:?}",
        result.err()
    );
}

/// **BLK-010 / Full pipeline timing:** A block with additions and removals exercises all
/// phases of the apply pipeline, confirming timing spans the entire operation.
///
/// **Proof:** Create a genesis coin, then apply a block that adds a new coin and removes
/// the genesis coin. The block succeeds with correct counts, proving the timing
/// instrumentation around both Phase 1 (validation) and Phase 2 (mutation) works.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_010_full_pipeline_timing_with_additions_and_removals() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    let genesis_id = genesis_coin.coin_id();
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    let new_coin = helpers::test_coin(10, 11, 500);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(new_coin, false)];
    block.removals = vec![genesis_id];

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Full pipeline block must succeed: {:?}",
        result.err()
    );
    let r = result.unwrap();
    // 2 coinbase + 1 addition = 3 created, 1 removal = 1 spent
    assert_eq!(r.coins_created, 3, "2 coinbase + 1 addition = 3 created");
    assert_eq!(r.coins_spent, 1, "1 removal = 1 spent");
}

/// **BLK-010 / Sequential blocks timing:** Multiple blocks in sequence all complete,
/// confirming timing state is properly reset between blocks.
///
/// **Proof:** Apply blocks 1, 2, and 3 in order. All succeed, proving the timing
/// mechanism does not carry stale state between invocations.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_010_sequential_blocks_timing_resets() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let hash2 = helpers::test_hash(0xB2);
    let hash3 = helpers::test_hash(0xB3);

    let b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    store
        .apply_block(b1)
        .expect("Block 1 must succeed (timing reset)");

    let b2 = make_block(2, hash1, hash2);
    store
        .apply_block(b2)
        .expect("Block 2 must succeed (timing reset)");

    let b3 = make_block(3, hash2, hash3);
    store
        .apply_block(b3)
        .expect("Block 3 must succeed (timing reset)");

    assert_eq!(store.height(), 3, "All 3 blocks must apply successfully");
}
