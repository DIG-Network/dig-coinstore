# Repomix

Context packing tool for LLM consumption. Concatenates source files into a single XML document.

## Usage

```bash
# Full source
npx repomix@latest src -o .repomix/pack-src.xml

# Tests only
npx repomix@latest tests -o .repomix/pack-tests.xml

# Specific module
npx repomix@latest src/storage -o .repomix/pack-storage.xml
npx repomix@latest src/merkle -o .repomix/pack-merkle.xml
npx repomix@latest src/cache -o .repomix/pack-cache.xml
```

## When to Use

- **Before implementation** — pack the relevant scope so you have full context
- **Before writing tests** — pack existing tests to follow conventions
- **Before refactoring** — pack the module being changed

## Loading into Context

After packing, load the file:
```
Read .repomix/pack-src.xml
```

This gives the LLM full visibility into the codebase state.
