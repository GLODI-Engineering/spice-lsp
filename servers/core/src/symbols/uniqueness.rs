//! Checks the docs/GRAMMAR.md §6.1 rule "all subcircuit and model names
//! are considered global and must be unique" — [`check_unique_names`] is
//! the entry point.

use crate::ast::{LineSpan, Statement};
use crate::symbols::scope::{Scope, ScopeKind};

/// A duplicate `.subckt` or `.model` name, found by [`check_unique_names`].
/// Subcircuit and model names share one global namespace (docs/
/// GRAMMAR.md §6.1), so a `.subckt` and a `.model` with the same name
/// collide too, not just two of the same statement kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateDiagnostic {
    /// A human-readable message naming the duplicate and both locations.
    pub message: String,
    /// The colliding name, uppercased (name comparison is
    /// case-insensitive, matching both dialects).
    pub name: String,
    /// Where the first (non-duplicate) definition is.
    pub first_span: LineSpan,
    /// Where the duplicate definition is.
    pub duplicate_span: LineSpan,
}

/// Find every duplicate `.subckt`/`.model` name in `scope_tree`, checked
/// case-insensitively across the whole tree (not just within one scope —
/// per docs/GRAMMAR.md §6.1, these names are global regardless of
/// nesting). Model names following the model-binning convention
/// (`basename.1`, `basename.2`, ... keyed by `lmin`/`lmax`/`wmin`/`wmax`)
/// are recognized and exempted from the duplicate check.
pub fn check_unique_names(scope_tree: &Scope) -> Vec<DuplicateDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen: Vec<(String, String, LineSpan)> = Vec::new();

    check_scope(scope_tree, &mut seen, &mut diagnostics);
    diagnostics
}

fn check_scope(
    scope: &Scope,
    seen: &mut Vec<(String, String, LineSpan)>,
    diagnostics: &mut Vec<DuplicateDiagnostic>,
) {
    for stmt in &scope.statements {
        match stmt {
            Statement::Subckt(subckt) => {
                let name = subckt.name.to_uppercase();
                check_and_record(
                    name.clone(),
                    "subcircuit".into(),
                    subckt.span.clone(),
                    seen,
                    diagnostics,
                );
            }
            Statement::Model(model) => {
                let name = model.name.to_uppercase();
                if !is_model_binning_suffix(&name) {
                    check_and_record(
                        name.clone(),
                        "model".into(),
                        model.span.clone(),
                        seen,
                        diagnostics,
                    );
                } else {
                    let base = model_binning_base(&name).unwrap_or(&name);
                    check_and_record_binning(
                        name.clone(),
                        base.to_string(),
                        "model".into(),
                        model.span.clone(),
                        seen,
                        diagnostics,
                    );
                }
            }
            _ => {}
        }
    }

    for child in &scope.children {
        // A real `.subckt` never appears as a `Statement::Subckt` in any
        // scope's `.statements` list — build_scope_tree() consumes it into
        // the Scope tree structure itself (the child Scope's own
        // kind/name). Check it here so duplicate `.subckt` names are
        // actually caught against the real parser pipeline's output, not
        // just against hand-built fixtures that place `Statement::Subckt`
        // directly into `.statements` (kept working above for backward
        // compatibility with such fixtures, but that shape never occurs in
        // practice).
        if child.kind == ScopeKind::Subckt {
            if let Some(name) = &child.name {
                check_and_record(
                    name.to_uppercase(),
                    "subcircuit".into(),
                    child.span.clone(),
                    seen,
                    diagnostics,
                );
            }
        }
        check_scope(child, seen, diagnostics);
    }
}

