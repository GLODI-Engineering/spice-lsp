//! Multi-file `.include`/`.lib` resolution: turns an entry netlist file plus
//! whatever it pulls in via `.include`/`.lib` into one merged statement
//! list, with diagnostics for unresolvable paths, circular includes, and
//! missing `.lib` sections. Start with [`graph::resolve_includes`].

pub mod graph;
pub mod lib_sections;
pub mod resolve;
pub mod source_map;
