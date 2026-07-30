---
name: commit
description: Gate-checked commit workflow — runs lint, type-check, and the full test suite before staging any files, then creates a conventional-commit message from the diff.
---

# `/commit` — Lint, test, then commit

Use this skill whenever you are about to commit changes — whether you are
the coder-agent, tester-agent, or any other agent role. No agent may attempt
a `git commit` without completing every step below first.

> **Non-negotiable.** This gate applies to every agent, every task, every
> branch. "It's a small change" is not an exception. If a step fails, fix
> the root cause before proceeding. Never use `--no-verify` or skip steps.

---

## Step 1 — Lint / format

```bash
# Rust (servers/core, and any future servers/* Rust crate)
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings

# TypeScript / JSON (extension/, once it exists; also biome.json/package.json etc at root)
pnpm exec biome check --write .
```

If clippy or biome report unfixable errors, fix them manually before
continuing.

---

## Step 2 — Type-check

```bash
# TypeScript, once extension/ exists
pnpm --filter <package> typecheck
```

Rust's type-check is implied by `cargo clippy`/`cargo test` — no separate
step.

---

## Step 3 — Full test suite

Run every test that covers the changed code. When in doubt, run all of them.

```bash
cargo test --workspace

# TypeScript, once test files exist
pnpm --filter <package> test
```

All tests must be green (or legitimately marked as an expected-failure with
a concrete reason) before continuing. If a test was passing before your
change and is now failing, fix it — do not silence it.

**Tester-agent scaffold carve-out:** When you are the tester-agent
committing new test files (Mode A of `/test-and-progress`), the
implementation may not exist yet. A test that is expected to fail until the
implementation lands should be marked accordingly with a concrete reason
(`#[ignore = "<epic-id> not yet implemented"]` in Rust,
`test.skip`/`test.todo` with a comment in TS). What is not acceptable: a
test that *errors* (compile failure, panic outside the assertion under
test) or that passes by coincidence against stub code. Fix errors before
committing; let intentional expected-failures through.

---

## Step 4 — Review the diff

```bash
git diff --staged   # if already staged
git diff            # unstaged changes
git status
```

Check for:
- accidentally included files (`.env`, large binaries, generated artifacts
  that should be gitignored)
- debug `println!`/`dbg!`/`console.log`, stray `TODO` comments introduced by you
- leftover merge-conflict markers

---

## Step 5 — Commit

Conventional-commit format, enforced by the `commit-msg` pre-commit hook:

```
<type>(<scope>): <short imperative description>

<optional body: one or two sentences on WHY, not WHAT>

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```

`type` ∈ `feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert`.
`scope` should match the epic's `area:` field when working from a progress
file (e.g. `core`, `lexer`, `parser`, `extension`).

Example:
```
feat(core): CORE-01 comment stripping and continuation joining

Implements the dialect-parameterized line-preprocessing pass from
docs/GRAMMAR.md §1 (ngspice $/// EOL comments vs Xyce ; only, both share the
+-prefix continuation rule).

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```
