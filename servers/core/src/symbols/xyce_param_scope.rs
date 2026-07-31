//! Xyce's `.param`/`.global_param` scoping (docs/GRAMMAR.md §4.2):
//! top-level `.param`/`.model`/`.func` are visible everywhere; a
//! subcircuit-local `.param` is visible only within that subcircuit and
//! ones nested inside it. Deliberately **not** the same shadow-then-
//! restore model [`crate::symbols::ngspice_param_scope`] implements for
//! ngspice — Xyce has no documented "local copy created when reassigning
//! a global name" mechanic, just plain lexical scoping. `.global_param`
//! is a stricter, separate mechanism: a flat namespace with no nesting
//! behavior at all, top-level-declaration-only, and (unlike `.param`)
//! never legal to redefine across hierarchy levels — see
//! [`collect_xyce_global_params`] and [`check_xyce_param_redefinitions`].

use crate::ast::Statement;
use crate::symbols::scope::Scope;

/// One `.param` (or `.subckt` formal-parameter) binding in Xyce's scope
/// model, tagged with the scope depth it was declared at. Produced by
/// [`build_xyce_param_scope_chain`], looked up via
/// [`resolve_xyce_param_ident`]. Unlike ngspice's
/// [`crate::symbols::ngspice_param_scope::ParamBinding`], lookup here is
/// scoped by a specific scope *identity* (see
/// [`resolve_xyce_param_ident`]'s `scope_id`), not just depth — because
/// Xyce siblings at the same depth must not see each other's bindings,
/// while ngspice's depth-only model doesn't need that distinction the
/// same way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyceParamBinding {
    /// The parameter name, uppercased (lookups are case-insensitive).
    pub name: String,
    /// The raw (unparsed) default/assigned value, if any.
    pub value: Option<String>,
    /// The [`Scope::depth`] this binding was declared at.
    pub scope_depth: u32,
}

/// One `.global_param` declaration, found by [`collect_xyce_global_params`].
/// Unlike [`XyceParamBinding`], this has no scope depth — `.global_param`
/// is a flat namespace, resolvable identically regardless of where in the
/// document the lookup originates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyceGlobalParam {
    /// The parameter name, uppercased.
    pub name: String,
    /// The raw (unparsed) assigned value.
    pub value: Option<String>,
}

/// A `.param`/`.global_param` scoping problem, found by
/// [`check_xyce_param_redefinitions`]. Severity varies by case — see
/// [`Severity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XyceParamDiagnostic {
    /// A human-readable description of the problem.
    pub message: String,
    /// The source line number the problem was found on.
    pub line: usize,
    /// How serious this particular case is — see [`Severity`].
    pub severity: Severity,
}

/// How serious a [`XyceParamDiagnostic`] is. `.global_param` redefinition
/// across hierarchy levels is always [`Severity::Error`] (illegal per
/// docs/GRAMMAR.md §4.2); same-scope `.param` redefinition is only
/// [`Severity::Info`] (last-definition-wins is tolerated, per the
/// confirmed Xyce redefinition-tolerance finding — this is not an error
/// case, just worth surfacing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Worth surfacing in a hover/lint UI, but not a real problem.
    Info,
    /// Suspicious but not confirmed illegal.
    Warning,
    /// A confirmed violation of Xyce's scoping rules.
    Error,
}

/// Collect every `.param`/formal-parameter binding in `scope_tree`,
/// grouped by a unique scope identity and depth. Feed the result into
/// [`resolve_xyce_param_ident`] to look up a name from a given scope,
/// respecting Xyce's downward-only lexical visibility (a sibling
/// subcircuit's local `.param`s are never visible, unlike a simple
/// depth-only model would allow).
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

/// Look up `name` from the scope identified by `scope_id` (at
/// `query_depth`), using `scope_chains` (the output of
/// [`build_xyce_param_scope_chain`]). Checks that exact scope's own
/// bindings first, then walks up through shallower-depth, lower-ID
/// ancestor scopes — never sideways to a sibling scope at the same or
/// deeper level, which is what makes this correctly Xyce's downward-only
/// visibility rather than ngspice's depth-only shadowing. Returns `None`
/// if `name` isn't bound anywhere reachable.
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

/// Collect every `.global_param` declaration anywhere in `scope_tree`.
/// Unlike `.param` bindings, these have no depth/scope-identity structure
/// to resolve against — a `.global_param` is visible identically from
/// anywhere in the document (docs/GRAMMAR.md §4.2), so a flat list is all
/// that's needed. Use [`check_xyce_param_redefinitions`] to detect illegal
/// redefinition (more than one declaration of the same name, anywhere).
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

/// Find `.param`/`.global_param` redefinition problems in `scope_tree`.
/// `.global_param` redefined anywhere (even at the same top-level scope)
/// is a [`Severity::Error`] — docs/GRAMMAR.md §4.2 confirms this is
/// stricter than plain `.param`, which tolerates same-scope redefinition
/// (last-definition-wins), surfaced here as [`Severity::Info`] rather than
/// treated as a hard error.
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
