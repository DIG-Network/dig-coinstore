//! MRK-006 — deterministic Merkle **leaf payload** from a [`crate::types::CoinRecord`].
//!
//! Block application (BLK-013) will insert/update leaves keyed by `coin_id` with the digest
//! returned here. Rollback removes by key only; spend transitions re-hash the updated row.
//!
//! # Algorithm (NORMATIVE MRK-006)
//!
//! `coin_record_hash(record) = SHA256( bincode_encode_STO008(record) )`
//!
//! - **SHA-256:** [`chia_sha2::Sha256`] — same crate Chia uses for `Coin::coin_id()` and our
//!   internal Merkle node hashing (`src/merkle/mod.rs`: [`super::merkle_node_hash`]), avoiding
//!   cross-implementation drift.
//! - **Bincode:** exactly [`crate::storage::kv_bincode::encode_coin_record`] — fixed-width
//!   integers + big-endian per **STO-008** / `docs/requirements/domains/storage/specs/STO-008.md`.
//!   Reusing the helper guarantees the Merkle leaf preimage matches the bytes persisted in
//!   `coin_records` column families.
//!
//! # Not `merkle_leaf_hash`
//!
//! [`super::merkle_leaf_hash`] applies domain byte `0x00` to arbitrary payloads for **generic**
//! SMT tests. MRK-006 leaf values are **raw** SHA-256 outputs of the STO-008 coin row encoding
//! (no extra domain prefix) — that is the value stored in [`super::SparseMerkleTree`] leaves for
//! production coin rows once BLK-013 lands.
//!
//! # Panics
//!
//! [`CoinRecord`] encoding must always succeed for invariants held by the type system. If
//! `encode_coin_record` fails, we panic with a clear message (same contract as MRK-006 spec
//! snippet using `expect`).

use chia_sha2::Sha256;

use crate::storage::kv_bincode;
use crate::types::CoinRecord;
use crate::Bytes32;

/// Compute the sparse-Merkle **leaf value** for `record` (MRK-006 / MRK-001 consumer).
///
/// # Returns
///
/// A [`Bytes32`] digest suitable as the `value` argument to [`super::SparseMerkleTree::batch_insert`]
/// / [`super::SparseMerkleTree::batch_update`] keyed by `record.coin_id()`.
///
/// # Determinism
///
/// Identical [`CoinRecord`] values always yield identical hashes on any platform where this crate
/// builds — fixed by STO-008 bincode options + pinned `bincode` / `serde` in `Cargo.lock`.
///
/// # See also
///
/// - `docs/requirements/domains/merkle/specs/MRK-006.md`
/// - [`crate::storage::kv_bincode::encode_coin_record`]
#[must_use]
pub fn coin_record_hash(record: &CoinRecord) -> Bytes32 {
    let serialized = kv_bincode::encode_coin_record(record)
        .expect("CoinRecord must bincode-encode with STO-008 options for merkle leaf preimage");
    let mut hasher = Sha256::new();
    hasher.update(&serialized);
    let out = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&out);
    Bytes32::from(bytes)
}
