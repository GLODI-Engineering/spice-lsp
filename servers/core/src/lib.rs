#![warn(missing_docs)]
//! `spice_core` — a dialect-agnostic parser and symbol-resolution engine
//! for SPICE-family circuit netlists (ngspice and Xyce today; a reference tool
//! planned once reference documentation is available).
//!
//! This crate does not simulate circuits. It turns netlist *text* into
//! structured data an editor or tool can use for diagnostics, hover,
//! go-to-definition, and completion — the kind of thing a language server
//! needs, without being tied to the Language Server Protocol itself. It is
//! consumed two ways in practice: embedded directly as a library (e.g. from
//! a Tauri app's Rust backend, calling straight into these functions with
//! no IPC layer involved), or wrapped by a separate `tower-lsp`-based
//! binary that speaks LSP over stdio for editors.
//!
//! # Is this crate right for your use case?
//!
//! Yes, if you want to: parse ngspice or Xyce netlist text into a
//! structured AST; detect duplicate `.subckt`/`.model` names, undefined
//! subcircuit/model references, or circular `.subckt`/`.include` chains;
//! resolve `.param` scoping (including the dialect-specific differences —
//! see `docs/GRAMMAR.md` in the repository); parse SPICE expressions
//! (arithmetic, behavioral B/E/G-source forms) into a typed tree; or
//! resolve multi-file `.include`/`.lib` netlists into one merged document.
//!
//! No, if you want to: actually *simulate* a circuit (this crate has no
//! solver — pair it with a real ngspice/Xyce process for that), fully
//! validate every SPICE statement type (some statements are captured
//! structurally as raw text rather than semantically modeled — see
//! `docs/GRAMMAR.md`'s phase plan for what's covered), or work with
//! a reference tool netlists (not yet supported).
//!
//! # Architecture
//!
//! One common core plus per-dialect overlays — never two independent
//! grammars, and never one grammar with dialect checks scattered through
//! shared logic. Concretely:
//!
//! - [`dialect`] — the [`Dialect`] enum and the device-letter overlay
//!   table. Some device letters mean different things in each dialect
//!   (`P` is a coupled-multiconductor line in ngspice but an S-parameter
//!   port device in Xyce); resolving that ambiguity is this module's job,
//!   done once, rather than re-derived by every caller.
//! - [`lexer`] — dialect-aware comment stripping and line-continuation
//!   joining, run before any statement-level parsing.
//! - [`ast`] — the common-core [`ast::Statement`] enum, shared by both
//!   dialects. Dialect-specific *meaning* lives in the overlay tables, not
//!   as extra AST variants.
//! - [`parser`] — turns preprocessed lines into [`ast::Statement`]s. Two
//!   entry points: [`parser::parse`] for a statement fragment (what
//!   every unit test in this crate uses), and [`parser::parse_document`]
//!   for a whole real file (which has a mandatory title line — see that
//!   function's docs for why the distinction matters).
//! - [`expr`] — SPICE expression parsing. Not one grammar: ngspice alone
//!   has a compile-time grammar (`.param`/`.func`/brace-expressions) and a
//!   separate runtime grammar (B/E/G-source expressions) with a
//!   genuinely different function set and operator semantics. Xyce has one
//!   unified grammar instead, but with context-dependent restrictions on
//!   what an expression may reference. [`expr::diagnostics`] flags the
//!   cross-dialect false cognates (`^` means power in ngspice, boolean XOR
//!   in Xyce; `log()` is natural log in ngspice, base-10 in Xyce).
//! - [`symbols`] — scope-tree construction ([`symbols::scope`]) and the
//!   diagnostic passes built on it: name uniqueness
//!   ([`symbols::uniqueness`]), `.subckt` call resolution with circular-
//!   reference detection ([`symbols::subckt_resolution`]), `.model`
//!   reference resolution ([`symbols::model_resolution`]), and dialect-
//!   specific `.param` scope chains ([`symbols::ngspice_param_scope`],
//!   [`symbols::xyce_param_scope`]).
//! - [`mod@include`] — multi-file `.include`/`.lib` resolution
//!   ([`include::graph`]), behind a [`include::resolve::FileSystem`] trait
//!   so it's testable without touching real disk.
//!
//! For the full grammar reference this crate implements against —
//! including exactly what's common between ngspice and Xyce, what
//! diverges, and why — see `docs/GRAMMAR.md` in the repository root. For
//! what's implemented vs. planned, see `progress.core.yaml`.
//!
//! # Quick start: parsing a single file
//!
//! ```
//! use spice_core::{ast::Statement, lexer, parser, Dialect};
//!
//! let source = "\
//! Example RC circuit
//! R1 1 0 1k
//! C1 1 0 10n
//! .end
//! ";
//!
//! let lines = lexer::preprocess(source, Dialect::Ngspice);
//! // parse_document (not parse) because this is a whole real file: the
//! // first line is always the title, per the SPICE convention both
//! // dialects share.
//! let results = parser::parse_document(&lines, Dialect::Ngspice);
//!
//! let mut device_names = Vec::new();
//! for result in &results {
//!     if let Ok(Statement::ElementInstance(ei)) = result {
//!         device_names.push(ei.name.as_str());
//!     }
//! }
//! assert_eq!(device_names, vec!["R1", "C1"]);
//! ```
//!
//! # Quick start: diagnostics (undefined subcircuit reference)
//!
//! ```
//! use spice_core::{ast::Statement, lexer, parser, symbols, Dialect};
//!
//! let source = "\
//! Circuit with a typo'd subckt reference
//! X1 in out amplifier
//! .end
//! ";
//!
//! let lines = lexer::preprocess(source, Dialect::Ngspice);
//! let parsed = parser::parse_document(&lines, Dialect::Ngspice);
//! let statements: Vec<Statement> = parsed.into_iter().flatten().collect();
//!
//! let tree = symbols::scope::build_scope_tree(&statements)
//!     .expect("well-formed .subckt/.ends nesting");
//! let diagnostics = symbols::subckt_resolution::resolve_subckt_calls(&tree, Dialect::Ngspice);
//!
//! assert_eq!(diagnostics.len(), 1);
//! assert!(diagnostics[0].message.contains("undefined subcircuit"));
//! ```
//!
//! # Quick start: multi-file `.include` resolution
//!
//! ```
//! use spice_core::include::graph::resolve_includes;
//! use spice_core::include::resolve::FakeFileSystem;
//! use spice_core::include::source_map::SourceMap;
//! use spice_core::Dialect;
//! use std::path::Path;
//!
//! let mut fs = FakeFileSystem::new();
//! fs.insert(
//!     "/top.cir",
//!     "top-level circuit\nR1 1 2 100\n.include /parts.cir\n.end\n",
//! );
//! fs.insert("/parts.cir", "C1 2 0 10n\n");
//!
//! let mut source_map = SourceMap::new();
//! let (statements, diagnostics) =
//!     resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut source_map);
//!
//! assert!(diagnostics.is_empty());
//! // title (Comment) + R1 (top.cir) + C1 (spliced in from parts.cir) +
//! // .end (not yet semantically modeled, captured as Unrecognized) —
//! // all merged in document order.
//! assert_eq!(statements.len(), 4);
//! ```
//!
//! `FakeFileSystem` is the same in-memory test double this crate's own
//! test suite uses; swap in a real [`include::resolve::FileSystem`]
//! implementation backed by `std::fs` to resolve real files.

pub mod ast;
pub mod dialect;
pub mod expr;
pub mod include;
pub mod lexer;
pub mod parser;
pub mod symbols;

pub use dialect::Dialect;
