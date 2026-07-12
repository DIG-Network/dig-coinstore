# Changelog

All notable changes to this project are documented here.
This project adheres to [Semantic Versioning](https://semver.org) and
[Conventional Commits](https://www.conventionalcommits.org).

## [0.1.2] - 2026-07-12

### CI
- Add flaky-test management (#489) (#2)

## [0.1.1] - 2026-07-04

### CI
- Enforce version increment in PRs (package.json / Cargo.toml)- Enforce Conventional Commits with commitlint on PRs- Enforce Conventional Commits with commitlint on PRs- Release automation (git-cliff changelog + tag on merge); publish is manual workflow_dispatch (#230)- Re-arm crates.io auto-publish on version tag (token in org secrets; auto-publish-everything #230)- Add PR quality gates (fmt/clippy/test/build) [#230] (#1)

### Chores
- **changelog:** Add git-cliff config for Conventional-Commit changelog

## [0.1.0] - 2026-04-17

### Features
- **api-005:** BlockData, CoinAddition, and crate-root exports- **api-008:** CoinStore snapshot, restore, and checkpoint retention- **api-009:** UnspentLineageInfo and crate-root export- **storage:** Complete STO-002 RocksDB column families- **storage:** STO-003 LMDB six named databases and MVCC- **storage:** Implement STO-004 RocksDB bloom configuration- **storage:** STO-005 durable WriteBatch commit on RocksDB- **storage:** Implement STO-006 compaction and memtable depth- **storage:** STO-007 compile-time gates and open_storage_backend factory- **merkle:** Complete MRK-002 memoized empty hashes in OnceLock array- **merkle:** MRK-003 persistent internal nodes and merkle flush- **merkle:** MRK-004 get_coin_proof and proof leaf_value- **merkle:** MRK-005 verify_coin_proof and proof verification tests- **merkle:** MRK-006 coin_record_hash over STO-008 bincode- **block:** BLK-001 apply_block() full pipeline implementation- **hints:** HNT-001 hint validation — validate_hint(), HintAction, MAX_HINT_LENGTH- **hints:** HNT-002/003/004 add_hint(), bidirectional indices, and hint queries- **hints:** HNT-005 remove_hints_for_coins() and HNT-006 variable-length hint key support- **queries:** QRY-001..005 coin query methods- **queries:** QRY-006..011 CoinState queries, batch pagination, aggregates, lineage- **rollback:** RBK-001..007 rollback_to_block() full pipeline- **concurrency:** CON-001 CoinStore is Send + Sync- **concurrency+perf:** CON-002..004 and PRF-001..009

### Bug Fixes
- **test:** Update api_010 rollback test for RBK-001 implementation- **prf-001:** Maintain unspent_ids incrementally in apply_block and rollback- **deps:** Use dig-clvm and dig-constants from crates.io

### Refactor
- **api-007:** Use root_observed only, refresh tests and tracking

### Documentation
- Add spec- Comprehensive SPEC.md citations and LLM-friendly comments across codebase- **test:** Clarify STO-001 proof table in sto_001_tests- Mark HNT-005, HNT-006 complete in IMPLEMENTATION_ORDER- Comprehensive README with full public API reference

### Testing
- **mrk-001:** Dedicated mrk_001_tests and traceability docs- **block:** BLK-002..BLK-014 dedicated test files and tracking

### CI
- Add crate publishing

### Styling
- Apply linter formatting to test files- Clippy fixes and cargo fmt

### Chores
- Ignore target/ and .repomix/ to prevent large binary commits- Remove repomix pack from version control; gitignore .gitnexus

### API-001
- CoinStore constructors (new, with_config, init_genesis)

### API-002
- CoinRecord, ChiaCoinRecord interop, and tests

### API-003
- CoinStoreConfig, StorageBackend, LMDB open path, tests

### API-004
- Full CoinStoreError enum, conversions, and tests

### API-006
- ApplyBlockResult, RollbackResult, CoinStore stubs, and tests

### API-007
- CoinStoreStats, CoinStore::stats, root_observed, tests

### Refactor
- Split legacy test files into per-requirement test files

### STR-001
- Cargo.toml with dependencies, feature gates, and metadata

### STR-002
- Module hierarchy with all source files and test infrastructure

### STR-003
- Storage module with StorageBackend trait and RocksDB implementation

### STR-004
- Sparse Merkle tree with proofs and memoized empty hashes

### STR-005
- Re-export Chia ecosystem types from crate root

### STR-006
- Test infrastructure with comprehensive helpers module


