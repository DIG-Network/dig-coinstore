# Step 5: Validate

Run all checks. ALL must pass.

```bash
# Tests (must include the new test)
cargo test

# Clippy (no warnings)
cargo clippy -- -D warnings

# Format check
cargo fmt --check

# Feature gate check (each feature compiles independently)
cargo check --features rocksdb-storage
cargo check --features lmdb-storage
cargo check --features full-storage
```

## Checks

- [ ] `cargo test` — all tests pass (including the new one)
- [ ] `cargo clippy -- -D warnings` — no warnings
- [ ] `cargo fmt --check` — properly formatted
- [ ] Feature gates compile independently

## If a check fails

Fix the issue. Do not skip the check. Do not suppress warnings with `#[allow(...)]` unless there's a documented reason.

**Next:** → [dt-wf-update-tracking.md](dt-wf-update-tracking.md)
