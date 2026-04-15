//! Domain types for dig-coinstore.
//!
//! Core data structures: [`CoinRecord`], [`ChiaCoinRecord`], [`BlockData`], [`CoinAddition`],
//! [`ApplyBlockResult`], [`RollbackResult`], [`CoinStoreStats`],
//! [`CoinStoreSnapshot`], [`UnspentLineageInfo`].
//!
//! Also defines type aliases: [`CoinId`] = [`Bytes32`], [`PuzzleHash`] = [`Bytes32`] (see API-009).
//!
//! # Requirements
//! - **API-002:** [`CoinRecord`], [`ChiaCoinRecord`], [`CoinId`]
//! - API-005..009: additional types (stubs tracked in those specs)
//!
//! ## `ChiaCoinRecord` vs `chia_protocol::CoinRecord`
//!
//! The upstream streamable type is documented at
//! [docs.rs `chia_protocol::CoinRecord`](https://docs.rs/chia-protocol/latest/chia_protocol/struct.CoinRecord.html).
//! This crate pins `chia-protocol` **0.26** together with [`dig_clvm`](https://github.com/DIG-Network/dig-clvm)
//! for a single `Coin` / [`Bytes32`] identity graph. That protocol release does **not** yet export
//! `CoinRecord`, so we define [`ChiaCoinRecord`] here with **identical fields and semantics** to the
//! current Chia reference implementation. When `dig-clvm` upgrades `chia-protocol`, [`ChiaCoinRecord`]
//! should become `pub use chia_protocol::CoinRecord as ChiaCoinRecord` (STR-005).

use serde::{Deserialize, Serialize};

use crate::Bytes32;
use crate::Coin;
use crate::CoinState;

// ─────────────────────────────────────────────────────────────────────────────
// Type aliases (API-002 / API-009)
// ─────────────────────────────────────────────────────────────────────────────

/// 32-byte coin identifier: `sha256(parent_coin_info || puzzle_hash || amount)`.
///
/// Alias of [`Bytes32`] for readable APIs (`get_coin_record(&CoinId)`).
///
/// See: [`Coin::coin_id`], docs/resources/SPEC.md §2.1
pub type CoinId = Bytes32;

/// Puzzle hash (SHA256 of serialized puzzle program). Same underlying type as [`CoinId`].
///
/// Fully specified under API-009; exported early for `CoinRecord::coin_id()` return type clarity.
pub type PuzzleHash = Bytes32;

// ─────────────────────────────────────────────────────────────────────────────
// Chia wire-shaped coin row (interop)
// ─────────────────────────────────────────────────────────────────────────────

/// Row-shaped coin metadata matching Chia full-node / light-wallet `CoinRecord` streamable layout.
///
/// **Why this exists:** `chia-protocol` 0.26 (bundled via `dig-clvm`) does not define this struct;
/// Chia’s reference layout is still the contract for RPC and cross-repo interop. Field names mirror
/// [`chia_protocol::CoinRecord`](https://docs.rs/chia-protocol/latest/chia_protocol/struct.CoinRecord.html).
///
/// **`spent_block_index` sentinel:** `0` means *unspent* in Chia’s encoding; positive values are
/// spent heights. Fast-forward–eligible rows (Python `spent_index == -1`) are not represented here;
/// dig-coinstore uses [`CoinRecord::ff_eligible`] instead once ingested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChiaCoinRecord {
    /// The coin identity and payload (parent id, puzzle hash, amount).
    pub coin: Coin,
    /// Height at which the coin was included (confirmed).
    pub confirmed_block_index: u32,
    /// `0` = unspent; otherwise spent at this height.
    pub spent_block_index: u32,
    /// Block-reward coin vs transaction output.
    pub coinbase: bool,
    /// Block timestamp at `confirmed_block_index`.
    pub timestamp: u64,
}

