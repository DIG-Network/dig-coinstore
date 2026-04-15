//! Query method implementations for dig-coinstore.
//!
//! All coin state query methods on [`crate::coin_store::CoinStore`]: by ID, puzzle hash, height,
//! parent, hint, batch pagination, singleton lineage, and aggregates.
//!
//! # Query method index ([SPEC.md §3.4-§3.11](../../docs/resources/SPEC.md))
//!
//! | Method | SPEC | CF scanned | Chia ref |
//! |--------|------|-----------|----------|
//! | `get_coin_record(id)` | §3.4 | [`CF_COIN_RECORDS`](crate::storage::schema::CF_COIN_RECORDS) | `coin_store.py:181` |
//! | `get_coin_records_by_puzzle_hash(ph)` | §3.5 | [`CF_COIN_BY_PUZZLE_HASH`](crate::storage::schema::CF_COIN_BY_PUZZLE_HASH) | `coin_store.py:257` |
//! | `get_coins_added_at_height(h)` | §3.6 | [`CF_COIN_BY_CONFIRMED_HEIGHT`](crate::storage::schema::CF_COIN_BY_CONFIRMED_HEIGHT) | `coin_store.py:223` |
//! | `get_coins_removed_at_height(h)` | §3.6 | [`CF_COIN_BY_SPENT_HEIGHT`](crate::storage::schema::CF_COIN_BY_SPENT_HEIGHT) | `coin_store.py:238` |
//! | `get_coin_records_by_parent_ids(ids)` | §3.7 | [`CF_COIN_BY_PARENT`](crate::storage::schema::CF_COIN_BY_PARENT) | `coin_store.py:380` |
//! | `batch_coin_states_by_puzzle_hashes(...)` | §3.5 | `CF_COIN_BY_PUZZLE_HASH` + [`CoinStateFilters`](crate::CoinStateFilters) | `coin_store.py:446` |
//! | `get_unspent_lineage_info_for_puzzle_hash(ph)` | §3.10 | [`CF_UNSPENT_BY_PUZZLE_HASH`](crate::storage::schema::CF_UNSPENT_BY_PUZZLE_HASH) | `coin_store.py:651` |
//! | `num_unspent()`, `total_unspent_value()` | §3.11 | materialized counters or scan | `coin_store.py:96` |
//! | `height()`, `tip_hash()`, `state_root()`, `stats()` | §3.12 | in-memory accessors | — |
//!
//! # Adopted Chia behaviors ([SPEC.md §1.5](../../docs/resources/SPEC.md))
//!
//! - **Block boundary pagination** (§1.5 #5): `batch_coin_states_by_puzzle_hashes` never splits blocks.
//! - **Deduplication** (§1.5 #6): dict-keyed by coin_id across direct + hinted results.
//! - **Deterministic sort** (§1.5 #7): `MAX(confirmed_height, spent_height) ASC`.
//! - **Batch size limit** (§1.5 #8): `MAX_PUZZLE_HASH_BATCH_SIZE` ([SPEC.md §2.7](../../docs/resources/SPEC.md)).
//! - **`min_amount` filter** (§1.5 #9): dust coin exclusion.
//! - **Large input batching** (§1.5 #10): chunk by `DEFAULT_LOOKUP_BATCH_SIZE` ([SPEC.md §2.7](../../docs/resources/SPEC.md)).
//!
//! # Requirements: QRY-001 through QRY-011
//! # Spec: docs/requirements/domains/queries/specs/
//! # SPEC.md: §3.4-§3.12 (Query APIs), §1.5 #5-10 (Adopted Chia Behaviors)

use chia_protocol::Bytes32;

use crate::coin_store::CoinStore;
use crate::error::CoinStoreError;
use crate::storage::schema;
use crate::types::{CoinId, CoinRecord};

// ─────────────────────────────────────────────────────────────────────────────
// QRY-001: Point lookups by coin ID
// SPEC.md §3.4, Chia: coin_store.py:181-221
// ─────────────────────────────────────────────────────────────────────────────

