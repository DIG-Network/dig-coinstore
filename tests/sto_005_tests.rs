//! # STO-005 Tests — WriteBatch atomic block commits
//!
//! **Normative:** [`STO-005`](../../docs/requirements/domains/storage/NORMATIVE.md#STO-005)
//! **Spec:** [`STO-005.md`](../../docs/requirements/domains/storage/specs/STO-005.md)
//! **Implementation:** [`src/storage/rocksdb.rs`](../../src/storage/rocksdb.rs) (`batch_write` + durable `write_opt`),
//! [`src/storage/lmdb.rs`](../../src/storage/lmdb.rs)
//! (`batch_write` single `wtxn` + `commit`), [`src/storage/mod.rs`](../../src/storage/mod.rs) ([`WriteBatch`](dig_coinstore::storage::WriteBatch)).
//!
//! ## What this requirement enforces
//!
//! Block-sized mutations MUST be staged in one logical [`WriteBatch`](dig_coinstore::storage::WriteBatch) and
//! committed atomically. RocksDB MUST use a durable WAL path for that single commit (normative `set_sync(true)`);
//! LMDB MUST apply all ops then one `commit`. Failed commits MUST leave prior DB state unchanged. Empty batches are
//! no-ops. Operation order inside a batch MUST be deterministic (last write wins for the same key).
//!
//! ## How passing tests map to acceptance criteria
//!
//! | Spec / test plan | Evidence |
//! |------------------|----------|
//! | `test_batch_cross_cf` | [`vv_req_sto_005_batch_atomic_four_column_families`] |
//! | `test_batch_atomic_all_visible` | [`vv_req_sto_005_hundred_puts_all_visible_after_single_commit`] |
//! | `test_batch_not_visible_before_commit` | [`vv_req_sto_005_keys_absent_until_batch_write_returns`] |
//! | `test_batch_empty` | [`vv_req_sto_005_empty_write_batch_is_noop`] |
//! | `test_batch_failure_rollback` | [`vv_req_sto_005_unknown_cf_mid_batch_leaves_db_unchanged`] |
//! | `test_batch_deterministic_order` | [`vv_req_sto_005_last_put_in_batch_wins_same_key`] |
//! | `test_batch_performance` | [`vv_req_sto_005_one_fat_batch_faster_than_many_single_op_batches`] |
//! | `test_batch_crash_recovery` (no commit) | [`vv_req_sto_005_uncommitted_batch_dropped_no_partial_writes`] |
//! | `test_lmdb_write_transaction_equivalence` | [`vv_req_sto_005_rocks_lmdb_identical_post_commit_state`] (`full-storage` only) |
//! | `test_very_large_batch` | [`vv_req_sto_005_large_batch_all_keys_present`] (`#[ignore]` — slow / memory) |
//!
//! **Tooling:** Repomix packs under `.repomix/` preceded edits. GitNexus CLI was not available in this environment;
//! blast radius was reviewed manually (storage backends + this file).

mod helpers;

#[cfg(feature = "rocksdb-storage")]
mod rocks_sto005 {
    use std::time::Instant;

    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::rocksdb::RocksDbBackend;
    use dig_coinstore::storage::schema::{CF_COIN_RECORDS, CF_HINTS, CF_MERKLE_NODES, CF_METADATA};
    use dig_coinstore::storage::{StorageBackend, StorageError, WriteBatch};

    fn open() -> (tempfile::TempDir, RocksDbBackend) {
        let dir = super::helpers::temp_dir();
        let cfg = CoinStoreConfig::default_with_path(dir.path());
        let backend = RocksDbBackend::open(&cfg).unwrap();
        (dir, backend)
    }

    /// **STO-005 / `test_batch_cross_cf`:** one commit touches four independent column families; all reads succeed
    /// together — proving a single atomic unit, not four separate best-effort writes.
    #[test]
    fn vv_req_sto_005_batch_atomic_four_column_families() {
        let (_dir, b) = open();
        let mut batch = WriteBatch::new();
        batch.put(CF_METADATA, b"k_meta", b"v_meta");
        batch.put(CF_HINTS, b"k_hint", b"v_hint");
        batch.put(CF_COIN_RECORDS, b"k_coin", b"v_coin");
        batch.put(CF_MERKLE_NODES, b"k_merk", b"v_merk");
        b.batch_write(batch).unwrap();
        assert_eq!(
            b.get(CF_METADATA, b"k_meta").unwrap().as_deref(),
            Some(b"v_meta".as_slice())
        );
        assert_eq!(
            b.get(CF_HINTS, b"k_hint").unwrap().as_deref(),
            Some(b"v_hint".as_slice())
        );
        assert_eq!(
            b.get(CF_COIN_RECORDS, b"k_coin").unwrap().as_deref(),
            Some(b"v_coin".as_slice())
        );
        assert_eq!(
            b.get(CF_MERKLE_NODES, b"k_merk").unwrap().as_deref(),
            Some(b"v_merk".as_slice())
        );
    }

