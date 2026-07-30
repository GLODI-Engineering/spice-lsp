---
name: fix-blocked
description: Coder-agent workflow for resolving a blocked epic — read the blocked note from the progress file, fix the implementation narrowly, re-run gates, and return the epic to review.
---

# `/fix-blocked` — Resolve a blocked epic

Use this skill when an epic has `status: blocked` in a `progress.*.yaml`
file with a `blocked:` note written by the tester-agent, and your job is to
fix the implementation so the tests pass.

This is the **coder-agent's response to tester-agent feedback** — narrower
than `/implement-epic`: you're fixing a specific reported failure, not
building from scratch.

## When NOT to use this skill

- The epic is `planned` or `in_progress` and has never been attempted — use
  `/implement-epic` instead.
- The `blocked:` note says a *dependency* is missing (another epic not
  complete, a crate/toolchain missing) — resolve the dependency first, not
  a code fix.
- The tester-agent hasn't written a `blocked:` note yet — wait for
  `/test-and-progress` to run and report before touching code.

---

## Role boundary

| You do | You do not do |
|---|---|
| Fix implementation files in `implementation.paths` | Edit test files in `tests.paths` |
| Fix only what the `blocked:` note describes | Refactor, rename, or improve surrounding code |
| Re-run the gates | Rewrite the tests to pass against broken code |
| Update the epic to `review` | Mark the epic `complete` |

**Critical:** If a test appears to be wrong (asserting a behavior
`docs/GRAMMAR.md` doesn't require, testing an undocumented side effect),
don't edit the test. Record the discrepancy in the `blocked:` note instead —
the tester-agent corrects wrong tests; you correct wrong code.

---

## Step 0 — Read the blocked note carefully

A well-written note tells you: which test failed (name + file), why (the
assertion violated, panic message, or compile error), and what the test
expects. If the note is vague, use `/test-and-progress` Mode B to get a
precise one first — do not guess.

---

## Step 1 — Confirm it is a code bug

1. Is the note describing a defect in `implementation.paths`? Proceed.
2. Is it describing a missing dependency (crate not in `Cargo.toml`, an
   epic this one depends on not yet complete)? Stop, record the dependency
   in a `blocked:` update, wait.
3. Does the test's assertion contradict `docs/GRAMMAR.md`? If so, don't
   patch implementation to match a wrong test — flag it and let the
   tester-agent confirm.
4. Do all `depends_on` epics have `status: complete`? If not, name the
   unmet dependency in `blocked:`.

---

## Step 2 — Reproduce the failure

```bash
cargo test -p spice-core test_name -- --exact --nocapture

# TypeScript
pnpm --filter <package> test -t "test name"
```

Confirm the failure matches the note. If it now passes (someone else fixed
it), update the progress file to `review` without changing code.

---

## Step 3 — Fix narrowly

Fix only what the `blocked:` note describes. Do not refactor outside the
reported failure path, add features not in `implementation.acceptance`,
rename identifiers (unless the mismatch *is* the bug), or touch
`tests.paths` files.

A narrow fix has a small diff. If your fix requires changing many files,
pause and re-read Step 1 — you may be solving the wrong problem.

---

## Step 4 — Run the gates

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p spice-core test_name    # the failing test first
cargo test --workspace                # full suite — confirm no regressions
```

If the originally failing test now passes but a different test breaks, fix
the regression before committing.

**Expected-failure carve-out:** tests that were intentionally `#[ignore]`d
before your fix may start passing after it — that's fine, remove the
marker. If an unrelated non-ignored test starts failing, that's a
regression — fix it.

---

## Step 5 — Commit

```
fix(<scope>): <EPIC-ID> resolve <short description of what was wrong>

<optional body: one sentence on why this was wrong and what the fix does>

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```

---

## Step 6 — Update the progress file

```yaml
status: review
review: >
  Fixed YYYY-MM-DD. <EPIC-ID>: <short description of the fix>.
  Gate: clippy clean, fmt clean, N/N cargo test pass.
  Previously blocked: <one line from the old note>.
```

Remove the old `blocked:` field entirely. Leave all other fields unchanged.

---

## Step 7 — Commit the progress file update

```bash
git add progress.core.yaml   # or the relevant progress file
git commit -m "chore(progress): mark <EPIC-ID> review after fix"
```

---

## Avoiding common mistakes

| Mistake | Correct approach |
|---|---|
| Editing tests to match broken code | Fix the code; flag spec ambiguity if needed |
| Fixing beyond the `blocked:` note's scope | Narrow the diff |
| Marking `complete` | Only the tester-agent marks complete |
| Committing with a failing test | Fix the regression first |
| Guessing when the note is vague | Re-run via `/test-and-progress` Mode B first |
| Using `--no-verify` | Fix the hook failure in the code |

---

## Related docs

- `.claude/skills/implement-epic/SKILL.md`
- `.claude/skills/test-and-progress/SKILL.md`
- `.claude/skills/write-progress/SKILL.md`
- `.claude/skills/commit/SKILL.md`
