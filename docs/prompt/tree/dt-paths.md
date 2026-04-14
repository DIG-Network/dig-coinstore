# Path Conventions

## Project Root
```
c:\Users\micha\workspace\dig_network\dig-coinstore\
```

## Source Code
```
src/lib.rs                    # Crate root, re-exports
src/coin_store.rs             # CoinStore struct, public API
src/config.rs                 # CoinStoreConfig, constants
src/error.rs                  # CoinStoreError enum
src/types.rs                  # BlockData, CoinAddition, CoinRecord, etc.
src/block_apply.rs            # Block application pipeline
src/rollback.rs               # Rollback pipeline
src/queries.rs                # Query implementations
src/hints.rs                  # Hint store logic
src/archive.rs                # Tiered archival
src/storage/mod.rs            # StorageBackend trait
src/storage/rocksdb.rs        # RocksDB implementation
src/storage/lmdb.rs           # LMDB implementation
src/storage/schema.rs         # Column families, key encoding
src/merkle/mod.rs             # SparseMerkleTree
src/merkle/proof.rs           # Proof generation/verification
src/merkle/persistent.rs      # Persistent node storage
src/cache/mod.rs              # Cache module
src/cache/unspent_set.rs      # In-memory unspent HashSet
src/cache/lru_cache.rs        # LRU CoinRecord cache
src/cache/counters.rs         # Materialized counters
```

## Tests
```
tests/helpers/mod.rs          # Shared utilities
tests/str_tests.rs            # Crate structure
tests/api_tests.rs            # API types
tests/blk_tests.rs            # Block application
tests/rbk_tests.rs            # Rollback
tests/qry_tests.rs            # Queries
tests/sto_tests.rs            # Storage backends
tests/mrk_tests.rs            # Merkle tree
tests/hnt_tests.rs            # Hints
tests/prf_tests.rs            # Performance
tests/con_tests.rs            # Concurrency
```

## Documentation
```
docs/resources/SPEC.md                           # Master specification
docs/requirements/README.md                      # Requirements overview
docs/requirements/SCHEMA.md                      # Data model
docs/requirements/REQUIREMENTS_REGISTRY.yaml     # Domain registry
docs/requirements/IMPLEMENTATION_ORDER.md        # Phased checklist
docs/requirements/domains/{domain}/NORMATIVE.md  # Requirements
docs/requirements/domains/{domain}/TRACKING.yaml # Status
docs/requirements/domains/{domain}/VERIFICATION.md # QA
docs/requirements/domains/{domain}/specs/*.md    # Individual specs
docs/prompt/prompt.md                            # Workflow overview
docs/prompt/start.md                             # Entry point
docs/prompt/chat.md                              # Chat prompt
docs/prompt/tree/*.md                            # Decision tree
```

## Related Projects
```
c:\Users\micha\workspace\dig_network\l2_driver_state_channel\  # Source codebase
c:\Users\micha\workspace\dig_network\dig-mempool\              # Sister crate (pattern reference)
c:\Users\micha\workspace\chia\chia-blockchain\                  # Chia reference implementation
```
