//! Column family names and key encoding/decoding helpers.
//!
//! Defines the storage schema: column family name constants and composite key
//! construction functions used by both RocksDB and LMDB backends.
//!
//! All key formats use **big-endian** encoding for heights/integers so that
//! lexicographic byte comparison matches numeric order. This is critical for
//! range scans in ordered key-value stores.
//!
//! # Requirement: STO-002, STO-008
//! # Spec: docs/requirements/domains/storage/specs/STO-008.md
//! # SPEC.md: Section 7.2 (column family key/value table)

use chia_protocol::Bytes32;

// ─────────────────────────────────────────────────────────────────────────────
// Column family name constants
// ─────────────────────────────────────────────────────────────────────────────
// These **logical** names are used by [`StorageBackend`](crate::storage::StorageBackend):
// RocksDB maps each to its own column family (STO-002); the LMDB backend maps them onto **six**
// physical LMDB databases with optional in-DB key tags (STO-003 — see `src/storage/lmdb.rs`).
// The twelve strings mirror SPEC Section 7.2 exactly.
//
// Requirement: STO-002 (RocksDB), STO-003 (LMDB)

/// Primary coin record storage. Key: coin_id (32 bytes). Value: bincode CoinRecord.
pub const CF_COIN_RECORDS: &str = "coin_records";

/// All coins indexed by puzzle hash. Key: puzzle_hash + coin_id (64 bytes). Value: coin_id.
pub const CF_COIN_BY_PUZZLE_HASH: &str = "coin_by_puzzle_hash";

/// Unspent-only coins by puzzle hash. Key: puzzle_hash + coin_id (64 bytes). Value: empty.
/// Much smaller than the full index — only contains currently-unspent coins.
/// Requirement: PRF-004
pub const CF_UNSPENT_BY_PUZZLE_HASH: &str = "unspent_by_puzzle_hash";

/// Coins indexed by parent coin info. Key: parent_id + coin_id (64 bytes). Value: coin_id.
pub const CF_COIN_BY_PARENT: &str = "coin_by_parent";

/// Coins indexed by confirmed (creation) height. Key: height_BE + coin_id (40 bytes). Value: coin_id.
/// Used for `get_coins_added_at_height()` and rollback coin deletion.
pub const CF_COIN_BY_CONFIRMED_HEIGHT: &str = "coin_by_confirmed_height";

/// Coins indexed by spent height. Key: height_BE + coin_id (40 bytes). Value: coin_id.
/// Used for `get_coins_removed_at_height()` and rollback un-spending.
pub const CF_COIN_BY_SPENT_HEIGHT: &str = "coin_by_spent_height";

/// Hints forward index. Key: coin_id + hint (up to 64 bytes). Value: empty.
/// Used to look up which hints a coin has.
pub const CF_HINTS: &str = "hints";

/// Hints reverse index. Key: hint + coin_id (up to 64 bytes). Value: empty.
/// Used to look up which coins have a given hint.
pub const CF_HINTS_BY_VALUE: &str = "hints_by_value";

/// Persistent Merkle tree internal nodes. Key: level(1) + path(32) = 33 bytes. Value: hash(32).
pub const CF_MERKLE_NODES: &str = "merkle_nodes";

/// Archived spent coin records (beyond rollback window). Key: coin_id (32). Value: bincode CoinRecord.
/// Requirement: PRF-005
pub const CF_ARCHIVE_COIN_RECORDS: &str = "archive_coin_records";

/// State snapshots keyed by height. Key: height_BE (8 bytes). Value: serialized snapshot.
pub const CF_STATE_SNAPSHOTS: &str = "state_snapshots";

/// Chain metadata (tip, config, materialized counters). Key: string. Value: bytes.
pub const CF_METADATA: &str = "metadata";

/// All column family names as a slice, useful for bulk creation during DB initialization.
pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    CF_COIN_RECORDS,
    CF_COIN_BY_PUZZLE_HASH,
    CF_UNSPENT_BY_PUZZLE_HASH,
    CF_COIN_BY_PARENT,
    CF_COIN_BY_CONFIRMED_HEIGHT,
    CF_COIN_BY_SPENT_HEIGHT,
    CF_HINTS,
    CF_HINTS_BY_VALUE,
    CF_MERKLE_NODES,
    CF_ARCHIVE_COIN_RECORDS,
    CF_STATE_SNAPSHOTS,
    CF_METADATA,
];

