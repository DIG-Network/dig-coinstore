# Hard Rules

These are non-negotiable. Violating any of these is a blocking defect.

1. **TDD** — Write the failing test BEFORE writing implementation code. No exceptions.
2. **One requirement per commit** — Each commit implements exactly one requirement ID.
3. **Phase order** — Complete Phase N before starting Phase N+1.
4. **Chia types first** — Use `Coin`, `Bytes32`, `CoinState` from chia-protocol. Never redefine them.
5. **No CLVM execution** — This crate never runs puzzles. It receives pre-validated state changes.
6. **Atomic block application** — Either the entire block applies or nothing changes. No partial mutations.
7. **Storage trait abstraction** — All storage access goes through the `StorageBackend` trait. No direct DB calls from business logic.
8. **WriteBatch for block writes** — All writes for a single block are committed in a single atomic batch.
9. **Rollback correctness** — `apply_block(B); rollback(h-1)` MUST produce identical state to before `apply_block(B)`.
10. **Update tracking** — After completing each requirement, update VERIFICATION.md, TRACKING.yaml, and IMPLEMENTATION_ORDER.md.
11. **Tools before code** — Run SocratiCode, Repomix, and GitNexus before reading or writing code.
12. **Comments** — All code must have high-signal, LLM-friendly comments with semantic links to docs and related code.
13. **Feature gates** — Storage backends must be feature-gated. Code must compile with any single feature enabled.
