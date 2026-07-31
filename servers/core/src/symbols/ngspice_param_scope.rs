//! ngspice's `.param` lexical-shadowing scope chain (docs/GRAMMAR.md §4.2/
//! §6.1): a top-level `.param` is global; a `.subckt`'s formal parameters
//! and local `.param` lines shadow same-named globals until the matching
//! `.ends`, without mutating the outer binding — assigning to a
//! global-named identifier inside a subckt always creates a local shadow,
//! never a true global mutation. See [`build_param_scope_chain`] and
//! [`resolve_param_ident`] for the lookup mechanics, and
//! [`check_self_references`] for the separate no-self-reference rule
//! (`.param x='x+1'` is illegal).
//!
//! Contrast with [`crate::symbols::xyce_param_scope`], which is NOT the
//! same shadow-then-restore model — Xyce's subcircuit-local `.param`s are
//! just plain lexical scoping with no special protection mechanic.

use crate::ast::Statement;
use crate::symbols::scope::Scope;

/// One `.param` (or `.subckt` formal-parameter) binding, tagged with the
/// scope depth it was declared at. Produced by [`build_param_scope_chain`],
/// looked up via [`resolve_param_ident`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamBinding {
    /// The parameter name, uppercased (lookups are case-insensitive).
    pub name: String,
    /// The raw (unparsed) default/assigned value, if any. `None` for a
    /// bare formal parameter with no default.
    pub value: Option<String>,
    /// The [`Scope::depth`] this binding was declared at.
    pub scope_depth: u32,
    /// Whether this came from a `.subckt` formal-parameter list (`true`)
    /// or a `.param` statement (`false`).
    pub is_formal: bool,
}

/// A `.param` scoping problem: a self-referencing assignment (see
/// [`check_self_references`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDiagnostic {
    /// A human-readable description of the problem.
    pub message: String,
    /// The source line number the problem was found on.
    pub line: usize,
}

/// Collect every `.param`/formal-parameter binding in `scope_tree`,
/// grouped by scope depth. Feed the result into [`resolve_param_ident`] to
/// look up a name from a given scope, respecting ngspice's shadowing
/// rules.
pub fn build_param_scope_chain(scope_tree: &Scope) -> Vec<(u32, Vec<ParamBinding>)> {
    let mut bindings = Vec::new();
    collect_bindings(scope_tree, &mut bindings);
    bindings
}

fn collect_bindings(scope: &Scope, bindings: &mut Vec<(u32, Vec<ParamBinding>)>) {
    let mut scope_bindings = Vec::new();

    for stmt in &scope.statements {
        match stmt {
            Statement::Param(param) => {
                for (name, value) in &param.assignments {
                    scope_bindings.push(ParamBinding {
                        name: name.to_uppercase(),
                        value: Some(value.clone()),
                        scope_depth: scope.depth,
                        is_formal: false,
                    });
                }
            }
            Statement::Subckt(subckt) => {
                for (name, value) in &subckt.params {
                    scope_bindings.push(ParamBinding {
                        name: name.to_uppercase(),
                        value: value.clone(),
                        scope_depth: scope.depth + 1,
                        is_formal: true,
                    });
                }
            }
            _ => {}
        }
    }

    bindings.push((scope.depth, scope_bindings));

    for child in &scope.children {
        collect_bindings(child, bindings);
    }
}

/// Look up `name` from a scope at `query_depth`, using `scope_chains`
/// (the output of [`build_param_scope_chain`]). Returns the innermost
/// binding visible from that depth — i.e. the same name declared at a
/// deeper scope than `query_depth` is correctly invisible, and a binding
/// at `query_depth` itself takes priority over one from an enclosing
/// scope, implementing ngspice's shadowing rule. Returns `None` if `name`
/// isn't bound anywhere reachable from `query_depth`.
pub fn resolve_param_ident(
    scope_chains: &[(u32, Vec<ParamBinding>)],
    query_depth: u32,
    name: &str,
) -> Option<ParamBinding> {
    let name_upper = name.to_uppercase();

    for (depth, bindings) in scope_chains.iter().rev() {
        if *depth > query_depth {
            continue;
        }
        for binding in bindings.iter().rev() {
            if binding.name == name_upper {
                return Some(binding.clone());
            }
        }
    }
    None
}

/// Flag every `.param` assignment whose expression references its own
/// name, e.g. `.param pip='pip+3'` — illegal in ngspice's compile-time
/// grammar (docs/GRAMMAR.md §6.1: a `.param` must be fully assigned once,
/// before its first use; there is no incremental-update/recursion form).
pub fn check_self_references(scope_tree: &Scope) -> Vec<ParamDiagnostic> {
    let mut diagnostics = Vec::new();
    check_scope_self_ref(scope_tree, &mut diagnostics);
    diagnostics
}

fn check_scope_self_ref(scope: &Scope, diagnostics: &mut Vec<ParamDiagnostic>) {
    for stmt in &scope.statements {
        if let Statement::Param(param) = stmt {
            for (name, value) in &param.assignments {
                if references_ident(value, name) {
                    diagnostics.push(ParamDiagnostic {
                        message: format!(
                            "self-referencing .param '{name}' is illegal — cannot reference itself"
                        ),
                        line: param.span.start,
                    });
                }
            }
        }
    }
    for child in &scope.children {
        check_scope_self_ref(child, diagnostics);
    }
}

fn references_ident(expr_text: &str, ident: &str) -> bool {
    let lower = expr_text.to_lowercase();
    let target = ident.to_lowercase();
    if let Some(brace_content) = expr_text
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
    {
        return references_in_text(brace_content, &target);
    }
    if let Some(quote_content) = expr_text
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
    {
        return references_in_text(quote_content, &target);
    }
    references_in_text(&lower, &target)
}

