//! LMDB storage backend (`heed`) — **six named databases** (STO-003).
//!
//! The [`StorageBackend`] trait is keyed by **logical** column-family names from
//! [`super::schema`] (the same twelve `CF_*` strings RocksDB uses per STO-002). The LMDB layout
//! multiplexes those logical stores onto **six** physical LMDB databases named exactly as in
//! [STO-003.md](../../docs/requirements/domains/storage/specs/STO-003.md): `coins`, `coins_by_ph`,
//! `hints`, `hints_by_value`, `snapshots`, `metadata`.
//!
//! ## Multiplexing and key prefixes
//!
//! Several logical CFs share one LMDB database (per the STO-003 mapping table). To avoid key
//! collisions (e.g. `coin_records` and `archive_coin_records` both use 32-byte `coin_id` keys), we
//! prepend a **single-byte logical tag** to the user key inside the shared DB only. Callers still
//! pass unmodified keys to [`StorageBackend`]; [`prefix_scan`](StorageBackend::prefix_scan) strips
//! the tag on the way out so iterator results match the RocksDB contract (logical keys only).
//!
//! ## Environment flags (STO-003)
//!
//! - [`heed::EnvFlags::NO_TLS`] — read transactions are not tied to OS threads; matches STO-003
//!   “NO_TLS enabled” and allows passing read txns across threads when operators need MVCC proofs.
//! - [`heed::EnvFlags::NO_READ_AHEAD`] — reduces OS readahead for random access (no-op on Windows per LMDB).
//!
//! ## `max_dbs` and `map_size`
//!
//! `max_dbs` is set to **8** (≥ 6 named databases plus LMDB bookkeeping headroom). `map_size` comes
//! from [`CoinStoreConfig::lmdb_map_size`] (API-003 / SPEC defaults).
//!
//! # Requirement: STO-003, STO-001, STR-003
//! # Spec: docs/requirements/domains/storage/specs/STO-003.md
//! # Normative: docs/requirements/domains/storage/NORMATIVE.md#STO-003

use std::borrow::Cow;
use std::fs;

use heed::types::Bytes;
use heed::{Database, Env, EnvFlags, EnvOpenOptions, Error as HeedError, MdbError, RoTxn};

use crate::config::CoinStoreConfig;

use super::schema::{
    CF_ARCHIVE_COIN_RECORDS, CF_COIN_BY_CONFIRMED_HEIGHT, CF_COIN_BY_PARENT,
    CF_COIN_BY_PUZZLE_HASH, CF_COIN_BY_SPENT_HEIGHT, CF_COIN_RECORDS, CF_HINTS, CF_HINTS_BY_VALUE,
    CF_MERKLE_NODES, CF_METADATA, CF_STATE_SNAPSHOTS, CF_UNSPENT_BY_PUZZLE_HASH,
};
use super::{StorageBackend, StorageError, WriteBatch, WriteOp};

// ─────────────────────────────────────────────────────────────────────────────
// STO-003 physical database names (normative order for `open` + public constants)
// ─────────────────────────────────────────────────────────────────────────────

/// LMDB database name for the multiplexed “coin-shaped” logical stores.
pub const LMDB_DB_COINS: &str = "coins";
/// LMDB database for puzzle-hash style indices (two logical CFs).
pub const LMDB_DB_COINS_BY_PH: &str = "coins_by_ph";
/// Forward hint index (one logical CF).
pub const LMDB_DB_HINTS: &str = "hints";
/// Reverse hint index (one logical CF).
pub const LMDB_DB_HINTS_BY_VALUE: &str = "hints_by_value";
/// Snapshot / checkpoint payloads (logical `state_snapshots`).
pub const LMDB_DB_SNAPSHOTS: &str = "snapshots";
/// Chain metadata string keys.
pub const LMDB_DB_METADATA: &str = "metadata";

/// The six STO-003 named databases, in [`LmdbBackend::open`] creation order (indices 0..5).
///
/// # Requirement: STO-003
pub const LMDB_NAMED_DATABASES: &[&str] = &[
    LMDB_DB_COINS,
    LMDB_DB_COINS_BY_PH,
    LMDB_DB_HINTS,
    LMDB_DB_HINTS_BY_VALUE,
    LMDB_DB_SNAPSHOTS,
    LMDB_DB_METADATA,
];

