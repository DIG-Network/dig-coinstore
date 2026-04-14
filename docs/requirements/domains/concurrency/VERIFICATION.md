# Concurrency — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [CON-001](NORMATIVE.md#CON-001) | -- | CoinStore is Send + Sync | Tests: compile-time assertion via `fn assert_send_sync<T: Send + Sync>()`, call public methods from spawned threads. |
| [CON-002](NORMATIVE.md#CON-002) | -- | RwLock strategy (shared read, exclusive write) | Tests: concurrent readers do not block each other, writer blocks readers, readers block writer, apply_block and rollback acquire exclusive lock. |
| [CON-003](NORMATIVE.md#CON-003) | -- | MVCC reads during writes | Tests: reader started before apply_block sees pre-block state, reader started after commit sees post-block state, tip swap is atomic, no torn reads during Phase 2. |
| [CON-004](NORMATIVE.md#CON-004) | -- | Parallel removal validation | Tests: par_iter used for removal checks, validation is lock-free (no deadlock under contention), results match sequential validation, performance improvement measurable on multi-core. |

**Status legend:** ✅ verified · ⚠️ partial · -- gap
