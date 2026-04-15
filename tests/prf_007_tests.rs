//! # PRF-007 Tests — Height-Partitioned Indices
//!
//! Verifies **PRF-007**: height-based indices use big-endian keys so byte-level
//! lexicographic ordering matches numerical ordering. This enables efficient range
//! scans and prefix queries by height.
//!
//! # Requirement: PRF-007
//! # SPEC.md: §1.6 #16 (Height-partitioned indices)
//!
//! ## How these tests prove the requirement
//!
//! - **Key encoding:** `height_coin_key` produces keys where lower heights sort before
//!   higher heights in byte order.
//! - **Prefix scan by height:** `get_coins_added_at_height` returns only coins at the
//!   requested height (no cross-height pollution).
//! - **Multi-height ordering:** Coins added at heights 1, 2, 3 are returned correctly
//!   when queried per height.
//! - **Spent height index:** `get_coins_removed_at_height` uses the same big-endian scheme.

mod helpers;

use dig_coinstore::{
    coin_store::CoinStore, storage::schema, BlockData, Bytes32, CoinAddition,
};

fn make_block(height: u64, parent_hash: Bytes32, block_hash: Bytes32) -> BlockData {
    let coinbase_coins = if height == 0 {
        vec![]
    } else {
        vec![
            helpers::test_coin(200 + height as u8, 201, 1_750_000_000_000),
            helpers::test_coin(202 + height as u8, 203, 250_000_000_000),
        ]
    };
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

/// **PRF-007:** Big-endian height keys sort numerically.
///
/// **Proof:** Keys for heights 0, 1, 255, 256, 65535 maintain ascending byte order.
#[test]
fn vv_req_prf_007_big_endian_key_ordering() {
    let dummy_coin_id = helpers::test_hash(1);
    let k0 = schema::height_coin_key(0, &dummy_coin_id);
    let k1 = schema::height_coin_key(1, &dummy_coin_id);
    let k255 = schema::height_coin_key(255, &dummy_coin_id);
    let k256 = schema::height_coin_key(256, &dummy_coin_id);
    let k65535 = schema::height_coin_key(65535, &dummy_coin_id);

    assert!(k0 < k1, "Height 0 key should sort before height 1");
    assert!(k1 < k255, "Height 1 key should sort before height 255");
    assert!(k255 < k256, "Height 255 key should sort before height 256");
    assert!(k256 < k65535, "Height 256 key should sort before height 65535");
}

/// **PRF-007:** `get_coins_added_at_height` returns only coins at that height.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_007_height_query_isolation() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let genesis_coin = helpers::test_coin(1, 2, 100);
    store
        .init_genesis(vec![(genesis_coin, false)], 1_700_000_000)
        .unwrap();

    // Block 1: add coin_a
    let coin_a = helpers::test_coin(10, 11, 500);
    let mut b1 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    b1.additions = vec![CoinAddition::from_coin(coin_a, false)];
    store.apply_block(b1).unwrap();

    // Block 2: add coin_b
    let coin_b = helpers::test_coin(20, 21, 700);
    let mut b2 = make_block(2, helpers::test_hash(0xB1), helpers::test_hash(0xB2));
    b2.additions = vec![CoinAddition::from_coin(coin_b, false)];
    store.apply_block(b2).unwrap();

    // Query height 0: should have only genesis coin
    let at_0 = store.get_coins_added_at_height(0).unwrap();
    assert_eq!(at_0.len(), 1, "Height 0 should have exactly the genesis coin");

    // Query height 1: should have coin_a + 2 coinbase
    let at_1 = store.get_coins_added_at_height(1).unwrap();
    assert_eq!(at_1.len(), 3, "Height 1: coin_a + 2 coinbase");

    // Query height 2: should have coin_b + 2 coinbase
    let at_2 = store.get_coins_added_at_height(2).unwrap();
    assert_eq!(at_2.len(), 3, "Height 2: coin_b + 2 coinbase");

    // Query height 3: should have no coins
    let at_3 = store.get_coins_added_at_height(3).unwrap();
    assert!(at_3.is_empty(), "Height 3 should have no coins");
}

/// **PRF-007:** Spent height index uses big-endian keys.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_prf_007_spent_height_isolation() {
    let dir = helpers::temp_dir();
    let mut store = CoinStore::new(dir.path()).unwrap();
    let c1 = helpers::test_coin(1, 2, 100);
    let c2 = helpers::test_coin(3, 4, 200);
    let id1 = c1.coin_id();
    store
        .init_genesis(vec![(c1, false), (c2, false)], 1_700_000_000)
        .unwrap();

    // Block 1: spend c1
    let mut b1 = make_block(1, Bytes32::from([0u8; 32]), helpers::test_hash(0xB1));
    b1.removals = vec![id1];
    store.apply_block(b1).unwrap();

    // Coins removed at height 1: should have c1
    let removed_1 = store.get_coins_removed_at_height(1).unwrap();
    assert_eq!(removed_1.len(), 1);
    assert_eq!(removed_1[0].coin.amount, 100);

    // Coins removed at height 2: should be empty
    let removed_2 = store.get_coins_removed_at_height(2).unwrap();
    assert!(removed_2.is_empty());
}

/// **PRF-007:** Height key is exactly 40 bytes (8 height + 32 coin_id).
#[test]
fn vv_req_prf_007_key_length() {
    let coin_id = helpers::test_hash(1);
    let key = schema::height_coin_key(12345, &coin_id);
    assert_eq!(key.len(), 40, "Height key should be 8 + 32 = 40 bytes");
}