const LMDB_MAX_DBS: u32 = 8;

// Tags inside `coins` LMDB database (arbitrary stable bytes; documented for debugging).
const TAG_COIN_RECORDS: u8 = 0x01;
const TAG_COIN_BY_PARENT: u8 = 0x02;
const TAG_COIN_BY_CONFIRMED_HEIGHT: u8 = 0x03;
const TAG_COIN_BY_SPENT_HEIGHT: u8 = 0x04;
const TAG_ARCHIVE_COIN_RECORDS: u8 = 0x05;
const TAG_MERKLE_NODES: u8 = 0x06;

// Tags inside `coins_by_ph`.
const TAG_COIN_BY_PUZZLE_HASH: u8 = 0x01;
const TAG_UNSPENT_BY_PUZZLE_HASH: u8 = 0x02;

/// LMDB-backed key-value store: one [`heed::Env`], six typed [`Database`] handles.
///
/// Implements [`StorageBackend`] so [`crate::coin_store::CoinStore`] can select LMDB via
/// [`CoinStoreConfig`](crate::config::CoinStoreConfig) the same way as RocksDB.
pub struct LmdbBackend {
    env: Env,
    /// Index `i` is the handle for [`LMDB_NAMED_DATABASES`]`[i]`.
    dbs: Vec<Database<Bytes, Bytes>>,
}

/// Map `heed` / LMDB failures into [`StorageError`], surfacing [`StorageError::MapFull`] per STO-003.
fn map_heed(ctx: impl Into<String>, err: HeedError) -> StorageError {
    match err {
        HeedError::Mdb(MdbError::MapFull) => StorageError::MapFull,
        other => StorageError::BackendError(format!("{}: {}", ctx.into(), other)),
    }
}

/// Resolve logical column family → `(database_index, optional_multiplex_tag)`.
///
/// Unknown `cf` strings yield [`StorageError::UnknownColumnFamily`] so behavior matches RocksDB.
fn logical_cf_route(cf: &str) -> Result<(usize, Option<u8>), StorageError> {
    Ok(match cf {
        CF_COIN_RECORDS => (0, Some(TAG_COIN_RECORDS)),
        CF_COIN_BY_PARENT => (0, Some(TAG_COIN_BY_PARENT)),
        CF_COIN_BY_CONFIRMED_HEIGHT => (0, Some(TAG_COIN_BY_CONFIRMED_HEIGHT)),
        CF_COIN_BY_SPENT_HEIGHT => (0, Some(TAG_COIN_BY_SPENT_HEIGHT)),
        CF_ARCHIVE_COIN_RECORDS => (0, Some(TAG_ARCHIVE_COIN_RECORDS)),
        CF_MERKLE_NODES => (0, Some(TAG_MERKLE_NODES)),

        CF_COIN_BY_PUZZLE_HASH => (1, Some(TAG_COIN_BY_PUZZLE_HASH)),
        CF_UNSPENT_BY_PUZZLE_HASH => (1, Some(TAG_UNSPENT_BY_PUZZLE_HASH)),

        CF_HINTS => (2, None),
        CF_HINTS_BY_VALUE => (3, None),
        CF_STATE_SNAPSHOTS => (4, None),
        CF_METADATA => (5, None),

        _ => return Err(StorageError::UnknownColumnFamily(cf.to_string())),
    })
}

fn encode_storage_key(tag: Option<u8>, user_key: &[u8]) -> Cow<'_, [u8]> {
    match tag {
        Some(t) => {
            let mut v = Vec::with_capacity(1 + user_key.len());
            v.push(t);
            v.extend_from_slice(user_key);
            Cow::Owned(v)
        }
        None => Cow::Borrowed(user_key),
    }
}

/// Strip multiplex tag for results returned through [`StorageBackend::prefix_scan`].
fn decode_user_key(tag: Option<u8>, stored: &[u8]) -> Vec<u8> {
    match tag {
        Some(_) => stored.get(1..).unwrap_or_default().to_vec(),
        None => stored.to_vec(),
    }
}

