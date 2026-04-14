# Step 2: Gather Context

Use ALL THREE tools before writing any code.

## 2a. SocratiCode — Semantic Search

Search for concepts related to the requirement:
```
codebase_search { query: "<requirement concept keywords>" }
```

Example for BLK-002 (height continuity):
```
codebase_search { query: "block height validation continuity apply" }
codebase_graph_query { filePath: "src/block_apply.rs" }
```

## 2b. Repomix — Context Packing

Pack the scope relevant to the requirement:
```bash
npx repomix@latest src -o .repomix/pack-src.xml
npx repomix@latest tests -o .repomix/pack-tests.xml
```

For storage-specific requirements:
```bash
npx repomix@latest src/storage -o .repomix/pack-storage.xml
```

## 2c. GitNexus — Impact Check

Check what depends on types/functions you'll create or modify:
```bash
npx gitnexus impact --symbol CoinStore
npx gitnexus impact --file src/block_apply.rs
```

## 2d. Load Repomix into Context

Load the packed context file before any code changes:
```
Read .repomix/pack-src.xml
```

**Gate:** All three tools have been used. You understand what exists, what context is available, and what impact your changes will have.

**Next:** → [dt-wf-test.md](dt-wf-test.md)
