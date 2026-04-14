# Step 4: Implement

Make the failing test pass.

## Rules

1. **Chia crates first** — use existing types and functions from the chia ecosystem before writing your own.
2. **Minimal code** — implement only what the requirement asks for. No extras.
3. **StorageBackend trait** — all storage access goes through the trait, not direct DB calls.
4. **Comments** — every public function, struct, and non-obvious block of code gets a comment explaining:
   - What it does
   - Why it exists (link to requirement/spec)
   - Key decisions and rationale
   - Semantic links to related code

## Implementation Order

1. Define types (structs, enums) if needed
2. Implement the trait/function the test exercises
3. Wire it into the CoinStore public API
4. Run the test — it should pass now

## Comment Template

```rust
/// Apply a block's state changes to the coinstate.
///
/// This is the primary state transition for the coinstate. It takes pre-validated
/// block data (additions, removals, coinbase, hints) and atomically updates all
/// persistent state including coin records, indices, hints, and the Merkle tree.
///
/// # Validation (Phase 1)
/// - Height continuity: BLK-002
/// - Parent hash: BLK-003
/// - Reward coins: BLK-004
/// - Removal existence: BLK-005
/// - Addition uniqueness: BLK-006
///
/// # Mutation (Phase 2)
/// All writes are accumulated into a WriteBatch (STO-005) and committed
/// atomically with a single WAL fsync.
///
/// See: SPEC.md Section 5, Chia coin_store.py:105-178
pub fn apply_block(&mut self, block: BlockData) -> Result<ApplyBlockResult, CoinStoreError> {
```

## Gate

- [ ] Test passes
- [ ] No compilation warnings
- [ ] Code has comprehensive comments

**Next:** → [dt-wf-validate.md](dt-wf-validate.md)
