//! # BLK-007 Tests — Coin Insertion with FF-Eligible Tracking
//!
//! Verifies requirement **BLK-007**: `apply_block()` inserts each addition as a
//! `CoinRecord` with fields derived from block metadata and the `CoinAddition`
//! payload. Singleton fast-forward eligibility (`ff_eligible`) is set from
//! `CoinAddition::same_as_parent` for transaction additions and always `false`
//! for coinbase coins.
//!
//! # Requirement: BLK-007
//! # Spec: docs/requirements/domains/block_application/specs/BLK-007.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-007
//! # SPEC.md: §1.5 #3 (fast-forward eligibility), Chia coin_store.py:128-129
//!
//! ## How these tests prove the requirement
//!
//! - **same_as_parent=true gets ff_eligible=true:** A transaction addition with
//!   `same_as_parent = true` produces a stored `CoinRecord` with `ff_eligible = true`.
//! - **same_as_parent=false gets ff_eligible=false:** A transaction addition with
//!   `same_as_parent = false` produces a `CoinRecord` with `ff_eligible = false`.
//! - **Coinbase always ff_eligible=false:** Coinbase reward coins never have
//!   `ff_eligible = true`, regardless of their puzzle hash lineage.
//! - **confirmed_height matches block height:** Every inserted coin's
//!   `confirmed_height` equals the block's height.
//! - **spent_height is None:** Newly inserted coins are unspent — `spent_height = None`.

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
// BLK-007: Coin insertion with FF-eligible tracking
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-007 / FF-eligible = true:** `same_as_parent = true` sets `ff_eligible = true`.
///
/// **Proof:** Apply a block with a transaction addition where `same_as_parent = true`.
/// Read the coin record from a snapshot and verify `ff_eligible == true`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_007_same_as_parent_true_ff_eligible() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let ff_coin = helpers::test_coin(30, 31, 500);
    let ff_coin_id = ff_coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(ff_coin, true)]; // same_as_parent = true

    store.apply_block(block).unwrap();

    // Verify via snapshot that the coin record has ff_eligible = true.
    let snap = store.snapshot().unwrap();
    let rec = snap
        .coins
        .get(&ff_coin_id)
        .expect("FF coin must be in snapshot");
    assert!(
        rec.ff_eligible,
        "Coin with same_as_parent=true must have ff_eligible=true"
    );
}

/// **BLK-007 / FF-eligible = false:** `same_as_parent = false` sets `ff_eligible = false`.
///
/// **Proof:** Apply a block with a transaction addition where `same_as_parent = false`.
/// Read the coin record from a snapshot and verify `ff_eligible == false`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_007_same_as_parent_false_not_ff_eligible() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let regular_coin = helpers::test_coin(40, 41, 600);
    let coin_id = regular_coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(regular_coin, false)]; // same_as_parent = false

    store.apply_block(block).unwrap();

    let snap = store.snapshot().unwrap();
    let rec = snap
        .coins
        .get(&coin_id)
        .expect("Regular coin must be in snapshot");
    assert!(
        !rec.ff_eligible,
        "Coin with same_as_parent=false must have ff_eligible=false"
    );
}

/// **BLK-007 / Coinbase always ff_eligible=false:** Reward coins never get FF eligibility.
///
/// **Proof:** After applying a block, inspect each coinbase coin record in the snapshot.
/// All must have `ff_eligible = false` and `coinbase = true`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_007_coinbase_never_ff_eligible() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let coinbase_ids: Vec<Bytes32> = block.coinbase_coins.iter().map(|c| c.coin_id()).collect();

    store.apply_block(block).unwrap();

    let snap = store.snapshot().unwrap();
    for cb_id in &coinbase_ids {
        let rec = snap
            .coins
            .get(cb_id)
            .expect("Coinbase coin must be in snapshot");
        assert!(rec.coinbase, "Coin must be flagged as coinbase");
        assert!(
            !rec.ff_eligible,
            "Coinbase coin must never have ff_eligible=true"
        );
    }
}

/// **BLK-007 / confirmed_height matches block height:** Inserted coins record the block height.
///
/// **Proof:** Apply block at height 1. All newly created coins (coinbase + additions)
/// must have `confirmed_height == 1`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_007_confirmed_height_matches_block() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let tx_coin = helpers::test_coin(50, 51, 700);
    let tx_coin_id = tx_coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let coinbase_ids: Vec<Bytes32> = block.coinbase_coins.iter().map(|c| c.coin_id()).collect();
    block.additions = vec![CoinAddition::from_coin(tx_coin, false)];

    store.apply_block(block).unwrap();

    let snap = store.snapshot().unwrap();

    // Check transaction addition.
    let tx_rec = snap
        .coins
        .get(&tx_coin_id)
        .expect("TX coin must be in snapshot");
    assert_eq!(
        tx_rec.confirmed_height, 1,
        "TX coin confirmed_height must match block height"
    );

    // Check coinbase coins.
    for cb_id in &coinbase_ids {
        let cb_rec = snap
            .coins
            .get(cb_id)
            .expect("Coinbase coin must be in snapshot");
        assert_eq!(
            cb_rec.confirmed_height, 1,
            "Coinbase confirmed_height must match block height"
        );
    }
}

/// **BLK-007 / spent_height is None:** Newly inserted coins are unspent.
///
/// **Proof:** After applying a block, all new coins (additions + coinbase) have
/// `spent_height == None`.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_007_spent_height_is_none() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let tx_coin = helpers::test_coin(60, 61, 800);
    let tx_coin_id = tx_coin.coin_id();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    let coinbase_ids: Vec<Bytes32> = block.coinbase_coins.iter().map(|c| c.coin_id()).collect();
    block.additions = vec![CoinAddition::from_coin(tx_coin, true)];

    store.apply_block(block).unwrap();

    let snap = store.snapshot().unwrap();

    let tx_rec = snap
        .coins
        .get(&tx_coin_id)
        .expect("TX coin must be in snapshot");
    assert_eq!(
        tx_rec.spent_height, None,
        "Newly inserted coin must have spent_height = None"
    );

    for cb_id in &coinbase_ids {
        let cb_rec = snap
            .coins
            .get(cb_id)
            .expect("Coinbase coin must be in snapshot");
        assert_eq!(
            cb_rec.spent_height, None,
            "Newly inserted coinbase coin must have spent_height = None"
        );
    }
}
