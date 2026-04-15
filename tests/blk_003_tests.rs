//! # BLK-003 Tests — Parent Hash Verification
//!
//! Verifies requirement **BLK-003**: `apply_block()` MUST reject a block whose
//! `parent_hash` does not match the current chain tip hash. Returns
//! `CoinStoreError::ParentHashMismatch { expected, got }` on violation.
//!
//! # Requirement: BLK-003
//! # Spec: docs/requirements/domains/block_application/specs/BLK-003.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-003
//! # SPEC.md: §3.2 (Block Application API), Chia coin_store.py implicit parent hash check
//!
//! ## How these tests prove the requirement
//!
//! - **Correct parent hash succeeds:** A block at height 1 with `parent_hash = [0u8; 32]`
//!   (the genesis tip) applies without error.
//! - **Wrong parent hash returns ParentHashMismatch:** A block at height 1 with a non-zero
//!   parent hash fails with the expected error carrying both `expected` and `got` fields.
//! - **Genesis tip is zero hash:** After `init_genesis`, the tip hash is `[0u8; 32]`,
//!   confirming that the first block's parent must be the zero hash.
//! - **Multi-block chaining works:** Three sequential blocks each reference the prior
//!   block's hash as their parent, and all succeed. Incorrect parent in block 3 fails.

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
// BLK-003: Parent hash verification
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-003 / Happy path:** Block with correct parent hash succeeds.
///
/// **Proof:** After genesis (tip = zero hash), a block at height 1 with
/// `parent_hash = [0u8; 32]` applies. Store tip updates to the new block hash.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_003_correct_parent_hash_succeeds() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let block = make_block(1, Bytes32::from([0u8; 32]), hash1);
    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Block with correct parent hash must succeed: {:?}",
        result.err()
    );
    assert_eq!(store.tip_hash(), hash1);
}

/// **BLK-003 / Wrong parent hash:** Returns `ParentHashMismatch`.
///
/// **Proof:** After genesis, the tip is `[0u8; 32]`. A block at height 1 with
/// `parent_hash = [0xFF; 32]` fails with `ParentHashMismatch` carrying
/// `expected = [0u8; 32]` and `got = [0xFF; 32]`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_003_wrong_parent_hash_returns_error() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let wrong_parent = helpers::filled_hash(0xFF);
    let block = make_block(1, wrong_parent, helpers::test_hash(0xB1));
    let err = store.apply_block(block).unwrap_err();

    match err {
        CoinStoreError::ParentHashMismatch { expected, got } => {
            assert_eq!(
                expected,
                Bytes32::from([0u8; 32]),
                "Expected the genesis zero hash as the expected parent"
            );
            assert_eq!(
                got, wrong_parent,
                "Got field must carry the incorrect parent hash we supplied"
            );
        }
        other => panic!("Expected ParentHashMismatch, got: {:?}", other),
    }
}

/// **BLK-003 / Genesis tip is zero hash:** Confirms the initial tip after genesis.
///
/// **Proof:** After `init_genesis`, `store.tip_hash()` returns `[0u8; 32]`.
/// This is the parent hash that the first apply_block must reference.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_003_genesis_tip_is_zero_hash() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    assert_eq!(
        store.tip_hash(),
        Bytes32::from([0u8; 32]),
        "Genesis tip hash must be the zero hash"
    );
}

/// **BLK-003 / Multi-block chaining:** Three sequential blocks with correct parent hashes.
///
/// **Proof:** Each block's parent_hash matches the prior block's block_hash.
/// All three succeed, and on the fourth attempt with a wrong parent hash, the
/// store rejects with `ParentHashMismatch`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_003_multi_block_chaining() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let hash1 = helpers::test_hash(0xB1);
    let hash2 = helpers::test_hash(0xB2);
    let hash3 = helpers::test_hash(0xB3);

    // Block 1: parent = genesis zero hash
    let b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    store.apply_block(b1).unwrap();
    assert_eq!(store.tip_hash(), hash1);

    // Block 2: parent = block 1's hash
    let b2 = make_block(2, hash1, hash2);
    store.apply_block(b2).unwrap();
    assert_eq!(store.tip_hash(), hash2);

    // Block 3: parent = block 2's hash
    let b3 = make_block(3, hash2, hash3);
    store.apply_block(b3).unwrap();
    assert_eq!(store.tip_hash(), hash3);

    // Block 4 with wrong parent (uses hash1 instead of hash3)
    let b4_bad = make_block(4, hash1, helpers::test_hash(0xB4));
    let err = store.apply_block(b4_bad).unwrap_err();
    match err {
        CoinStoreError::ParentHashMismatch { expected, got } => {
            assert_eq!(expected, hash3, "Expected current tip hash3");
            assert_eq!(got, hash1, "Got the stale hash1 we supplied");
        }
        other => panic!("Expected ParentHashMismatch, got: {:?}", other),
    }
}
