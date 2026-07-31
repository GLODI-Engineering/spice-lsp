//! Scope-tree construction and every symbol-resolution diagnostic pass
//! built on it. Start with [`scope::build_scope_tree`] to turn a parsed
//! statement list into a [`scope::Scope`] tree, then feed that tree into
//! whichever diagnostic passes you need — they're independent of each
//! other and can be run in any order (or skipped).

pub mod ident_resolution;
pub mod model_resolution;
pub mod ngspice_param_scope;
pub mod scope;
pub mod subckt_resolution;
pub mod uniqueness;
pub mod xyce_param_scope;