impl LmdbBackend {
    /// Open or create an LMDB environment at [`CoinStoreConfig::storage_path`].
    ///
    /// Creates/opens all databases in [`LMDB_NAMED_DATABASES`] inside one write transaction
    /// (`DatabaseFlags::CREATE` via heed’s [`heed::Database::create`]). Reopen is idempotent.
    pub fn open(config: &CoinStoreConfig) -> Result<Self, StorageError> {
        let path = &config.storage_path;
        fs::create_dir_all(path).map_err(|e| {
            StorageError::BackendError(format!("Failed to create LMDB directory: {}", e))
        })?;

        let mut opts = EnvOpenOptions::new();
        opts.map_size(config.lmdb_map_size);
        opts.max_dbs(LMDB_MAX_DBS);
        // SAFETY: flag bits are valid LMDB env flags; required for STO-003 NO_TLS / NO_READAHEAD.
        unsafe {
            opts.flags(EnvFlags::NO_TLS | EnvFlags::NO_READ_AHEAD);
        }

        let env = unsafe { opts.open(path) }.map_err(|e| map_heed("Failed to open LMDB env", e))?;

        let mut wtxn = env
            .write_txn()
            .map_err(|e| map_heed("LMDB write_txn (open)", e))?;

        let mut dbs = Vec::with_capacity(LMDB_NAMED_DATABASES.len());
        for name in LMDB_NAMED_DATABASES {
            let db: Database<Bytes, Bytes> = env
                .create_database(&mut wtxn, Some(*name))
                .map_err(|e| map_heed(format!("LMDB create db {name}"), e))?;
            dbs.push(db);
        }

        wtxn.commit()
            .map_err(|e| map_heed("LMDB commit (open)", e))?;

        debug_assert_eq!(dbs.len(), LMDB_NAMED_DATABASES.len());
        Ok(Self { env, dbs })
    }

    /// Borrow the underlying LMDB environment.
    ///
    /// Intended for **MVCC proofs** (STO-003 § MVCC Semantics) and operator tooling: callers may
    /// start a long-lived [`RoTxn`](heed::RoTxn), then perform writes on other threads; the read
    /// transaction continues to observe the snapshot from open time until dropped.
    pub fn environment(&self) -> &Env {
        &self.env
    }

    fn db(&self, index: usize) -> Result<Database<Bytes, Bytes>, StorageError> {
        self.dbs
            .get(index)
            .copied()
            .ok_or_else(|| StorageError::BackendError("LMDB internal db index".into()))
    }

    fn with_rotxn<R>(
        &self,
        f: impl for<'a> FnOnce(&'a RoTxn) -> Result<R, StorageError>,
    ) -> Result<R, StorageError> {
        let rtxn = self
            .env
            .read_txn()
            .map_err(|e| map_heed("LMDB read_txn", e))?;
        f(&rtxn)
    }
}

