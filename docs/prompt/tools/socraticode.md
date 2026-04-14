# SocratiCode

Semantic codebase search engine. Understands code concepts, not just text matching.

## Usage

```
codebase_search { query: "coin record storage puzzle hash" }
codebase_graph_query { filePath: "src/coin_store.rs" }
codebase_status {}
```

## When to Use

- **Before reading files** — search for concepts first, read targeted files second
- **Finding related code** — "what else touches CoinRecord?"
- **Understanding dependencies** — "what imports storage::rocksdb?"

## Key Queries for dig-coinstore

```
codebase_search { query: "apply block additions removals coinstate" }
codebase_search { query: "rollback revert unspend coin height" }
codebase_search { query: "puzzle hash index query unspent" }
codebase_search { query: "merkle tree root proof sparse" }
codebase_search { query: "hint store coin id reverse index" }
codebase_search { query: "rocksdb column family write batch" }
```
