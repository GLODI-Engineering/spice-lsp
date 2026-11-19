# spice-lsp

[![CI](https://github.com/GLODI-Engineering/spice-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/GLODI-Engineering/spice-lsp/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/general-spice-core.svg)](https://crates.io/crates/general-spice-core)
[![docs.rs](https://img.shields.io/docsrs/general-spice-core)](https://docs.rs/general-spice-core)
[![license: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

**What this repository ships today is a library, not a language server.**
[`general-spice-core`](servers/core/) parses SPICE-family netlist text — ngspice and Xyce, with
a third proprietary dialect planned once reference documentation is available — into a
structured AST and resolves symbols (`.subckt`/`.model` references, `.param` scoping, multi-file
`.include` resolution). Those are the building blocks a language server needs (diagnostics,
hover, go-to-definition, completion), and they are useful to anything else that has to
understand a netlist, which is what they are used for.

The repository is named for the LSP wrapper that does not exist yet. The crate is on crates.io
and is what you depend on.

## Status

**Core library: all 31 planned epics complete** (`progress.core.yaml`),
tested against 71 real, independently-authored ngspice and Xyce netlists
in addition to ~380 unit tests (`servers/core/tests/`). See
[`servers/core/README.md`](servers/core/README.md) for how to use it and
[`docs/GRAMMAR.md`](docs/GRAMMAR.md) for the full grammar reference it's
built against.

**Not started:** an LSP-protocol wrapper (`tower-lsp`) for editors that cannot link a Rust
library, and support for a third proprietary SPICE dialect. The `extension/` directory is a
placeholder and not a plan — netlist editing in VS Code is
[general-simulator-gui-for-vscode](https://github.com/GLODI-Engineering/general-simulator-gui-for-vscode),
which is a different project and not a client of this one.

| Dialect | Status |
|---|---|
| ngspice   | lexer, parser (devices + statements), expression grammar, symbol resolution, multi-file `.include`/`.lib` — all implemented and tested |
| Xyce      | same coverage as ngspice |
| (third proprietary dialect) | not started — reference docs not yet available |

## Quick start

```toml
[dependencies]
general-spice-core = "0.1"
```

```rust
use general_spice_core::{lexer, parser, Dialect};

let source = "Example circuit\nR1 1 0 1k\nC1 1 0 10n\n.end\n";
let lines = lexer::preprocess(source, Dialect::Ngspice);
let statements = parser::parse_document(&lines, Dialect::Ngspice);
```

See [`servers/core/README.md`](servers/core/README.md) for the full usage
guide (diagnostics, expression parsing, multi-file resolution), or run
`cargo doc -p general-spice-core --open` for the complete API reference.

## Layout

```
servers/core/      general_spice_core — the Rust parser/symbol-resolution library (see its README)
servers/           future home for other server-side pieces, e.g. a tower-lsp wrapper binary
extension/         placeholder, holds only a README -- see Status
docs/GRAMMAR.md    the netlist grammar reference this crate is built against —
                    common vs. ngspice-specific vs. Xyce-specific, with citations
                    back to each simulator's own manual
progress.core.yaml  epic-by-epic build log for servers/core (what's done, how it
                    was verified)
docs/gotchas/       institutional-memory records for non-obvious bugs found along
                    the way (see AGENTS.md's /record-gotcha skill)
AGENTS.md          project conventions, architecture invariants, agent workflow
```

## Why a Rust library instead of "just" an LSP server

Because the consumer is a Rust program, not an editor process.
[`general-mna`](https://github.com/GLODI-Engineering/general-mna) links this crate to turn a
netlist into modified-nodal-analysis matrices, and the
[general-simulator](https://github.com/GLODI-Engineering/general-simulator) engine links it too
— `crates/dae-runtime` and `crates/general-simulator-cli` both name it as a dependency. The
parse result is handed straight to the next stage as typed data. An LSP server in that path would mean
serialising an AST over a wire protocol to a process in the same address space.

A [`tower-lsp`](https://github.com/ebkalderon/tower-lsp) wrapper remains worth building for
editors that cannot link a Rust library, and the crate is structured so that it is a thin
adapter rather than a rewrite — one grammar engine, two adapters. See `docs/GRAMMAR.md` §9.2 for
the architecture decision this follows (common core plus per-dialect overlays, applied at the
crate-consumption level too).

An earlier version of this section named a Tauri + React desktop application as the primary
target. That application no longer exists; the text outlived it.

## Development

This project uses an epic-based workflow — see `AGENTS.md` and
`.claude/skills/` (`write-progress`, `implement-epic`, `test-and-progress`,
`fix-blocked`, `record-gotcha`, `commit`). Pre-commit hooks are installed
via `pre-commit install` (see `.pre-commit-config.yaml`).

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
