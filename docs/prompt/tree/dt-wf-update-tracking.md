# Step 6: Update Tracking

Update ALL THREE tracking files. This is not optional.

## 6a. TRACKING.yaml

Find the item matching your requirement ID. Update:
- `status: gap` → `status: verified`
- `tests:` → add the test function name(s)
- `notes:` → brief description of what was implemented

## 6b. VERIFICATION.md

Find the row matching your requirement ID. Update:
- Status: `❌` → `✅`
- Verification Approach: describe what the test proves

## 6c. IMPLEMENTATION_ORDER.md

Find the line matching your requirement. Update:
- `- [ ]` → `- [x]`

## Checklist

- [ ] TRACKING.yaml updated (status, tests, notes)
- [ ] VERIFICATION.md updated (status, approach)
- [ ] IMPLEMENTATION_ORDER.md checked off

**Next:** → [dt-wf-commit.md](dt-wf-commit.md)
