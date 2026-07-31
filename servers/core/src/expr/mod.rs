//! Expression parsing: multiple dialect- and context-specific grammars
//! (ngspice has a distinct compile-time `.PARAM` grammar and a separate
//! runtime/behavioral-source grammar; Xyce has one grammar plus special
//! forms for `TABLE`/polynomial sources), all producing the same shared
//! [`Expr`] tree defined in [`ast`]. [`token`] holds the shared tokenizer
//! every parser here is built on; [`diagnostics`] holds cross-dialect
//! "gotcha" lints; `wiring_*` connects parsed expressions back to the
//! statements that contain them.

pub mod ast;
pub mod diagnostics;
pub mod ngspice_compiletime;
pub mod ngspice_runtime;
pub mod token;
pub mod wiring_compiletime;
pub mod wiring_runtime;
pub mod xyce;
pub mod xyce_sources;

pub use ast::Expr;
pub use ast::RefKind;
pub use token::{tokenize, SpannedToken, Token, TokenError};
