//! # BLK-013 Tests — Merkle Tree Batch Update
//!
//! Verifies requirement **BLK-013**: `apply_block()` performs a batch update of the
//! Sparse Merkle Tree after all coin insertions and spend markings in Phase 2. The
//! resulting state root reflects all additions (transaction + coinbase) and removals.
//!
//! # Requirement: BLK-013
//! # Spec: docs/requirements/domains/block_application/specs/BLK-013.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-013
//! # SPEC.md: §1.6 #7 (Merkle batch update), §9 (Sparse Merkle Tree)
//!
//! ## How these tests prove the requirement
//!
//! - **State root changes after additions:** Applying a block with new coins changes the
//!   Merkle state root compared to the pre-block root.
//! - **State root changes after removals:** Spending coins in a subsequent block changes
//!   the root again, proving removal hashes update the tree.
//! - **Deterministic state root:** Applying the same block content to two independent
//!   stores produces the same state root — the Merkle update is deterministic.
//! - **Coinbase-only block updates Merkle tree:** Even a block with no transaction
//!   additions/removals (only coinbase coins) updates the Merkle tree, because coinbase
//!   coins are added as leaves.

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
// BLK-013: Merkle tree batch update
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-013 / State root changes after additions:** Adding coins changes the state root.
///
/// **Proof:** Record the state root after genesis (empty tree). Apply block 1 with
/// coinbase + a transaction addition. The new state root differs from the genesis root.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_013_state_root_changes_after_additions() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let root_before = store.state_root();

    let tx_coin = helpers::test_coin(10, 11, 500);
    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(tx_coin, false)];

    let result = store.apply_block(block).unwrap();

    assert_ne!(
        result.state_root, root_before,
        "State root must change after adding coins"
    );
    assert_eq!(
        result.state_root,
        store.state_root(),
        "Result state_root must match store state_root"
    );
}

/// **BLK-013 / State root changes after removals:** Spending coins changes the state root.
///
/// **Proof:** Create a genesis coin, apply block 1 (adds coinbase), then apply block 2
/// which spends the genesis coin. The state root after block 2 differs from block 1.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_013_state_root_changes_after_removals() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();

    let genesis_coin = helpers::test_coin(1, 2, 1_000_000);
    let genesis_id = genesis_coin.coin_id();
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    // Block 1: coinbase only (no removals).
    let hash1 = helpers::test_hash(0xB1);
    let b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    let r1 = store.apply_block(b1).unwrap();
    let root_after_block1 = r1.state_root;

    // Block 2: spend the genesis coin.
    let hash2 = helpers::test_hash(0xB2);
    let mut b2 = make_block(2, hash1, hash2);
    b2.removals = vec![genesis_id];
    let r2 = store.apply_block(b2).unwrap();

    assert_ne!(
        r2.state_root, root_after_block1,
        "State root must change after spending coins"
    );
}

/// **BLK-013 / Deterministic state root:** Same block content produces the same root
/// on independent stores.
///
/// **Proof:** Apply identical block 1 to two freshly-initialized stores. Both must
/// produce the same state root, proving the Merkle update is deterministic.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_013_deterministic_state_root() {
    // Store A.
    let dir_a = helpers::temp_dir();
    let mut store_a = CoinStore::new(dir_a.path()).unwrap();
    store_a.init_genesis(vec![], 1_700_000_000).unwrap();

    let tx_coin = helpers::test_coin(10, 11, 500);
    let mut block_a = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block_a.additions = vec![CoinAddition::from_coin(tx_coin, false)];
    let result_a = store_a.apply_block(block_a).unwrap();

    // Store B — identical setup and block.
    let dir_b = helpers::temp_dir();
    let mut store_b = CoinStore::new(dir_b.path()).unwrap();
    store_b.init_genesis(vec![], 1_700_000_000).unwrap();

    let tx_coin_b = helpers::test_coin(10, 11, 500); // same seeds → same coin
    let mut block_b = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block_b.additions = vec![CoinAddition::from_coin(tx_coin_b, false)];
    let result_b = store_b.apply_block(block_b).unwrap();

    assert_eq!(
        result_a.state_root, result_b.state_root,
        "Same block content must produce the same state root on independent stores"
    );
}

/// **BLK-013 / Coinbase-only block updates Merkle tree:** A block with no transaction
/// additions or removals still updates the Merkle tree (coinbase coins are added).
///
/// **Proof:** Apply a block with only coinbase coins (no additions, no removals). The
/// state root changes from the genesis root, proving the coinbase coins were inserted
/// into the Merkle tree.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_013_coinbase_only_updates_merkle() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let root_before = store.state_root();

    // Block with only coinbase — no tx additions, no removals.
    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let result = store.apply_block(block).unwrap();

    assert_ne!(
        result.state_root, root_before,
        "State root must change even for a coinbase-only block (coinbase coins added to tree)"
    );
    assert_eq!(
        result.coins_created, 2,
        "Only 2 coinbase coins created (no tx additions)"
    );
}

/// **BLK-013 / Sequential Merkle updates are cumulative:** Each block incrementally
/// updates the tree — the root after block 2 differs from block 1 and from genesis.
///
/// **Proof:** Apply blocks 1 and 2 in sequence with different additions. All three
/// roots (genesis, block 1, block 2) are distinct.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_013_sequential_merkle_updates_cumulative() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let root_genesis = store.state_root();

    // Block 1 with addition.
    let hash1 = helpers::test_hash(0xB1);
    let coin_1 = helpers::test_coin(10, 11, 500);
    let mut b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    b1.additions = vec![CoinAddition::from_coin(coin_1, false)];
    let r1 = store.apply_block(b1).unwrap();

    // Block 2 with a different addition.
    let hash2 = helpers::test_hash(0xB2);
    let coin_2 = helpers::test_coin(20, 21, 600);
    let mut b2 = make_block(2, hash1, hash2);
    b2.additions = vec![CoinAddition::from_coin(coin_2, false)];
    let r2 = store.apply_block(b2).unwrap();

    assert_ne!(
        r1.state_root, root_genesis,
        "Block 1 root must differ from genesis root"
    );
    assert_ne!(
        r2.state_root, root_genesis,
        "Block 2 root must differ from genesis root"
    );
    assert_ne!(
        r2.state_root, r1.state_root,
        "Block 2 root must differ from block 1 root"
    );
}
