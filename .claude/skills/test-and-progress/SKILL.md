---
name: test-and-progress
description: Run epic tests, map outcomes back to progress.core.yaml (or another progress.*.yaml), and update epic status plus review/blocked notes with concrete results.
---

# `/test-and-progress` — Run tests and update the progress file

Use this skill when you are acting as the tester-agent for an epic in a
`progress.*.yaml` file, or when the user asks to rerun tests and update the
progress file with current pass/fail reasons.

## Two operating modes — read the task carefully

### Mode A — write-tests

**Trigger phrases:** "write the tests", "add tests", "scaffold the tests",
"create test files", "implement the test suite".

**What to do:**
1. Write the test files at the `tests.paths` declared in the progress file
   (Rust: `#[cfg(test)] mod tests` in the same file, or `servers/core/tests/*.rs`
   for integration tests; TS: `*.test.ts` next to the source).
2. Follow "How tests should be written" below.
3. If the implementation isn't there yet, mark the test as an expected
   failure with a concrete reason rather than deleting it or leaving it to
   error: Rust `#[ignore = "CORE-XX not yet implemented"]`, TS
   `test.skip("...", () => {})` with a comment naming the epic.
4. Stop. Do **not** run the tests for real (an `#[ignore]`d Rust test won't
   run by default anyway; for TS, don't un-skip it).
5. Do **not** update the progress file status.

**Why:** The coder-agent may not have finished the implementation yet.
Running tests prematurely produces meaningless failures. Marking them
ignored/skipped keeps the suite green for already-complete epics. Write
first; run separately when explicitly asked.

### Mode B — run-tests

**Trigger phrases:** "run the tests", "test", "check the tests", "retest",
"verify the tests", "update the progress file".

**What to do:**
1. Remove the epic's `#[ignore]`/`test.skip` markers before running, if
   present. If none, proceed without change.
2. Run the tests.
3. Record the outcome in the progress file (`review` or `blocked`).
4. Stop. Do **not** fix failing implementation code.

**Critical constraint:** When tests fail, your job ends at writing a precise
`blocked:` note. Do **not** edit implementation files to make tests pass.
That is the coder-agent's responsibility (`/fix-blocked`).

## When NOT to use this skill

- You are implementing code (that is the coder-agent's role, `/implement-epic`).
- The task is only to explain the current progress plan without executing tests.

## Authority

Read in this order:
1. `AGENTS.md`
2. The relevant progress file
3. `docs/GRAMMAR.md`, the section named by the epic's `area:` field

When the code, progress file, and spec disagree, the spec wins. Do not mark
an epic complete based on drifted behavior alone.

## Core rules

### 1. Test the epic's declared surface, not a random larger slice

Start from the epic's `tests.paths` entries. Broaden only if the epic notes
require it, a shared type/module changed, or the local result is ambiguous.

### 2. Distinguish four outcomes

- `review` — the epic's relevant tests are green, or green with intentional
  skips/ignores matching an accepted not-yet-implemented seam.
- `blocked` — tests fail, the environment can't run them, a dependency is
  missing, or the implementation is missing and the epic isn't actually done.
- `complete` — use when: (1) every test in `tests.paths` passes with no
  unintended skips, (2) the broader suite for the epic's area shows no
  regressions, (3) `cargo clippy`/`cargo fmt --check` (or the TS equivalent)
  are clean. The tester-agent sets `complete` directly.
- `planned` / `in_progress` — do not set these after a real test run unless
  explicitly asked to reset status.

There is no `tested` status in this repo. Use `review` or `blocked`.

### 3. An expected-failure marker is not automatically success

If a test is `#[ignore]`/skipped because the implementation module doesn't
exist yet, the epic is still `blocked`, not `review`. If the marker is a
deliberate, documented seam outside the epic's scope and the rest of the
acceptance criteria are satisfied, the epic may still be `review`.

### 4. Environment failures are real blockers

Missing crate, toolchain mismatch, missing `pnpm` package, broken fixture —
record as `blocked` with the exact reason.

## How tests should be written

1. Read `implementation.acceptance` and `tests.acceptance`.
2. Ensure every `tests.acceptance` bullet is covered directly.
3. Add edge cases beyond `implementation.acceptance` (empty input, malformed
   netlist fragments, dialect-boundary cases like the `^`/`log()`
   meaning-flip from `docs/GRAMMAR.md` §4.3) without drifting from the spec.
4. Assert the documented contract, not private implementation details.
5. Rust: prefer table-driven tests (`#[test]` per case, or a `rstest`-style
   parametrized case list) for grammar rules that have many small variants
   (e.g. one test per device letter, one per numeric-suffix case) — this
   mirrors the shape of `docs/GRAMMAR.md`'s own tables.

## How to run tests

```bash
cargo test -p spice-core                     # smallest correct scope
cargo test -p spice-core lexer::             # narrow to a module
cargo test --workspace                       # broaden when needed

# once extension/ exists:
pnpm --filter <package> test
```

Capture, per epic: pass count if green, fail count if red, ignored/skip
count when relevant, and the first concrete blocking reason when the suite
can't run at all.

## How to update the progress file

### Notes format

Green:
```yaml
status: review
review: >
  Retested YYYY-MM-DD. All 14 tests in servers/core/src/lexer.rs pass.
```

Blocked by a failure:
```yaml
status: blocked
blocked: >
  Retested YYYY-MM-DD. 1/6 tests fail in servers/core/src/lexer.rs:
  test_ngspice_continuation_backslash — preprocess() does not join a
  line ending in `\\`, only `+`-prefixed continuations. docs/GRAMMAR.md §1.
```

Blocked by environment:
```yaml
status: blocked
blocked: >
  Retested YYYY-MM-DD. `cargo test -p spice-core` fails to compile:
  crate `spice-core` not found — Cargo.toml workspace members list is stale.
```

### Writing good blocked notes

Include the exact test name, the exact assertion/error, and any API
mismatch, so a coder-agent can act without rerunning the investigation.
Avoid "tests failing" / "implementation incomplete" — those aren't
actionable.

## Do not lose prior signal

When editing a progress entry: replace outdated `review:`/`blocked:` text
with the latest result; keep the rest of the epic unchanged; do not rewrite
`summary`, `acceptance`, or `paths` unless the user asked for it.

## Related docs

- `AGENTS.md`
- `.claude/skills/implement-epic/SKILL.md`
- `.claude/skills/fix-blocked/SKILL.md`
- `docs/GRAMMAR.md`
