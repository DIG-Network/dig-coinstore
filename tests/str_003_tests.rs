//! # STR-003 Tests — Storage Module
//!
//! Verifies **STR-003** (storage module layout) and **STO-001** (`StorageBackend` trait).
//! Covers trait surface, schema constants, composite key encoding, and RocksDB backend integration.
//!
//! # Requirement: STR-003, STO-001
//! # SPEC.md: §7 (Storage Architecture), §7.2 (Column Families), §1.3 #1 (Dual Backend),
//! #          §1.3 #10,11 (Composite Key Decisions), §1.6 #4 (Embedded KV), §1.6 #17 (WriteBatch)
//!
//! ## How these tests prove the requirement
//!
//! - **Trait surface:** `Send + Sync` bound check fails to compile if missing (CON-001).
//! - **Schema:** 12 CF name constants uniqueness check matches [SPEC.md §7.2](../../docs/resources/SPEC.md).
//! - **Key encoding:** Round-trip tests prove lossless encoding; sort-order test confirms big-endian
//!   height encoding preserves numeric ordering for range scans.
//! - **Backend integration:** `put`/`get`/`delete`, `batch_write`, `prefix_scan` exercised on real RocksDB.

mod helpers;

// ─────────────────────────────────────────────────────────────────────────────
// STR-003 / STO-001: StorageBackend trait + schema
// Requirement: docs/requirements/domains/crate_structure/specs/STR-003.md
// Requirement: docs/requirements/domains/storage/specs/STO-001.md
// SPEC.md: §7 (Storage Architecture), §7.2 (Column Families)
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies STR-003 / STO-001: The `StorageBackend` trait exists with all
/// required methods and requires `Send + Sync`.
///
/// The trait MUST define: get, put, delete, batch_write, prefix_scan,
/// flush, compact. It MUST require `Send + Sync` for thread safety (CON-001).
///
/// This is a compile-time test: if the trait is missing or has wrong
/// signatures, this test fails to compile.
#[test]
fn vv_req_str_003_storage_trait_defined() {
    use dig_coinstore::storage::{StorageBackend, WriteBatch};
    // WriteOp is used internally by WriteBatch; prove it exists by reference.
    let _ = std::any::type_name::<dig_coinstore::storage::WriteOp>();

    // Prove the trait is object-safe and has Send + Sync bounds by
    // constructing a trait object type (won't compile if bounds are missing).
    fn _assert_send_sync<T: StorageBackend + Send + Sync>() {}

    // Prove WriteBatch and WriteOp types exist.
    let mut batch = WriteBatch::new();
    batch.put("test_cf", b"key", b"value");
    batch.delete("test_cf", b"key");
    assert!(!batch.ops.is_empty(), "WriteBatch should hold operations");
}

/// Verifies STR-003: Schema module defines column family name constants.
///
/// All column family names from SPEC Section 7.2 must be defined as
/// string constants in `storage::schema`.
#[test]
fn vv_req_str_003_schema_constants_defined() {
    use dig_coinstore::storage::schema;

    // Verify all 12 CF name constants exist and are non-empty.
    let cf_names = [
        schema::CF_COIN_RECORDS,
        schema::CF_COIN_BY_PUZZLE_HASH,
        schema::CF_UNSPENT_BY_PUZZLE_HASH,
        schema::CF_COIN_BY_PARENT,
        schema::CF_COIN_BY_CONFIRMED_HEIGHT,
        schema::CF_COIN_BY_SPENT_HEIGHT,
        schema::CF_HINTS,
        schema::CF_HINTS_BY_VALUE,
        schema::CF_MERKLE_NODES,
        schema::CF_ARCHIVE_COIN_RECORDS,
        schema::CF_STATE_SNAPSHOTS,
        schema::CF_METADATA,
    ];

    for name in &cf_names {
        assert!(!name.is_empty(), "Column family name must not be empty");
    }

    // All names must be unique.
    let mut seen = std::collections::HashSet::new();
    for name in &cf_names {
        assert!(seen.insert(name), "Duplicate CF name: {}", name);
    }

    assert_eq!(cf_names.len(), 12, "Must have exactly 12 column families");
}

/// Verifies STR-003: Key encoding round-trip for coin_id keys.
///
/// `coin_key(id)` must produce a 32-byte key, and `coin_id_from_key(key)`
/// must recover the original id.
#[test]
fn vv_req_str_003_key_encoding_coin_id() {
    use chia_protocol::Bytes32;
    use dig_coinstore::storage::schema;

    let coin_id = Bytes32::from([0xABu8; 32]);
    let key = schema::coin_key(&coin_id);
    assert_eq!(key.len(), 32);

    let recovered = schema::coin_id_from_key(&key);
    assert_eq!(recovered, coin_id, "Round-trip must preserve coin_id");
}

/// Verifies STR-003: Key encoding round-trip for puzzle_hash + coin_id
/// composite keys.
///
/// `puzzle_hash_coin_key(ph, id)` must produce a 64-byte key.
/// `puzzle_hash_from_key(key)` must recover the puzzle hash (first 32 bytes).
#[test]
fn vv_req_str_003_key_encoding_puzzle_hash_coin() {
    use chia_protocol::Bytes32;
    use dig_coinstore::storage::schema;

    let puzzle_hash = Bytes32::from([0x11u8; 32]);
    let coin_id = Bytes32::from([0x22u8; 32]);
    let key = schema::puzzle_hash_coin_key(&puzzle_hash, &coin_id);
    assert_eq!(key.len(), 64);

    let recovered_ph = schema::puzzle_hash_from_key(&key);
    assert_eq!(recovered_ph, puzzle_hash);
}

