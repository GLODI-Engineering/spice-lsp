---
name: write-progress
description: Author or extend a progress.*.yaml epic plan — format, field semantics, two-agent-track convention, and rules for adding epics, phases, and status notes.
---

# `/write-progress` — Author or extend a progress file

Use this skill when you are:

- Starting a new epic plan for a project (new `progress.*.yaml` file).
- Adding one or more epics to an existing plan.
- Splitting a plan across phases.
- Revising the acceptance criteria of a `planned` epic before work begins.

## When NOT to use this skill

- You are updating `status`, `review:`, or `blocked:` after running tests —
  that is `/test-and-progress` (Mode B).
- You are updating `status` after implementing code — that is
  `/implement-epic` Step 6.
- You are fixing a blocked epic — that is `/fix-blocked`.

---

## What a progress file is for

A progress file is the **single source of truth for what needs to be built
and who is building it**. It is not a Gantt chart, a changelog, or a test
log. It answers three questions per epic:

1. What is the contract? (`implementation.acceptance` + `tests.acceptance`)
2. Which files does each track own? (`implementation.paths`, `tests.paths`)
3. What is the current state? (`status` + `review:` / `blocked:` notes)

`docs/GRAMMAR.md` (and any future `docs/specs/*.md`) says *how* to build.
The progress file says *what* and *when*, and tracks *whether it's done*.

---

## File naming and location

```
progress.<scope>.yaml
```

Examples for this repo: `progress.core.yaml` (the Rust `spice_core` crate),
`progress.extension.yaml` (the VS Code TS client, once it exists),
`progress.lsp.yaml` (the `tower-lsp` binary wrapper, once it exists).

Keep at the repo root next to `AGENTS.md`. One file per major scope; don't
make a file per phase or per sprint.

---

## Top-level header

```yaml
# progress.<scope>.yaml — spice-lsp <scope> v<N> epic plan
#
# Scope:  <one line: what this file covers>
# Spec:   docs/GRAMMAR.md <or the relevant section>
# Cutoff: <what v1 ships>
#
# ── Conventions ─────────────────────────────────────────────────────────────
# Two agent tracks per epic:
#   - implementation.owner: coder-agent    — writes the code
#   - tests.owner:          tester-agent   — writes and runs the tests
# The two tracks may run in parallel once the epic's contract is fixed.
# An epic is COMPLETE only when:
#   - all implementation.acceptance items pass
#   - all tests.acceptance items pass
#   - the test suite at tests.paths is green
#   - the gates in AGENTS.md relevant to this scope are green
#
# Status values: planned | in_progress | review | complete | blocked
# depends_on:   list of epic ids that must be status: complete before this starts
# ────────────────────────────────────────────────────────────────────────────

version: 1
target: <scope>-v1
spec: docs/GRAMMAR.md
```

---

## Phases

Group related epics into named phases. Phases are informational — they do
not enforce ordering (use `depends_on` for that).

```yaml
phases:
  - id: P0
    name: Foundation
    epics: [CORE-01, CORE-02, CORE-03]
  - id: P1
    name: ngspice dialect overlay
    epics: [CORE-04, CORE-05]
```

Rules:
- Every epic id must appear in exactly one phase.
- Phase ids use a prefix matching the file scope: `P0`/`P1` is fine for a
  single-scope file; use a distinguishing prefix (`LP0`, `EP0`) if multiple
  progress files need to cross-reference phases unambiguously.
- Do not name a phase "Done" or "Backlog" — use `status` fields instead.

---

## Epic structure

```yaml
epics:

  - id: CORE-01
    title: <Short noun phrase — what is being built>
    area: <slug — matches a section of docs/GRAMMAR.md or another spec>
    status: planned
    depends_on: []
    summary: >
      One paragraph. Explain WHAT and WHY. Include the v1 scope boundary
      if relevant. Do not describe HOW — that belongs in the spec.
    implementation:
      owner: coder-agent
      paths:
        - servers/core/src/file_to_create_or_modify.rs
      acceptance:
        - <Concrete, falsifiable statement about code behavior.>
        - <Each bullet is something the tester-agent can independently verify.>
        - <Prefer "fn foo() returns X when Y" over "foo works correctly".>
    tests:
      owner: tester-agent
      paths:
        - servers/core/src/file_to_create_or_modify.rs   # #[cfg(test)] mod tests, or:
        - servers/core/tests/core01_integration.rs
      acceptance:
        - <Test scenario 1 — maps to one or more implementation.acceptance bullets.>
        - <Test scenario 2.>
        - <Edge case worth covering that is not in implementation.acceptance.>
```

### Required fields

