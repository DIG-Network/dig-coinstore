//! # BLK-009 Tests — State Root Verification
//!
//! Verifies requirement **BLK-009**: `apply_block()` optionally verifies the computed
//! post-apply Merkle state root against `BlockData::expected_state_root`. When `None`,
//! verification is skipped. When `Some(root)` and the computed root matches, the block
//! succeeds. When the roots disagree, `CoinStoreError::StateRootMismatch { expected, computed }`
//! is returned and the store state is unchanged (atomicity guarantee).
//!
//! # Requirement: BLK-009
//! # Spec: docs/requirements/domains/block_application/specs/BLK-009.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-009
//! # SPEC.md: §3.2 (Block Application API — expected_state_root field)
//!
//! ## How these tests prove the requirement
//!
//! - **expected_state_root = None skips verification:** A block with `expected_state_root = None`
//!   applies successfully — the pipeline does not fail even though no root was provided.
//! - **expected_state_root = Some(correct_root) succeeds:** Apply block 1 to learn the
//!   resulting state root. Then apply block 2 with `expected_state_root = Some(block_2_root)`
//!   where the root is obtained by replaying the same scenario. The block succeeds.
//! - **expected_state_root = Some(wrong_root) returns StateRootMismatch:** Providing an
//!   incorrect expected root causes the block to fail with `StateRootMismatch`.
//! - **Atomicity after StateRootMismatch:** After a `StateRootMismatch` failure, the store
//!   height, tip_hash, and coin state remain unchanged — proving the atomic rollback.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinStoreError};

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
// BLK-009: State root verification
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-009 / None skips verification:** `expected_state_root = None` does not trigger
/// any state root check — the block applies successfully.
///
/// **Proof:** Apply a block with `expected_state_root = None`. The block succeeds
/// regardless of the computed root value.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_009_none_skips_verification() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.expected_state_root = None; // explicitly None — skip verification

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Block with expected_state_root=None must succeed: {:?}",
        result.err()
    );
}

/// **BLK-009 / Correct root succeeds:** `expected_state_root = Some(correct_root)` passes.
///
/// **Proof:** Apply block 1 without root verification to learn the computed root.
/// Then, in a fresh store, apply the same block 1 with `expected_state_root = Some(root)`
/// using the root obtained from the first run. The block succeeds.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_009_correct_root_succeeds() {
    // First pass: apply block 1 without verification to learn the state root.
    let dir1 = helpers::temp_dir();
    let mut store1 = CoinStore::new(dir1.path()).unwrap();
    store1.init_genesis(vec![], 1_700_000_000).unwrap();

    let block1 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let result1 = store1.apply_block(block1).unwrap();
    let known_root = result1.state_root;

    // Second pass: apply the same block with the known root as expected.
    let dir2 = helpers::temp_dir();
    let mut store2 = CoinStore::new(dir2.path()).unwrap();
    store2.init_genesis(vec![], 1_700_000_000).unwrap();

    let mut block2 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block2.expected_state_root = Some(known_root);

    let result2 = store2.apply_block(block2);
    assert!(
        result2.is_ok(),
        "Block with correct expected_state_root must succeed: {:?}",
        result2.err()
    );
    assert_eq!(
        result2.unwrap().state_root, known_root,
        "Computed root must match the known correct root"
    );
}

/// **BLK-009 / Wrong root returns StateRootMismatch:** `expected_state_root = Some(wrong)`
/// causes `CoinStoreError::StateRootMismatch { expected, computed }`.
///
/// **Proof:** Apply a block with `expected_state_root = Some(filled_hash(0xFF))`.
/// Since the actual state root is different, the error variant is returned with
/// both the expected and computed values.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_009_wrong_root_returns_state_root_mismatch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let wrong_root = helpers::filled_hash(0xFF);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.expected_state_root = Some(wrong_root);

    let err = store.apply_block(block).unwrap_err();
    match err {
        CoinStoreError::StateRootMismatch { expected, computed } => {
            assert_eq!(
                expected, wrong_root,
                "Error must report the expected root we provided"
            );
            assert_ne!(
                computed, wrong_root,
                "Computed root must differ from the wrong expected root"
            );
        }
        other => panic!("Expected StateRootMismatch, got: {:?}", other),
    }
}

/// **BLK-009 / Atomicity after StateRootMismatch:** On failure, the store state is unchanged.
///
/// **Proof:** Record height and tip_hash before attempting a block with a wrong expected
/// state root. After the `StateRootMismatch` error, verify height and tip_hash are
/// unchanged. Also verify no new coins were persisted.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_009_atomicity_after_state_root_mismatch() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let height_before = store.height();
    let tip_before = store.tip_hash();

    // Snapshot before to count coins.
    let snap_before = store.snapshot().unwrap();
    let coin_count_before = snap_before.coins.len();

    // Attempt block with wrong expected state root.
    let wrong_root = helpers::filled_hash(0xAA);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.expected_state_root = Some(wrong_root);

    let result = store.apply_block(block);
    assert!(
        matches!(result, Err(CoinStoreError::StateRootMismatch { .. })),
        "Expected StateRootMismatch, got: {:?}",
        result
    );

    // Verify no state changes — atomicity guarantee.
    assert_eq!(
        store.height(),
        height_before,
        "Height must not change after StateRootMismatch"
    );
    assert_eq!(
        store.tip_hash(),
        tip_before,
        "Tip hash must not change after StateRootMismatch"
    );

    // Verify no new coins were added.
    let snap_after = store.snapshot().unwrap();
    assert_eq!(
        snap_after.coins.len(),
        coin_count_before,
        "No coins must be added after StateRootMismatch failure"
    );
}
