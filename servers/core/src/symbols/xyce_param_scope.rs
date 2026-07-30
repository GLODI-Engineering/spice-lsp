use crate::ast::Statement;
use crate::symbols::scope::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyceParamBinding {
    pub name: String,
    pub value: Option<String>,
    pub scope_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyceGlobalParam {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyceParamDiagnostic {
    pub message: String,
    pub line: usize,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

pub fn build_xyce_param_scope_chain(
    scope_tree: &Scope,
) -> Vec<(usize, u32, Vec<XyceParamBinding>)> {
    let mut bindings = Vec::new();
    let mut next_id = 0usize;
    collect_xyce_bindings(scope_tree, &mut bindings, &mut next_id);
    bindings
}

fn collect_xyce_bindings(
    scope: &Scope,
    bindings: &mut Vec<(usize, u32, Vec<XyceParamBinding>)>,
    next_id: &mut usize,
) {
    let scope_id = *next_id;
    *next_id += 1;
    let mut scope_bindings = Vec::new();

    for stmt in &scope.statements {
        match stmt {
            Statement::Param(param) => {
                for (name, value) in &param.assignments {
                    scope_bindings.push(XyceParamBinding {
                        name: name.to_uppercase(),
                        value: Some(value.clone()),
                        scope_depth: scope.depth,
                    });
                }
            }
            crate::ast::Statement::Subckt(subckt) => {
                for (name, value) in &subckt.params {
                    scope_bindings.push(XyceParamBinding {
                        name: name.to_uppercase(),
                        value: value.clone(),
                        scope_depth: scope.depth + 1,
                    });
                }
            }
            _ => {}
        }
    }

    bindings.push((scope_id, scope.depth, scope_bindings));

    for child in &scope.children {
        collect_xyce_bindings(child, bindings, next_id);
    }
}

pub fn resolve_xyce_param_ident(
    scope_chains: &[(usize, u32, Vec<XyceParamBinding>)],
    scope_id: usize,
    query_depth: u32,
    name: &str,
) -> Option<XyceParamBinding> {
    let name_upper = name.to_uppercase();

    for (id, _depth, bindings) in scope_chains.iter() {
        if *id == scope_id {
            for binding in bindings.iter().rev() {
                if binding.name == name_upper {
                    return Some(binding.clone());
                }
            }
            break;
        }
    }

    // Search ancestor scopes (shallower depth, lower ID)
    for (id, depth, bindings) in scope_chains.iter().rev() {
        if *depth < query_depth && *id < scope_id {
            for binding in bindings.iter().rev() {
                if binding.name == name_upper {
                    return Some(binding.clone());
                }
            }
        }
    }

    None
}

pub fn collect_xyce_global_params(scope_tree: &Scope) -> Vec<XyceGlobalParam> {
    let mut globals = Vec::new();
    collect_global_params_in_scope(scope_tree, &mut globals);
    globals
}

fn collect_global_params_in_scope(scope: &Scope, globals: &mut Vec<XyceGlobalParam>) {
    for stmt in &scope.statements {
        if let Statement::GlobalParam(param) = stmt {
            for (name, value) in &param.assignments {
                globals.push(XyceGlobalParam {
                    name: name.to_uppercase(),
                    value: Some(value.clone()),
                });
            }
        }
    }
    for child in &scope.children {
        collect_global_params_in_scope(child, globals);
    }
}

pub fn check_xyce_param_redefinitions(scope_tree: &Scope) -> Vec<XyceParamDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen_global: Vec<(String, usize)> = Vec::new();
    let mut seen_params_per_scope: Vec<(u32, String, usize)> = Vec::new();

    check_xyce_redefs_in_scope(
        scope_tree,
        &mut seen_global,
        &mut seen_params_per_scope,
        &mut diagnostics,
    );
    diagnostics
}

fn check_xyce_redefs_in_scope(
    scope: &Scope,
    seen_global: &mut Vec<(String, usize)>,
    seen_params_per_scope: &mut Vec<(u32, String, usize)>,
    diagnostics: &mut Vec<XyceParamDiagnostic>,
) {
    for stmt in &scope.statements {
        match stmt {
            Statement::GlobalParam(param) => {
                for (name, _) in &param.assignments {
                    let upper = name.to_uppercase();
                    let saw = seen_global.iter().any(|(n, _)| n == &upper);
                    if saw {
                        diagnostics.push(XyceParamDiagnostic {
                            message: format!(
                                ".global_param '{name}' redefined — global_params cannot be redefined anywhere"
                            ),
                            line: param.span.start,
                            severity: Severity::Error,
                        });
                    }
                    seen_global.push((upper, param.span.start));
                }
            }
            Statement::Param(param) => {
                for (name, _) in &param.assignments {
                    let upper = name.to_uppercase();
                    let saw = seen_params_per_scope
                        .iter()
                        .any(|(d, n, _)| d == &scope.depth && n == &upper);
                    if saw {
                        diagnostics.push(XyceParamDiagnostic {
                            message: format!(
                                ".param '{name}' redefined in same scope — last definition wins"
                            ),
                            line: param.span.start,
                            severity: Severity::Info,
                        });
                    }
                    seen_params_per_scope.push((scope.depth, upper, param.span.start));
                }
            }
            _ => {}
        }
    }
    for child in &scope.children {
        check_xyce_redefs_in_scope(child, seen_global, seen_params_per_scope, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn build_chains(root: &Scope) -> Vec<(usize, u32, Vec<XyceParamBinding>)> {
        build_xyce_param_scope_chain(root)
    }

    fn top_level() -> Scope {
        Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Param(Param {
                assignments: vec![("g".into(), "5".into())],
                span: 1..2,
            })],
            children: Vec::new(),
            span: 0..1,
        }
    }

    #[test]
    fn test_top_level_param_visible_from_subckt_scope() {
        let mut root = top_level();
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("child".into()),
            depth: 1,
            statements: vec![],
            children: Vec::new(),
            span: 3..4,
        });
        let chains = build_chains(&root);
        let b = resolve_xyce_param_ident(&chains, 1, 1, "g");
        assert!(b.is_some());
        assert_eq!(b.unwrap().value.as_deref(), Some("5"));
    }

    #[test]
    fn test_subckt_local_param_same_name_as_top_level_no_shadow_restore_needed() {
        let mut root = top_level();
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("child".into()),
            depth: 1,
            statements: vec![Statement::Param(Param {
                assignments: vec![("g".into(), "10".into())],
                span: 3..4,
            })],
            children: Vec::new(),
            span: 2..5,
        });
        let chains = build_chains(&root);
        let b = resolve_xyce_param_ident(&chains, 1, 1, "g");
        assert!(b.is_some());
        assert_eq!(b.unwrap().value.as_deref(), Some("10"));
    }

    #[test]
    fn test_global_param_resolves_uniformly_regardless_of_query_origin() {
        let mut root = Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::GlobalParam(Param {
                assignments: vec![("GLOBAL_X".into(), "100".into())],
                span: 1..2,
            })],
            children: Vec::new(),
            span: 0..1,
        };
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("child".into()),
            depth: 1,
            statements: vec![],
            children: Vec::new(),
            span: 3..4,
        });
        let globals = collect_xyce_global_params(&root);
        assert_eq!(globals.len(), 1);
        assert_eq!(globals[0].name, "GLOBAL_X");
        assert_eq!(globals[0].value.as_deref(), Some("100"));
    }

    #[test]
    fn test_global_param_redefinition_anywhere_flagged() {
        let stmts = vec![
            Statement::GlobalParam(Param {
                assignments: vec![("G".into(), "10".into())],
                span: 1..2,
            }),
            Statement::GlobalParam(Param {
                assignments: vec![("G".into(), "20".into())],
                span: 3..4,
            }),
        ];
        let root = Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: stmts,
            children: Vec::new(),
            span: 0..1,
        };
        let diags = check_xyce_param_redefinitions(&root);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("redefined"));
    }

    #[test]
    fn test_param_same_scope_redefinition_is_info_not_error() {
        let stmts = vec![
            Statement::Param(Param {
                assignments: vec![("x".into(), "1".into())],
                span: 1..2,
            }),
            Statement::Param(Param {
                assignments: vec![("x".into(), "2".into())],
                span: 3..4,
            }),
        ];
        let root = Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: stmts,
            children: Vec::new(),
            span: 0..1,
        };
        let diags = check_xyce_param_redefinitions(&root);
        assert!(!diags.is_empty());
        assert!(matches!(diags[0].severity, Severity::Info));
    }

    #[test]
    fn test_sibling_subckts_do_not_see_each_others_local_params() {
        let mut root = Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![],
            children: Vec::new(),
            span: 0..1,
        };
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("A".into()),
            depth: 1,
            statements: vec![Statement::Param(Param {
                assignments: vec![("local_a".into(), "5".into())],
                span: 2..3,
            })],
            children: Vec::new(),
            span: 1..4,
        });
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("B".into()),
            depth: 1,
            statements: vec![],
            children: Vec::new(),
            span: 5..6,
        });
        let chains = build_chains(&root);
        let b = resolve_xyce_param_ident(&chains, 2, 1, "local_a");
        assert!(b.is_none());
    }

    #[test]
    fn test_undefined_returns_none_not_panic() {
        let root = top_level();
        let chains = build_chains(&root);
        let b = resolve_xyce_param_ident(&chains, 0, 0, "nonexistent");
        assert!(b.is_none());
    }
}
