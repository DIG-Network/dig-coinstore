# Storage — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [STO-001](NORMATIVE.md#STO-001) | :white_check_mark: | StorageBackend trait | `tests/sto_001_tests.rs`: Send+Sync compile checks; per-`rocksdb-storage` integration tests for all seven methods + `dyn` coercion + unknown CF; per-`lmdb-storage` mirror suite including empty `batch_write`. Trait contract table in `src/storage/mod.rs`. |
| [STO-002](NORMATIVE.md#STO-002) | :white_check_mark: | RocksDB 12 column families | `tests/sto_002_tests.rs`: twelve CFs on disk, isolation, reopen, `dyn` trait, per-CF round-trip, `STO002_ROCKS_WRITE_BUFFER_BYTES` proof, 11→12 CF reopen. `src/storage/rocksdb.rs`: global STO-002 options, per-CF buffers/blooms/compaction. |
| [STO-003](NORMATIVE.md#STO-003) | :white_check_mark: | LMDB 6 named databases | `tests/sto_003_tests.rs` (feature `lmdb-storage`): six names via `open_database`, `dyn StorageBackend`, MVCC snapshot, concurrent reads + writer, configurable `map_size`, reopen, logical CF isolation, multiplexed `prefix_scan`, `MapFull`, all logical CFs round-trip. Implementation: `src/storage/lmdb.rs`. |
| [STO-004](NORMATIVE.md#STO-004) | :x: | Bloom filter configuration | Tests: verify bloom config per CF via RocksDB options. |
| [STO-005](NORMATIVE.md#STO-005) | :x: | Atomic WriteBatch | Tests: crash recovery, partial write not visible. |
| [STO-006](NORMATIVE.md#STO-006) | :x: | Compaction strategy | Tests: verify compaction style per CF via RocksDB options. |
| [STO-007](NORMATIVE.md#STO-007) | :x: | Feature gates | Tests: compile with each feature flag, default is rocksdb. |
| [STO-008](NORMATIVE.md#STO-008) | :x: | Bincode serialization | Tests: round-trip CoinRecord, key encoding helpers. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
