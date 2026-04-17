//! # BLK-012 Tests — Hint Storage in Phase 2
//!
//! Verifies requirement **BLK-012**: `apply_block()` stores validated hints in Phase 2
//! (mutation). After a successful block application, hints are persisted in the forward
//! hint index (`CF_HINTS`) and reverse index (`CF_HINTS_BY_VALUE`). The `stats().hint_count`
//! reflects the number of stored hint entries. Duplicate `(coin_id, hint)` pairs are
//! idempotent (no error, no extra rows), and multiple distinct hints for the same coin
//! each produce a separate index entry.
//!
//! # Requirement: BLK-012
//! # Spec: docs/requirements/domains/block_application/specs/BLK-012.md
//! # NORMATIVE: docs/requirements/domains/block_application/NORMATIVE.md#BLK-012
//! # SPEC.md: §1.5 #14 (hint storage in Phase 2)
//!
//! ## How these tests prove the requirement
//!
//! - **Hints stored (forward index):** After applying a block with hints,
//!   `stats().hint_count` increases by the number of unique hint entries stored.
//! - **Duplicate (coin_id, hint) pairs are idempotent:** Applying a block with a
//!   duplicate hint pair does not cause an error and does not double-count in stats.
//! - **Multiple hints for same coin stored correctly:** Multiple distinct hints for the
//!   same `coin_id` each produce a separate forward-index entry, reflected in hint_count.
//!
//! ## Note on snapshot.hints
//!
//! The `CoinStoreSnapshot::hints` field is currently always empty (the snapshot method
//! does not scan CF_HINTS). Hint storage is verified through `stats().hint_count` which
//! counts entries in the CF_HINTS column family.

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
// BLK-012: Hint storage in Phase 2
// ─────────────────────────────────────────────────────────────────────────────

/// **BLK-012 / Hints stored (forward index):** After applying a block with hints,
/// the forward index (CF_HINTS) contains the stored entries.
///
/// **Proof:** Apply a block with one addition and one hint for that addition's coin ID.
/// Verify `stats().hint_count` increased from 0 to 1.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_012_hints_stored_in_forward_index() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    assert_eq!(
        store.stats().hint_count,
        0,
        "No hints before any block applied"
    );

    let coin = helpers::test_coin(10, 11, 500);
    let coin_id = coin.coin_id();
    let hint_value = helpers::test_hash(42);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(coin, false)];
    block.hints = vec![(coin_id, hint_value)];

    store.apply_block(block).unwrap();

    assert_eq!(
        store.stats().hint_count,
        1,
        "One hint must be stored in the forward index after apply_block"
    );
}

/// **BLK-012 / Duplicate hints are idempotent:** Applying a block with the same
/// `(coin_id, hint)` pair twice does not cause an error and does not create extra rows.
///
/// **Proof:** Apply a block that contains the same `(coin_id, hint_value)` pair
/// duplicated in the hints vector. The block succeeds and hint_count is 1 (not 2),
/// because the second write to the same key is a no-op in the KV store.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_012_duplicate_hints_idempotent() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin = helpers::test_coin(10, 11, 500);
    let coin_id = coin.coin_id();
    let hint_value = helpers::test_hash(42);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(coin, false)];
    // Duplicate the same hint pair.
    block.hints = vec![(coin_id, hint_value), (coin_id, hint_value)];

    let result = store.apply_block(block);
    assert!(
        result.is_ok(),
        "Duplicate hint pairs must not cause an error: {:?}",
        result.err()
    );

    // The key-value store deduplicates identical keys, so hint_count should be 1.
    assert_eq!(
        store.stats().hint_count,
        1,
        "Duplicate (coin_id, hint) should be idempotent — only 1 entry in forward index"
    );
}

/// **BLK-012 / Multiple hints for same coin:** Different hints for the same `coin_id`
/// each produce a separate forward-index entry.
///
/// **Proof:** Apply a block with two distinct hint values for the same coin ID. After
/// apply, `stats().hint_count == 2` — each unique (coin_id, hint) pair is a separate key.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_012_multiple_hints_same_coin() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin = helpers::test_coin(10, 11, 500);
    let coin_id = coin.coin_id();
    let hint_a = helpers::test_hash(42);
    let hint_b = helpers::test_hash(43);

    let mut block = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    block.additions = vec![CoinAddition::from_coin(coin, false)];
    block.hints = vec![(coin_id, hint_a), (coin_id, hint_b)];

    store.apply_block(block).unwrap();

    assert_eq!(
        store.stats().hint_count,
        2,
        "Two distinct hints for the same coin must produce 2 forward-index entries"
    );
}

/// **BLK-012 / Hints accumulate across blocks:** Hints from sequential blocks are
/// all persisted.
///
/// **Proof:** Apply block 1 with a hint for coin A, then block 2 with a hint for coin B.
/// After block 2, `stats().hint_count == 2` — hints from both blocks are retained.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_blk_012_hints_accumulate_across_blocks() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    store.init_genesis(vec![], 1_700_000_000).unwrap();

    let coin_a = helpers::test_coin(10, 11, 500);
    let coin_a_id = coin_a.coin_id();
    let hint_a = helpers::test_hash(42);

    let coin_b = helpers::test_coin(20, 21, 600);
    let coin_b_id = coin_b.coin_id();
    let hint_b = helpers::test_hash(43);

    // Block 1: add coin A with hint.
    let hash1 = helpers::test_hash(0xB1);
    let mut b1 = make_block(1, Bytes32::from([0u8; 32]), hash1);
    b1.additions = vec![CoinAddition::from_coin(coin_a, false)];
    b1.hints = vec![(coin_a_id, hint_a)];
    store.apply_block(b1).unwrap();

    assert_eq!(store.stats().hint_count, 1, "One hint after block 1");

    // Block 2: add coin B with hint.
    let hash2 = helpers::test_hash(0xB2);
    let mut b2 = make_block(2, hash1, hash2);
    b2.additions = vec![CoinAddition::from_coin(coin_b, false)];
    b2.hints = vec![(coin_b_id, hint_b)];
    store.apply_block(b2).unwrap();

    assert_eq!(
        store.stats().hint_count,
        2,
        "Two hints after block 2 (hints from both blocks must be retained)"
    );
}
