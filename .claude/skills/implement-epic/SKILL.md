---
name: implement-epic
description: Implement the code for one epic from progress.core.yaml (or another progress.*.yaml), following its acceptance criteria, then mark it review-ready. Counterpart to /test-and-progress.
---

# `/implement-epic` — Implement an epic

Use this skill when you are acting as the **coder-agent** for an epic in a
`progress.*.yaml` file.

This skill defines:

- how to read an epic's contract before writing a single line
- what to implement and what to leave for the tester-agent
- how to run the gates that gate a `review` status
- how to update the progress file when done or blocked

## Role boundary — read this first

This skill owns the **implementation** side only.

| You do | You do not do |
|---|---|
| Implement the files in `implementation.paths` | Write the test files at `tests.paths` |
| Run lint, clippy/typecheck, and the existing suite | Write new tests from scratch |
| Mark status `in_progress` then `review` | Mark status `complete` |
| Write a `review:` note with concrete evidence | Run tester-agent steps |

The tester-agent (skill `/test-and-progress`) independently validates the
tests and marks the epic `complete`. Do not collapse the two roles into one
pass.

**Exception:** if the epic's `implementation.paths` include a test file
(rare — e.g. the epic's entire deliverable is a golden-file fixture set),
write that file. Document it explicitly in your `review:` note.

---

## Step 0 — Read before writing

1. `AGENTS.md` — invariants, gate commands, commit conventions
2. `.claude/skills/commit/SKILL.md` — the gate you must pass before marking `review`
3. The relevant progress file — find your epic by ID
4. `docs/GRAMMAR.md`, the section named by the epic's `area:` field

When the spec and the progress file disagree, **the spec wins**. Raise the
conflict in your `review:` note but implement to the spec.

---

## Step 1 — Confirm preconditions

1. **Check `depends_on`.** Every epic listed there must be `status: complete`.
   If any is not, stop and record a `blocked:` note explaining which
   dependency is not ready.
2. **Verify the implementation paths are free.** For each path in
   `implementation.paths`, check it either doesn't exist yet or can be
   safely modified without colliding with another in-progress epic.
3. **Set status to `in_progress`** in the progress file immediately.

---

## Step 2 — Understand the contract

Read `implementation.acceptance` line by line. For each bullet, identify the
files it touches and the exact interface it specifies (function/type/field
names, error variants, diagnostic messages) — these are not suggestions,
they're what the tester-agent will verify.

Do not start implementing until you can describe, in one sentence per
bullet, what concrete code change each bullet requires.

---

## Step 3 — Implement

### General rules

- Implement exactly what the acceptance bullets specify. Do not add
  features, refactor surrounding code, or introduce abstractions not
  required by the epic.
- Default to writing no comments. Only add one when a hidden invariant or
  subtle workaround would surprise a future reader.
- Never bypass pre-commit hooks (`--no-verify`). Fix the root cause.
- Follow the core+overlay architecture invariant in `AGENTS.md` — dialect
  handling goes in a per-dialect table/module, never as `if dialect == ...`
  branches inside shared parsing logic.

### Rust epics (`servers/core`, and any future `servers/*` crate)

```bash
cargo build -p spice-core          # or the relevant crate
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

- New modules go where `implementation.paths` says; add `pub mod` wiring in
  `lib.rs` as needed.
- Prefer `Result<T, E>` with a crate-local error enum over `panic!`/`unwrap`
  for anything that can fail on malformed netlist input — a parser must
  degrade to a diagnostic, not a crash, on bad input. `unwrap`/`expect` are
  fine only for invariants the type system already guarantees.
- Keep dialect-specific tables (device letters, operators, functions) as
  data (`match`/const tables/a `DialectOverlay` trait impl per dialect), not
  scattered conditionals.

### TypeScript epics (`extension/`, once it exists)

```bash
pnpm --filter <package> typecheck
pnpm exec biome check --write extension/
```

### Both

- After the implementation compiles and the existing test suite is green,
  stop. Do not write new test files — those belong in `tests.paths` and are
  the tester-agent's work.
- If the tester-agent pre-scaffolded `tests.paths` with expected-failure
  markers (`#[ignore = "..."]` in Rust, `test.skip`/`test.todo` in TS), those
  may still be marked that way after your implementation if the epic isn't
  fully done — do not remove the markers yourself; the tester-agent does
  that via `/test-and-progress` Mode B.

---

## Step 4 — Run the commit gates

Follow the `/commit` skill exactly. All gates must be green before you
commit.

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# once extension/ exists:
pnpm exec biome check --write .
pnpm --filter <package> typecheck
pnpm --filter <package> test
```

If any gate fails: fix the root cause in the implementation files, re-run
the full gate sequence from the beginning. Do not commit with a failing
gate.

---

## Step 5 — Commit

```
feat(<scope>): <EPIC-ID> <short imperative description>

<optional body: one or two sentences on WHY, not WHAT>

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```

Scope should match the epic's `area:` field (e.g. `lexer`, `parser`,
`symbols`, `extension`).

Example:
```
feat(lexer): CORE-01 comment stripping and continuation joining

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```

---

## Step 6 — Update the progress file

```yaml
status: review
review: >
  Implemented YYYY-MM-DD. All implementation.acceptance bullets satisfied.
  Gate: cargo clippy clean, cargo fmt clean, N/N cargo test pass.
  <note any acceptance bullet that required a non-obvious decision>
```

If blocked at any step:

```yaml
status: blocked
blocked: >
  YYYY-MM-DD. Blocked on <exact reason>. <what would unblock it>.
```

---

## Step 7 — Commit the progress file update

```bash
git add progress.core.yaml   # or the relevant progress file
git commit -m "chore(progress): mark <EPIC-ID> review"
```

The tester-agent will then pick up the epic, write and run the tests, and
either mark it `complete` or return it to `blocked`.

---

## Avoiding common mistakes

| Mistake | Correct approach |
|---|---|
| Writing tests from `tests.paths` | Stop at implementation; leave tests.paths to the tester-agent |
| Marking `complete` | Only the tester-agent marks `complete`; you mark `review` |
| Bypassing `--no-verify` | Fix the hook failure in the code |
| Adding a feature not in `implementation.acceptance` | Remove it; scope creep obscures test signal |
| Using `#[allow(...)]` without a comment | Add an inline comment explaining the unavoidable suppression |
| Baking dialect logic into shared parsing code | Move it into a per-dialect overlay table (see AGENTS.md invariant) |
| Implementing across multiple epics in one commit | One commit per epic maximum |
| Editing `tests.paths` files to make tests pass | That is the tester-agent's domain |

---

## Related docs

- `.claude/skills/commit/SKILL.md` — gate procedure
- `.claude/skills/test-and-progress/SKILL.md` — tester counterpart
- `docs/GRAMMAR.md`
- `progress.core.yaml`
