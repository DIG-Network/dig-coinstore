# Crate Structure — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md) — Sections 1, 7, 10

---

## §1 Cargo.toml

<a id="STR-001"></a>**STR-001** The crate MUST be named `dig-coinstore` with `lib` crate type. `Cargo.toml` MUST declare dependencies on `chia-protocol`, `chia-sdk-coinset`, `dig-clvm`, `dig-constants`, `rocksdb`, `heed` (LMDB), `bincode`, `serde`, `parking_lot`, `thiserror`, and `tracing`. Storage backends MUST be feature-gated: `lmdb-storage`, `rocksdb-storage`, `full-storage` (both). The default feature MUST be `rocksdb-storage`.
> **Spec:** [`STR-001.md`](specs/STR-001.md)

---

## §2 Module Hierarchy

<a id="STR-002"></a>**STR-002** `src/lib.rs` MUST be the crate root. It MUST re-export all public types and define the top-level module structure. The source tree MUST follow this layout:

```
dig-coinstore/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Crate root, re-exports
│   ├── coin_store.rs           # CoinStore struct, public API orchestration
│   ├── config.rs               # CoinStoreConfig, constants
│   ├── error.rs                # CoinStoreError enum
│   ├── types.rs                # BlockData, CoinAddition, CoinRecord, CoinState, etc.
│   ├── block_apply.rs          # Block application pipeline (Phase 1 + 2)
│   ├── rollback.rs             # Rollback pipeline
│   ├── queries.rs              # All query method implementations
│   ├── hints.rs                # Hint store logic
│   ├── storage/
│   │   ├── mod.rs              # Storage trait definition
│   │   ├── rocksdb.rs          # RocksDB backend implementation
│   │   ├── lmdb.rs             # LMDB backend implementation
│   │   └── schema.rs           # Column family names, key encoding helpers
│   ├── merkle/
│   │   ├── mod.rs              # SparseMerkleTree struct
│   │   ├── proof.rs            # Proof generation and verification
│   │   └── persistent.rs       # Persistent node storage
│   ├── cache/
│   │   ├── mod.rs              # Cache module root
│   │   ├── unspent_set.rs      # In-memory HashSet<CoinId> for O(1) unspent checks
│   │   ├── lru_cache.rs        # LRU CoinRecord cache
│   │   └── counters.rs         # Materialized aggregate counters
│   └── archive.rs              # Tiered spent coin archival
├── tests/
│   ├── helpers/
│   │   └── mod.rs              # Shared test utilities, coin builders
│   ├── str_tests.rs            # Crate structure verification tests
│   ├── api_tests.rs            # API type tests
│   ├── blk_tests.rs            # Block application tests
│   ├── rbk_tests.rs            # Rollback tests
│   ├── qry_tests.rs            # Query tests
│   ├── sto_tests.rs            # Storage backend tests
│   ├── mrk_tests.rs            # Merkle tree tests
│   ├── hnt_tests.rs            # Hint store tests
│   ├── prf_tests.rs            # Performance tests
│   └── con_tests.rs            # Concurrency tests
├── benches/
│   └── coinstate_bench.rs      # Criterion benchmarks
└── docs/
    ├── resources/
    │   └── SPEC.md
    ├── requirements/
    │   └── ...
    └── prompt/
        └── ...
```

> **Spec:** [`STR-002.md`](specs/STR-002.md)

---

## §3 Storage Module

<a id="STR-003"></a>**STR-003** `src/storage/mod.rs` MUST define a `StorageBackend` trait that abstracts over LMDB and RocksDB. Both `src/storage/rocksdb.rs` and `src/storage/lmdb.rs` MUST implement this trait. `src/storage/schema.rs` MUST define column family names and key encoding/decoding helpers.
> **Spec:** [`STR-003.md`](specs/STR-003.md)

---

## §4 Merkle Module

<a id="STR-004"></a>**STR-004** `src/merkle/mod.rs` MUST define the `SparseMerkleTree` struct with `batch_insert`, `batch_update`, `batch_remove`, and `root()` methods. `src/merkle/proof.rs` MUST define `SparseMerkleProof` with `verify()`. `src/merkle/persistent.rs` MUST implement loading/flushing dirty nodes to storage.
> **Spec:** [`STR-004.md`](specs/STR-004.md)

---

## §5 Re-export Strategy

<a id="STR-005"></a>**STR-005** `src/lib.rs` MUST re-export `Coin`, `Bytes32`, and `CoinState` from `chia-protocol` via `dig-clvm`. The crate MUST NOT define its own `Coin` or `Bytes32` types. All Chia ecosystem types MUST be accessed through `dig-clvm` re-exports to maintain a single integration point.
> **Spec:** [`STR-005.md`](specs/STR-005.md)

---

## §6 Test Infrastructure

<a id="STR-006"></a>**STR-006** The `tests/` directory MUST contain one test file per requirement domain (e.g., `blk_tests.rs`, `qry_tests.rs`). `tests/helpers/mod.rs` MUST provide shared utilities: coin builder functions, temporary directory management, genesis state initialization, and block builder helpers. All test files MUST import helpers via `mod helpers;`.
> **Spec:** [`STR-006.md`](specs/STR-006.md)
