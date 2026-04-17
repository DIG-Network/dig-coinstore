//! # MRK-003 Tests — Persistent internal nodes (`merkle_nodes`) + metadata state root
//!
//! **Normative:** [`MRK-003`](../../docs/requirements/domains/merkle/NORMATIVE.md#MRK-003)
//! **Spec:** [`MRK-003.md`](../../docs/requirements/domains/merkle/specs/MRK-003.md) (column family, dirty set,
//! [`WriteBatch`](dig_coinstore::storage::WriteBatch) flush, single-read startup contract)
//! **Schema / CF:** [`dig_coinstore::storage::schema`](../../src/storage/schema.rs) — [`CF_MERKLE_NODES`],
//! [`metadata_key`](dig_coinstore::storage::schema::metadata_key) + [`MERKLE_STATE_ROOT_META_KEY`](dig_coinstore::merkle::MERKLE_STATE_ROOT_META_KEY)
//!
//! ## What MRK-003 mandates (and how we prove it)
//!
//! | MRK-003 rule | Evidence in this file |
//! |--------------|------------------------|
//! | `merkle_nodes` rows are 32-byte internal digests | [`vv_req_mrk_003_flush_persists_merkle_rows`] reads back one flushed key |
//! | Dirty diff is produced for persistence | [`vv_req_mrk_003_dirty_map_populated_after_root`], [`vv_req_mrk_003_flush_clears_dirty`] |
//! | Deferred MRK-001: diff materializes at `root()` / flush boundary | Doc comments + [`vv_req_mrk_003_dirty_map_populated_after_root`] (empty before `root()` when cache cleared) |
//! | `flush_to_batch` + same `batch_write` as other CFs (atomic surface) | [`vv_req_mrk_003_atomic_batch_with_coin_records`] |
//! | `load_from_store` = one metadata read + leaf recompute | [`vv_req_mrk_003_load_roundtrip`], [`vv_req_mrk_003_load_rejects_tampered_metadata`], [`vv_req_mrk_003_load_missing_metadata_fails`] |
//! | Empty-subtree pruning uses `Delete` ops | [`vv_req_mrk_003_removal_emits_delete_ops`] |
//! | In-memory MRK-004 proof still matches loaded root | [`vv_req_mrk_003_proof_verifies_after_load`] |
//!
//! **Lazy `merkle_nodes` reads during proof walks** remain MRK-004 scope; this file proves the persistence
//! wire shape and reload validation that MRK-004 will consume later.
//!
//! **GitNexus / Repomix / SocratiCode:** follow `docs/prompt/start.md` before changing `src/merkle` or `src/storage`.

use std::collections::HashMap;

use chia_protocol::Bytes32;
use dig_coinstore::config::CoinStoreConfig;
use dig_coinstore::merkle::{
    empty_hash, merkle_leaf_hash, MerkleError, MerkleNodePersistOp, SparseMerkleTree,
    MERKLE_STATE_ROOT_META_KEY, SMT_HEIGHT,
};
use dig_coinstore::storage::schema;
use dig_coinstore::storage::{StorageBackend, WriteBatch};

// ─────────────────────────────────────────────────────────────────────────────
// RocksDB (default `rocksdb-storage`)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "rocksdb-storage")]
mod rocks_mrk003 {
    use super::*;

    use dig_coinstore::storage::rocksdb::RocksDbBackend;

