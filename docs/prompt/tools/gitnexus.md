# GitNexus

Knowledge graph and dependency analysis tool.

## Usage

```bash
npx gitnexus status           # Check index freshness
npx gitnexus analyze          # Rebuild index
npx gitnexus impact --symbol CoinStore
npx gitnexus impact --file src/storage/rocksdb.rs
```

## When to Use

- **Before renaming or moving** — check what depends on a symbol
- **Before deleting** — ensure nothing references the target
- **After implementation** — update the index for the next cycle

## Key Symbols for dig-coinstore

```bash
npx gitnexus impact --symbol CoinStore
npx gitnexus impact --symbol CoinRecord
npx gitnexus impact --symbol StorageBackend
npx gitnexus impact --symbol SparseMerkleTree
npx gitnexus impact --symbol BlockData
```
