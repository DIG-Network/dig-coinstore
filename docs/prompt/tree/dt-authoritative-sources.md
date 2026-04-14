# Authoritative Sources

## Traceability Chain

```
SPEC.md (master spec)
  └── NORMATIVE.md#PREFIX-NNN (authoritative requirement)
        └── specs/PREFIX-NNN.md (detailed specification + test plan)
              └── tests/{domain}_tests.rs (verification)
                    └── src/*.rs (implementation)
```

**Always read top-down.** The spec is the source of truth. Requirements are derived from the spec. Tests are derived from requirements. Code satisfies tests.

## Master Spec

[SPEC.md](../../resources/SPEC.md) — the single authoritative document for dig-coinstore.

| Section | Domain | Prefix |
|---------|--------|--------|
| 1 (Overview) + 7 (Storage) + 10 (Internal) | Crate Structure | STR |
| 2 (Data Model) + 3 (Public API) + 4 (Errors) | Crate API | API |
| 5 (Block Application) | Block Application | BLK |
| 6 (Rollback) | Rollback | RBK |
| 3.4-3.11 (Query Methods) | Queries | QRY |
| 7 (Storage Architecture) | Storage | STO |
| 9 (Merkle Tree) | Merkle | MRK |
| 8 (Hint Store) | Hints | HNT |
| 13 (Performance) | Performance | PRF |
| 11 (Concurrency) | Concurrency | CON |

## Chia Reference

When the spec cites a Chia source file, the authoritative version is at:
```
c:\Users\micha\workspace\chia\chia-blockchain\
```

Key files:
- `chia/full_node/coin_store.py` — CoinStore (primary reference)
- `chia/full_node/hint_store.py` — HintStore
- `chia/full_node/hint_management.py` — Hint extraction
- `chia/consensus/coin_store_protocol.py` — CoinStoreProtocol interface

## Existing Implementation

The working code being extracted lives at:
```
c:\Users\micha\workspace\dig_network\l2_driver_state_channel\
```

Key files:
- `src/coinset/state.rs` — CoinSetState (in-memory coinset)
- `src/coinset/coin.rs` — Coin, CoinRecord types
- `src/storage/state_store.rs` — StateStore (LMDB/RocksDB persistence)
- `src/storage/rocksdb.rs` — RocksDB storage adapter
