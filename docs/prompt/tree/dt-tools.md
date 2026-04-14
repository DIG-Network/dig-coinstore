# Tools

## Required Tools

All three tools MUST be used before writing or modifying code.

### 1. SocratiCode — Semantic Search

**When:** Before reading any source files. Search for concepts, not filenames.

```
codebase_search { query: "coin record storage puzzle hash index" }
codebase_graph_query { filePath: "src/storage/rocksdb.rs" }
```

**Purpose:** Find all files relevant to a requirement before you start. Prevents missed dependencies and accidental duplication.

### 2. Repomix — Context Packing

**When:** Before implementation. Pack the relevant scope.

```bash
npx repomix@latest src -o .repomix/pack-src.xml
npx repomix@latest tests -o .repomix/pack-tests.xml
npx repomix@latest src/storage -o .repomix/pack-storage.xml
```

**Purpose:** Provides full context to the LLM. Without this, implementations miss existing patterns and conventions.

### 3. GitNexus — Dependency Analysis

**When:** Before renaming, moving, or deleting anything.

```bash
npx gitnexus status
npx gitnexus analyze
npx gitnexus impact --symbol CoinRecord
```

**Purpose:** Shows what depends on what. Prevents breaking changes.

## Tool Execution Order

```
SocratiCode  →  understand what exists
Repomix      →  pack full context
GitNexus     →  check impact of planned changes
```

Do not skip any tool. Do not proceed to implementation without all three.
