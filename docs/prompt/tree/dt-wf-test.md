# Step 3: Write Failing Test (TDD)

**Write the test BEFORE the implementation.** The test defines the contract.

## Process

1. Open the spec file `specs/PREFIX-NNN.md`
2. Read the **Verification / Test Plan** section
3. Create the test file `tests/{domain}_tests.rs` (if it doesn't exist)
4. Write each test from the test plan as a `#[test]` function
5. Add comprehensive comments:
   - What the test verifies
   - Which requirement it satisfies
   - How it proves the requirement is met
   - Link to the spec file

## Test Template

```rust
/// Verifies requirement PREFIX-NNN: {summary}
///
/// This test proves that {description of what is being tested}.
///
/// Spec: docs/requirements/domains/{domain}/specs/PREFIX-NNN.md
/// NORMATIVE: docs/requirements/domains/{domain}/NORMATIVE.md#PREFIX-NNN
/// SPEC.md: Section X.Y
#[test]
fn vv_req_prefix_nnn_description() {
    // Setup: create CoinStore with temp directory
    let dir = TempDir::new().unwrap();
    let store = CoinStore::new(dir.path()).unwrap();

    // Action: exercise the requirement
    // ...

    // Assert: verify the requirement is satisfied
    // ...
}
```

## Gate

- [ ] Test file exists
- [ ] Test compiles
- [ ] Test FAILS (because the implementation doesn't exist yet)

**Next:** → [dt-wf-implement.md](dt-wf-implement.md)
