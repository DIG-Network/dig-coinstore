# Start

## Immediate Actions

1. **Sync**
   ```bash
   git fetch origin && git pull origin main
   ```

2. **Check tools — ALL THREE MUST BE FRESH**
   ```bash
   npx gitnexus status          # GitNexus index fresh?
   npx gitnexus analyze         # Update if stale
   # SocratiCode: verify Docker running, index current
   codebase_status {}            # SocratiCode MCP status
   ```
   **Do not proceed until tools are confirmed operational.** Coding without tools leads to redundant work and missed dependencies.

3. **Pick work** — open `docs/requirements/IMPLEMENTATION_ORDER.md`
   - Choose the first `- [ ]` item
   - Every `- [x]` is done on main — skip it
   - Work phases in order: Phase 0 before Phase 1, etc.

4. **Pack context — BEFORE reading any code**
   ```bash
   npx repomix@latest src -o .repomix/pack-src.xml
   npx repomix@latest tests -o .repomix/pack-tests.xml
   ```

5. **Search with SocratiCode — BEFORE reading files**
   ```
   codebase_search { query: "coin store apply block rollback" }
   codebase_graph_query { filePath: "src/coin_store.rs" }
   ```

6. **Read spec** — follow the full trace:
   - `NORMATIVE.md#PREFIX-NNN` → authoritative requirement
   - `specs/PREFIX-NNN.md` → detailed specification + **test plan**
   - `VERIFICATION.md` → how to verify
   - `TRACKING.yaml` → current status

7. **Continue** → [dt-wf-select.md](tree/dt-wf-select.md)

---

## Hard Requirements

1. **Use chia crate ecosystem first** — never reimplement what `chia-protocol`, `chia-sha2`, `chia-traits`, `chia-consensus` provide. Use `chia-sha2::Sha256` for all SHA-256 operations. Use `chia-protocol::CoinStateFilters` for batch query filters.
2. **No custom coin ID computation** — use `Coin::coin_id()` from `chia-protocol`.
3. **No custom serialization for Chia types** — use `Streamable` from `chia-traits` for wire format; `bincode` for internal KV storage only.
4. **Re-export, don't redefine** — `Coin`, `Bytes32`, `CoinState`, `CoinStateFilters` from upstream via dig-clvm. `chia_protocol::CoinRecord` aliased as `ChiaCoinRecord` for interop.
5. **No CLVM execution** — this crate stores and queries state; it never runs puzzles.
6. **No block production** — this crate applies pre-validated blocks; it never selects transactions.
7. **TEST FIRST (TDD)** — write the failing test before writing implementation code. The test defines the contract. The spec's Test Plan section tells you exactly what tests to write.
8. **One requirement per commit** — don't batch unrelated work.
9. **Update tracking after each requirement** — VERIFICATION.md, TRACKING.yaml, IMPLEMENTATION_ORDER.md.
10. **SocratiCode before file reads** — search semantically first, read targeted files second.
11. **Repomix before implementation** — pack relevant scope for full context.
12. **GitNexus before refactoring** — check dependency impact before renaming or moving symbols.
13. **Follow the decision tree to completion** — dt-wf-select through dt-wf-commit, no shortcuts.
14. **Storage is the foundation** — get the StorageBackend trait and RocksDB column families right before building anything on top.

---

## Tech Stack

| Component | Crate | Version |
|-----------|-------|---------|
| CLVM types (re-export) | `dig-clvm` | 0.1.0 |
| Network constants | `dig-constants` | 0.1.0 |
| Protocol types | `chia-protocol` | 0.42 |
| SHA-256 | `chia-sha2` | 0.42 |
| Streamable trait | `chia-traits` | 0.42 |
| Merkle cross-check (dev) | `chia-consensus` | 0.42 |
| Test oracle (dev) | `chia-sdk-test` | 0.33 |
| Storage (RocksDB) | `rocksdb` | 0.22 |
| Storage (LMDB) | `heed` | 0.20 |
| Serialization (storage) | `bincode` | 1.3 |
| Serialization | `serde` | latest |
| Locking | `parking_lot` | 0.12 |
| Error handling | `thiserror` | 2 |
| Logging | `tracing` | 0.1 |
| Caching | `lru` | 0.12 |
| Parallelism | `rayon` | 1.10 |
| Testing | `tempfile` | 3 |