fn is_model_binning_suffix(name: &str) -> bool {
    if let Some(dot_pos) = name.rfind('.') {
        let suffix = &name[dot_pos + 1..];
        suffix.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

fn model_binning_base(name: &str) -> Option<&str> {
    name.rfind('.').map(|pos| &name[..pos])
}

fn check_and_record(
    name: String,
    kind: String,
    span: LineSpan,
    seen: &mut Vec<(String, String, LineSpan)>,
    diagnostics: &mut Vec<DuplicateDiagnostic>,
) {
    for (existing_name, _existing_kind, existing_span) in seen.iter() {
        if *existing_name == name {
            diagnostics.push(DuplicateDiagnostic {
                message: format!(
                    "duplicate {kind} name '{name}' (first defined at lines {:?}, duplicate at lines {:?})",
                    existing_span, span
                ),
                name: name.clone(),
                first_span: existing_span.clone(),
                duplicate_span: span,
            });
            return;
        }
    }
    seen.push((name, kind, span));
}

fn check_and_record_binning(
    name: String,
    _base: String,
    kind: String,
    span: LineSpan,
    seen: &mut Vec<(String, String, LineSpan)>,
    diagnostics: &mut Vec<DuplicateDiagnostic>,
) {
    for (existing_name, _existing_kind, existing_span) in seen.iter() {
        if *existing_name == name {
            diagnostics.push(DuplicateDiagnostic {
                message: format!(
                    "duplicate {kind} name '{name}' (first defined at lines {:?}, duplicate at lines {:?})",
                    existing_span, span
                ),
                name: name.clone(),
                first_span: existing_span.clone(),
                duplicate_span: span,
            });
            return;
        }
    }
    seen.push((name, kind, span));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn scope_from_stmts(stmts: Vec<Statement>) -> Scope {
        Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: stmts,
            children: Vec::new(),
            span: 0..1,
        }
    }

    #[test]
    fn test_duplicate_subckt_names_flagged() {
        let stmts = vec![
            Statement::Subckt(Subckt {
                name: "A".into(),
                nodes: vec![],
                params: vec![],
                span: 1..2,
            }),
            Statement::Subckt(Subckt {
                name: "A".into(),
                nodes: vec![],
                params: vec![],
                span: 5..6,
            }),
        ];
        let scope = scope_from_stmts(stmts);
        let diags = check_unique_names(&scope);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("duplicate"));
    }

    #[test]
    fn test_duplicate_model_names_flagged() {
        let stmts = vec![
            Statement::Model(Model {
                name: "NPN".into(),
                model_type: "NPN".into(),
                raw_params: String::new(),
                span: 1..2,
            }),
            Statement::Model(Model {
                name: "NPN".into(),
                model_type: "NPN".into(),
                raw_params: String::new(),
                span: 3..4,
            }),
        ];
        let scope = scope_from_stmts(stmts);
        let diags = check_unique_names(&scope);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_model_binning_suffix_not_flagged_as_duplicate() {
        let stmts = vec![
            Statement::Model(Model {
                name: "NMOS.1".into(),
                model_type: "NMOS".into(),
                raw_params: String::new(),
                span: 1..2,
            }),
            Statement::Model(Model {
                name: "NMOS.2".into(),
                model_type: "NMOS".into(),
                raw_params: String::new(),
                span: 3..4,
            }),
        ];
        let scope = scope_from_stmts(stmts);
        let diags = check_unique_names(&scope);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_subckt_and_model_name_collision_flagged() {
        let stmts = vec![
            Statement::Subckt(Subckt {
                name: "MYBLOCK".into(),
                nodes: vec![],
                params: vec![],
                span: 1..2,
            }),
            Statement::Model(Model {
                name: "myblock".into(),
                model_type: "R".into(),
                raw_params: String::new(),
                span: 3..4,
            }),
        ];
        let scope = scope_from_stmts(stmts);
        let diags = check_unique_names(&scope);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_case_insensitive_duplicate_detection() {
        let stmts = vec![
            Statement::Subckt(Subckt {
                name: "OpAmp".into(),
                nodes: vec![],
                params: vec![],
                span: 1..2,
            }),
            Statement::Subckt(Subckt {
                name: "opamp".into(),
                nodes: vec![],
                params: vec![],
                span: 5..6,
            }),
        ];
        let scope = scope_from_stmts(stmts);
        let diags = check_unique_names(&scope);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_diagnostic_for_all_unique_names() {
        let stmts = vec![
            Statement::Subckt(Subckt {
                name: "A".into(),
                nodes: vec![],
                params: vec![],
                span: 1..2,
            }),
            Statement::Subckt(Subckt {
                name: "B".into(),
                nodes: vec![],
                params: vec![],
                span: 4..5,
            }),
            Statement::Model(Model {
                name: "NPN".into(),
                model_type: "NPN".into(),
                raw_params: String::new(),
                span: 7..8,
            }),
        ];
        let scope = scope_from_stmts(stmts);
        let diags = check_unique_names(&scope);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_duplicate_subckt_names_flagged_via_real_build_scope_tree_pipeline() {
        // Regression test for the CORE-15 pipeline-integration bug: unlike
        // test_duplicate_subckt_names_flagged above (which places
        // Statement::Subckt directly into .statements, a shape that never
        // occurs in real usage), this test goes through the actual
        // build_scope_tree() parser pipeline, where a `.subckt`/`.ends`
        // pair becomes a child Scope rather than a Statement::Subckt
        // entry — the exact shape that previously made duplicate .subckt
        // detection a no-op in practice.
        use crate::symbols::scope::build_scope_tree;

        let stmts = vec![
            Statement::Subckt(Subckt {
                name: "opamp".into(),
                nodes: vec![],
                params: vec![],
                span: 1..2,
            }),
            Statement::Ends(Some("opamp".into()), 2..3),
            Statement::Subckt(Subckt {
                name: "OPAMP".into(),
                nodes: vec![],
                params: vec![],
                span: 5..6,
            }),
            Statement::Ends(Some("opamp".into()), 6..7),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(
            tree.children.len(),
            2,
            "sanity check: two sibling scopes expected"
        );

        let diags = check_unique_names(&tree);
        assert_eq!(
            diags.len(),
            1,
            "expected one duplicate-subckt diagnostic (case-insensitive 'opamp'/'OPAMP'), got: {diags:?}"
        );
        assert!(diags[0].message.contains("subcircuit"));
    }
}
