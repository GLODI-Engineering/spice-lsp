# general_spice_core

A dialect-agnostic parser and symbol-resolution engine for SPICE-family
circuit netlists — ngspice and Xyce today, a third proprietary dialect planned once reference
docs are available. Pure Rust, no external dependencies.

This crate does **not** simulate circuits. It turns netlist *text* into
structured data an editor or tool can use for diagnostics, hover,
go-to-definition, and completion. It's designed to be consumed two ways:

1. **Embedded directly** as a Cargo dependency — e.g. from a Tauri app's
   Rust backend, called straight from `#[tauri::command]` handlers with no
   IPC/protocol overhead. This is the primary target this crate was built
   for.
2. **Wrapped in a `tower-lsp` binary** that speaks real LSP over stdio for
   VS Code, Neovim, or any other LSP-capable editor — not implemented yet,
   but the core is structured so this is a thin adapter, not a rewrite.

Full API docs: run `cargo doc -p general-spice-core --open` from the repo root, or
read the doc comments in `src/lib.rs` — they include runnable examples
(`cargo test -p general-spice-core --doc`).

## Status

All 31 planned "v1.1" epics are complete (`progress.core.yaml`) and the
crate has been tested against 71 real, independently-authored ngspice and
Xyce netlists (`tests/real_netlists.rs`, `tests/fixtures/`) — not just
hand-written unit fixtures. See `docs/GRAMMAR.md` in the repository root
for the exact grammar reference this crate implements against, including
what's common between the two dialects, what diverges, and why.

What's implemented: lexing (comments, line continuation), full statement
parsing for both dialects (devices, `.subckt`/`.model`/`.param`/`.func`,
common and dialect-specific analysis statements), expression parsing
(arithmetic, behavioral B/E/G-source forms, with the documented
cross-dialect semantic differences), symbol resolution (`.subckt`/`.model`
reference resolution with circular-reference detection, name-uniqueness
checking, dialect-correct `.param` scope chains), and multi-file
`.include`/`.lib` resolution.

What's not: full semantic validation of every statement type (some are
captured structurally as raw text rather than deeply modeled — this is
intentional, see `docs/GRAMMAR.md`'s phase plan), expression *evaluation*
(this crate parses expressions into a typed tree; it doesn't compute their
values), and the third proprietary dialect (not yet targeted).

## Installation

Not published to crates.io. Use a path or git dependency:

```toml
[dependencies]
general-spice-core = { path = "../spice-lsp/servers/core" }
# or
general-spice-core = { git = "https://github.com/Elvis-codeur/spice-lsp", package = "general-spice-core" }
```

## Usage

### Parse a single file

```rust
use general_spice_core::{lexer, parser, Dialect};

let source = "\
Example RC circuit
R1 1 0 1k
C1 1 0 10n
.end
";

let lines = lexer::preprocess(source, Dialect::Ngspice);
// parse_document, not parse: this is a whole real file, and the first
// line is always its title (per the SPICE convention both dialects
// share — see parser::parse_document's doc comment for why there are
// two entry points).
let results = parser::parse_document(&lines, Dialect::Ngspice);

for result in &results {
    match result {
        Ok(stmt) => println!("{stmt:?}"),
        Err(e) => eprintln!("parse error: {} (line {:?})", e.message, e.span),
    }
}
```

`parser::parse` (without `_document`) is the other entry point — it treats
every line as ordinary statement content with no title-line handling. Use
it when you're parsing a fragment, not a whole file (this is what every
unit test in this crate does for brevity).

### Get diagnostics

```rust
use general_spice_core::{ast::Statement, lexer, parser, symbols, Dialect};

let source = "circuit\nX1 in out amplifier\n.end\n";
let lines = lexer::preprocess(source, Dialect::Ngspice);
let statements: Vec<Statement> = parser::parse_document(&lines, Dialect::Ngspice)
    .into_iter()
    .flatten()
    .collect();

let tree = symbols::scope::build_scope_tree(&statements).unwrap();

// Undefined .subckt references, circular X-call chains:
let subckt_diags = symbols::subckt_resolution::resolve_subckt_calls(&tree, Dialect::Ngspice);
// Undefined .model references:
let model_diags = symbols::model_resolution::resolve_model_references(&tree, Dialect::Ngspice);
// Duplicate .subckt/.model names:
let uniqueness_diags = symbols::uniqueness::check_unique_names(&tree);

for d in &subckt_diags {
    println!("{}", d.message); // "undefined subcircuit reference: 'amplifier'"
}
```

### Parse expressions

```rust
use general_spice_core::expr::ngspice_compiletime::parse_ngspice_compiletime;
use general_spice_core::expr::xyce::parse_xyce;

// ngspice's compile-time grammar (.param/.func/brace-expressions):
let expr = parse_ngspice_compiletime("1k * 2 + sqrt(4)").unwrap();

// Xyce's unified grammar — note `^` means XOR here, not power (the
// opposite of ngspice's compile-time grammar, where `^` means power just
// like `**`). See expr::diagnostics for lints that catch this exact
// false-cognate mistake.
let xyce_expr = parse_xyce("a ^ b").unwrap();
```

### Resolve multi-file `.include`/`.lib` netlists

```rust
use general_spice_core::include::graph::resolve_includes;
use general_spice_core::include::resolve::FakeFileSystem; // in production, implement
                                                    // include::resolve::FileSystem
                                                    // over std::fs instead
use general_spice_core::include::source_map::SourceMap;
use general_spice_core::Dialect;
use std::path::Path;

let mut fs = FakeFileSystem::new();
fs.insert("/top.cir", "top\n.include /sub.cir\n.end\n");
fs.insert("/sub.cir", "R1 1 2 100\n");

let mut source_map = SourceMap::new();
let (statements, diagnostics) =
    resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut source_map);

// diagnostics covers: unresolvable includes, circular includes (A
// includes B includes A), unresolvable .lib sections. `statements` is the
// fully merged, in-order statement list, with source-line spans remapped
// into a shared "virtual line number" space so `source_map.
// resolve_virtual_line(span.start)` can always recover which real file
// (and real line) any statement or diagnostic came from.
```

## Architecture: one core, per-dialect overlays

The adopted, load-bearing design decision (see `docs/GRAMMAR.md` §9.2 and
`AGENTS.md`): a common core plus per-dialect overlay tables for device
letters, operators, and built-in functions — never two independent
grammars, and never one grammar with `if dialect == ...` checks scattered
through shared logic.

This matters in practice because ngspice and Xyce are **not** the same
grammar with cosmetic differences. Some concrete false-cognates this crate
has to get right:

- Device letter `P` is a coupled-multiconductor line in ngspice, an
  S-parameter port device in Xyce. Same letter, unrelated devices.
- Operator `^` means power in ngspice's compile-time grammar, boolean XOR
  in Xyce.
- `log()` is natural log in ngspice, base-10 in Xyce.
- ngspice's runtime (behavioral-source) expression grammar is a genuinely
  *different* grammar from its own compile-time grammar — not a superset —
  with a different function set (`tanh` is missing from the runtime one)
  and different `pow`/`pwr` sign-handling semantics.

See `docs/GRAMMAR.md` for the complete reference this crate is built
against, including a quick-index table of what's common vs. dialect-specific
across every grammar area.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # unit tests + the real-netlist conformance suite + doctests
cargo doc -p general-spice-core --open  # browse the full API docs
```

See the repository root's `AGENTS.md` and `.claude/skills/` for the
epic-based development workflow this project uses
(`write-progress`/`implement-epic`/`test-and-progress`/`fix-blocked`).
