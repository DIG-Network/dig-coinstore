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
//! - **API-005:** [`BlockData`], [`CoinAddition`]
//! - **API-006:** [`ApplyBlockResult`], [`RollbackResult`]
//! - **API-007:** [`CoinStoreStats`]
//! - **API-008:** [`CoinStoreSnapshot`]
//! - **API-009:** [`CoinId`], [`PuzzleHash`], [`UnspentLineageInfo`]
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

use std::collections::HashMap;

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

// ─────────────────────────────────────────────────────────────────────────────
// Block application input (API-005)
// ─────────────────────────────────────────────────────────────────────────────
//
// See: docs/requirements/domains/crate_api/specs/API-005.md,
// docs/resources/SPEC.md §2.4, Chia `coin_store.py` `new_block()` parameters.

/// One transaction-created coin plus ingestion metadata for [`BlockData::additions`].
///
/// **`same_as_parent`:** `true` when this coin’s [`Coin::puzzle_hash`] and [`Coin::amount`] match the
/// spent parent’s puzzle hash and amount — the block pipeline uses this for singleton **fast-forward**
/// eligibility ([`CoinRecord::ff_eligible`], BLK-007).
///
/// **`coin_id`:** For valid blocks this MUST equal [`Coin::coin_id`] on [`Self::coin`]. The struct does not
/// enforce equality at construction time; BLK-* / `apply_block` validation rejects mismatches so callers
/// cannot poison the store with an inconsistent ID ([API-005 test plan](docs/requirements/domains/crate_api/specs/API-005.md#verification)).
///
/// **Chia reference:** `tx_additions` tuples in
/// [`coin_store.py`](https://github.com/Chia-Network/chia-blockchain/blob/main/chia/full_node/coin_store.py).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinAddition {
    /// Coin ID (`sha256(parent || puzzle_hash || amount)`) — use [`Coin::coin_id`], never a custom hash.
    pub coin_id: CoinId,
    /// The created coin (parent id, puzzle hash, amount).
    pub coin: Coin,
    /// Same puzzle hash and amount as the parent coin being spent in this block.
    pub same_as_parent: bool,
}

impl CoinAddition {
    /// Build from a [`Coin`] using [`Coin::coin_id`] as [`Self::coin_id`] (recommended for callers).
    ///
    /// **Rationale:** Centralizes the “no custom coin ID” rule ([STR-005](docs/requirements/domains/crate_structure/specs/STR-005.md),
    /// project `start.md` hard rules).
    #[must_use]
    pub fn from_coin(coin: Coin, same_as_parent: bool) -> Self {
        let coin_id = coin.coin_id();
        Self {
            coin_id,
            coin,
            same_as_parent,
        }
    }
}

/// Pre-validated block state changes: input to `CoinStore::apply_block` (BLK-*).
///
/// The coinstore **does not** run CLVM — the caller extracts additions, removals, coinbase rewards, and
/// hints from execution results, then fills this struct ([API-005](docs/requirements/domains/crate_api/specs/API-005.md#summary)).
///
/// | Field | Role |
/// |-------|------|
/// | `height` / `timestamp` / `block_hash` / `parent_hash` | Chain linkage + time (validated in BLK-002, BLK-003) |
/// | `additions` / `removals` | UTXO delta |
/// | `coinbase_coins` | Farmer + pool rewards (count rules: BLK-004) |
/// | `hints` | CREATE_COIN hint bytes for the hint index (HNT-*) |
/// | `expected_state_root` | Optional post-apply root check (BLK-009) |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockData {
    /// Block height; must be `current_height + 1` when applied ([BLK-002](docs/requirements/domains/block_application/specs/BLK-002.md)).
    pub height: u64,
    /// Unix timestamp (seconds) of the block.
    pub timestamp: u64,
    /// This block’s header hash (tip / chain tracking).
    pub block_hash: Bytes32,
    /// Parent block header hash; must match current tip ([BLK-003](docs/requirements/domains/block_application/specs/BLK-003.md)).
    pub parent_hash: Bytes32,
    /// Transaction-created coins (+ metadata); Chia `tx_additions`.
    pub additions: Vec<CoinAddition>,
    /// Spent coin IDs from transaction spends in this block.
    pub removals: Vec<CoinId>,
    /// Block reward outputs (empty at genesis; ≥ 2 after — [BLK-004](docs/requirements/domains/block_application/specs/BLK-004.md)).
    pub coinbase_coins: Vec<Coin>,
    /// Hint bytes per coin id from CREATE_COIN conditions (wallet / subscription index).
    pub hints: Vec<(CoinId, Bytes32)>,
    /// If set, `apply_block` verifies the computed state root matches ([BLK-009](docs/requirements/domains/block_application/specs/BLK-009.md)).
    pub expected_state_root: Option<Bytes32>,
}

