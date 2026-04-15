//! # BLK-006 Tests — Addition Validation
//!
//! Verifies requirement **BLK-006**: `apply_block()` MUST reject a block that introduces
//! a coin whose ID already exists in the store (either from a prior block or from the
//! current block's coinbase/additions). Returns `CoinStoreError::CoinAlreadyExists(coin_id)`.
//!
//! # Requirement: BLK-006
//! # Spec: docs/requirements/domains/block_application/specs/BLK-006.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-006
//! # SPEC.md: §3.2 (Block Application API — addition uniqueness)
//!
//! ## How these tests prove the requirement
//!
//! - **New additions succeed:** A block with fresh transaction additions applies,
//!   and `coins_created` counts additions + coinbase.
//! - **Existing coin returns CoinAlreadyExists:** Attempting to add a coin that already
//!   exists in the store (from genesis) returns the correct error.
//! - **Duplicate within same block rejected:** Two additions with the same coin ID in
//!   the same block's `additions` list fail.
//! - **Coinbase duplicate detected:** A coinbase coin whose ID collides with a
//!   transaction addition in the same block is rejected.

mod helpers;

use dig_coinstore::{coin_store::CoinStore, BlockData, Bytes32, CoinAddition, CoinStoreError};

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
// BLK-006: Addition validation
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-006 / Happy path:** New additions succeed.
///
/// **Proof:** A block with two fresh transaction additions applies. The result
/// reports `coins_created = 4` (2 coinbase + 2 additions).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_006_new_additions_succeed() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = helpers::test_coin(10, 11, 500);
    let coin_b = helpers::test_coin(12, 13, 600);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![
        CoinAddition::from_coin(coin_a, false),
        CoinAddition::from_coin(coin_b, false),
    ];

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Fresh additions must succeed: {:?}",
        result.err()
    );
    let r = result.unwrap();
    assert_eq!(
        r.coins_created, 4,
        "2 coinbase + 2 additions = 4 coins created"
    );
}

/// **BLK-006 / Existing coin:** Adding a coin already in the store returns `CoinAlreadyExists`.
///
/// **Proof:** A coin is created in genesis. A block that includes the same coin as an
/// addition fails with `CoinAlreadyExists(coin_id)`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_006_existing_coin_returns_error() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    let genesis_id = genesis_coin.coin_id();
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(genesis_coin, false)];

    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::CoinAlreadyExists(id) if id == genesis_id),
        "Expected CoinAlreadyExists({:?}), got: {:?}",
        genesis_id,
        err
    );
}

/// **BLK-006 / Duplicate within same block:** Two additions with the same coin ID fail.
///
/// **Proof:** A single block contains the same coin in its additions list twice.
/// The store detects the duplicate and returns `CoinAlreadyExists`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_006_duplicate_within_same_block_rejected() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin = helpers::test_coin(10, 11, 500);
    let coin_id = coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    // Same coin added twice in one block.
    block.additions = vec![
        CoinAddition::from_coin(coin, false),
        CoinAddition::from_coin(coin, false),
    ];

    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::CoinAlreadyExists(id) if id == coin_id),
        "Expected CoinAlreadyExists({:?}), got: {:?}",
        coin_id,
        err
    );
}

/// **BLK-006 / Coinbase duplicate detected:** A coinbase coin that collides with
/// a transaction addition is rejected.
///
/// **Proof:** The block's coinbase_coins and additions share an identical coin.
/// The store detects the collision and returns `CoinAlreadyExists`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_006_coinbase_duplicate_detected() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    // Use the same coin in both additions and coinbase.
    let shared_coin = helpers::test_coin(10, 11, 500);
    let shared_id = shared_coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(shared_coin, false)];
    // Add the same coin to coinbase (third coinbase beyond the default 2).
    block.coinbase_coins.push(shared_coin);

    let err = store.apply_block(block).unwrap_err();
    assert!(
        matches!(err, CoinStoreError::CoinAlreadyExists(id) if id == shared_id),
        "Expected CoinAlreadyExists({:?}), got: {:?}",
        shared_id,
        err
    );
}
