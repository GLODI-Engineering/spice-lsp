use std::path::{Path, PathBuf};

use crate::ast::{LineSpan, Statement};
use crate::dialect::Dialect;
use crate::include::lib_sections;
use crate::include::resolve::{resolve_include_path, FileSystem};
use crate::include::source_map::{FileId, SourceMap};
use crate::lexer;
use crate::parser;

fn line_count(text: &str) -> usize {
    text.lines().count().max(1)
}

fn shift_span(span: LineSpan, offset: usize) -> LineSpan {
    // Real line 1 must land exactly on `offset` (the base
    // SourceMap::assign_offset reserved for this file), not `offset + 1` —
    // real line L (1-based) maps to virtual line `offset + (L - 1)`.
    (span.start + offset - 1)..(span.end + offset - 1)
}

/// Shift a Statement's own line span by `offset` virtual lines. This is how
/// CORE-25 makes statements from different files mergeable into one
/// Vec<Statement> without adding a `file` field to Statement/Scope: each
/// file gets a disjoint block of virtual line numbers (see
/// SourceMap::assign_offset), so a span is globally unique across the
/// merged document and SourceMap::resolve_virtual_line can always recover
/// which file a given span's line number actually came from.
///
/// Exhaustive over every Statement variant on purpose (no `_` arm) so this
/// stays in sync if a new variant is ever added.
fn remap_statement_span(stmt: Statement, offset: usize) -> Statement {
    match stmt {
        Statement::ElementInstance(mut v) => {
            v.span = shift_span(v.span, offset);
            Statement::ElementInstance(v)
        }
        Statement::Subckt(mut v) => {
            v.span = shift_span(v.span, offset);
            Statement::Subckt(v)
        }
        Statement::Ends(name, span) => Statement::Ends(name, shift_span(span, offset)),
        Statement::Model(mut v) => {
            v.span = shift_span(v.span, offset);
            Statement::Model(v)
        }
        Statement::Param(mut v) => {
            v.span = shift_span(v.span, offset);
            Statement::Param(v)
        }
        Statement::GlobalParam(mut v) => {
            v.span = shift_span(v.span, offset);
            Statement::GlobalParam(v)
        }
        Statement::Func(mut v) => {
            v.span = shift_span(v.span, offset);
            Statement::Func(v)
        }
        Statement::Include(s, span) => Statement::Include(s, shift_span(span, offset)),
        Statement::Lib(a, b, span) => Statement::Lib(a, b, shift_span(span, offset)),
        Statement::Comment(span) => Statement::Comment(shift_span(span, offset)),
        Statement::Unrecognized(s, span) => Statement::Unrecognized(s, shift_span(span, offset)),
        Statement::Global(v, span) => Statement::Global(v, shift_span(span, offset)),
        Statement::Ic(v, span) => Statement::Ic(v, shift_span(span, offset)),
        Statement::Nodeset(v, span) => Statement::Nodeset(v, shift_span(span, offset)),
        Statement::NodesetAll(s, span) => Statement::NodesetAll(s, shift_span(span, offset)),
        Statement::Temp(s, span) => Statement::Temp(s, shift_span(span, offset)),
        Statement::Csparam(v, span) => Statement::Csparam(v, shift_span(span, offset)),
        Statement::Options {
            package,
            assignments,
            span,
        } => Statement::Options {
            package,
            assignments,
            span: shift_span(span, offset),
        },
        Statement::CsparamInfo(s, span) => Statement::CsparamInfo(s, shift_span(span, offset)),
        Statement::Ac(s, span) => Statement::Ac(s, shift_span(span, offset)),
        Statement::Dc(s, span) => Statement::Dc(s, shift_span(span, offset)),
        Statement::Op(span) => Statement::Op(shift_span(span, offset)),
        Statement::Tran(s, span) => Statement::Tran(s, shift_span(span, offset)),
        Statement::Analysis {
            keyword,
            dialect_tag,
            raw_args,
            span,
        } => Statement::Analysis {
            keyword,
            dialect_tag,
            raw_args,
            span: shift_span(span, offset),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDiagnostic {
    pub message: String,
    pub file: FileId,
    pub span: crate::ast::LineSpan,
}

pub fn resolve_includes(
    entry_path: &Path,
    fs: &dyn FileSystem,
    dialect: Dialect,
    source_map: &mut SourceMap,
) -> (Vec<(FileId, Statement)>, Vec<IncludeDiagnostic>) {
    let mut statements = Vec::new();
    let mut diagnostics = Vec::new();
    let mut visited: Vec<PathBuf> = Vec::new();
    let exec_dir = PathBuf::from(".");

    resolve_file(
        entry_path,
        fs,
        dialect,
        source_map,
        &mut visited,
        &mut statements,
        &mut diagnostics,
        &exec_dir,
    );

    (statements, diagnostics)
}

#[allow(clippy::too_many_arguments)]
fn resolve_file(
    path: &Path,
    fs: &dyn FileSystem,
    dialect: Dialect,
    source_map: &mut SourceMap,
    visited: &mut Vec<PathBuf>,
    statements: &mut Vec<(FileId, Statement)>,
    diagnostics: &mut Vec<IncludeDiagnostic>,
    top_level_dir: &Path,
) {
    if visited.contains(&path.to_path_buf()) {
        let chain: Vec<String> = visited.iter().map(|p| p.display().to_string()).collect();
        diagnostics.push(IncludeDiagnostic {
            message: format!(
                "circular include detected: {} -> {}",
                chain.join(" -> "),
                path.display()
            ),
            file: FileId::new_dummy(),
            span: 0..1,
        });
        return;
    }

    visited.push(path.to_path_buf());

    let content = match fs.read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            diagnostics.push(IncludeDiagnostic {
                message: format!("cannot read file '{}': {e}", path.display()),
                file: FileId::new_dummy(),
                span: 0..1,
            });
            return;
        }
    };

    let file_id = source_map.register(path.to_path_buf(), content.clone());
    let base = source_map.assign_offset(file_id, line_count(&content));

    let processed = lexer::preprocess(&content, dialect);
    let parsed = parser::parse(&processed, dialect);

    for result in parsed {
        let stmt = match result {
            Ok(s) => s,
            Err(_) => continue,
        };

        match &stmt {
            Statement::Include(filename, span) => {
                let including_dir = path.parent().unwrap_or(Path::new("."));
                let exec_dir = PathBuf::from(".");
                match resolve_include_path(
                    fs,
                    including_dir,
                    top_level_dir,
                    &exec_dir,
                    filename,
                    &[],
                    dialect,
                ) {
                    Ok(resolved) => {
                        resolve_file(
                            &resolved,
                            fs,
                            dialect,
                            source_map,
                            visited,
                            statements,
                            diagnostics,
                            top_level_dir,
                        );
                    }
                    Err(e) => {
                        diagnostics.push(IncludeDiagnostic {
                            message: format!("unresolvable include '{filename}': {}", e.message),
                            file: file_id,
                            span: span.clone(),
                        });
                    }
                }
            }
            Statement::Lib(filename, section, span) => {
                let including_dir = path.parent().unwrap_or(Path::new("."));
                let exec_dir = PathBuf::from(".");
                match resolve_include_path(
                    fs,
                    including_dir,
                    top_level_dir,
                    &exec_dir,
                    filename,
                    &[],
                    dialect,
                ) {
                    Ok(resolved) => {
                        if let Some(sect) = section {
                            if let Ok(lib_content) = fs.read_to_string(&resolved) {
                                // Register and offset the LIBRARY file itself, not the
                                // including file — statements spliced from a .lib
                                // section must be attributed to the file they actually
                                // came from, per CORE-25's file-identity requirement.
                                let lib_file_id =
                                    source_map.register(resolved.clone(), lib_content.clone());
                                let lib_base =
                                    source_map.assign_offset(lib_file_id, line_count(&lib_content));
                                let lib_processed = lexer::preprocess(&lib_content, dialect);
                                match lib_sections::extract_lib_section(&lib_processed, sect) {
                                    Ok(section_lines) => {
                                        let lib_parsed = parser::parse(&section_lines, dialect);
                                        for s in lib_parsed.into_iter().flatten() {
                                            statements.push((
                                                lib_file_id,
                                                remap_statement_span(s, lib_base),
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        diagnostics.push(IncludeDiagnostic {
                                            message: format!("lib section error: {}", e.message),
                                            file: lib_file_id,
                                            span: span.clone(),
                                        });
                                    }
                                }
                            }
                        } else {
                            resolve_file(
                                &resolved,
                                fs,
                                dialect,
                                source_map,
                                visited,
                                statements,
                                diagnostics,
                                top_level_dir,
                            );
                        }
                    }
                    Err(e) => {
                        diagnostics.push(IncludeDiagnostic {
                            message: format!("unresolvable lib '{filename}': {}", e.message),
                            file: file_id,
                            span: span.clone(),
                        });
                    }
                }
            }
            _ => {
                statements.push((file_id, remap_statement_span(stmt.clone(), base)));
            }
        }
    }

    visited.pop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::include::resolve::FakeFileSystem;

    #[test]
    fn test_single_level_include_splices_statements_in_order() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/top.cir", "R1 1 2 100\n.include /sub.cir\nC1 3 0 1u\n");
        fs.insert("/sub.cir", "R2 4 5 200\n");

        let mut sm = SourceMap::new();
        let (stmts, diags) =
            resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(diags.is_empty());
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_direct_self_include_detected_not_hung() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/self.cir", ".include /self.cir\n");

        let mut sm = SourceMap::new();
        let (_stmts, diags) =
            resolve_includes(Path::new("/self.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(diags.iter().any(|d| d.message.contains("circular")));
    }

    #[test]
    fn test_transitive_include_cycle_detected_and_chain_named() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/a.cir", ".include /b.cir\n");
        fs.insert("/b.cir", ".include /c.cir\n");
        fs.insert("/c.cir", ".include /a.cir\n");

        let mut sm = SourceMap::new();
        let (_stmts, diags) = resolve_includes(Path::new("/a.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(diags.iter().any(|d| d.message.contains("circular")));
    }

    #[test]
    fn test_missing_include_file_flagged_rest_of_document_still_resolves() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/top.cir", ".include /missing.cir\nR1 1 2 100\n");

        let mut sm = SourceMap::new();
        let (stmts, diags) =
            resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(!diags.is_empty());
        assert!(!stmts.is_empty());
    }

    #[test]
    fn test_nested_includes_resolve_transitively() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/a.cir", "R1 1 2 100\n.include /b.cir\n");
        fs.insert("/b.cir", "R2 3 4 200\n.include /c.cir\n");
        fs.insert("/c.cir", "R3 5 6 300\n");

        let mut sm = SourceMap::new();
        let (stmts, diags) = resolve_includes(Path::new("/a.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        assert_eq!(stmts.len(), 3, "expected R1, R2, R3 spliced in order");
        let names: Vec<String> = stmts
            .iter()
            .map(|(_, s)| match s {
                Statement::ElementInstance(ei) => ei.name.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(names, vec!["R1", "R2", "R3"]);
    }

    #[test]
    fn test_lib_section_reference_resolves_via_graph() {
        let mut fs = FakeFileSystem::new();
        fs.insert(
            "/top.cir",
            "R1 1 2 100\n.lib /parts.lib typical\nC1 3 0 1u\n",
        );
        fs.insert(
            "/parts.lib",
            ".lib typical\nR2 7 8 500\n.endl typical\n.lib fast\nR3 9 10 5\n.endl fast\n",
        );

        let mut sm = SourceMap::new();
        let (stmts, diags) =
            resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        let names: Vec<String> = stmts
            .iter()
            .map(|(_, s)| match s {
                Statement::ElementInstance(ei) => ei.name.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        // Only the "typical" section's R2 should be spliced in, not "fast"'s R3.
        assert_eq!(names, vec!["R1", "R2", "C1"]);
    }

    #[test]
    fn test_lib_unresolvable_section_flagged() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/top.cir", ".lib /parts.lib nonexistent_section\n");
        fs.insert("/parts.lib", ".lib typical\nR2 7 8 500\n.endl typical\n");

        let mut sm = SourceMap::new();
        let (_stmts, diags) =
            resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(
            diags.iter().any(|d| d.message.contains("lib section")),
            "expected a lib-section diagnostic, got: {diags:?}"
        );
    }
}
