use std::path::{Path, PathBuf};

use crate::ast::Statement;
use crate::dialect::Dialect;
use crate::include::lib_sections;
use crate::include::resolve::{resolve_include_path, FileSystem};
use crate::include::source_map::{FileId, SourceMap};
use crate::lexer;
use crate::parser;

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
                            if let Ok(content) = fs.read_to_string(&resolved) {
                                let lib_processed = lexer::preprocess(&content, dialect);
                                match lib_sections::extract_lib_section(&lib_processed, sect) {
                                    Ok(section_lines) => {
                                        let lib_parsed = parser::parse(&section_lines, dialect);
                                        for s in lib_parsed.into_iter().flatten() {
                                            statements.push((file_id, s));
                                        }
                                    }
                                    Err(e) => {
                                        diagnostics.push(IncludeDiagnostic {
                                            message: format!("lib section error: {}", e.message),
                                            file: file_id,
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
                statements.push((file_id, stmt.clone()));
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
}
