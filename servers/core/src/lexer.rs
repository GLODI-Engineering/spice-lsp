//! Line-level preprocessing: comment stripping and continuation-joining.
//!
//! Per docs/GRAMMAR.md §1, this must happen *before* statement tokenizing,
//! and the exact comment/continuation rules are dialect-dependent (ngspice
//! allows `$`/`//` end-of-line comments and `+`-prefixed or `\`-suffixed
//! continuation; Xyce uses `;` for end-of-line comments and only the
//! `+`-prefix continuation form). Not yet implemented — see CORE-01 in
//! progress.core.yaml.

use crate::dialect::Dialect;

/// Placeholder for the line-preprocessing entry point.
pub fn preprocess(_source: &str, _dialect: Dialect) -> Vec<String> {
    todo!("CORE-01: comment stripping + continuation joining, see docs/GRAMMAR.md §1")
}
