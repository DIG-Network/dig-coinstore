//! # STO-006 Tests — RocksDB compaction strategy per column family
//!
//! **Normative:** [`STO-006`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-006)
//! **Spec:** [`STO-006.md`](../../docs/requirements/domains/storage/specs/STO-006.md)
//! **Implementation:** [`src/storage/rocksdb.rs`](../../src/storage/rocksdb.rs) (FIFO vs Level, tuning helpers,
//! `max_write_buffer_number`), [`src/storage/schema.rs`](../../src/storage/schema.rs) (`STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER`),
//! [`src/storage/lmdb.rs`](../../src/storage/lmdb.rs) (`compact` no-op).
//!
//! ## What this requirement enforces
//!
//! Read-heavy logical stores use **Leveled** compaction with the normative byte / L0 / level-count table so
//! point, prefix, and range scans keep bounded read amplification. Append-mostly stores (`archive_coin_records`,
//! `state_snapshots`) use **FIFO** with a 1 GiB on-disk cap so old SSTs age out without rewrite amplification.
//! Every CF must pair **STO-002** `write_buffer_size` with **STO-006** `max_write_buffer_number`. Manual
//! [`StorageBackend::compact`] must invoke RocksDB range compaction for known CF names and stay a no-op on LMDB.
//!
//! ## How passing tests map to acceptance criteria
//!
//! | STO-006 acceptance row | Evidence in this file |
//! |------------------------|------------------------|
//! | Read-optimized CFs → Level | [`vv_req_sto_006_compaction_matrix_matches_spec_table`] loops [`ALL_COLUMN_FAMILIES`] |
//! | Append CFs → FIFO | same — expects FIFO only for `archive_coin_records` + `state_snapshots` |
//! | Level parameter table | [`vv_req_sto_006_level_params_match_normative_table`] vs [`Sto006LevelCompactionParams::NORMATIVE`] |
//! | FIFO `max_table_files_size` | [`vv_req_sto_006_fifo_cap_matches_spec`] vs [`STO006_FIFO_MAX_TABLE_FILES_SIZE`] |
//! | Per-CF `max_write_buffer_number` | [`vv_req_sto_006_max_write_buffer_numbers_match_schema_table`] |
//! | Manual `compact` | [`vv_req_sto_006_manual_compact_known_cf_ok`] + [`vv_req_sto_006_manual_compact_unknown_cf_errors`] |
//! | LMDB `compact` no-op | [`vv_req_sto_006_lmdb_compact_is_noop`] (`lmdb-storage` feature) |
//! | Schema alignment | [`vv_req_sto_006_schema_arrays_align_with_all_column_families`] |
//!
//! **Tooling (`docs/prompt/start.md`):** Repomix packs under `.repomix/` should precede large edits; GitNexus MCP/CLI
//! was not relied on in this agent shell (run `npx gitnexus status` locally before high-risk refactors).

mod helpers;

/// **STO-006 / per-CF buffers:** prove `STO002_*` and `STO006_*` slices stay index-aligned with [`ALL_COLUMN_FAMILIES`].
///
/// This is a cheap guard that never touches native backends — if someone reorders `ALL_COLUMN_FAMILIES` without
/// updating the parallel arrays, CI fails before any Rocks open path mis-assigns memtable depth.
mod sto006_schema_alignment {
    use dig_coinstore::storage::schema::{
        ALL_COLUMN_FAMILIES, STO002_ROCKS_WRITE_BUFFER_BYTES, STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER,
    };

    #[test]
    fn vv_req_sto_006_schema_arrays_align_with_all_column_families() {
        assert_eq!(
            ALL_COLUMN_FAMILIES.len(),
            STO002_ROCKS_WRITE_BUFFER_BYTES.len(),
            "STO-002 write-buffer row count must match CF count"
        );
        assert_eq!(
            ALL_COLUMN_FAMILIES.len(),
            STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER.len(),
            "STO-006 max_write_buffer_number row count must match CF count"
        );
        for (i, nbuf) in STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER.iter().enumerate() {
            assert!(
                *nbuf >= 2 && *nbuf <= 3,
                "STO-006 table only uses 2 or 3 buffers; cf index {i} has {nbuf}"
            );
            assert!(
                STO002_ROCKS_WRITE_BUFFER_BYTES[i] > 0,
                "write_buffer_size must be positive (index {i})"
            );
        }
    }
}

#[cfg(feature = "rocksdb-storage")]
mod rocks_sto006 {
    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::storage::rocksdb::{
        sto006_compaction_style_for_cf, sto006_max_write_buffer_number_for_cf, RocksDbBackend,
        Sto006CompactionStyle, Sto006LevelCompactionParams, STO006_FIFO_MAX_TABLE_FILES_SIZE,
    };
    use dig_coinstore::storage::schema::{
        ALL_COLUMN_FAMILIES, CF_ARCHIVE_COIN_RECORDS, CF_COIN_RECORDS, CF_MERKLE_NODES,
        CF_METADATA, CF_STATE_SNAPSHOTS, STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER,
    };
    use dig_coinstore::storage::{StorageBackend, StorageError};