| Field | Type | Description |
|---|---|---|
| `id` | string | Unique within the file. Convention: `SCOPE-NN` (e.g. `CORE-01`, `EXT-04`). |
| `title` | string | Short noun phrase. Verb-less; describes the artifact, not the action. |
| `area` | string | Slug mapping to a `docs/GRAMMAR.md` section or another spec doc. |
| `status` | enum | One of: `planned`, `in_progress`, `review`, `complete`, `blocked`. New epics start `planned`. |
| `depends_on` | list | Epic ids (same or other file). Empty list if none. |
| `summary` | block scalar | One paragraph. WHAT + WHY. Not HOW. |
| `implementation.owner` | string | Always `coder-agent`. |
| `implementation.paths` | list | Files the coder-agent must create or modify. Be exhaustive. |
| `implementation.acceptance` | list | Concrete, falsifiable bullets. These are the coder's deliverables. |
| `tests.owner` | string | Always `tester-agent`. |
| `tests.paths` | list | Files the tester-agent must create/extend. Rust: usually `#[cfg(test)]` in the same file, or `servers/core/tests/*.rs` for integration tests. |
| `tests.acceptance` | list | Test scenarios. Must cover every `implementation.acceptance` bullet. |

### Optional fields

```yaml
    review: >
      Retested YYYY-MM-DD. All N tests in servers/core/src/lexer.rs pass.
      Gate: cargo clippy clean, cargo fmt clean.
```

```yaml
    blocked: >
      YYYY-MM-DD. Blocked on <exact reason>. <what would unblock it>.
```

`review:` and `blocked:` are written by agents after work, not by the
progress-file author. Leave them absent on new epics.

---

## Writing good acceptance criteria

### Implementation acceptance

Each bullet must be:

- **Concrete**: names a specific function, type, field, or observable behavior.
- **Falsifiable**: a passing or failing state exists; "works correctly" is
  not falsifiable.
- **Bounded**: one behavioral fact per bullet, not a paragraph.

Good:
```yaml
acceptance:
  - "preprocess() strips a full-line `*` comment (ngspice and Xyce) and returns it as an empty logical line, not a dropped line — line numbers must stay stable for diagnostics."
  - "preprocess() joins a `+`-continuation line onto the previous logical line with a single space separator, per docs/GRAMMAR.md §1."
  - "preprocess(dialect=Xyce) treats `;` as starting an end-of-line comment; preprocess(dialect=Ngspice) treats `$` and `//` as starting one instead."
```

Bad:
```yaml
acceptance:
  - "The lexer works."
  - "Comments are handled correctly."
```

### Tests acceptance

- Mirror every `implementation.acceptance` bullet with at least one test scenario.
- Add edge cases not in implementation.acceptance (empty input, boundary
  values, error paths).
- Do not duplicate the implementation bullet verbatim — describe the *test
  scenario*, not the behavior.

---

## Status lifecycle

```
planned → in_progress → review → complete
                     ↘ blocked → in_progress (after fix)
```

| Status | Set by | Meaning |
|---|---|---|
| `planned` | progress-file author | Not started; preconditions not met or not yet assigned. |
| `in_progress` | coder-agent (on start) | Claimed and being implemented. |
| `review` | coder-agent (after commit) or tester-agent (after green run) | Implementation committed; awaiting tester-agent validation. |
| `complete` | tester-agent | All acceptance criteria met; test suite green; gates green. |
| `blocked` | coder-agent or tester-agent | Cannot proceed; reason in `blocked:` note. |

Rules:
- Only the tester-agent marks `complete`. The coder-agent marks `review`.
- A `blocked` epic must always have a `blocked:` note. "blocked" without a
  note is forbidden.

---

## Ordering and `depends_on`

- `depends_on` lists epic ids that must reach `status: complete` before this
  epic can start.
- Cross-file dependencies are allowed: `depends_on: [CORE-03]` in
  `progress.lsp.yaml` is valid.
- If an epic has no dependencies, use an explicit empty list: `depends_on: []`.
- Do not express dependencies through phase membership alone.

---

## What NOT to put in a progress file

- **HOW to implement** — that belongs in `docs/GRAMMAR.md` or another spec.
- **Meeting notes or decisions** — commit messages are the record.
- **Vague status notes** — every `blocked:` or `review:` note must be
  actionable.
- **Duplicate content from the spec** — a one-line `summary:` is enough.

---

## Cross-references

- `/implement-epic` — coder-agent workflow for a planned epic
- `/test-and-progress` — tester-agent workflow (write tests or run + update)
- `/fix-blocked` — coder-agent workflow for a blocked epic
- `AGENTS.md` — where specs, progress files, and skills fit together
