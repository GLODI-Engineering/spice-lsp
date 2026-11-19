# AGENTS.md — spice-lsp

Agent guidance for this repo. Skills in `.claude/skills/` read this file
first; read it yourself before touching code.

## What this is

A dialect-agnostic SPICE netlist parser/analysis core (Rust), consumed two
ways:
1. **Primary target** — linked directly as a Cargo dependency. `general-mna`
   turns the parse result into modified-nodal-analysis matrices, and
   `general-simulator`'s `dae-runtime` and `general-simulator-cli` depend on
   this crate as well. Native function calls, no protocol.
2. **Secondary target** — later wrapped with `tower-lsp` as a standalone
   binary speaking real LSP over stdio, for editors that cannot link a Rust
   library. Same core, thin adapter.

The Tauri desktop application this section used to name as the primary target
no longer exists, and `general-simulator` has no `apps/desktop/src-tauri`.

See `docs/GRAMMAR.md` for the netlist grammar reference (ngspice/Xyce, common
vs. dialect-specific) — that document is the closest thing this project has
to a spec, and epics under `area: core` should point to its sections.

## Layout

```
servers/core/     Rust crate `spice_core` — the dialect-agnostic engine
servers/lsp/      (future) tower-lsp binary wrapping spice_core
extension/        (future) VS Code client extension (TypeScript)
docs/GRAMMAR.md   netlist grammar reference — the closest thing to a spec
docs/gotchas/      institutional-memory bug records, see /record-gotcha
progress.core.yaml epic plan for servers/core
```

## Architecture invariant

Per `docs/GRAMMAR.md` §9.2 (adopted decision): model the grammar as a common
core plus per-dialect overlay tables (device letters, operators, built-in
functions) — never as two independent grammars, and never as one grammar
with dialect checks scattered through the logic. New device/statement/
operator support belongs in a dialect overlay table, not as an `if dialect
== ...` branch inside shared parsing logic.

## Commit cadence

Commit after every epic is implemented and all gates pass —
one commit per epic, not one commit at the end of a long session.
An epic is done when its `implementation.acceptance` items are met,
**all tests (including the epic's own `tests.acceptance` list) pass**,
and `cargo fmt` + `cargo clippy -D warnings` + `cargo test --workspace`
are all green. Never commit an epic that hasn't been tested.

## Development cycle

The gates below say how code is written. This says how the work is recorded, which matters for
the same reason: someone reads it later, and what they can reconstruct is what was written down.

1. **A large instruction becomes a milestone**, named for the outcome rather than the task
   ("Rules and diagnostics", not "implement the rules spec").
2. **One issue per deliverable phase or decision**, each stating the problem rather than the
   patch. The instruction as a whole — its reasoning, what it leaves open — is an issue in that
   milestone too, and the rest reference it with `Part of #N`.
3. **A branch and a pull request per issue**, the description ending `Closes #N`, assigned to the
   milestone. Nothing lands on the default branch directly.
4. **The merge closes the issue.** Closing it by hand loses the link between the issue and the
   commit that answered it, which is the whole point of the exercise.
5. **Anything found but not done becomes its own issue**, not a sentence in a pull request body.
   A merged pull request's prose is invisible; an open issue is not. A deliberate omission with
   a reason is worth more than a half-finished corner of a change.
6. **Gates run before pushing**, never after. A red branch spends someone's CI and tells the
   reader the author did not look.

Finish a repository's open pull request before starting new work in it, so review has one thing
to hold at a time.

## Gates (run before every commit — see `/commit`)

Rust (`servers/core`, and any future `servers/*` Rust crate):
```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

TypeScript (`extension/`, once it exists):
```bash
pnpm exec biome check --write .
pnpm --filter <package> typecheck
pnpm --filter <package> test
```

Never use `--no-verify`. Fix the root cause of a failing hook or gate.

## Commit convention

Conventional commits, enforced by the `commit-msg` pre-commit hook:
`feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert(scope): ...`.
Scope should match the epic's `area:` field (e.g. `core`, `lexer`, `parser`,
`extension`).

## Two-agent-track convention (progress files)

Epics in `progress.*.yaml` split into an `implementation` track
(coder-agent, via `/implement-epic`) and a `tests` track (tester-agent, via
`/test-and-progress`). See `.claude/skills/write-progress/SKILL.md` for the
full format.

## Skills

- `/write-progress` — author or extend a `progress.*.yaml` epic plan
- `/implement-epic` — coder-agent: implement one epic
- `/test-and-progress` — tester-agent: write or run an epic's tests
- `/fix-blocked` — coder-agent: resolve a `blocked` epic
- `/record-gotcha` — capture an off-epic bug for future agents
- `/commit` — gate-checked commit workflow

These were adapted from the `general-simulator` project's skill set, trimmed
to what applies to a much smaller Rust+TS project (no Archon multi-agent
orchestration, no functional-block scaffolding, no schematic-symbol import —
those were specific to `general-simulator`'s own architecture).