    /// **Test plan `test_level_compaction_config` / matrix:** every CF name maps to the same style the spec table
    /// lists (FIFO only for the two append-mostly stores).
    #[test]
    fn vv_req_sto_006_compaction_matrix_matches_spec_table() {
        for &cf in ALL_COLUMN_FAMILIES {
            let want = match cf {
                CF_ARCHIVE_COIN_RECORDS | CF_STATE_SNAPSHOTS => Sto006CompactionStyle::Fifo,
                _ => Sto006CompactionStyle::Level,
            };
            assert_eq!(
                sto006_compaction_style_for_cf(cf),
                want,
                "Compaction strategy drift for {cf}"
            );
        }
    }

    /// **STO-006** § Level Compaction Configuration — constants are the contract `column_family_descriptor` applies
    /// via `sto006_apply_level_compaction_options` in `src/storage/rocksdb.rs`.
    #[test]
    fn vv_req_sto_006_level_params_match_normative_table() {
        let p = Sto006LevelCompactionParams::NORMATIVE;
        assert_eq!(p.max_bytes_for_level_base, 256 * 1024 * 1024);
        assert_eq!(p.max_bytes_for_level_multiplier, 10.0);
        assert_eq!(p.level0_file_num_compaction_trigger, 4);
        assert_eq!(p.level0_slowdown_writes_trigger, 20);
        assert_eq!(p.level0_stop_writes_trigger, 36);
        assert_eq!(p.target_file_size_base, 64 * 1024 * 1024);
        assert_eq!(p.num_levels, 7);
    }

    /// **STO-006** FIFO table — 1 GiB cap matches the value wired into [`sto006_apply_fifo_compaction_options`].
    #[test]
    fn vv_req_sto_006_fifo_cap_matches_spec() {
        assert_eq!(STO006_FIFO_MAX_TABLE_FILES_SIZE, 1024 * 1024 * 1024);
    }

    /// **STO-006** § Per-CF Write Buffer Allocation — `max_write_buffer_number` column vs [`sto006_max_write_buffer_number_for_cf`].
    #[test]
    fn vv_req_sto_006_max_write_buffer_numbers_match_schema_table() {
        for (i, &cf) in ALL_COLUMN_FAMILIES.iter().enumerate() {
            assert_eq!(
                sto006_max_write_buffer_number_for_cf(cf),
                STO006_ROCKS_MAX_WRITE_BUFFER_NUMBER[i],
                "index {i} cf={cf}"
            );
        }
        assert_eq!(sto006_max_write_buffer_number_for_cf(CF_COIN_RECORDS), 3);
        assert_eq!(sto006_max_write_buffer_number_for_cf(CF_MERKLE_NODES), 3);
        assert_eq!(sto006_max_write_buffer_number_for_cf(CF_METADATA), 2);
    }

    /// **STO-006** § Manual Compaction — happy path calls [`StorageBackend::compact`] on a real CF after a `put`.
    #[test]
    fn vv_req_sto_006_manual_compact_known_cf_ok() {
        let dir = super::helpers::temp_dir();
        let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(Engine::RocksDb);
        let db = RocksDbBackend::open(&cfg).unwrap();
        StorageBackend::put(&db, CF_COIN_RECORDS, b"k-sto006", b"v").unwrap();
        assert!(StorageBackend::compact(&db, CF_COIN_RECORDS).is_ok());
    }

    /// **STO-006 / STO-001** — unknown logical CF must surface [`StorageError::UnknownColumnFamily`] without panicking.
    #[test]
    fn vv_req_sto_006_manual_compact_unknown_cf_errors() {
        let dir = super::helpers::temp_dir();
        let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(Engine::RocksDb);
        let db = RocksDbBackend::open(&cfg).unwrap();
        let r = StorageBackend::compact(&db, "not_a_real_column_family");
        assert!(matches!(r, Err(StorageError::UnknownColumnFamily(_))));
    }
}

#[cfg(feature = "lmdb-storage")]
mod lmdb_sto006 {
    use dig_coinstore::config::{CoinStoreConfig, StorageBackend as Engine};
    use dig_coinstore::storage::lmdb::LmdbBackend;
    use dig_coinstore::storage::schema::CF_COIN_RECORDS;
    use dig_coinstore::storage::StorageBackend;

    /// **STO-006** acceptance: LMDB has no Rocks-style compaction — [`LmdbBackend::compact`] returns `Ok` and does
    /// not require a physical operation (see `src/storage/lmdb.rs` implementation comments).
    #[test]
    fn vv_req_sto_006_lmdb_compact_is_noop() {
        let dir = super::helpers::temp_dir();
        let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(Engine::Lmdb);
        let db = LmdbBackend::open(&cfg).unwrap();
        assert!(StorageBackend::compact(&db, CF_COIN_RECORDS).is_ok());
    }
}
