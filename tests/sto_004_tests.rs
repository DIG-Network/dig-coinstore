//! # STO-004 Tests — RocksDB bloom filter configuration
//!
//! **Normative:** [`STO-004`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-004)
//! **Spec:** [`STO-004.md`](../../docs/requirements/domains/storage/specs/STO-004.md)
//! **Implementation:** [`src/storage/rocksdb.rs`](../../src/storage/rocksdb.rs)
//! **Schema CF names:** [`src/storage/schema.rs`](../../src/storage/schema.rs)
//!
//! ## What this requirement enforces
//!
//! RocksDB SST and memtable filters are tuned **per column family** so point lookups skip disk reads on
//! misses, puzzle-hash prefix scans get prefix-aware memtable support, and purely sequential indices do not
//! waste RAM on useless blooms. **STO-004** also requires pinning L0 filter/index blocks for every bloom-enabled
//! CF so freshly flushed data stays hot in the block cache.
//!
//! ## How passing tests prove acceptance
//!
//! | Spec / test plan row | Mechanism |
//! |----------------------|-----------|
//! | Full bloom (10 bits/key) on point CFs | [`dig_coinstore::storage::rocksdb::sto004_bloom_plan_for_column_family`] on `coin_records` matches `Some(10)`, `block_based = false`, `pin_l0 = true`. |
//! | Prefix bloom (32-byte prefix + memtable ratio) | Plan on `coin_by_puzzle_hash` matches prefix row + [`STO004_MEMTABLE_PREFIX_BLOOM_RATIO`](../../src/storage/rocksdb.rs). |
//! | No bloom on sequential CFs | Plan on `coin_by_confirmed_height` has all bloom fields absent / false. |
//! | Bloom disabled via **API-003** (`bloom_filter = false`) | Plan collapses to “all off” while RocksDB still opens (prefix extractors remain in implementation — verified indirectly via [`vv_req_sto_004_open_succeeds_with_bloom_disabled`]). |
//! | Configuration matrix (test plan `test_bloom_config_all_cfs`) | Loop [`dig_coinstore::storage::schema::ALL_COLUMN_FAMILIES`] and compare to the STO-004 configuration table. |
//!
//! **Feature gate:** `rocksdb-storage` (default). LMDB is intentionally untouched (**STO-004** acceptance: LMDB unaffected).
//!
//! **Tooling note:** GitNexus CLI failed in this agent environment (`npm` / `node.target`); blast radius was reviewed
//! manually (only `src/storage/rocksdb.rs` and this file). Repomix packs under `.repomix/` preceded edits per `docs/prompt/start.md`.

mod helpers;

#[cfg(feature = "rocksdb-storage")]
mod rocks_sto004 {
    use dig_coinstore::config::{
        CoinStoreConfig, StorageBackend as Engine, BLOOM_FILTER_BITS_PER_KEY,
    };
    use dig_coinstore::storage::rocksdb::{
        sto004_bloom_plan_for_column_family, RocksDbBackend, Sto004BloomPlan,
        STO004_MEMTABLE_PREFIX_BLOOM_RATIO,
    };
    use dig_coinstore::storage::schema::{
        ALL_COLUMN_FAMILIES, CF_ARCHIVE_COIN_RECORDS, CF_COIN_BY_CONFIRMED_HEIGHT,
        CF_COIN_BY_PARENT, CF_COIN_BY_PUZZLE_HASH, CF_COIN_BY_SPENT_HEIGHT, CF_COIN_RECORDS,
        CF_HINTS, CF_HINTS_BY_VALUE, CF_MERKLE_NODES, CF_METADATA, CF_STATE_SNAPSHOTS,
        CF_UNSPENT_BY_PUZZLE_HASH,
    };

    fn cfg_bloom_on(path: &std::path::Path) -> CoinStoreConfig {
        CoinStoreConfig::default_with_path(path)
            .with_backend(Engine::RocksDb)
            .with_bloom_filter(true)
    }

    fn cfg_bloom_off(path: &std::path::Path) -> CoinStoreConfig {
        CoinStoreConfig::default_with_path(path)
            .with_backend(Engine::RocksDb)
            .with_bloom_filter(false)
    }

    fn plan_off() -> Sto004BloomPlan {
        Sto004BloomPlan {
            sst_bloom_bits_per_key: None,
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: false,
            memtable_prefix_bloom_ratio: None,
        }
    }