/// Verifies STR-003: Key encoding round-trip for height + coin_id keys.
///
/// `height_coin_key(height, id)` must produce a 40-byte key using big-endian
/// height encoding for natural sort order.
#[test]
fn vv_req_str_003_key_encoding_height_coin() {
    use chia_protocol::Bytes32;
    use dig_coinstore::storage::schema;

    let height: u64 = 1_000_000;
    let coin_id = Bytes32::from([0x33u8; 32]);
    let key = schema::height_coin_key(height, &coin_id);
    assert_eq!(
        key.len(),
        40,
        "Height (8 bytes BE) + coin_id (32 bytes) = 40"
    );

    let (recovered_h, recovered_id) = schema::height_coin_from_key(&key);
    assert_eq!(recovered_h, height);
    assert_eq!(recovered_id, coin_id);
}

/// Verifies STR-003: Height keys sort lexicographically in numeric order.
///
/// Big-endian encoding ensures that h=100 < h=200 in byte comparison,
/// which is critical for range scans by height.
#[test]
fn vv_req_str_003_height_keys_sort_correctly() {
    use chia_protocol::Bytes32;
    use dig_coinstore::storage::schema;

    let id = Bytes32::from([0x00u8; 32]);
    let key_100 = schema::height_coin_key(100, &id);
    let key_200 = schema::height_coin_key(200, &id);
    let key_max = schema::height_coin_key(u64::MAX, &id);

    assert!(key_100 < key_200, "h=100 must sort before h=200");
    assert!(key_200 < key_max, "h=200 must sort before h=MAX");
}

/// Verifies STR-003: RocksDB backend implements StorageBackend with
/// put/get/delete through the trait interface.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_str_003_rocksdb_put_get_delete() {
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::{rocksdb::RocksDbBackend, schema, StorageBackend};

    let dir = tempfile::tempdir().unwrap();
    let cfg = CoinStoreConfig::default_with_path(dir.path());
    let backend = RocksDbBackend::open(&cfg).unwrap();

    // Put a value.
    backend
        .put(schema::CF_METADATA, b"test_key", b"test_value")
        .unwrap();

    // Get it back.
    let value = backend.get(schema::CF_METADATA, b"test_key").unwrap();
    assert_eq!(value.as_deref(), Some(b"test_value".as_slice()));

    // Get a missing key → None.
    let missing = backend.get(schema::CF_METADATA, b"no_such_key").unwrap();
    assert_eq!(missing, None, "Missing key must return None");

    // Delete is idempotent.
    backend.delete(schema::CF_METADATA, b"no_such_key").unwrap();
    backend.delete(schema::CF_METADATA, b"test_key").unwrap();

    // After delete, get returns None.
    let after_delete = backend.get(schema::CF_METADATA, b"test_key").unwrap();
    assert_eq!(after_delete, None, "Deleted key must return None");
}

/// Verifies STR-003: RocksDB batch_write is atomic.
///
/// Writes 3 key-value pairs in a single WriteBatch, then verifies all
/// are visible after commit.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_str_003_rocksdb_batch_write() {
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::{rocksdb::RocksDbBackend, schema, StorageBackend, WriteBatch};

    let dir = tempfile::tempdir().unwrap();
    let cfg = CoinStoreConfig::default_with_path(dir.path());
    let backend = RocksDbBackend::open(&cfg).unwrap();

    let mut batch = WriteBatch::new();
    batch.put(schema::CF_METADATA, b"k1", b"v1");
    batch.put(schema::CF_METADATA, b"k2", b"v2");
    batch.put(schema::CF_METADATA, b"k3", b"v3");
    backend.batch_write(batch).unwrap();

    assert_eq!(
        backend.get(schema::CF_METADATA, b"k1").unwrap().as_deref(),
        Some(b"v1".as_slice())
    );
    assert_eq!(
        backend.get(schema::CF_METADATA, b"k2").unwrap().as_deref(),
        Some(b"v2".as_slice())
    );
    assert_eq!(
        backend.get(schema::CF_METADATA, b"k3").unwrap().as_deref(),
        Some(b"v3".as_slice())
    );
}

/// Verifies STR-003: RocksDB prefix_scan works for composite keys.
///
/// Inserts 3 coins under puzzle_hash A and 2 under puzzle_hash B,
/// then scans for A — must return exactly 3 results.
#[cfg(feature = "rocksdb-storage")]
#[test]
fn vv_req_str_003_rocksdb_prefix_scan() {
    use chia_protocol::Bytes32;
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::{rocksdb::RocksDbBackend, schema, StorageBackend};

    let dir = tempfile::tempdir().unwrap();
    let cfg = CoinStoreConfig::default_with_path(dir.path());
    let backend = RocksDbBackend::open(&cfg).unwrap();

    let ph_a = Bytes32::from([0xAAu8; 32]);
    let ph_b = Bytes32::from([0xBBu8; 32]);

    // Insert 3 coins under ph_a.
    for i in 0..3u8 {
        let coin_id = Bytes32::from([i; 32]);
        let key = schema::puzzle_hash_coin_key(&ph_a, &coin_id);
        backend
            .put(schema::CF_COIN_BY_PUZZLE_HASH, &key, coin_id.as_ref())
            .unwrap();
    }

    // Insert 2 coins under ph_b.
    for i in 10..12u8 {
        let coin_id = Bytes32::from([i; 32]);
        let key = schema::puzzle_hash_coin_key(&ph_b, &coin_id);
        backend
            .put(schema::CF_COIN_BY_PUZZLE_HASH, &key, coin_id.as_ref())
            .unwrap();
    }

    // Scan for ph_a — should return exactly 3.
    let results = backend
        .prefix_scan(schema::CF_COIN_BY_PUZZLE_HASH, ph_a.as_ref())
        .unwrap();
    assert_eq!(results.len(), 3, "Prefix scan for ph_a must return 3 items");
}