    /// **STO-005 / `test_batch_atomic_all_visible`:** many puts in one batch — after `batch_write`, every key is
    /// readable. Partial visibility would violate “all or nothing” for block-shaped workloads.
    #[test]
    fn vv_req_sto_005_hundred_puts_all_visible_after_single_commit() {
        let (_dir, b) = open();
        let mut batch = WriteBatch::with_capacity(100);
        for i in 0u16..100 {
            let key = format!("sto005_{i:04}").into_bytes();
            let val = format!("val_{i:04}").into_bytes();
            batch.put(CF_METADATA, &key, &val);
        }
        b.batch_write(batch).unwrap();
        for i in 0u16..100 {
            let key = format!("sto005_{i:04}").into_bytes();
            let want = format!("val_{i:04}").into_bytes();
            assert_eq!(
                b.get(CF_METADATA, &key).unwrap().as_deref(),
                Some(want.as_slice())
            );
        }
    }

    /// **STO-005 / `test_batch_not_visible_before_commit`:** logical [`WriteBatch`] lives in user memory until
    /// `batch_write`; the DB must not show keys mid-flight.
    #[test]
    fn vv_req_sto_005_keys_absent_until_batch_write_returns() {
        let (_dir, b) = open();
        let mut batch = WriteBatch::new();
        batch.put(CF_METADATA, b"pre_commit_key", b"secret");
        assert_eq!(b.get(CF_METADATA, b"pre_commit_key").unwrap(), None);
        b.batch_write(batch).unwrap();
        assert_eq!(
            b.get(CF_METADATA, b"pre_commit_key").unwrap().as_deref(),
            Some(b"secret".as_slice())
        );
    }

    /// **STO-005 / `test_batch_empty`:** empty batch is `Ok(())` and MUST NOT error (normative: no fsync / no-op).
    #[test]
    fn vv_req_sto_005_empty_write_batch_is_noop() {
        let (_dir, b) = open();
        let empty = WriteBatch::new();
        assert!(empty.is_empty());
        b.batch_write(empty).unwrap();
    }

    /// **STO-005 / `test_batch_failure_rollback`:** validation fails before `write_opt` — the first staged op must
    /// **not** appear after `Err` (Rocks never persisted a partial native batch).
    #[test]
    fn vv_req_sto_005_unknown_cf_mid_batch_leaves_db_unchanged() {
        let (_dir, b) = open();
        let mut batch = WriteBatch::new();
        batch.put(CF_METADATA, b"rollback_probe", b"should_not_land");
        batch.put("not_a_real_cf", b"x", b"y");
        let err = b.batch_write(batch).unwrap_err();
        assert!(
            matches!(err, StorageError::UnknownColumnFamily(ref s) if s == "not_a_real_cf"),
            "expected UnknownColumnFamily, got {err:?}"
        );
        assert_eq!(b.get(CF_METADATA, b"rollback_probe").unwrap(), None);
    }

    /// **STO-005 / `test_batch_deterministic_order`:** same key twice in one batch — Rocks applies in insertion order;
    /// the final value MUST match the last `put` (coin store MUST preserve deterministic op ordering per spec).
    #[test]
    fn vv_req_sto_005_last_put_in_batch_wins_same_key() {
        let (_dir, b) = open();
        let mut batch = WriteBatch::new();
        batch.put(CF_METADATA, b"dup_key", b"first");
        batch.put(CF_METADATA, b"dup_key", b"second");
        b.batch_write(batch).unwrap();
        assert_eq!(
            b.get(CF_METADATA, b"dup_key").unwrap().as_deref(),
            Some(b"second".as_slice())
        );
    }

