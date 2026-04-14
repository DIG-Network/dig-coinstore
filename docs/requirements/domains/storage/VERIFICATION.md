# Storage — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [STO-001](NORMATIVE.md#STO-001) | :x: | StorageBackend trait | Tests: trait methods exist, both backends implement trait. |
| [STO-002](NORMATIVE.md#STO-002) | :x: | RocksDB 12 column families | Tests: all CFs created on open, data isolated per CF. |
| [STO-003](NORMATIVE.md#STO-003) | :x: | LMDB 6 named databases | Tests: all DBs created on open, data isolated per DB. |
| [STO-004](NORMATIVE.md#STO-004) | :x: | Bloom filter configuration | Tests: verify bloom config per CF via RocksDB options. |
| [STO-005](NORMATIVE.md#STO-005) | :x: | Atomic WriteBatch | Tests: crash recovery, partial write not visible. |
| [STO-006](NORMATIVE.md#STO-006) | :x: | Compaction strategy | Tests: verify compaction style per CF via RocksDB options. |
| [STO-007](NORMATIVE.md#STO-007) | :x: | Feature gates | Tests: compile with each feature flag, default is rocksdb. |
| [STO-008](NORMATIVE.md#STO-008) | :x: | Bincode serialization | Tests: round-trip CoinRecord, key encoding helpers. |

**Status legend:** :white_check_mark: verified · :warning: partial · :x: gap
