# Step 7: Commit

## Commit

```bash
git add -A
git commit -m "PREFIX-NNN: summary

Details of implementation.

Requirement: PREFIX-NNN
Spec: docs/requirements/domains/{domain}/specs/PREFIX-NNN.md
Tests: tests/{domain}_tests.rs"
```

## Push

```bash
git push origin main
```

## Update GitNexus

```bash
npx gitnexus analyze
```

## Loop

Go back to [dt-wf-select.md](dt-wf-select.md) and pick the next requirement.

## Checklist

- [ ] Committed with proper message format
- [ ] Pushed to origin
- [ ] GitNexus index updated
- [ ] Ready for next requirement
