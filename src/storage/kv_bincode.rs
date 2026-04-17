//! Bincode options and encode/decode helpers for **KV-persisted** rows ([`STO-008`](../../docs/requirements/domains/storage/specs/STO-008.md)).
//!
//! # Role
//!
//! - **Values:** [`CoinRecord`] in `coin_records` / `archive_coin_records`, and [`CoinStoreSnapshot`] in
//!   `state_snapshots`, are serialized with **fixed-width integers** and **big-endian** byte order so
//!   on-disk layout is deterministic and stable across platforms.
//! - **Not in scope:** Chia `Streamable` wire types — callers use `chia-traits` outside this module
//!   ([`STO-008.md`](../../docs/requirements/domains/storage/specs/STO-008.md) summary vs NORMATIVE).
//!
//! # Backward compatibility
//!
//! Before STO-008 landed, `ff_eligible` coin rows and snapshot blobs used `bincode::serialize` with
//! library defaults (variable-width / little-endian for scalars). Readers therefore **try** the
//! normative options first, then fall back to `bincode::deserialize` without custom options so existing
//! test corpora and early deployments keep loading ([`decode_coin_record_storage`],
//! [`decode_coin_store_snapshot_storage`]).
//!
//! # Usage
//!
//! - Writers: [`encode_coin_record`], [`encode_coin_store_snapshot`] (via [`crate::coin_store::CoinStore`]
//!   internal paths for snapshots and FF coin rows).
//! - Readers: [`decode_coin_record_storage`] / [`decode_coin_store_snapshot_storage`] from decode paths
//!   that must accept both eras.
//!
//! # Requirement: STO-008
//! # Spec: docs/requirements/domains/storage/specs/STO-008.md

use bincode::Options;

use crate::types::{CoinRecord, CoinStoreSnapshot};

/// Shared `bincode::Options` for persisted dig-coinstore structured values.
///
/// Matches the normative table in STO-008: `with_fixint_encoding` + `with_big_endian`.
#[inline]
pub fn kv_bincode_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_big_endian()
}

/// Encode [`CoinRecord`] for `coin_records` / `archive_coin_records` **bincode** values (FF path).
#[inline]
pub fn encode_coin_record(rec: &CoinRecord) -> Result<Vec<u8>, bincode::Error> {
    kv_bincode_options().serialize(rec)
}

/// Decode a coin row using **only** STO-008 options (strict; for tests and tooling).
#[inline]
pub fn decode_coin_record(bytes: &[u8]) -> Result<CoinRecord, bincode::Error> {
    kv_bincode_options().deserialize(bytes)
}

/// Decode a coin row from disk: STO-008 first, then legacy default bincode.
#[inline]
pub fn decode_coin_record_storage(bytes: &[u8]) -> Result<CoinRecord, bincode::Error> {
    decode_coin_record(bytes).or_else(|_| bincode::deserialize(bytes))
}

/// Encode [`CoinStoreSnapshot`] for `state_snapshots` values.
#[inline]
pub fn encode_coin_store_snapshot(snap: &CoinStoreSnapshot) -> Result<Vec<u8>, bincode::Error> {
    kv_bincode_options().serialize(snap)
}

/// Decode a snapshot blob with **only** STO-008 options.
#[inline]
pub fn decode_coin_store_snapshot(bytes: &[u8]) -> Result<CoinStoreSnapshot, bincode::Error> {
    kv_bincode_options().deserialize(bytes)
}

/// Decode a snapshot from disk: STO-008 first, then legacy default bincode.
#[inline]
pub fn decode_coin_store_snapshot_storage(
    bytes: &[u8],
) -> Result<CoinStoreSnapshot, bincode::Error> {
    decode_coin_store_snapshot(bytes).or_else(|_| bincode::deserialize(bytes))
}
