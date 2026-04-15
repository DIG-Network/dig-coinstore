//! # BLK-014 Tests — Chain Tip Atomic Commit
//!
//! Verifies requirement **BLK-014**: `apply_block()` atomically updates the chain tip
//! metadata (height, tip_hash, timestamp) as part of the block application commit.
//! After a successful `apply_block`, the store's `height()`, `tip_hash()`, and
//! `timestamp()` reflect the applied block's values. On failure, none of these fields
//! are updated.
//!
//! # Requirement: BLK-014
//! # Spec: docs/requirements/domains/block_application/specs/BLK-014.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-014
//! # SPEC.md: §1.6 #17 (chain tip atomic commit)
//!
//! ## How these tests prove the requirement
//!
//! - **height updated:** After `apply_block` at height N, `store.height() == N`.
//! - **tip_hash updated:** After `apply_block`, `store.tip_hash() == block.block_hash`.
//! - **timestamp updated:** After `apply_block`, `store.timestamp() == block.timestamp`.
//! - **Sequential blocks update tip correctly:** Blocks 1, 2, 3 each update all tip
//!   fields to their respective block values.
//! - **Failure does not update tip:** A `HeightMismatch` error leaves height, tip_hash,
//!   and timestamp unchanged.

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
// BLK-014: Chain tip atomic commit
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-014 / Height updated:** After `apply_block`, `store.height()` equals the
/// applied block's height.
///
/// **Proof:** Apply block at height 1. Verify `store.height() == 1`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_014_height_updated() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    assert_eq!(store.height(), 0, "Genesis height must be 0");

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    store.apply_block(block).unwrap();

    assert_eq!(
        store.height(),
        1,
        "Height must be 1 after applying block at height 1"
    );
}

/// **BLK-014 / Tip hash updated:** After `apply_block`, `store.tip_hash()` equals the
/// applied block's `block_hash`.
///
/// **Proof:** Apply block with `block_hash = test_hash(0xB1)`. Verify `store.tip_hash()`
/// matches that hash.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_014_tip_hash_updated() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block_hash = helpers::test_hash(0xB1);
    let block = make_block(1, Bytes32::from([0u8; 32]), block_hash);
    store.apply_block(block).unwrap();

    assert_eq!(
        store.tip_hash(),
        block_hash,
        "Tip hash must equal the applied block's block_hash"
    );
}

/// **BLK-014 / Timestamp updated:** After `apply_block`, `store.timestamp()` equals the
/// applied block's `timestamp`.
///
/// **Proof:** Apply block with a known timestamp. Verify `store.timestamp()` matches.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_014_timestamp_updated() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let expected_ts = block.timestamp;
    store.apply_block(block).unwrap();

    assert_eq!(
        store.timestamp(),
        expected_ts,
        "Timestamp must equal the applied block's timestamp"
    );
}

/// **BLK-014 / Sequential blocks update tip:** Blocks 1, 2, 3 each update height,
/// tip_hash, and timestamp to their respective values.
///
/// **Proof:** Apply three blocks in sequence. After each, verify all three tip fields
/// match the most recently applied block.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_014_sequential_blocks_update_tip() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let hash2 = helpers::test_hash(0xB2);
    let hash3 = helpers::test_hash(0xB3);

    // Block 1.
    let b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    let ts1 = b1.timestamp;
    store.apply_block(b1).unwrap();
    assert_eq!(store.height(), 1, "Height after block 1");
    assert_eq!(store.tip_hash(), hash1, "Tip hash after block 1");
    assert_eq!(store.timestamp(), ts1, "Timestamp after block 1");

    // Block 2.
    let b2 = make_block(2, hash1, hash2);
    let ts2 = b2.timestamp;
    store.apply_block(b2).unwrap();
    assert_eq!(store.height(), 2, "Height after block 2");
    assert_eq!(store.tip_hash(), hash2, "Tip hash after block 2");
    assert_eq!(store.timestamp(), ts2, "Timestamp after block 2");

    // Block 3.
    let b3 = make_block(3, hash2, hash3);
    let ts3 = b3.timestamp;
    store.apply_block(b3).unwrap();
    assert_eq!(store.height(), 3, "Height after block 3");
    assert_eq!(store.tip_hash(), hash3, "Tip hash after block 3");
    assert_eq!(store.timestamp(), ts3, "Timestamp after block 3");
}

/// **BLK-014 / Failure does not update tip:** On `HeightMismatch`, the chain tip
/// metadata (height, tip_hash, timestamp) remains unchanged.
///
/// **Proof:** Record tip state before a bad block. Apply a block with wrong height
/// (triggers `HeightMismatch`). Verify all three tip fields are unchanged.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_014_failure_does_not_update_tip() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Apply block 1 successfully to establish a non-genesis tip.
    let hash1 = helpers::test_hash(0xB1);
    let b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    store.apply_block(b1).unwrap();

    let height_before = store.height();
    let tip_before = store.tip_hash();
    let ts_before = store.timestamp();

    // Attempt block with wrong height (expect height 2, provide 5).
    let bad_block = make_block(5, hash1, helpers::test_hash(0xB5));
    let err = store.apply_block(bad_block).unwrap_err();
    assert!(
        matches!(
            err,
            CoinStoreError::HeightMismatch {
                expected: 2,
                got: 5
            }
        ),
        "Expected HeightMismatch, got: {:?}",
        err
    );

    // Verify tip is unchanged.
    assert_eq!(
        store.height(),
        height_before,
        "Height must not change after HeightMismatch"
    );
    assert_eq!(
        store.tip_hash(),
        tip_before,
        "Tip hash must not change after HeightMismatch"
    );
    assert_eq!(
        store.timestamp(),
        ts_before,
        "Timestamp must not change after HeightMismatch"
    );
}
