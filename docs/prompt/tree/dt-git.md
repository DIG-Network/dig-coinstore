# Git Workflow

## Branch Strategy

- Work on `main` unless told otherwise.
- One requirement per commit.

## Commit Message Format

```
{PREFIX}-{NNN}: {summary}

{Details of what was implemented and why}

Requirement: {PREFIX}-{NNN}
Spec: docs/requirements/domains/{domain}/specs/{PREFIX-NNN}.md
Tests: tests/{domain}_tests.rs
```

Example:
```
BLK-002: Height continuity validation

Block application now validates that block.height == self.height() + 1
before any state mutation. Returns HeightMismatch error on violation.

Requirement: BLK-002
Spec: docs/requirements/domains/block_application/specs/BLK-002.md
Tests: tests/blk_tests.rs
```

## Commit Checklist

Before committing:
- [ ] Tests pass: `cargo test`
- [ ] Clippy clean: `cargo clippy -- -D warnings`
- [ ] Formatted: `cargo fmt --check`
- [ ] TRACKING.yaml updated
- [ ] VERIFICATION.md updated
- [ ] IMPLEMENTATION_ORDER.md checked off
- [ ] GitNexus index updated: `npx gitnexus analyze`
