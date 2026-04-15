# Crate Structure — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [STR-001](NORMATIVE.md#STR-001) | ✅ | Cargo.toml | 13 tests: default feature is rocksdb, all 12 deps importable+functional, serde derives compile, feature gates compile independently (rocksdb, lmdb, full, no-default). Clippy clean. |
| [STR-002](NORMATIVE.md#STR-002) | ✅ | Module hierarchy | 5 tests: crate compiles, all 12 top-level modules accessible, storage/merkle/cache submodules resolve. 21 source files + 11 test files + benchmark. |
| [STR-003](NORMATIVE.md#STR-003) | ✅ | Storage module | 9 tests: StorageBackend trait (Send+Sync, 7 methods), 12 CF name constants (unique, non-empty), 4 key encoding round-trips, RocksDB put/get/delete/batch_write/prefix_scan. |
| [STR-004](NORMATIVE.md#STR-004) | ✅ | Merkle module | `tests/str_004_tests.rs`: `SparseMerkleTree` on the public `dig_coinstore::merkle` surface. Merkle semantics live under domain specs (`tests/mrk_001_tests.rs`, `mrk_002`, `mrk_004`, `mrk_005`). |
| [STR-005](NORMATIVE.md#STR-005) | ✅ | Re-export strategy | 7 tests: Coin/Bytes32/CoinState/CoinStateFilters importable from crate root, type identity proven via assignment (dig_coinstore::X == dig_clvm::X). |
| [STR-006](NORMATIVE.md#STR-006) | ✅ | Test infrastructure | 8 tests: helpers compile, coin builders (single + batch), hash determinism, temp dir lifecycle, block builder with builder pattern, CoinState builders, all 10 test files importable. |

**Status legend:** ✅ verified · ⚠️ partial · ❌ gap
