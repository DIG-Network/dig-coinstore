# Storage — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [STO-001](NORMATIVE.md#STO-001) | :white_check_mark: | StorageBackend trait | `tests/sto_001_tests.rs`: Send+Sync compile checks; per-`rocksdb-storage` integration tests for all seven methods + `dyn` coercion + unknown CF; per-`lmdb-storage` mirror suite including empty `batch_write`. Trait contract table in `src/storage/mod.rs`. |
| [STO-002](NORMATIVE.md#STO-002) | :white_check_mark: | RocksDB 12 column families | `tests/sto_002_tests.rs`: `list_column_families` matches schema (12), CF isolation, reopen, `dyn StorageBackend`, per-CF put/get. Code: `src/storage/rocksdb.rs` (global options, FIFO on `state_snapshots`, per-CF buffers/blooms). Schema-evolution (11→12 CF) and RocksDB property assertions deferred per STO-002 test plan. |
| [STO-003](NORMATIVE.md#STO-003) | :x: | LMDB 6 named databases | Tests: all DBs created on open, data isolated per DB. |
| [STO-004](NORMATIVE.md#STO-004) | :x: | Bloom filter configuration | Tests: verify bloom config per CF via RocksDB options. |
| [STO-005](NORMATIVE.md#STO-005) | :x: | Atomic WriteBatch | Tests: crash recovery, partial write not visible. |
| [STO-006](NORMATIVE.md#STO-006) | :x: | Compaction strategy | Tests: verify compaction style per CF via RocksDB options. |
| [STO-007](NORMATIVE.md#STO-007) | :x: | Feature gates | Tests: compile with each feature flag, default is rocksdb. |
| [STO-008](NORMATIVE.md#STO-008) | :x: | Bincode serialization | Tests: round-trip CoinRecord, key encoding helpers. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