/// Summary returned after a successful [`crate::coin_store::CoinStore::apply_block`] (success path of
/// `Result<ApplyBlockResult, CoinStoreError>`).
///
/// **Source of truth:** [`docs/resources/SPEC.md`](../../docs/resources/SPEC.md) §3.2. Field meanings:
/// post-apply Merkle **state root**, how many coins were **created** (tx additions + coinbase),
/// how many were **marked spent**, and the new tip **height** (= input block height when validation passes).
///
/// **Chia note:** Chia’s `new_block()` updates storage in place and returns nothing; this struct is the
/// dig-coinstore contract for observability and tests ([API-006](docs/requirements/domains/crate_api/specs/API-006.md)).
///
/// # Requirement: API-006
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyBlockResult {
    /// Merkle root after inserting additions, marking removals, and batch-updating the tree (BLK-013).
    pub state_root: Bytes32,
    /// `block.additions.len() + block.coinbase_coins.len()` after successful apply (API-006 field table).
    pub coins_created: usize,
    /// `block.removals.len()` — each removal marks one coin spent at this height.
    pub coins_spent: usize,
    /// New chain tip height (same as applied [`BlockData::height`] on success).
    pub height: u64,
}

/// Summary returned after a successful rollback ([`crate::coin_store::CoinStore::rollback_to_block`],
/// [`crate::coin_store::CoinStore::rollback_n_blocks`]).
///
/// **`modified_coins`:** Chia’s `rollback_to_block` returns `dict[bytes32, CoinRecord]`
/// ([`coin_store.py:567`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L567)).
/// dig-coinstore keeps that map and adds explicit **`coins_deleted`** / **`coins_unspent`** counts
/// (SPEC §1.6 improvement #11; [API-006](docs/requirements/domains/crate_api/specs/API-006.md)).
///
/// **Count invariant (well-formed results):** For each entry in `modified_coins`, the rollback either
/// **deleted** a coin confirmed after the target height (`coins_deleted`) or **reverted a spend**
/// for a coin spent after the target (`coins_unspent`). Callers assembling this struct should ensure
/// `coins_deleted + coins_unspent == modified_coins.len()` when every modified coin is accounted for once.
///
/// # Requirement: API-006
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackResult {
    /// Affected coin IDs → post-rollback–relevant [`CoinRecord`] snapshot (deleted or un-spent row).
    pub modified_coins: HashMap<CoinId, CoinRecord>,
    /// Coins removed from the store (created strictly after the rollback target height).
    pub coins_deleted: usize,
    /// Coins whose `spent_height` was cleared (were spent strictly after target height).
    pub coins_unspent: usize,
    /// Chain tip height after rollback (equals target height on success).
    pub new_height: u64,
}

/// Aggregated chain + coinset metrics returned by [`crate::coin_store::CoinStore::stats`] (API-007 / QRY-010).
///
/// **Design goal (SPEC §1.6 #18):** eventually all aggregate fields are **O(1)** materialized counters
/// updated in the same write batch as `apply_block` / rollback ([`docs/resources/SPEC.md`](../../docs/resources/SPEC.md)).
/// Until PRF-003 lands, [`CoinStore::stats`](crate::coin_store::CoinStore::stats) may derive some fields by
/// scanning `coin_records` (documented on that method) while still returning this single struct shape.
///
/// **Operational use:** dashboards, health checks, and mempool admission logic read one snapshot instead of
/// issuing multiple Chia-style COUNT queries ([`coin_store.py:96-103`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L96)).
///
/// # Requirement: API-007
/// # Spec: docs/requirements/domains/crate_api/specs/API-007.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinStoreStats {
    /// Current chain tip height (same source as [`crate::coin_store::CoinStore::height`]).
    pub height: u64,
    /// Timestamp (seconds) of the current tip block.
    pub timestamp: u64,
    /// Count of coins with [`CoinRecord::spent_height`] == [`None`].
    pub unspent_count: u64,
    /// Count of coins with [`CoinRecord::spent_height`] present (historical spends retained).
    pub spent_count: u64,
    /// Sum of [`Coin::amount`](crate::Coin::amount) over all unspent [`CoinRecord`] rows.
    pub total_unspent_value: u64,
    /// Sparse Merkle root over coin record leaves ([`crate::merkle::SparseMerkleTree`]).
    pub state_root: Bytes32,
    /// Header hash of the current tip block.
    pub tip_hash: Bytes32,
    /// Rows in the forward hint index ([`crate::storage::schema::CF_HINTS`]).
    pub hint_count: u64,
    /// Rows in [`crate::storage::schema::CF_STATE_SNAPSHOTS`] (retained checkpoints).
    pub snapshot_count: usize,
}