/// Per-column-family RocksDB `write_buffer_size` (bytes) from STO-002 “Per-CF Configuration Summary”.
///
/// **Invariant:** element `i` applies to `ALL_COLUMN_FAMILIES[i]`. The RocksDB backend
/// (`src/storage/rocksdb.rs`, feature `rocksdb-storage`) and `tests/sto_002_tests.rs` rely on this
/// alignment so tuning stays
/// single-sourced (STO-002 § Per-CF Configuration Summary, “Write Buffer” column).
///
/// # Requirements: STO-002
/// # Spec: docs/requirements/domains/storage/specs/STO-002.md
pub const STO002_ROCKS_WRITE_BUFFER_BYTES: [usize; 12] = [
    64 * 1024 * 1024, // coin_records
    32 * 1024 * 1024, // coin_by_puzzle_hash
    32 * 1024 * 1024, // unspent_by_puzzle_hash
    16 * 1024 * 1024, // coin_by_parent
    16 * 1024 * 1024, // coin_by_confirmed_height
    16 * 1024 * 1024, // coin_by_spent_height
    16 * 1024 * 1024, // hints
    16 * 1024 * 1024, // hints_by_value
    64 * 1024 * 1024, // merkle_nodes
    16 * 1024 * 1024, // archive_coin_records
    8 * 1024 * 1024,  // state_snapshots
    4 * 1024 * 1024,  // metadata
];

const _: () = assert!(ALL_COLUMN_FAMILIES.len() == STO002_ROCKS_WRITE_BUFFER_BYTES.len());

// ─────────────────────────────────────────────────────────────────────────────
// Key encoding helpers
// ─────────────────────────────────────────────────────────────────────────────
// All composite keys use fixed-width, big-endian encoding so that byte-level
// lexicographic comparison matches semantic ordering (heights sort numerically,
// prefix scans work on the first component).

/// Encode a coin_id as a 32-byte key (identity — coin_id is already 32 bytes).
///
/// Used for: `coin_records`, `archive_coin_records`.
#[inline]
pub fn coin_key(coin_id: &Bytes32) -> [u8; 32] {
    let mut key = [0u8; 32];
    key.copy_from_slice(coin_id.as_ref());
    key
}

/// Decode a 32-byte key back to a coin_id.
#[inline]
pub fn coin_id_from_key(key: &[u8]) -> Bytes32 {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&key[..32]);
    Bytes32::from(bytes)
}

/// Encode puzzle_hash + coin_id as a 64-byte composite key.
///
/// Used for: `coin_by_puzzle_hash`, `unspent_by_puzzle_hash`.
/// The puzzle_hash prefix enables prefix scans for all coins with a given puzzle hash.
#[inline]
pub fn puzzle_hash_coin_key(puzzle_hash: &Bytes32, coin_id: &Bytes32) -> [u8; 64] {
    let mut key = [0u8; 64];
    key[..32].copy_from_slice(puzzle_hash.as_ref());
    key[32..].copy_from_slice(coin_id.as_ref());
    key
}

/// Extract the puzzle_hash (first 32 bytes) from a composite key.
#[inline]
pub fn puzzle_hash_from_key(key: &[u8]) -> Bytes32 {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&key[..32]);
    Bytes32::from(bytes)
}

/// Encode parent_coin_info + coin_id as a 64-byte composite key.
///
/// Used for: `coin_by_parent`.
#[inline]
pub fn parent_coin_key(parent_id: &Bytes32, coin_id: &Bytes32) -> [u8; 64] {
    let mut key = [0u8; 64];
    key[..32].copy_from_slice(parent_id.as_ref());
    key[32..].copy_from_slice(coin_id.as_ref());
    key
}

/// Encode height (u64 big-endian) + coin_id as a 40-byte composite key.
///
/// Big-endian ensures lexicographic sort matches numeric order.
/// Used for: `coin_by_confirmed_height`, `coin_by_spent_height`.
#[inline]
pub fn height_coin_key(height: u64, coin_id: &Bytes32) -> [u8; 40] {
    let mut key = [0u8; 40];
    key[..8].copy_from_slice(&height.to_be_bytes());
    key[8..].copy_from_slice(coin_id.as_ref());
    key
}

/// Decode a height + coin_id composite key.
#[inline]
pub fn height_coin_from_key(key: &[u8]) -> (u64, Bytes32) {
    let height = u64::from_be_bytes(key[..8].try_into().expect("key must be >= 8 bytes"));
    let mut coin_bytes = [0u8; 32];
    coin_bytes.copy_from_slice(&key[8..40]);
    (height, Bytes32::from(coin_bytes))
}

/// Encode a height as an 8-byte big-endian key.
///
/// Used for: `state_snapshots`.
#[inline]
pub fn snapshot_key(height: u64) -> [u8; 8] {
    height.to_be_bytes()
}

/// Decode a snapshot key back to a height.
#[inline]
pub fn height_from_snapshot_key(key: &[u8]) -> u64 {
    u64::from_be_bytes(key[..8].try_into().expect("key must be 8 bytes"))
}
