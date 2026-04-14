# Requirements Schema

This document defines the data model and conventions for all requirements in the
dig-coinstore project.

---

## Three-Document Pattern

Each domain has exactly three files in `docs/requirements/domains/{domain}/`:

| File | Purpose |
|------|---------|
| `NORMATIVE.md` | Authoritative requirement statements with MUST/SHOULD/MAY keywords |
| `VERIFICATION.md` | QA approach and verification status per requirement |
| `TRACKING.yaml` | Machine-readable status, test references, and implementation notes |

Each requirement also has a dedicated specification file in
`docs/requirements/domains/{domain}/specs/{PREFIX-NNN}.md`.

---

## Requirement ID Format

**Pattern:** `{PREFIX}-{NNN}`

- **PREFIX**: 2-4 letter domain identifier (uppercase)
- **NNN**: Zero-padded numeric ID starting at 001

| Domain | Directory | Prefix | Description |
|--------|-----------|--------|-------------|
| Crate Structure | `crate_structure/` | `STR` | Crate folder and file layout |
| Crate API | `crate_api/` | `API` | Public types, config, errors, construction |
| Block Application | `block_application/` | `BLK` | Block application pipeline |
| Rollback | `rollback/` | `RBK` | Rollback and reorg recovery |
| Queries | `queries/` | `QRY` | Coin state query methods |
| Storage | `storage/` | `STO` | Persistence backends, column families |
| Merkle Tree | `merkle/` | `MRK` | Sparse Merkle tree and proofs |
| Hints | `hints/` | `HNT` | Hint store and hint queries |
| Performance | `performance/` | `PRF` | Caching, archival, fast sync |
| Concurrency | `concurrency/` | `CON` | Thread safety, MVCC, parallel validation |

**Immutability:** Requirement IDs are permanent. Deprecate requirements rather
than renumbering.

---

## Requirement Keywords

Per RFC 2119:

| Keyword | Meaning | Impact |
|---------|---------|--------|
| **MUST** | Absolute requirement | Blocks "done" status if not met |
| **MUST NOT** | Absolute prohibition | Blocks "done" status if violated |
| **SHOULD** | Expected behavior; may be deferred with rationale | Phase 2+ polish items |
| **SHOULD NOT** | Discouraged behavior | Phase 2+ polish items |
| **MAY** | Optional, nice-to-have | Stretch goals |

---

## Status Values

| Status | Description |
|--------|-------------|
| `gap` | Not implemented |
| `partial` | Implementation in progress or incomplete |
| `implemented` | Code complete, awaiting verification |
| `verified` | Implemented and verified per VERIFICATION.md |
| `deferred` | Explicitly postponed with rationale |

---

## TRACKING.yaml Item Schema

```yaml
- id: PREFIX-NNN           # Requirement ID (required)
  section: "Section Name"  # Logical grouping within domain (required)
  summary: "Brief title"   # Human-readable description (required)
  status: gap              # One of: gap, partial, implemented, verified, deferred
  spec_ref: "docs/requirements/domains/{domain}/specs/{PREFIX-NNN}.md"
  tests: []                # Array of test names or ["manual"]
  notes: ""                # Implementation notes, blockers, or evidence
```

---

## Testing Requirements

All dig-coinstore requirements MUST be tested using:

### 1. Unit Tests (MUST)

All storage, query, and state transition paths MUST be tested with:

1. **Create** a `CoinStore` instance with a temporary directory
2. **Initialize** genesis state with known coins
3. **Apply** blocks with known additions and removals
4. **Query** and verify resulting state matches expectations
5. **Rollback** and verify state reverts correctly

### 2. Integration Tests (MUST for multi-domain requirements)

Tests MUST demonstrate correct interaction between domains by:
- Applying sequences of blocks and verifying query results
- Rolling back and re-applying blocks (fork simulation)
- Snapshot/restore round-trips
- Concurrent read/write scenarios

### 3. Benchmark Tests (SHOULD for performance requirements)

Performance-related requirements (PRF domain) SHOULD include benchmarks:
- Block application throughput at various sizes
- Point lookup latency (cache hit vs miss)
- Puzzle hash scan latency at various cardinalities

### 4. Required Test Infrastructure

```toml
# Cargo.toml [dev-dependencies]
tempfile = "3"
rand = "0.8"
```

```rust
use dig_coinstate::{CoinStore, CoinStoreConfig, CoinStoreError, BlockData, CoinAddition};
use chia::protocol::{Bytes32, Coin};
use tempfile::TempDir;
```

---

## Master Spec Reference

All requirements trace back to the SPEC:
[SPEC.md](../../resources/SPEC.md)