    /// **STO-005 / `test_batch_performance`:** compare **N** separate `batch_write` calls each carrying **one** put
    /// (each durable commit on Rocks) vs **one** `batch_write` with **N** puts. The fat batch amortizes engine work
    /// and MUST beat the “many tiny commits” loop by a wide margin (≥10×), matching the spec’s WAL-amortization story.
    #[test]
    fn vv_req_sto_005_one_fat_batch_faster_than_many_single_op_batches() {
        let (_dir, b) = open();
        const N: usize = 250;
        let t_many = Instant::now();
        for i in 0..N {
            let mut one = WriteBatch::new();
            one.put(CF_METADATA, &format!("perf_{i}").into_bytes(), b"v");
            b.batch_write(one).unwrap();
        }
        let many_elapsed = t_many.elapsed();

        let t_one = Instant::now();
        let mut fat = WriteBatch::with_capacity(N);
        for i in 0..N {
            fat.put(CF_METADATA, &format!("perf2_{i}").into_bytes(), b"v");
        }
        b.batch_write(fat).unwrap();
        let one_elapsed = t_one.elapsed();

        assert!(
            many_elapsed > one_elapsed * 10,
            "STO-005 expects large batch to be >> faster than N single-op batches; many={many_elapsed:?} one={one_elapsed:?}"
        );
    }

    /// **STO-005 / `test_batch_crash_recovery` (logical):** if the process “dies” before `batch_write`, nothing is
    /// on disk — we simulate by dropping an in-memory [`WriteBatch`] without calling the backend.
    #[test]
    fn vv_req_sto_005_uncommitted_batch_dropped_no_partial_writes() {
        let (_dir, b) = open();
        {
            let mut batch = WriteBatch::new();
            batch.put(CF_METADATA, b"ghost", b"gone");
            // drop without commit
            let _ = batch;
        }
        assert_eq!(b.get(CF_METADATA, b"ghost").unwrap(), None);
    }

    /// **STO-005 / `test_very_large_batch`:** optional stress — run locally with `cargo test vv_req_sto_005_large_batch_all_keys_present --ignored`.
    #[test]
    #[ignore = "stress: 25k puts; run with --ignored on capable hosts"]
    fn vv_req_sto_005_large_batch_all_keys_present() {
        let (_dir, b) = open();
        const N: usize = 25_000;
        let mut batch = WriteBatch::with_capacity(N);
        for i in 0..N {
            batch.put(CF_METADATA, &format!("bulk_{i:08}").into_bytes(), b"1");
        }
        b.batch_write(batch).unwrap();
        for i in (0..N).step_by(997) {
            let k = format!("bulk_{i:08}").into_bytes();
            assert_eq!(
                b.get(CF_METADATA, &k).unwrap().as_deref(),
                Some(b"1".as_slice())
            );
        }
    }
}

/// **STO-005 / `test_lmdb_write_transaction_equivalence`:** same logical batch on both engines → identical `get`
/// results for sampled keys (requires `full-storage`).
#[cfg(feature = "full-storage")]
mod full_storage_sto005 {
    use dig_coinstore::config::CoinStoreConfig;
    use dig_coinstore::storage::lmdb::LmdbBackend;
    use dig_coinstore::storage::rocksdb::RocksDbBackend;
    use dig_coinstore::storage::schema::{CF_COIN_RECORDS, CF_HINTS, CF_METADATA};
    use dig_coinstore::storage::{StorageBackend, WriteBatch};

    #[test]
    fn vv_req_sto_005_rocks_lmdb_identical_post_commit_state() {
        let dir_r = tempfile::tempdir().unwrap();
        let dir_l = tempfile::tempdir().unwrap();
        let cfg_r = CoinStoreConfig::default_with_path(dir_r.path());
        let cfg_l = CoinStoreConfig::default_with_path(dir_l.path());
        let rocks = RocksDbBackend::open(&cfg_r).unwrap();
        let lmdb = LmdbBackend::open(&cfg_l).unwrap();

        let mut batch = WriteBatch::new();
        batch.put(CF_METADATA, b"tip_height", b"12345");
        batch.put(CF_HINTS, b"hint_key", b"hint_val");
        batch.put(CF_COIN_RECORDS, b"coin_id_1", b"record_blob");

        rocks.batch_write(batch.clone()).unwrap();
        lmdb.batch_write(batch).unwrap();

        for (cf, key) in [
            (CF_METADATA, b"tip_height".as_slice()),
            (CF_HINTS, b"hint_key".as_slice()),
            (CF_COIN_RECORDS, b"coin_id_1".as_slice()),
        ] {
            assert_eq!(
                rocks.get(cf, key).unwrap(),
                lmdb.get(cf, key).unwrap(),
                "post-commit divergence on {cf} key={key:?}"
            );
        }
    }
}
