//! # BLK-002 Tests — Height Continuity Validation
//!
//! Verifies requirement **BLK-002**: `apply_block()` MUST reject a block whose
//! `height` field is not exactly `current_height + 1`. The store returns
//! `CoinStoreError::HeightMismatch { expected, got }` on violation.
//!
//! # Requirement: BLK-002
//! # Spec: docs/requirements/domains/block_application/specs/BLK-002.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-002
//! # SPEC.md: §3.2 (Block Application API), §1.1 (Chain validation on insert)
//!
//! ## How these tests prove the requirement
//!
//! - **Correct height succeeds:** A block at height `current + 1` applies without error,
//!   confirming the happy path is not accidentally rejected.
//! - **Too-high height fails:** A block at height `current + 5` returns `HeightMismatch`
//!   with `expected = current + 1` and `got = current + 5`.
//! - **Too-low height fails:** A block at height 0 (re-submitting genesis height) after
//!   genesis returns `HeightMismatch` with `expected = 1, got = 0`.
//! - **Duplicate height fails:** Applying two blocks at the same height returns
//!   `HeightMismatch` on the second attempt.
//! - **Gap of 2 fails:** After applying block 1, a block at height 3 (skipping 2) returns
//!   `HeightMismatch { expected: 2, got: 3 }`.

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
// BLK-002: Height continuity
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-002 / Happy path:** Block at exactly `current_height + 1` succeeds.
///
/// **Proof:** After `init_genesis` (height 0), a block at height 1 with correct
/// parent hash applies successfully, and store height advances to 1.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_002_correct_height_succeeds() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Block at height 1 after genesis must succeed: {:?}",
        result.err()
    );
    assert_eq!(store.height(), 1);
}

/// **BLK-002 / Too-high height:** Block with `height = current + 5` returns `HeightMismatch`.
///
/// **Proof:** After genesis (height 0), attempting to apply a block at height 5 returns
/// `HeightMismatch { expected: 1, got: 5 }`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_002_too_high_height_fails() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block(5, Bytes32::from([0u8; 32]), helpers::test_hash(0xB5));
    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(
            err,
            CoinStoreError::HeightMismatch {
                expected: 1,
                got: 5
            }
        ),
        "Expected HeightMismatch {{ expected: 1, got: 5 }}, got: {:?}",
        err
    );
}

/// **BLK-002 / Too-low height:** Block at height 0 after genesis returns `HeightMismatch`.
///
/// **Proof:** After genesis (height 0), a block at height 0 (re-submitting genesis level)
/// is rejected with `HeightMismatch { expected: 1, got: 0 }`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_002_too_low_height_fails() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block(0, Bytes32::from([0u8; 32]), helpers::test_hash(0xB0));
    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(
            err,
            CoinStoreError::HeightMismatch {
                expected: 1,
                got: 0
            }
        ),
        "Expected HeightMismatch {{ expected: 1, got: 0 }}, got: {:?}",
        err
    );
}

/// **BLK-002 / Duplicate height:** Applying two blocks at the same height fails on the second.
///
/// **Proof:** Block 1 succeeds (height advances to 1). Attempting to apply another block
/// at height 1 returns `HeightMismatch { expected: 2, got: 1 }`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_002_duplicate_height_fails() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let block1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    store.apply_block(block1).unwrap();
    assert_eq!(store.height(), 1);

    // Second block at height 1 — should fail.
    let block1_dup = make_block(1, hash1, helpers::test_hash(0xB2));
    let err = store.apply_block(block1_dup).unwrap_err();
    assert!(
        matches!(
            err,
            CoinStoreError::HeightMismatch {
                expected: 2,
                got: 1
            }
        ),
        "Expected HeightMismatch {{ expected: 2, got: 1 }}, got: {:?}",
        err
    );
}

/// **BLK-002 / Gap of 2:** After block 1, a block at height 3 (skipping 2) fails.
///
/// **Proof:** After applying block 1, attempting height 3 returns
/// `HeightMismatch { expected: 2, got: 3 }`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_002_gap_of_two_fails() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let block1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    store.apply_block(block1).unwrap();
    assert_eq!(store.height(), 1);

    // Skip height 2, try height 3.
    let block3 = make_block(3, hash1, helpers::test_hash(0xB3));
    let err = store.apply_block(block3).unwrap_err();
    assert!(
        matches!(
            err,
            CoinStoreError::HeightMismatch {
                expected: 2,
                got: 3
            }
        ),
        "Expected HeightMismatch {{ expected: 2, got: 3 }}, got: {:?}",
        err
    );
}
