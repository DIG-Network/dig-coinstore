# dig-coinstore Requirements

This directory contains the formal requirements for the dig-coinstore crate,
following the same two-tier requirements structure as dig-mempool
with full traceability.

## Quick Links

- [SCHEMA.md](SCHEMA.md) — Data model and conventions
- [REQUIREMENTS_REGISTRY.yaml](REQUIREMENTS_REGISTRY.yaml) — Central domain registry
- [domains/](domains/) — All requirement domains

## Structure

```
requirements/
├── README.md                    # This file
├── SCHEMA.md                    # Data model and conventions
├── REQUIREMENTS_REGISTRY.yaml   # Central registry
├── IMPLEMENTATION_ORDER.md      # Phased implementation checklist
└── domains/
    ├── crate_structure/         # STR-* Crate folder and file layout
    ├── crate_api/               # API-* Public types, config, errors, construction
    ├── block_application/       # BLK-* Block application pipeline
    ├── rollback/                # RBK-* Rollback and reorg recovery
    ├── queries/                 # QRY-* Coin state query methods
    ├── storage/                 # STO-* Persistence backends, column families
    ├── merkle/                  # MRK-* Sparse Merkle tree, proofs
    ├── hints/                   # HNT-* Hint store and hint queries
    ├── performance/             # PRF-* Caching, archival, fast sync
    └── concurrency/             # CON-* Thread safety, MVCC, parallel validation
```

## Three-Document Pattern

Each domain contains:

| File | Purpose |
|------|---------|
| `NORMATIVE.md` | Authoritative requirement statements (MUST/SHOULD/MAY) |
| `VERIFICATION.md` | QA approach and status per requirement |
| `TRACKING.yaml` | Machine-readable status, tests, and notes |

## Specification Files

Individual requirement specifications are in each domain's `specs/` subdirectory:

```
domains/
├── crate_structure/specs/         # STR-001.md through STR-NNN.md
├── crate_api/specs/               # API-001.md through API-NNN.md
├── block_application/specs/       # BLK-001.md through BLK-NNN.md
├── rollback/specs/                # RBK-001.md through RBK-NNN.md
├── queries/specs/                 # QRY-001.md through QRY-NNN.md
├── storage/specs/                 # STO-001.md through STO-NNN.md
├── merkle/specs/                  # MRK-001.md through MRK-NNN.md
├── hints/specs/                   # HNT-001.md through HNT-NNN.md
├── performance/specs/             # PRF-001.md through PRF-NNN.md
└── concurrency/specs/             # CON-001.md through CON-NNN.md
```

## Reference Document

All requirements are derived from:
- [SPEC.md](../resources/SPEC.md) — dig-coinstore specification

## Requirement Count

| Domain | Prefix | Count |
|--------|--------|-------|
| Crate Structure | STR | 6 |
| Crate API | API | 10 |
| Block Application | BLK | 14 |
| Rollback | RBK | 7 |
| Queries | QRY | 11 |
| Storage | STO | 8 |
| Merkle Tree | MRK | 6 |
| Hints | HNT | 6 |
| Performance | PRF | 9 |
| Concurrency | CON | 4 |
| **Total** | | **81** |
