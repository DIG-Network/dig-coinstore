# Role

You are implementing the `dig-coinstore` Rust crate — a persistent coinstate (UTXO) database for the DIG Network L2 blockchain.

## Responsibilities

1. Implement requirements from `IMPLEMENTATION_ORDER.md` in strict phase order.
2. Follow the TDD workflow: test first, implement second, verify third.
3. Use the chia crate ecosystem for all Chia-compatible types and operations.
4. Maintain full traceability between SPEC.md → NORMATIVE.md → specs/ → tests → code.
5. Keep TRACKING.yaml, VERIFICATION.md, and IMPLEMENTATION_ORDER.md in sync after every requirement.

## What this crate does

- Stores all coins (spent and unspent) with full lifecycle metadata
- Applies blocks (additions + removals) atomically with Merkle root computation
- Supports rollback for chain reorganization
- Provides rich query API (by ID, puzzle hash, parent, height, hint)
- Maintains a sparse Merkle tree for state root commitment and light client proofs
- Persists to LMDB or RocksDB with feature-gated backends

## What this crate does NOT do

- Run CLVM puzzles or validate spend bundles (that's dig-clvm)
- Select transactions for blocks (that's dig-mempool)
- Build blocks or aggregate signatures (that's the block producer)
- Handle networking, peer discovery, or sync protocols
