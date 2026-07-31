//! Scope-tree construction and every symbol-resolution diagnostic pass
//! built on it. Start with [`scope::build_scope_tree`] to turn a parsed
//! statement list into a [`scope::Scope`] tree, then feed that tree into
//! whichever diagnostic passes you need — they're independent of each
//! other and can be run in any order (or skipped).

/// Cross-scope identifier resolution for parsed expressions —
/// "undefined parameter" diagnostics.
pub mod ident_resolution;
/// Resolves device-instance `.model` references, distinguishing them from
/// bare numeric parameter values.
pub mod model_resolution;
/// ngspice's `.param` lexical-shadowing scope chain.
pub mod ngspice_param_scope;
/// Builds the [`scope::Scope`] tree every other module in this crate
/// operates on.
pub mod scope;
/// Resolves `X`-element subcircuit calls, including circular-reference
/// detection.
pub mod subckt_resolution;
/// Global `.subckt`/`.model` name-uniqueness checking.
pub mod uniqueness;
/// Xyce's `.param`/`.global_param` scope chain.
pub mod xyce_param_scope;
