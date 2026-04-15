//! LMDB storage backend (`heed`).
//!
//! Implements [`super::StorageBackend`] using one LMDB named database per logical column family
//! (schema [`super::schema::ALL_COLUMN_FAMILIES`]), matching the RocksDB layout so higher layers
//! stay identical (STO-003, STR-003).
//!
//! This is the **minimal** operational backend needed so API-003’s default `StorageBackend::Lmdb`
//! path works when `full-storage` / `lmdb-storage` is enabled; deeper tuning (readers, compaction
//! policy) lands in later STO specs.
//!
//! # Requirement: STO-003 (implementation), API-003 (config-driven open)
//! # Spec: docs/requirements/domains/storage/specs/STO-003.md

use heed::types::Bytes;
use heed::{Database, Env, EnvOpenOptions};
use std::fs;

use crate::config::CoinStoreConfig;

use super::schema::ALL_COLUMN_FAMILIES;
use super::{StorageBackend, StorageError, WriteBatch, WriteOp};

/// LMDB-backed key-value store for coinstate.
///
/// Owns a [`heed::Env`] and one [`Database`] per column family name in [`ALL_COLUMN_FAMILIES`].
pub struct LmdbBackend {
    env: Env,
    /// Same order as [`ALL_COLUMN_FAMILIES`].
    dbs: Vec<Database<Bytes, Bytes>>,
}

impl LmdbBackend {
    /// Open or create an LMDB environment at [`CoinStoreConfig::storage_path`].
    ///
    /// Respects [`CoinStoreConfig::lmdb_map_size`] for `EnvOpenOptions::map_size`. Creates every
    /// named database on first run; re-opens existing environments on restart.
    pub fn open(config: &CoinStoreConfig) -> Result<Self, StorageError> {
        let path = &config.storage_path;
        fs::create_dir_all(path).map_err(|e| {
            StorageError::BackendError(format!("Failed to create LMDB directory: {}", e))
        })?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(config.lmdb_map_size)
                .max_dbs(32)
                .open(path)
                .map_err(|e| {
                    StorageError::BackendError(format!("Failed to open LMDB env: {}", e))
                })?
        };

        let mut wtxn = env
            .write_txn()
            .map_err(|e| StorageError::BackendError(format!("LMDB write_txn: {}", e)))?;

        let mut dbs = Vec::with_capacity(ALL_COLUMN_FAMILIES.len());
        for name in ALL_COLUMN_FAMILIES {
            let db: Database<Bytes, Bytes> =
                env.create_database(&mut wtxn, Some(*name)).map_err(|e| {
                    StorageError::BackendError(format!("LMDB create db {}: {}", name, e))
                })?;
            dbs.push(db);
        }

        wtxn.commit()
            .map_err(|e| StorageError::BackendError(format!("LMDB commit: {}", e)))?;

        Ok(Self { env, dbs })
    }

    fn cf_index(cf: &str) -> Result<usize, StorageError> {
        ALL_COLUMN_FAMILIES
            .iter()
            .position(|&n| n == cf)
            .ok_or_else(|| StorageError::UnknownColumnFamily(cf.to_string()))
    }
}

impl StorageBackend for LmdbBackend {
    fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let idx = Self::cf_index(cf)?;
        let db = self.dbs[idx];
        let rtxn = self
            .env
            .read_txn()
            .map_err(|e| StorageError::BackendError(format!("LMDB read_txn: {}", e)))?;
        let v = db
            .get(&rtxn, key)
            .map_err(|e| StorageError::BackendError(format!("LMDB get: {}", e)))?;
        Ok(v.map(|b| b.to_vec()))
    }

    fn put(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let idx = Self::cf_index(cf)?;
        let db = self.dbs[idx];
        let mut wtxn = self
            .env
            .write_txn()
            .map_err(|e| StorageError::BackendError(format!("LMDB write_txn: {}", e)))?;
        db.put(&mut wtxn, key, value)
            .map_err(|e| StorageError::BackendError(format!("LMDB put: {}", e)))?;
        wtxn.commit()
            .map_err(|e| StorageError::BackendError(format!("LMDB commit: {}", e)))
    }

    fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StorageError> {
        let idx = Self::cf_index(cf)?;
        let db = self.dbs[idx];
        let mut wtxn = self
            .env
            .write_txn()
            .map_err(|e| StorageError::BackendError(format!("LMDB write_txn: {}", e)))?;
        db.delete(&mut wtxn, key)
            .map_err(|e| StorageError::BackendError(format!("LMDB delete: {}", e)))?;
        wtxn.commit()
            .map_err(|e| StorageError::BackendError(format!("LMDB commit: {}", e)))
    }

    fn batch_write(&self, batch: WriteBatch) -> Result<(), StorageError> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut wtxn = self
            .env
            .write_txn()
            .map_err(|e| StorageError::BackendError(format!("LMDB write_txn: {}", e)))?;

        for op in &batch.ops {
            match op {
                WriteOp::Put { cf, key, value } => {
                    let idx = Self::cf_index(cf)?;
                    let db = self.dbs[idx];
                    db.put(&mut wtxn, key.as_slice(), value.as_slice())
                        .map_err(|e| {
                            StorageError::BackendError(format!("LMDB batch put: {}", e))
                        })?;
                }
                WriteOp::Delete { cf, key } => {
                    let idx = Self::cf_index(cf)?;
                    let db = self.dbs[idx];
                    db.delete(&mut wtxn, key.as_slice()).map_err(|e| {
                        StorageError::BackendError(format!("LMDB batch del: {}", e))
                    })?;
                }
            }
        }

        wtxn.commit()
            .map_err(|e| StorageError::BackendError(format!("LMDB commit: {}", e)))
    }

    fn prefix_scan(&self, cf: &str, prefix: &[u8]) -> Result<Vec<super::KvPair>, StorageError> {
        let idx = Self::cf_index(cf)?;
        let db = self.dbs[idx];
        let rtxn = self
            .env
            .read_txn()
            .map_err(|e| StorageError::BackendError(format!("LMDB read_txn: {}", e)))?;

        let mut out = Vec::new();

        if prefix.is_empty() {
            let mut iter = db
                .iter(&rtxn)
                .map_err(|e| StorageError::BackendError(format!("LMDB iter: {}", e)))?;
            loop {
                match iter
                    .next()
                    .transpose()
                    .map_err(|e| StorageError::BackendError(format!("LMDB iter next: {}", e)))?
                {
                    None => break,
                    Some((k, v)) => out.push((k.to_vec(), v.to_vec())),
                }
            }
        } else {
            let mut iter = db
                .prefix_iter(&rtxn, prefix)
                .map_err(|e| StorageError::BackendError(format!("LMDB prefix_iter: {}", e)))?;
            loop {
                match iter
                    .next()
                    .transpose()
                    .map_err(|e| StorageError::BackendError(format!("LMDB prefix next: {}", e)))?
                {
                    None => break,
                    Some((k, v)) => out.push((k.to_vec(), v.to_vec())),
                }
            }
        }

        Ok(out)
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.env
            .force_sync()
            .map_err(|e| StorageError::BackendError(format!("LMDB force_sync: {}", e)))
    }

    fn compact(&self, _cf: &str) -> Result<(), StorageError> {
        // LMDB uses copy-on-write B+trees; there is no per-database “compact” equivalent to
        // RocksDB’s manual compaction. Expose as no-op until a maintenance story exists.
        Ok(())
    }
}
