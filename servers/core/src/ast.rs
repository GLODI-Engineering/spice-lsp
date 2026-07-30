//! Common-core AST types shared by every dialect (docs/GRAMMAR.md §3, §6, §7).
//!
//! Dialect-specific device letters / statement forms are not modeled here —
//! per the adopted architecture (docs/GRAMMAR.md §9.2), those live in
//! per-dialect overlay tables consumed by the parser, not as extra AST
//! variants baked into this common core. Not yet implemented — see CORE-02
//! in progress.core.yaml.