    fn open_backend() -> (tempfile::TempDir, RocksDbBackend) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CoinStoreConfig::default_with_path(dir.path());
        let backend = RocksDbBackend::open(&cfg).unwrap();
        (dir, backend)
    }

    /// **MRK-003 / NORMATIVE “dirty during batch updates” (operational meaning):** Mutations mark the
    /// MRK-001 root cache dirty; the *persistable* `(level, path)` diff is the output of the same
    /// traversal as [`SparseMerkleTree::root`], so we intentionally materialize it there (one walk
    /// for digest + MRK-003 rows). This test proves the dirty map stays empty while only the leaf
    /// map changed, then becomes non-empty immediately after the first post-mutation [`root`].
    #[test]
    fn vv_req_mrk_003_dirty_map_populated_after_root() {
        let mut tree = SparseMerkleTree::new();
        let key = Bytes32::from([0xABu8; 32]);
        let leaf = merkle_leaf_hash(b"mrk003_dirty");
        tree.batch_insert(&[(key, leaf)]).unwrap();
        assert!(
            tree.dirty_nodes().is_empty(),
            "MRK-001 defers internal recomputation until root(); dirty persistence map mirrors that boundary"
        );
        let _ = tree.root();
        assert!(
            !tree.dirty_nodes().is_empty(),
            "MRK-003 requires a non-empty dirty set after root() so flush_to_batch can enqueue merkle_nodes ops"
        );
    }

    /// **MRK-003 §flush:** [`SparseMerkleTree::flush_to_batch`] drains [`SparseMerkleTree::dirty_nodes`]
    /// into [`WriteBatch`] puts/deletes and writes the 32-byte state root under metadata
    /// [`MERKLE_STATE_ROOT_META_KEY`].
    #[test]
    fn vv_req_mrk_003_flush_persists_merkle_rows() {
        let (_dir, backend) = open_backend();
        let mut tree = SparseMerkleTree::new();
        let k1 = Bytes32::from([0x11u8; 32]);
        let k2 = Bytes32::from([0x22u8; 32]);
        tree.batch_insert(&[(k1, merkle_leaf_hash(b"a")), (k2, merkle_leaf_hash(b"b"))])
            .unwrap();
        let _ = tree.root();
        // `dirty_nodes` mixes Put and Delete ops; iteration order is hash-randomized, so picking
        // `keys().next()` can land on a Delete row (nothing to `get` after flush). Require a Put.
        let sample_key = tree
            .dirty_nodes()
            .iter()
            .find_map(|(k, op)| match op {
                MerkleNodePersistOp::Put(_) => Some(*k),
                MerkleNodePersistOp::Delete => None,
            })
            .expect("internal walk should enqueue ≥1 merkle_nodes Put for two leaves");

        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        backend.batch_write(batch).unwrap();

        let persisted = backend
            .get(schema::CF_MERKLE_NODES, &sample_key)
            .unwrap()
            .expect("flush should have written the sampled merkle_nodes key");
        assert_eq!(
            persisted.len(),
            32,
            "MRK-003 value encoding is a single 32-byte digest"
        );

        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        let meta = backend
            .get(schema::CF_METADATA, &meta_key)
            .unwrap()
            .expect("metadata root");
        assert_eq!(meta.len(), 32);
    }

    /// **MRK-003 behavior §3:** After a successful flush enqueue, the dirty map must be empty so a
    /// second flush is a no-op until the next [`root`]-eligible recompute.
    #[test]
    fn vv_req_mrk_003_flush_clears_dirty() {
        let (_dir, backend) = open_backend();
        let mut tree = SparseMerkleTree::new();
        tree.batch_insert(&[(Bytes32::from([7u8; 32]), merkle_leaf_hash(b"x"))])
            .unwrap();
        let _ = tree.root();
        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        assert!(
            tree.dirty_nodes().is_empty(),
            "flush_to_batch must clear dirty after mem::take per MRK-003 §3"
        );
        backend.batch_write(batch).unwrap();
    }

    /// **MRK-003 §4 + BLK-014 precursor:** Merkle persistence ops share one [`WriteBatch`] surface with
    /// unrelated coin-record keys so production can commit atomically.
    #[test]
    fn vv_req_mrk_003_atomic_batch_with_coin_records() {
        let (_dir, backend) = open_backend();
        let mut tree = SparseMerkleTree::new();
        tree.batch_insert(&[(Bytes32::from([3u8; 32]), merkle_leaf_hash(b"atomic"))])
            .unwrap();
        let _ = tree.root();

        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        batch.put(
            schema::CF_COIN_RECORDS,
            b"mrk003_atomic_smoke_key",
            b"mrk003_atomic_smoke_val",
        );
        backend.batch_write(batch).unwrap();

        assert_eq!(
            backend
                .get(schema::CF_COIN_RECORDS, b"mrk003_atomic_smoke_key")
                .unwrap()
                .as_deref(),
            Some(b"mrk003_atomic_smoke_val".as_slice())
        );
        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        assert_eq!(
            backend
                .get(schema::CF_METADATA, &meta_key)
                .unwrap()
                .map(|v| v.len()),
            Some(32)
        );
    }

    /// **MRK-003 §5:** [`SparseMerkleTree::load_from_store`] reads exactly the metadata root row and
    /// validates it against an out-of-band leaf map (simulating coin-record replay) before caching.
    #[test]
    fn vv_req_mrk_003_load_roundtrip() {
        let (_dir, backend) = open_backend();
        let mut leaves = HashMap::new();
        let key = Bytes32::from([0xC3u8; 32]);
        let lh = merkle_leaf_hash(b"roundtrip");
        leaves.insert(key, lh);

        let mut tree = SparseMerkleTree::new();
        tree.batch_insert(&[(key, lh)]).unwrap();
        let expect = tree.root();

        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        backend.batch_write(batch).unwrap();

        let mut loaded = SparseMerkleTree::load_from_store(&backend, leaves.clone()).unwrap();
        assert_eq!(loaded.root(), expect);
        assert_eq!(loaded.root_observed(), expect);
    }

    /// **MRK-003 corruption path:** Tampering the metadata root must surface [`MerkleError::PersistedRootMismatch`]
    /// because [`load_from_store`] recomputes from leaves before trusting disk.
    #[test]
    fn vv_req_mrk_003_load_rejects_tampered_metadata() {
        let (_dir, backend) = open_backend();
        let mut leaves = HashMap::new();
        let key = Bytes32::from([0x5Au8; 32]);
        let lh = merkle_leaf_hash(b"integrity");
        leaves.insert(key, lh);

        let mut tree = SparseMerkleTree::new();
        tree.batch_insert(&[(key, lh)]).unwrap();
        let _ = tree.root();
        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        backend.batch_write(batch).unwrap();

        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        backend
            .put(schema::CF_METADATA, &meta_key, &[0xEEu8; 32])
            .unwrap();

        let err = SparseMerkleTree::load_from_store(&backend, leaves).unwrap_err();
        assert!(
            matches!(err, MerkleError::PersistedRootMismatch { .. }),
            "expected PersistedRootMismatch, got {err:?}"
        );
    }

    /// **MRK-003 §5:** Missing metadata row cannot fabricate a root — [`MerkleError::PersistedRootMissing`].
    #[test]
    fn vv_req_mrk_003_load_missing_metadata_fails() {
        let (_dir, backend) = open_backend();
        let err = SparseMerkleTree::load_from_store(&backend, HashMap::new()).unwrap_err();
        assert_eq!(err, MerkleError::PersistedRootMissing);
    }

    /// **MRK-003 §2 (empty-subtree pruning):** Removing the last leaf should enqueue `Delete` ops for
    /// nodes that collapse back to canonical [`empty_hash`] levels (space optimization vs leaving
    /// stale rows on disk).
    #[test]
    fn vv_req_mrk_003_removal_emits_delete_ops() {
        let mut tree = SparseMerkleTree::new();
        let key = Bytes32::from([0x01u8; 32]);
        tree.batch_insert(&[(key, merkle_leaf_hash(b"lonely"))])
            .unwrap();
        let _ = tree.root();
        tree.batch_remove(&[key]).unwrap();
        let _ = tree.root();
        let deletes = tree
            .dirty_nodes()
            .values()
            .filter(|op| matches!(op, MerkleNodePersistOp::Delete))
            .count();
        assert!(
            deletes > 0,
            "collapsing to the global empty tree should propose deleting previously materialized internal rows"
        );
    }

    /// **MRK-003 cross MRK-004:** After [`load_from_store`], [`SparseMerkleTree::get_coin_proof`] still
    /// builds siblings from the resident leaf multiset; verification against the loaded metadata root
    /// proves end-to-end consistency (optional lazy `merkle_nodes` reads without full leaves remain future work).
    #[test]
    fn vv_req_mrk_003_proof_verifies_after_load() {
        let (_dir, backend) = open_backend();
        let mut leaves = HashMap::new();
        let key = Bytes32::from([0x77u8; 32]);
        let lh = merkle_leaf_hash(b"proof_after_load");
        leaves.insert(key, lh);

        let mut tree = SparseMerkleTree::new();
        tree.batch_insert(&[(key, lh)]).unwrap();
        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        backend.batch_write(batch).unwrap();

        let mut loaded = SparseMerkleTree::load_from_store(&backend, leaves).unwrap();
        let root = loaded.root();
        let proof = loaded.get_coin_proof(&key).unwrap();
        assert!(
            proof.verify(&root),
            "MRK-005 verifier must accept MRK-004 proof for loaded tree"
        );
    }

    /// **MRK-003 edge:** Metadata holds the canonical empty-tree root while the leaf map is empty —
    /// [`load_from_store`] must accept the pair (startup after “no coins” commit).
    #[test]
    fn vv_req_mrk_003_load_empty_leaves_with_persisted_empty_root() {
        let (_dir, backend) = open_backend();
        let empty_root = empty_hash(SMT_HEIGHT);
        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        backend
            .put(schema::CF_METADATA, &meta_key, empty_root.as_ref())
            .unwrap();
        let mut tree = SparseMerkleTree::load_from_store(&backend, HashMap::new()).unwrap();
        assert_eq!(tree.root(), empty_root);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LMDB (`lmdb-storage`) — same persistence contract via [`StorageBackend`]
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "lmdb-storage")]
mod lmdb_mrk003 {
    use super::*;