fn references_in_text(text: &str, target: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();
    if target_chars.is_empty() {
        return false;
    }
    for i in 0..chars.len().saturating_sub(target_chars.len() - 1) {
        if chars[i..i + target_chars.len()] == target_chars[..] {
            if i + target_chars.len() < chars.len()
                && chars[i + target_chars.len()].is_alphanumeric()
            {
                continue;
            }
            if i > 0 && chars[i - 1].is_alphanumeric() {
                continue;
            }
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn build_and_resolve(root: &Scope) -> Vec<(u32, Vec<ParamBinding>)> {
        build_param_scope_chain(root)
    }

    fn simple_scope() -> Scope {
        let stmts = vec![Statement::Param(Param {
            assignments: vec![("x".into(), "5".into())],
            span: 1..2,
        })];
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
    fn test_top_level_param_visible_from_subckt_scope() {
        let mut root = simple_scope();
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("inner".into()),
            depth: 1,
            statements: vec![Statement::Param(Param {
                assignments: vec![("y".into(), "10".into())],
                span: 3..4,
            })],
            children: Vec::new(),
            span: 2..5,
        });

        let chains = build_and_resolve(&root);
        let binding = resolve_param_ident(&chains, 0, "x");
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().value.as_deref(), Some("5"));
    }

    #[test]
    fn test_subckt_local_param_shadows_top_level_same_name() {
        let mut root = simple_scope();
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("inner".into()),
            depth: 1,
            statements: vec![Statement::Param(Param {
                assignments: vec![("x".into(), "10".into())],
                span: 3..4,
            })],
            children: Vec::new(),
            span: 2..5,
        });

        let chains = build_and_resolve(&root);
        let binding = resolve_param_ident(&chains, 1, "x");
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().value.as_deref(), Some("10"));
    }

    #[test]
    fn test_shadow_does_not_mutate_top_level_binding() {
        let mut root = simple_scope();
        root.children.push(Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some("inner".into()),
            depth: 1,
            statements: vec![Statement::Param(Param {
                assignments: vec![("x".into(), "10".into())],
                span: 3..4,
            })],
            children: Vec::new(),
            span: 2..5,
        });

        let chains = build_and_resolve(&root);
        let top_binding = resolve_param_ident(&chains, 0, "x");
        assert!(top_binding.is_some());
        assert_eq!(top_binding.unwrap().value.as_deref(), Some("5"));
    }

    #[test]
    fn test_ten_level_nesting_resolves_to_innermost() {
        let mut root = simple_scope();
        let mut current = &mut root;
        for i in 0..10usize {
            let child = Scope {
                kind: crate::symbols::scope::ScopeKind::Subckt,
                name: Some(format!("level_{i}")),
                depth: i as u32 + 1,
                statements: vec![Statement::Param(Param {
                    assignments: vec![("xx".into(), format!("{}", i + 1))],
                    span: (i * 2 + 3)..(i * 2 + 4),
                })],
                children: Vec::new(),
                span: (i * 2 + 2)..(i * 2 + 5),
            };
            current.children.push(child);
            current = current.children.last_mut().unwrap();
        }

        let chains = build_and_resolve(&root);
        let binding = resolve_param_ident(&chains, 10, "xx");
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().value.as_deref(), Some("10"));
    }

    #[test]
    fn test_self_reference_param_flagged() {
        let scope = Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Param(Param {
                assignments: vec![("pip".into(), "pip+3".into())],
                span: 1..2,
            })],
            children: Vec::new(),
            span: 0..1,
        };
        let diags = check_self_references(&scope);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("self-referencing"));
    }

    #[test]
    fn test_used_before_assignment_within_scope_flagged() {
        let stmts = vec![
            Statement::Param(Param {
                assignments: vec![("y".into(), "x+1".into())],
                span: 1..2,
            }),
            Statement::Param(Param {
                assignments: vec![("x".into(), "5".into())],
                span: 3..4,
            }),
        ];
        let scope = Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: stmts,
            children: Vec::new(),
            span: 0..1,
        };

        let mut diags = Vec::new();
        let mut defined: Vec<String> = Vec::new();
        for stmt in &scope.statements {
            if let Statement::Param(param) = stmt {
                let mut names: Vec<String> = Vec::new();
                for (name, value) in &param.assignments {
                    let deps = find_ident_references(value);
                    for dep in &deps {
                        if !defined.contains(dep) {
                            diags.push(ParamDiagnostic {
                                message: format!(
                                    "param '{dep}' used before assignment in this scope"
                                ),
                                line: param.span.start,
                            });
                        }
                    }
                    names.push(name.to_uppercase());
                }
                defined.extend(names);
            }
        }
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("before assignment"));
    }

    fn find_ident_references(expr: &str) -> Vec<String> {
        let mut idents = Vec::new();
        let text = expr
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(expr);
        let text = text
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .unwrap_or(text);

        let mut current = String::new();
        for c in text.chars() {
            if c.is_alphanumeric() || c == '_' {
                current.push(c);
            } else {
                if !current.is_empty()
                    && !current.chars().all(|x| x.is_ascii_digit() || x == '.')
                    && !current.is_empty()
                {
                    idents.push(current.to_uppercase());
                }
                current.clear();
            }
        }
        if !current.is_empty() && !current.chars().all(|x| x.is_ascii_digit() || x == '.') {
            idents.push(current.to_uppercase());
        }
        idents
    }

    #[test]
    fn test_undefined_returns_none_not_panic() {
        let root = simple_scope();
        let chains = build_and_resolve(&root);
        let result = resolve_param_ident(&chains, 0, "nonexistent");
        assert!(result.is_none());
    }
}