impl CoinStore {
    /// Get a single coin record by its coin ID.
    ///
    /// Returns `Ok(None)` if the coin has never existed (not an error).
    /// Returns both spent and unspent coins.
    ///
    /// # Chia reference
    /// [`coin_store.py:181-193`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L181)
    ///
    /// # Requirement: QRY-001
    /// # SPEC.md: §3.4
    pub fn get_coin_record(&self, coin_id: &CoinId) -> Result<Option<CoinRecord>, CoinStoreError> {
        let key = schema::coin_key(coin_id);
        match self.backend.get(schema::CF_COIN_RECORDS, &key)? {
            None => Ok(None),
            Some(bytes) => Ok(Self::decode_coin_record_bytes(&bytes)),
        }
    }

    /// Get multiple coin records by their IDs in a single batch lookup.
    ///
    /// Missing coin IDs are silently skipped (no error). The returned vector
    /// MAY be in any order. Duplicate IDs in the input produce one record.
    ///
    /// # Chia reference
    /// [`coin_store.py:195-221`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L195)
    ///
    /// # Requirement: QRY-001
    /// # SPEC.md: §3.4
    pub fn get_coin_records(
        &self,
        coin_ids: &[CoinId],
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        let mut results = Vec::with_capacity(coin_ids.len());
        for coin_id in coin_ids {
            if let Some(rec) = self.get_coin_record(coin_id)? {
                results.push(rec);
            }
        }
        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-002: Puzzle hash queries
    // SPEC.md §3.5, Chia: coin_store.py:257-307
    // ─────────────────────────────────────────────────────────────────────

    /// Get coin records matching a puzzle hash, with spent/height filters.
    ///
    /// Scans [`CF_COIN_BY_PUZZLE_HASH`](crate::storage::schema::CF_COIN_BY_PUZZLE_HASH)
    /// with `puzzle_hash` as prefix. Filters applied:
    /// - `include_spent = false` → skip coins where `is_spent() == true`
    /// - `start_height..=end_height` → only coins whose `confirmed_height` is in range
    ///
    /// # Chia reference
    /// [`coin_store.py:257-278`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L257)
    ///
    /// # Requirement: QRY-002
    /// # SPEC.md: §3.5
    pub fn get_coin_records_by_puzzle_hash(
        &self,
        include_spent: bool,
        puzzle_hash: &Bytes32,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        let entries = self
            .backend
            .prefix_scan(schema::CF_COIN_BY_PUZZLE_HASH, puzzle_hash.as_ref())?;

        let mut results = Vec::new();
        for (key, _value) in entries {
            // Extract coin_id from trailing 32 bytes of the 64-byte composite key.
            if key.len() < 64 {
                continue;
            }
            let coin_id = schema::coin_id_from_key(&key[32..64]);
            if let Some(rec) = self.get_coin_record(&coin_id)? {
                // Apply filters.
                if !include_spent && rec.is_spent() {
                    continue;
                }
                if rec.confirmed_height < start_height || rec.confirmed_height > end_height {
                    continue;
                }
                results.push(rec);
            }
        }
        Ok(results)
    }

    /// Batch version: get coin records matching any of the given puzzle hashes.
    ///
    /// # Chia reference
    /// [`coin_store.py:280-307`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L280)
    ///
    /// # Requirement: QRY-002
    /// # SPEC.md: §3.5
    pub fn get_coin_records_by_puzzle_hashes(
        &self,
        include_spent: bool,
        puzzle_hashes: &[Bytes32],
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        let mut results = Vec::new();
        for ph in puzzle_hashes {
            let batch =
                self.get_coin_records_by_puzzle_hash(include_spent, ph, start_height, end_height)?;
            results.extend(batch);
        }
        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-003: Height queries
    // SPEC.md §3.6, Chia: coin_store.py:223-254
    // ─────────────────────────────────────────────────────────────────────

    /// Get all coins created (confirmed) at a specific height.
    ///
    /// Scans [`CF_COIN_BY_CONFIRMED_HEIGHT`](crate::storage::schema::CF_COIN_BY_CONFIRMED_HEIGHT)
    /// with `height` (big-endian 8 bytes) as prefix.
    ///
    /// # Chia reference
    /// [`coin_store.py:223-236`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L223)
    ///
    /// # Requirement: QRY-003
    /// # SPEC.md: §3.6
    pub fn get_coins_added_at_height(
        &self,
        height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        let prefix = height.to_be_bytes();
        let entries = self
            .backend
            .prefix_scan(schema::CF_COIN_BY_CONFIRMED_HEIGHT, &prefix)?;

        let mut results = Vec::new();
        for (key, _value) in entries {
            if key.len() < 40 {
                continue;
            }
            let coin_id = schema::coin_id_from_key(&key[8..40]);
            if let Some(rec) = self.get_coin_record(&coin_id)? {
                results.push(rec);
            }
        }
        Ok(results)
    }

    /// Get all coins spent (removed) at a specific height.
    ///
    /// Returns empty vec for height 0 ([SPEC.md §1.5 #12](../../docs/resources/SPEC.md)):
    /// in Chia, unspent coins have `spent_index=0`, so querying height 0 would
    /// match all unspent coins. We adopt the same special case.
    ///
    /// # Chia reference
    /// [`coin_store.py:238-254`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L238)
    ///
    /// # Requirement: QRY-003
    /// # SPEC.md: §3.6, §1.5 #12
    pub fn get_coins_removed_at_height(
        &self,
        height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        // Special case: height 0 always returns empty (SPEC.md §1.5 #12).
        if height == 0 {
            return Ok(Vec::new());
        }

        let prefix = height.to_be_bytes();
        let entries = self
            .backend
            .prefix_scan(schema::CF_COIN_BY_SPENT_HEIGHT, &prefix)?;

        let mut results = Vec::new();
        for (key, _value) in entries {
            if key.len() < 40 {
                continue;
            }
            let coin_id = schema::coin_id_from_key(&key[8..40]);
            if let Some(rec) = self.get_coin_record(&coin_id)? {
                results.push(rec);
            }
        }
        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-004: Parent ID queries
    // SPEC.md §3.7, Chia: coin_store.py:380-406
    // ─────────────────────────────────────────────────────────────────────

    /// Get coin records whose `parent_coin_info` matches one of the given IDs.
    ///
    /// Scans [`CF_COIN_BY_PARENT`](crate::storage::schema::CF_COIN_BY_PARENT)
    /// with each `parent_id` as prefix. Applies spent/height filters.
    ///
    /// # Chia reference
    /// [`coin_store.py:380-406`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L380)
    ///
    /// # Requirement: QRY-004
    /// # SPEC.md: §3.7
    pub fn get_coin_records_by_parent_ids(
        &self,
        include_spent: bool,
        parent_ids: &[CoinId],
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        let mut results = Vec::new();
        for parent_id in parent_ids {
            let entries = self
                .backend
                .prefix_scan(schema::CF_COIN_BY_PARENT, parent_id.as_ref())?;

            for (key, _value) in entries {
                if key.len() < 64 {
                    continue;
                }
                let coin_id = schema::coin_id_from_key(&key[32..64]);
                if let Some(rec) = self.get_coin_record(&coin_id)? {
                    if !include_spent && rec.is_spent() {
                        continue;
                    }
                    if rec.confirmed_height < start_height || rec.confirmed_height > end_height {
                        continue;
                    }
                    results.push(rec);
                }
            }
        }
        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-005: Name queries (by coin IDs with filters)
    // SPEC.md §3.4, Chia: coin_store.py:309-335
    // ─────────────────────────────────────────────────────────────────────

    /// Get coin records by coin IDs with `include_spent` and height range filters.
    ///
    /// Like [`get_coin_records`](Self::get_coin_records) but filters out spent coins
    /// (when `include_spent = false`) and coins outside the height range.
    ///
    /// # Chia reference
    /// [`coin_store.py:309-335`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L309)
    ///
    /// # Requirement: QRY-005
    /// # SPEC.md: §3.4
    pub fn get_coin_records_by_names(
        &self,
        include_spent: bool,
        names: &[CoinId],
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<CoinRecord>, CoinStoreError> {
        let mut results = Vec::new();
        for coin_id in names {
            if let Some(rec) = self.get_coin_record(coin_id)? {
                if !include_spent && rec.is_spent() {
                    continue;
                }
                if rec.confirmed_height < start_height || rec.confirmed_height > end_height {
                    continue;
                }
                results.push(rec);
            }
        }
        Ok(results)
    }
}