impl StorageBackend for LmdbBackend {
    fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let (dbi, tag) = logical_cf_route(cf)?;
        let db = self.db(dbi)?;
        let enc = encode_storage_key(tag, key);
        self.with_rotxn(|rtxn| {
            let v = db
                .get(rtxn, enc.as_ref())
                .map_err(|e| map_heed("LMDB get", e))?;
            Ok(v.map(|b| b.to_vec()))
        })
    }

    fn put(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let (dbi, tag) = logical_cf_route(cf)?;
        let db = self.db(dbi)?;
        let enc = encode_storage_key(tag, key);
        let mut wtxn = self
            .env
            .write_txn()
            .map_err(|e| map_heed("LMDB write_txn", e))?;
        db.put(&mut wtxn, enc.as_ref(), value)
            .map_err(|e| map_heed("LMDB put", e))?;
        wtxn.commit().map_err(|e| map_heed("LMDB commit", e))
    }

    fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StorageError> {
        let (dbi, tag) = logical_cf_route(cf)?;
        let db = self.db(dbi)?;
        let enc = encode_storage_key(tag, key);
        let mut wtxn = self
            .env
            .write_txn()
            .map_err(|e| map_heed("LMDB write_txn", e))?;
        let _existed: bool = db
            .delete(&mut wtxn, enc.as_ref())
            .map_err(|e| map_heed("LMDB delete", e))?;
        wtxn.commit().map_err(|e| map_heed("LMDB commit", e))
    }

    /// Apply every [`WriteOp`] inside one LMDB write transaction, then **one** `commit` (**STO-005**).
    ///
    /// This mirrors the RocksDB story in [`crate::storage::rocksdb::RocksDbBackend::batch_write`]: either
    /// every mutation lands on disk or none do — LMDB aborts the txn if any `put`/`delete` fails before
    /// commit. Durability follows `heed`/`mdb_txn_commit` semantics (see [`STO-005.md`](../../docs/requirements/domains/storage/specs/STO-005.md)).
    fn batch_write(&self, batch: WriteBatch) -> Result<(), StorageError> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut wtxn = self
            .env
            .write_txn()
            .map_err(|e| map_heed("LMDB write_txn", e))?;

        for op in &batch.ops {
            match op {
                WriteOp::Put { cf, key, value } => {
                    let (dbi, tag) = logical_cf_route(cf)?;
                    let db = self.db(dbi)?;
                    let enc = encode_storage_key(tag, key.as_slice());
                    db.put(&mut wtxn, enc.as_ref(), value.as_slice())
                        .map_err(|e| map_heed("LMDB batch put", e))?;
                }
                WriteOp::Delete { cf, key } => {
                    let (dbi, tag) = logical_cf_route(cf)?;
                    let db = self.db(dbi)?;
                    let enc = encode_storage_key(tag, key.as_slice());
                    let _ = db
                        .delete(&mut wtxn, enc.as_ref())
                        .map_err(|e| map_heed("LMDB batch del", e))?;
                }
            }
        }

        wtxn.commit().map_err(|e| map_heed("LMDB commit", e))
    }

    /// Prefix scan implemented with heed’s [`Database::prefix_iter`] (LMDB `MDB_SET_RANGE` positioning).
    ///
    /// For multiplexed databases, the seek prefix is `tag || user_prefix` so results stay confined
    /// to the logical CF. Returned keys are **decoded** (tag stripped) to preserve the STO-001
    /// “logical key” contract shared with RocksDB.
    fn prefix_scan(&self, cf: &str, prefix: &[u8]) -> Result<Vec<super::KvPair>, StorageError> {
        let (dbi, tag) = logical_cf_route(cf)?;
        let db = self.db(dbi)?;
        let rtxn = self
            .env
            .read_txn()
            .map_err(|e| map_heed("LMDB read_txn", e))?;

        let mut out = Vec::new();

        match tag {
            Some(t) => {
                let seek_prefix: Vec<u8> = {
                    let mut p = Vec::with_capacity(1 + prefix.len());
                    p.push(t);
                    p.extend_from_slice(prefix);
                    p
                };
                let mut iter = db
                    .prefix_iter(&rtxn, seek_prefix.as_slice())
                    .map_err(|e| map_heed("LMDB prefix_iter", e))?;
                loop {
                    match iter
                        .next()
                        .transpose()
                        .map_err(|e| map_heed("LMDB prefix next", e))?
                    {
                        None => break,
                        Some((k, v)) => {
                            out.push((decode_user_key(Some(t), k), v.to_vec()));
                        }
                    }
                }
            }
            None => {
                if prefix.is_empty() {
                    let mut iter = db.iter(&rtxn).map_err(|e| map_heed("LMDB iter", e))?;
                    loop {
                        match iter
                            .next()
                            .transpose()
                            .map_err(|e| map_heed("LMDB iter next", e))?
                        {
                            None => break,
                            Some((k, v)) => out.push((k.to_vec(), v.to_vec())),
                        }
                    }
                } else {
                    let mut iter = db
                        .prefix_iter(&rtxn, prefix)
                        .map_err(|e| map_heed("LMDB prefix_iter", e))?;
                    loop {
                        match iter
                            .next()
                            .transpose()
                            .map_err(|e| map_heed("LMDB prefix next", e))?
                        {
                            None => break,
                            Some((k, v)) => out.push((k.to_vec(), v.to_vec())),
                        }
                    }
                }
            }
        }

        Ok(out)
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.env
            .force_sync()
            .map_err(|e| map_heed("LMDB force_sync", e))
    }

    fn compact(&self, _cf: &str) -> Result<(), StorageError> {
        // LMDB COW B+trees — no per-CF compaction knob; **STO-006** explicitly requires this path to stay a
        // no-op while RocksDB applies Leveled/FIFO policies per CF (`docs/requirements/domains/storage/specs/STO-006.md`).
        Ok(())
    }
}
