//! spice-core — dialect-agnostic SPICE netlist lexer/parser/symbol-table core.
//!
//! Architecture (docs/GRAMMAR.md §9.2, adopted): a common core plus
//! per-dialect overlay tables (device letters, operators, built-in
//! functions), not two independent grammars and not one grammar with
//! scattered dialect if/else checks. Consumed directly as a Cargo dependency
//! by the Tauri backend in general-simulator (primary target), and later
//! wrapped with `tower-lsp` as a standalone LSP binary for editor use.

pub mod ast;
pub mod dialect;
pub mod lexer;

pub use dialect::Dialect;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_variants_are_distinct() {
        assert_ne!(Dialect::Ngspice, Dialect::Xyce);
    }
}
