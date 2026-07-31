//! Expression parsing: multiple dialect- and context-specific grammars
//! (ngspice has a distinct compile-time `.PARAM` grammar and a separate
//! runtime/behavioral-source grammar; Xyce has one grammar plus special
//! forms for `TABLE`/polynomial sources), all producing the same shared
//! [`Expr`] tree defined in [`ast`]. [`token`] holds the shared tokenizer
//! every parser here is built on; [`diagnostics`] holds cross-dialect
//! "gotcha" lints; `wiring_*` connects parsed expressions back to the
//! statements that contain them.

/// The dialect-agnostic expression tree ([`Expr`]) and its `RefKind`.
pub mod ast;
/// Cross-dialect expression lints (`^`/`log()` meaning, ternary/node-path
/// ambiguity).
pub mod diagnostics;
/// ngspice's `.PARAM`-definition (compile-time) expression grammar.
pub mod ngspice_compiletime;
/// ngspice's behavioral-source (runtime) expression grammar.
pub mod ngspice_runtime;
/// The shared tokenizer every expression parser in this module is built on.
pub mod token;
/// Connects parsed compile-time expressions back to the `.PARAM`/`.model`/
/// `.func` statements that contain them.
pub mod wiring_compiletime;
/// Connects parsed runtime expressions back to the behavioral-source
/// statements that contain them.
pub mod wiring_runtime;
/// Xyce's expression grammar (used for both `.PARAM` and behavioral
/// sources — Xyce doesn't split compile-time/runtime the way ngspice does).
pub mod xyce;
/// Special-form parsing for Xyce's `TABLE()` and polynomial (`POLY`)
/// source syntax.
pub mod xyce_sources;

pub use ast::Expr;
pub use ast::RefKind;
pub use token::{tokenize, SpannedToken, Token, TokenError};
