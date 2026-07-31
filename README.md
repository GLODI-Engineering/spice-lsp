# spice-lsp

Language tooling for SPICE-family circuit netlists — ngspice and Xyce
today, LTspice planned once reference documentation is available.

The core is a Rust library, [`spice_core`](servers/core/), that parses
netlist text into a structured AST and resolves symbols
(`.subckt`/`.model` references, `.param` scoping, multi-file `.include`
resolution) — the building blocks a language server needs (diagnostics,
hover, go-to-definition, completion), without being tied to the Language
Server Protocol itself.

## Status

**Core library: all 31 planned epics complete** (`progress.core.yaml`),
tested against 71 real, independently-authored ngspice and Xyce netlists
in addition to ~380 unit tests (`servers/core/tests/`). See
[`servers/core/README.md`](servers/core/README.md) for how to use it and
[`docs/GRAMMAR.md`](docs/GRAMMAR.md) for the full grammar reference it's
built against.

**Not started yet:** the VS Code extension (`extension/`), an LSP-protocol
wrapper (`tower-lsp`) for editor use outside a directly-embedding host app,
and LTspice support.

| Simulator | Status |
|---|---|
| ngspice   | lexer, parser (devices + statements), expression grammar, symbol resolution, multi-file `.include`/`.lib` — all implemented and tested |
| Xyce      | same coverage as ngspice |
| LTspice   | not started — reference docs not yet available |

## Quick start

```toml
[dependencies]
spice-core = { path = "path/to/spice-lsp/servers/core" }
```

```rust
use spice_core::{lexer, parser, Dialect};

let source = "Example circuit\nR1 1 0 1k\nC1 1 0 10n\n.end\n";
let lines = lexer::preprocess(source, Dialect::Ngspice);
let statements = parser::parse_document(&lines, Dialect::Ngspice);
```

See [`servers/core/README.md`](servers/core/README.md) for the full usage
guide (diagnostics, expression parsing, multi-file resolution), or run
`cargo doc -p spice-core --open` for the complete API reference.

## Layout

```
servers/core/      spice_core — the Rust parser/symbol-resolution library (see its README)
servers/           future home for other server-side pieces, e.g. a tower-lsp wrapper binary
extension/         (not started) VS Code client extension (TypeScript)
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

The primary target for this project is a separate Tauri + React desktop
app (a circuit simulator front-end) whose netlist editor is Monaco. A Tauri
backend is Rust, and Monaco doesn't need real LSP-over-JSON-RPC to get
diagnostics/hover/completion — it has its own provider APIs you drive
directly. So `spice_core` is designed to be embedded straight into that
Rust backend via `#[tauri::command]`, with zero IPC/protocol overhead —
not spawned as a subprocess speaking a wire protocol.

That said, the same core is meant to support a second target later: wrap
it with [`tower-lsp`](https://github.com/ebkalderon/tower-lsp) as a
standalone binary that speaks real LSP over stdio, for VS Code, Neovim, or
any other LSP-capable editor. One grammar engine, two thin adapters — see
`docs/GRAMMAR.md` §9.2 for the architecture decision this follows
(common core + per-dialect overlays, applied at the crate-consumption
level too).

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
