# AGENTS.md — spice-lsp

Agent guidance for this repo. Skills in `.claude/skills/` read this file
first; read it yourself before touching code.

## What this is

A dialect-agnostic SPICE netlist parser/analysis core (Rust), consumed two
ways:
1. **Primary target** — embedded directly as a Cargo dependency inside
   `general-simulator`'s Tauri backend (`apps/desktop/src-tauri`), exposed to
   its Monaco-based netlist editor via `#[tauri::command]`. No LSP protocol
   involved for this path — native function calls.
2. **Secondary target** — later wrapped with `tower-lsp` as a standalone
   binary speaking real LSP over stdio, for VS Code / Neovim / any other
   LSP-capable editor. Same core, thin adapter.

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