    use dig_coinstore::config::StorageBackend as BackendKind;
    use dig_coinstore::storage::lmdb::LmdbBackend;

    fn open_backend() -> (tempfile::TempDir, LmdbBackend) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CoinStoreConfig::default_with_path(dir.path()).with_backend(BackendKind::Lmdb);
        let backend = LmdbBackend::open(&cfg).unwrap();
        (dir, backend)
    }

    #[test]
    fn vv_req_mrk_003_lmdb_flush_and_load_metadata() {
        let (_dir, backend) = open_backend();
        let mut tree = SparseMerkleTree::new();
        let key = Bytes32::from([0x99u8; 32]);
        let lh = merkle_leaf_hash(b"lmdb_mrk003");
        let leaves = HashMap::from([(key, lh)]);
        tree.batch_insert(&[(key, lh)]).unwrap();
        let expect = tree.root();
        let mut batch = WriteBatch::new();
        tree.flush_to_batch(&mut batch).unwrap();
        backend.batch_write(batch).unwrap();

        let mut loaded = SparseMerkleTree::load_from_store(&backend, leaves).unwrap();
        assert_eq!(loaded.root(), expect);

        let meta_key = schema::metadata_key(MERKLE_STATE_ROOT_META_KEY);
        let meta = backend
            .get(schema::CF_METADATA, &meta_key)
            .unwrap()
            .expect("metadata root");
        assert_eq!(meta.len(), 32);
    }
}
