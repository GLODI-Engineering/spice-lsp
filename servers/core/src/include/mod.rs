//! Multi-file `.include`/`.lib` resolution: turns an entry netlist file plus
//! whatever it pulls in via `.include`/`.lib` into one merged statement
//! list, with diagnostics for unresolvable paths, circular includes, and
//! missing `.lib` sections. Start with [`graph::resolve_file`].

/// Builds the include/`.lib` dependency graph and merges resolved files
/// into a single statement list. Start here: [`graph::resolve_file`].
pub mod graph;
/// Extracts a single named section out of a `.lib` file's `.lib`/`.endl`
/// blocks.
pub mod lib_sections;
/// Dialect-specific include-path search order (relative to the including
/// file, the top-level netlist directory, or the exec directory).
pub mod resolve;
/// Virtual line-number remapping so diagnostics from an included file point
/// at real `(file, line)` locations instead of colliding line numbers.
pub mod source_map;