impl ChiaCoinRecord {
    /// Construct a protocol-shaped row (mainly for tests and RPC adapters).
    #[inline]
    pub const fn new(
        coin: Coin,
        confirmed_block_index: u32,
        spent_block_index: u32,
        coinbase: bool,
        timestamp: u64,
    ) -> Self {
        Self {
            coin,
            confirmed_block_index,
            spent_block_index,
            coinbase,
            timestamp,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CoinRecord — authoritative stored row (API-002)
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle state of one coin in the coinstore.
///
/// Persists after spending for history + rollback (SPEC.md §2.2). Prefer [`Option<u64>`] for
/// [`CoinRecord::spent_height`] over Chia’s `spent_block_index == 0` sentinel to keep Rust matches
/// exhaustive and avoid double meanings for `0`.
///
/// See: [`API-002`](../../docs/requirements/domains/crate_api/specs/API-002.md),
/// Chia reference: <https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/coin_store.py>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinRecord {
    /// Immutable coin identity and value.
    pub coin: Coin,
    /// Height where the coin was created / confirmed.
    pub confirmed_height: u64,
    /// Spend height when spent; [`None`] if still unspent.
    pub spent_height: Option<u64>,
    /// Whether this coin came from the block reward (not a normal tx output).
    pub coinbase: bool,
    /// Timestamp of the confirming block.
    pub timestamp: u64,
    /// Singleton fast-forward candidate (set at ingestion when `same_as_parent`; recomputed on rollback).
    pub ff_eligible: bool,
}

impl CoinRecord {
    /// New **unspent** coin at `confirmed_height` with `ff_eligible = false`.
    ///
    /// Callers set [`CoinRecord::ff_eligible`] later (e.g. `apply_block` when `CoinAddition::same_as_parent`).
    #[must_use]
    pub fn new(coin: Coin, confirmed_height: u64, timestamp: u64, coinbase: bool) -> Self {
        Self {
            coin,
            confirmed_height,
            spent_height: None,
            coinbase,
            timestamp,
            ff_eligible: false,
        }
    }

    /// `true` iff [`CoinRecord::spent_height`] is present.
    #[must_use]
    pub fn is_spent(&self) -> bool {
        self.spent_height.is_some()
    }

    /// Mark spent at `height` (struct does **not** assert double-spend; pipeline validates).
    pub fn spend(&mut self, height: u64) {
        self.spent_height = Some(height);
    }

    /// Same digest as [`Coin::coin_id`] on the embedded coin (spec: never reimplement ID math).
    #[must_use]
    pub fn coin_id(&self) -> CoinId {
        self.coin.coin_id()
    }

    /// Lightweight sync view: maps heights to [`Option<u32>`] per [`CoinState`] wire encoding.
    ///
    /// **Truncation note:** `u64` heights are cast to `u32`. For practical chains this fits; debug
    /// builds assert no loss when truncating `confirmed_height`.
    #[must_use]
    pub fn to_coin_state(&self) -> CoinState {
        debug_assert!(self.confirmed_height <= u64::from(u32::MAX));
        let created = Some(self.confirmed_height as u32);
        let spent = self.spent_height.map(|h| {
            debug_assert!(h <= u64::from(u32::MAX));
            h as u32
        });
        CoinState::new(self.coin, spent, created)
    }

    /// Ingest a Chia-shaped row into the native coinstore model.
    ///
    /// Mapping rules match API-002 / SPEC §2.2:
    /// - `spent_block_index == 0` → [`None`] spent height
    /// - `spent_block_index > 0` → [`Some`] as `u64`
    /// - `ff_eligible` is always reset to `false` (not carried on wire)
    #[must_use]
    pub fn from_chia_coin_record(record: ChiaCoinRecord) -> Self {
        let spent_height = if record.spent_block_index == 0 {
            None
        } else {
            Some(u64::from(record.spent_block_index))
        };
        Self {
            coin: record.coin,
            confirmed_height: u64::from(record.confirmed_block_index),
            spent_height,
            coinbase: record.coinbase,
            timestamp: record.timestamp,
            ff_eligible: false,
        }
    }

    /// Export to Chia wire-shaped row. Loses [`CoinRecord::ff_eligible`].
    ///
    /// # Panics
    /// If `confirmed_height` or `spent_height` exceed `u32::MAX` (should not occur before ~4B blocks).
    #[must_use]
    pub fn to_chia_coin_record(&self) -> ChiaCoinRecord {
        assert!(self.confirmed_height <= u64::from(u32::MAX));
        let spent_block_index = match self.spent_height {
            None => 0,
            Some(h) => {
                assert!(h <= u64::from(u32::MAX));
                h as u32
            }
        };
        ChiaCoinRecord::new(
            self.coin,
            self.confirmed_height as u32,
            spent_block_index,
            self.coinbase,
            self.timestamp,
        )
    }
}

// Placeholder re-exports for modules that import `crate::types::*` (API-005+ will flesh these out).
// They are `pub` so the module graph compiles; tracked as gaps in IMPLEMENTATION_ORDER.md.

/// Placeholder — API-005 (`BlockData`).
#[derive(Debug, Clone, Default)]
pub struct BlockData;

/// Placeholder — API-005 (`CoinAddition`).
#[derive(Debug, Clone, Default)]
pub struct CoinAddition;

/// Placeholder — API-006 (`ApplyBlockResult`).
#[derive(Debug, Clone, Default)]
pub struct ApplyBlockResult;

/// Placeholder — API-006 (`RollbackResult`).
#[derive(Debug, Clone, Default)]
pub struct RollbackResult;

/// Placeholder — API-007 (`CoinStoreStats`).
#[derive(Debug, Clone, Default)]
pub struct CoinStoreStats;

/// Placeholder — API-008 (`CoinStoreSnapshot`).
#[derive(Debug, Clone, Default)]
pub struct CoinStoreSnapshot;

/// Placeholder — API-009 (`UnspentLineageInfo`).
#[derive(Debug, Clone, Default)]
pub struct UnspentLineageInfo;
