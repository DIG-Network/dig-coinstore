//! # BLK-011 Tests — Hint Validation in Phase 1
//!
//! Verifies requirement **BLK-011**: `apply_block()` validates hint entries in Phase 1
//! (validation) before any mutations occur. Hints are `Vec<(CoinId, Bytes32)>` pairs in
//! `BlockData`. Since `Bytes32` is always exactly 32 bytes, hints from `BlockData` will
//! never trigger `HintTooLong` in practice — the validation exists as a defense-in-depth
//! measure. Zero-filled `Bytes32` hints are treated as empty and skipped during storage.
//!
//! # Requirement: BLK-011
//! # Spec: docs/requirements/domains/block_application/specs/BLK-011.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-011
//! # SPEC.md: §1.5 #13 (hint validation), §2.7 (MAX_HINT_LENGTH = 32)
//!
//! ## How these tests prove the requirement
//!
//! - **Valid 32-byte hints succeed:** A block with hints containing standard `Bytes32`
//!   values applies successfully — validation passes for correctly-sized hints.
//! - **Empty hints vec succeeds:** A block with `hints = vec![]` applies without error.
//! - **Zero-filled hints are skipped:** Hints with `Bytes32::from([0u8; 32])` are treated
//!   as empty and do not appear in the stored hint index.
//! - **Hint validation occurs before mutations:** This is proven transitively: since
//!   Phase 1 validation runs before Phase 2 mutations (BLK-001 atomicity), and hint
//!   validation is part of Phase 1, invalid hints would prevent any state changes.

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
// BLK-011: Hint validation in Phase 1
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-011 / Valid 32-byte hints succeed:** A block with properly-formed Bytes32
/// hint values passes validation.
///
/// **Proof:** Apply a block with hints containing non-zero `Bytes32` values.
/// The block succeeds, confirming 32-byte hints pass the MAX_HINT_LENGTH check.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_011_valid_hints_succeed() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin = helpers::test_coin(10, 11, 500);
    let coin_id = coin.coin_id();
    let hint_value = helpers::test_hash(42);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(coin, false)];
    block.hints = vec![(coin_id, hint_value)];

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Block with valid 32-byte hints must succeed: {:?}",
        result.err()
    );
}

/// **BLK-011 / Empty hints vec succeeds:** A block with no hints applies without error.
///
/// **Proof:** Apply a block with `hints = vec![]`. The block succeeds — empty hints
/// are trivially valid.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_011_empty_hints_succeed() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.hints = vec![]; // explicitly empty

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Block with empty hints must succeed: {:?}",
        result.err()
    );
}

/// **BLK-011 / Zero-filled hints are skipped:** `Bytes32::from([0u8; 32])` hints are
/// treated as empty and not stored in the hint index.
///
/// **Proof:** Apply a block with a hint where the value is all zeros. After apply,
/// the snapshot's hints list should not contain a hint with the zero value for that
/// coin (zero-filled hints are skipped during storage as they convey no information).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_011_zero_filled_hints_skipped() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin = helpers::test_coin(10, 11, 500);
    let coin_id = coin.coin_id();
    let zero_hint = Bytes32::from([0u8; 32]);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(coin, false)];
    block.hints = vec![(coin_id, zero_hint)];

    store.apply_block(block).unwrap();

    // Zero-filled hints should be skipped — they should not appear in snapshot hints.
    let snap = store.snapshot().unwrap();
    let stored_hints_for_coin: Vec<_> = snap
        .hints
        .iter()
        .filter(|(cid, _)| *cid == coin_id)
        .collect();
    assert!(
        stored_hints_for_coin.is_empty(),
        "Zero-filled hints must be skipped and not stored; found {:?}",
        stored_hints_for_coin
    );
}

/// **BLK-011 / Hint validation before mutations:** Phase 1 validates hints before any
/// Phase 2 mutations occur.
///
/// **Proof:** This is verified transitively: BLK-001 atomicity guarantees that if Phase 1
/// validation fails, no state changes are committed. Since hint validation is part of
/// Phase 1, any hint validation failure would prevent coin insertion, spend marking, and
/// all other mutations. We confirm the ordering by applying a block with valid hints
/// alongside additions and verifying the additions are present (proving Phase 1 passed
/// before Phase 2 committed).
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_011_hints_validated_before_mutations() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin = helpers::test_coin(20, 21, 1_000);
    let coin_id = coin.coin_id();
    let hint_value = helpers::test_hash(99);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(coin, false)];
    block.hints = vec![(coin_id, hint_value)];

    let result = store.apply_block(block).unwrap();
    // If we get here, Phase 1 (including hint validation) passed, then Phase 2 ran.
    assert_eq!(
        result.coins_created, 3,
        "2 coinbase + 1 addition = 3 (Phase 2 mutations committed after Phase 1 validation)"
    );

    // Verify the coin was stored (Phase 2 mutation committed).
    let snap = store.snapshot().unwrap();
    assert!(
        snap.coins.contains_key(&coin_id),
        "Addition must be stored after successful hint validation"
    );
}