/// Serializable checkpoint of the full coinstate at one instant (fast sync / backup / restore).
///
/// **Why it exists:** Chia has no first-class snapshot object; nodes replay from genesis. dig-coinstore
/// standardizes this struct for bincode persistence and future checkpoint sync ([`SPEC.md`](../../docs/resources/SPEC.md)
/// §1.6 improvement #6, §3.14; [API-008](docs/requirements/domains/crate_api/specs/API-008.md)).
///
/// **`block_hash`:** Tip header hash at capture time (same role as [`crate::coin_store::CoinStore::tip_hash`]
/// when produced by a future `CoinStore::snapshot` implementation — tracked under PRF-008).
///
/// **`coins` / `hints`:** Full materialized rows and `(coin_id, hint)` pairs — same `hints` element type as
/// [`BlockData::hints`] for consistency across APIs.
///
/// **`total_coins` / `total_value`:** Per API-008 field table, `total_coins` SHOULD equal `coins.len()` as `u64`,
/// and `total_value` SHOULD be the sum of **unspent** [`CoinRecord`] amounts at capture time. The type does
/// not enforce these invariants; `CoinStore::snapshot` (PRF-008) must populate them coherently.
///
/// # Requirement: API-008
/// # Spec: docs/requirements/domains/crate_api/specs/API-008.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinStoreSnapshot {
    /// Tip height when the snapshot was taken.
    pub height: u64,
    /// Tip block header hash when the snapshot was taken.
    pub block_hash: Bytes32,
    /// Sparse Merkle root over coin record leaves at capture time.
    pub state_root: Bytes32,
    /// Tip block unix timestamp (seconds).
    pub timestamp: u64,
    /// Every coin row (spent and unspent) needed to rebuild indices and history.
    pub coins: HashMap<CoinId, CoinRecord>,
    /// Hint pairs carried on coins at capture time (same shape as [`BlockData::hints`]).
    pub hints: Vec<(CoinId, Bytes32)>,
    /// Count of coin rows (`coins.len()` when built by [`crate::coin_store::CoinStore::snapshot`]).
    pub total_coins: u64,
    /// Sum of unspent coin amounts (mojos) at capture time.
    pub total_value: u64,
}

/// Lineage chain for **one unspent coin**, used when resolving singleton **fast-forward** candidates
/// ([`SPEC.md`](../../docs/resources/SPEC.md) §2.5, [`QRY-008`](../../docs/requirements/domains/queries/specs/QRY-008.md)).
///
/// # Field semantics
///
/// - **`coin_id`:** The unspent leaf’s identity (`sha256(parent || puzzle_hash || amount)`), same as
///   [`Coin::coin_id()`](crate::Coin::coin_id).
/// - **`parent_id`:** That coin’s `parent_coin_info` (the parent coin’s name / id in Chia terminology).
/// - **`parent_parent_id`:** The parent coin row’s `parent_coin_info` (grandparent link in the lineage chain).
///
/// # Genesis / missing rows
///
/// Chia encodes coinbase parents with fixed sentinel bytes; there may be **no** grandparent coin row in the
/// coinset. Callers still store a [`CoinId`] value (often all-zero bytes) for `parent_parent_id` when the
/// wallet or mempool has no further ancestor ([`API-009.md`](../../docs/requirements/domains/crate_api/specs/API-009.md) implementation notes).
///
/// **Serde:** Not required by API-009; QRY-008 returns this in-process. Add `Serialize`/`Deserialize` only if
/// an RPC boundary needs a stable wire shape.
///
/// # Requirement: API-009
/// # Spec: docs/requirements/domains/crate_api/specs/API-009.md
#[derive(Debug, Clone, PartialEq)]
pub struct UnspentLineageInfo {
    /// This coin’s id (the unspent singleton or leaf being queried).
    pub coin_id: CoinId,
    /// `parent_coin_info` of the coin identified by [`Self::coin_id`].
    pub parent_id: CoinId,
    /// `parent_coin_info` of the coin identified by [`Self::parent_id`].
    pub parent_parent_id: CoinId,
}
