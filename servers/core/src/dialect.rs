//! Which SPICE dialect a document is being parsed as.
//!
//! See docs/GRAMMAR.md §9.1: dialect must be explicit (pinned per file or
//! workspace), never silently sniffed — same-letter device collisions (`P`,
//! `U`) and operator-meaning flips (`^`, `log()`) make full auto-detection
//! unreliable in the general case.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Ngspice,
    Xyce,
    // LTspice — not yet implemented, see docs/GRAMMAR.md §7.
}
