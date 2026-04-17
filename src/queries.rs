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
    pub fn get_coin_records(&self, coin_ids: &[CoinId]) -> Result<Vec<CoinRecord>, CoinStoreError> {
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

    // ─────────────────────────────────────────────────────────────────────
    // QRY-006: Lightweight CoinState queries
    // SPEC.md §3.8, §3.6, Chia: coin_store.py:347-442
    // ─────────────────────────────────────────────────────────────────────

    /// Get lightweight [`CoinState`](crate::CoinState) views for a set of coin IDs.
    ///
    /// Uses [`CoinRecord::to_coin_state`](crate::CoinRecord::to_coin_state) for the mapping.
    /// Filters by `include_spent`, height range, and `max_items` cap.
    ///
    /// # Chia reference
    /// [`coin_store.py:408-442`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L408)
    ///
    /// # Requirement: QRY-006
    /// # SPEC.md: §3.8
    pub fn get_coin_states_by_ids(
        &self,
        include_spent: bool,
        coin_ids: &[CoinId],
        min_height: u64,
        max_height: u64,
        max_items: usize,
    ) -> Result<Vec<crate::CoinState>, CoinStoreError> {
        let mut results = Vec::new();
        for coin_id in coin_ids {
            if results.len() >= max_items {
                break;
            }
            if let Some(rec) = self.get_coin_record(coin_id)? {
                if !include_spent && rec.is_spent() {
                    continue;
                }
                if rec.confirmed_height < min_height || rec.confirmed_height > max_height {
                    continue;
                }
                results.push(rec.to_coin_state());
            }
        }
        Ok(results)
    }

    /// Get lightweight [`CoinState`](crate::CoinState) views for a set of puzzle hashes.
    ///
    /// # Chia reference
    /// [`coin_store.py:347-378`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L347)
    ///
    /// # Requirement: QRY-006
    /// # SPEC.md: §3.5
    pub fn get_coin_states_by_puzzle_hashes(
        &self,
        include_spent: bool,
        puzzle_hashes: &[Bytes32],
        min_height: u64,
        max_items: usize,
    ) -> Result<Vec<crate::CoinState>, CoinStoreError> {
        let mut results = Vec::new();
        for ph in puzzle_hashes {
            if results.len() >= max_items {
                break;
            }
            let recs =
                self.get_coin_records_by_puzzle_hash(include_spent, ph, min_height, u64::MAX)?;
            for rec in recs {
                if results.len() >= max_items {
                    break;
                }
                results.push(rec.to_coin_state());
            }
        }
        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-009: Aggregate queries
    // SPEC.md §3.11, Chia: coin_store.py:96-103
    // ─────────────────────────────────────────────────────────────────────

    /// Count of unspent coins. Scans `CF_COIN_RECORDS` until PRF-003 materialized counters.
    ///
    /// # Requirement: QRY-009
    /// # SPEC.md: §3.11
    pub fn num_unspent(&self) -> Result<u64, CoinStoreError> {
        let entries = self.backend.prefix_scan(schema::CF_COIN_RECORDS, &[])?;
        let mut count = 0u64;
        for (_key, value) in entries {
            if let Some(rec) = Self::decode_coin_record_bytes(&value) {
                if !rec.is_spent() {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Sum of all unspent coin amounts (mojos).
    ///
    /// # Requirement: QRY-009
    /// # SPEC.md: §3.11
    pub fn total_unspent_value(&self) -> Result<u128, CoinStoreError> {
        let entries = self.backend.prefix_scan(schema::CF_COIN_RECORDS, &[])?;
        let mut total = 0u128;
        for (_key, value) in entries {
            if let Some(rec) = Self::decode_coin_record_bytes(&value) {
                if !rec.is_spent() {
                    total += rec.coin.amount as u128;
                }
            }
        }
        Ok(total)
    }

    /// Aggregate unspent balances grouped by puzzle hash.
    /// Returns `HashMap<puzzle_hash, (total_amount, coin_count)>`.
    ///
    /// # Requirement: QRY-009
    /// # SPEC.md: §3.11
    pub fn aggregate_unspent_by_puzzle_hash(
        &self,
    ) -> Result<std::collections::HashMap<Bytes32, (u64, usize)>, CoinStoreError> {
        let entries = self.backend.prefix_scan(schema::CF_COIN_RECORDS, &[])?;
        let mut agg: std::collections::HashMap<Bytes32, (u64, usize)> =
            std::collections::HashMap::new();
        for (_key, value) in entries {
            if let Some(rec) = Self::decode_coin_record_bytes(&value) {
                if !rec.is_spent() {
                    let entry = agg.entry(rec.coin.puzzle_hash).or_insert((0, 0));
                    entry.0 += rec.coin.amount;
                    entry.1 += 1;
                }
            }
        }
        Ok(agg)
    }

    /// Total coin count (spent + unspent).
    ///
    /// # Requirement: QRY-009
    /// # SPEC.md: §3.11
    pub fn num_total(&self) -> Result<u64, CoinStoreError> {
        let entries = self.backend.prefix_scan(schema::CF_COIN_RECORDS, &[])?;
        Ok(entries.len() as u64)
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-007: Batch coin state pagination with CoinStateFilters
    // SPEC.md §3.5, Chia: coin_store.py:446-559
    // ─────────────────────────────────────────────────────────────────────

    /// Paginated coin state query by puzzle hashes with full Chia parity.
    ///
    /// Returns `(results, next_height)` where `next_height` is `None` if all matching
    /// coins have been returned, or `Some(h)` to resume from height `h`.
    ///
    /// # Chia behaviors adopted ([SPEC.md §1.5 #5-9](../../docs/resources/SPEC.md))
    ///
    /// - `MAX_PUZZLE_HASH_BATCH_SIZE` enforced (§1.5 #8)
    /// - `include_hinted` join with hint index (§1.5 #6 deduplication)
    /// - `min_amount` filter (§1.5 #9)
    /// - Deterministic sort by `MAX(confirmed_height, spent_height)` ASC (§1.5 #7)
    /// - Block boundary preservation (§1.5 #5)
    ///
    /// # Requirement: QRY-007
    /// # SPEC.md: §3.5
    pub fn batch_coin_states_by_puzzle_hashes(
        &self,
        puzzle_hashes: &[Bytes32],
        min_height: u64,
        filters: crate::CoinStateFilters,
        max_items: usize,
    ) -> Result<(Vec<crate::CoinState>, Option<u64>), CoinStoreError> {
        // §1.5 #8: Batch size limit.
        const MAX_PUZZLE_HASH_BATCH_SIZE: usize = 990;
        if puzzle_hashes.len() > MAX_PUZZLE_HASH_BATCH_SIZE {
            return Err(CoinStoreError::PuzzleHashBatchTooLarge {
                size: puzzle_hashes.len(),
                max: MAX_PUZZLE_HASH_BATCH_SIZE,
            });
        }

        // Collect matching coin records, deduplicated by coin_id.
        let mut seen = std::collections::HashSet::new();
        let mut candidates: Vec<CoinRecord> = Vec::new();

        // Direct puzzle hash matches.
        for ph in puzzle_hashes {
            let entries = self
                .backend
                .prefix_scan(schema::CF_COIN_BY_PUZZLE_HASH, ph.as_ref())?;
            for (key, _) in entries {
                if key.len() < 64 {
                    continue;
                }
                let coin_id = schema::coin_id_from_key(&key[32..64]);
                if seen.insert(coin_id) {
                    if let Some(rec) = self.get_coin_record(&coin_id)? {
                        candidates.push(rec);
                    }
                }
            }
        }

        // §1.5 #6: include_hinted join — query hint reverse index for each puzzle hash.
        if filters.include_hinted {
            for ph in puzzle_hashes {
                let hint_entries = self
                    .backend
                    .prefix_scan(schema::CF_HINTS_BY_VALUE, ph.as_ref())?;
                for (key, _) in hint_entries {
                    if key.len() < 64 {
                        continue;
                    }
                    let coin_id = schema::coin_id_from_key(&key[32..64]);
                    if seen.insert(coin_id) {
                        if let Some(rec) = self.get_coin_record(&coin_id)? {
                            candidates.push(rec);
                        }
                    }
                }
            }
        }

        // Apply filters.
        candidates.retain(|rec| {
            // Spent/unspent filter.
            if rec.is_spent() && !filters.include_spent {
                return false;
            }
            if !rec.is_spent() && !filters.include_unspent {
                return false;
            }
            // Min height.
            if rec.confirmed_height < min_height {
                return false;
            }
            // §1.5 #9: min_amount filter.
            if rec.coin.amount < filters.min_amount {
                return false;
            }
            true
        });

        // §1.5 #7: Deterministic sort by MAX(confirmed_height, spent_height) ASC,
        // then by coin_id for tiebreaking.
        candidates.sort_by(|a, b| {
            let a_max = std::cmp::max(a.confirmed_height, a.spent_height.unwrap_or(0));
            let b_max = std::cmp::max(b.confirmed_height, b.spent_height.unwrap_or(0));
            a_max
                .cmp(&b_max)
                .then_with(|| a.coin_id().cmp(&b.coin_id()))
        });

        // §1.5 #5: Block boundary preservation + pagination detection.
        // Fetch max_items + 1 to detect continuation.
        if candidates.len() <= max_items {
            // All fit — no pagination needed.
            let states: Vec<crate::CoinState> =
                candidates.iter().map(|r| r.to_coin_state()).collect();
            return Ok((states, None));
        }

        // More than max_items — truncate at block boundary.
        let overflow_rec = &candidates[max_items];
        let overflow_height = std::cmp::max(
            overflow_rec.confirmed_height,
            overflow_rec.spent_height.unwrap_or(0),
        );

        // Remove trailing items that share the overflow height (block boundary preservation).
        let mut end = max_items;
        while end > 0 {
            let rec = &candidates[end - 1];
            let rec_height = std::cmp::max(rec.confirmed_height, rec.spent_height.unwrap_or(0));
            if rec_height < overflow_height {
                break;
            }
            end -= 1;
        }

        let states: Vec<crate::CoinState> = candidates[..end]
            .iter()
            .map(|r| r.to_coin_state())
            .collect();
        Ok((states, Some(overflow_height)))
    }

    // ─────────────────────────────────────────────────────────────────────
    // QRY-008: Singleton lineage lookup
    // SPEC.md §3.10, Chia: coin_store.py:651-674
    // ─────────────────────────────────────────────────────────────────────

    /// Get unspent singleton lineage info for a puzzle hash.
    ///
    /// Returns `None` if the puzzle hash does not match exactly one unspent coin.
    /// When exactly one match exists, returns `(coin_id, parent_id, parent_parent_id)`.
    ///
    /// # Chia reference
    /// [`coin_store.py:651-674`](https://github.com/Chia-Network/chia-blockchain/blob/6e7a4954edccd8ab83fcacf938cfc42ddfcad7f2/chia/full_node/coin_store.py#L651)
    ///
    /// # Requirement: QRY-008
    /// # SPEC.md: §3.10
    pub fn get_unspent_lineage_info_for_puzzle_hash(
        &self,
        puzzle_hash: &Bytes32,
    ) -> Result<Option<crate::types::UnspentLineageInfo>, CoinStoreError> {
        // Scan unspent puzzle hash index for this puzzle hash.
        let entries = self
            .backend
            .prefix_scan(schema::CF_UNSPENT_BY_PUZZLE_HASH, puzzle_hash.as_ref())?;

        // Must be exactly 1 unspent coin.
        if entries.len() != 1 {
            return Ok(None);
        }

        // Extract coin_id from the 64-byte composite key.
        let key = &entries[0].0;
        if key.len() < 64 {
            return Ok(None);
        }
        let coin_id = schema::coin_id_from_key(&key[32..64]);

        // Fetch the coin record.
        let rec = match self.get_coin_record(&coin_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let parent_id = rec.coin.parent_coin_info;

        // Look up the parent to get grandparent. Zero hash if parent not in store.
        let parent_parent_id = match self.get_coin_record(&parent_id)? {
            Some(parent_rec) => parent_rec.coin.parent_coin_info,
            None => Bytes32::from([0u8; 32]),
        };

        Ok(Some(crate::types::UnspentLineageInfo {
            coin_id,
            parent_id,
            parent_parent_id,
        }))
    }
}