    /// **STO-004 / test plan `test_bloom_full_config`:** `coin_records` row in the configuration matrix.
    #[test]
    fn vv_req_sto_004_full_bloom_plan_coin_records() {
        let dir = super::helpers::temp_dir();
        let cfg = cfg_bloom_on(dir.path());
        let p = sto004_bloom_plan_for_column_family(CF_COIN_RECORDS, &cfg);
        assert_eq!(
            p,
            Sto004BloomPlan {
                sst_bloom_bits_per_key: Some(BLOOM_FILTER_BITS_PER_KEY),
                sst_bloom_uses_block_based_builder: false,
                pin_l0_filter_and_index_in_cache: true,
                memtable_prefix_bloom_ratio: None,
            },
            "STO-004 requires classic full-key bloom (rust-rocksdb block_based=false) at SPEC bits/key"
        );
    }

    /// **STO-004 / test plan `test_bloom_prefix_config`:** prefix CF uses same SST bloom policy + memtable ratio.
    #[test]
    fn vv_req_sto_004_prefix_bloom_plan_coin_by_puzzle_hash() {
        let dir = super::helpers::temp_dir();
        let cfg = cfg_bloom_on(dir.path());
        let p = sto004_bloom_plan_for_column_family(CF_COIN_BY_PUZZLE_HASH, &cfg);
        assert_eq!(
            p,
            Sto004BloomPlan {
                sst_bloom_bits_per_key: Some(BLOOM_FILTER_BITS_PER_KEY),
                sst_bloom_uses_block_based_builder: false,
                pin_l0_filter_and_index_in_cache: true,
                memtable_prefix_bloom_ratio: Some(STO004_MEMTABLE_PREFIX_BLOOM_RATIO),
            },
            "STO-004 normative snippet uses set_bloom_filter(10, false) even for prefix CFs"
        );
    }

    /// **STO-004 / test plan `test_bloom_disabled_sequential`:** height indices are sequential-access (no bloom).
    #[test]
    fn vv_req_sto_004_no_bloom_plan_coin_by_confirmed_height() {
        let dir = super::helpers::temp_dir();
        let cfg = cfg_bloom_on(dir.path());
        let p = sto004_bloom_plan_for_column_family(CF_COIN_BY_CONFIRMED_HEIGHT, &cfg);
        assert_eq!(p, plan_off());
    }

    /// **STO-004 / API-003 interplay:** operator disables blooms — plan is all-off (cheap CI / constrained hosts).
    #[test]
    fn vv_req_sto_004_bloom_globally_disabled_yields_empty_plan() {
        let dir = super::helpers::temp_dir();
        let cfg = cfg_bloom_off(dir.path());
        assert_eq!(
            sto004_bloom_plan_for_column_family(CF_COIN_RECORDS, &cfg),
            plan_off()
        );
        assert_eq!(
            sto004_bloom_plan_for_column_family(CF_COIN_BY_PUZZLE_HASH, &cfg),
            plan_off()
        );
    }

    /// **STO-004 / test plan `test_bloom_config_all_cfs`:** every logical CF matches the published matrix.
    #[test]
    fn vv_req_sto_004_configuration_matrix_all_column_families() {
        let dir = super::helpers::temp_dir();
        let cfg = cfg_bloom_on(dir.path());

        let full = Sto004BloomPlan {
            sst_bloom_bits_per_key: Some(BLOOM_FILTER_BITS_PER_KEY),
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: true,
            memtable_prefix_bloom_ratio: None,
        };
        let prefix = Sto004BloomPlan {
            sst_bloom_bits_per_key: Some(BLOOM_FILTER_BITS_PER_KEY),
            sst_bloom_uses_block_based_builder: false,
            pin_l0_filter_and_index_in_cache: true,
            memtable_prefix_bloom_ratio: Some(STO004_MEMTABLE_PREFIX_BLOOM_RATIO),
        };

        for cf in ALL_COLUMN_FAMILIES {
            let got = sto004_bloom_plan_for_column_family(cf, &cfg);
            let want = match *cf {
                CF_COIN_BY_PUZZLE_HASH | CF_UNSPENT_BY_PUZZLE_HASH | CF_HINTS_BY_VALUE => prefix,
                CF_COIN_BY_CONFIRMED_HEIGHT | CF_COIN_BY_SPENT_HEIGHT | CF_STATE_SNAPSHOTS => {
                    plan_off()
                }
                CF_COIN_RECORDS
                | CF_COIN_BY_PARENT
                | CF_HINTS
                | CF_MERKLE_NODES
                | CF_ARCHIVE_COIN_RECORDS
                | CF_METADATA => full,
                other => panic!("unexpected CF in ALL_COLUMN_FAMILIES: {other}"),
            };
            assert_eq!(got, want, "STO-004 matrix mismatch for {cf}");
        }
    }

    /// Regression guard: descriptors must still open RocksDB after bloom wiring changes.
    #[test]
    fn vv_req_sto_004_open_succeeds_with_bloom_disabled() {
        let dir = super::helpers::temp_dir();
        let cfg = cfg_bloom_off(dir.path());
        let _db = RocksDbBackend::open(&cfg).expect("open with blooms disabled");
    }
}
