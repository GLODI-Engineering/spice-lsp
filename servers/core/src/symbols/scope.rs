use crate::ast::{LineSpan, Statement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    TopLevel,
    Subckt,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub kind: ScopeKind,
    pub name: Option<String>,
    pub depth: u32,
    pub statements: Vec<Statement>,
    pub children: Vec<Scope>,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeError {
    pub message: String,
    pub span: LineSpan,
}

pub fn build_scope_tree(statements: &[Statement]) -> Result<Scope, Vec<ScopeError>> {
    let mut errors = Vec::new();
    let mut root = new_top_level();
    let mut stack: Vec<Scope> = Vec::new();

    for stmt in statements {
        match stmt {
            Statement::Subckt(subckt) => {
                let child = new_subckt_scope(
                    subckt.name.clone(),
                    stack.len() as u32 + 1,
                    subckt.span.clone(),
                );
                stack.push(child);
            }
            Statement::Ends(name, span) => match stack.pop() {
                Some(mut child) => {
                    if let Some(ends_name) = name {
                        let child_name = child.name.clone().unwrap_or_default();
                        if ends_name.to_uppercase() != child_name.to_uppercase() {
                            errors.push(ScopeError {
                                    message: format!(
                                        ".ends name '{ends_name}' does not match .subckt name '{child_name}'"
                                    ),
                                    span: span.clone(),
                                });
                        }
                    }
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(child);
                    } else {
                        child.depth = 1;
                        root.children.push(child);
                    }
                }
                None => {
                    errors.push(ScopeError {
                        message: ".ends without matching .subckt".into(),
                        span: span.clone(),
                    });
                }
            },
            other => {
                if let Some(current) = stack.last_mut() {
                    current.statements.push(other.clone());
                } else {
                    root.statements.push(other.clone());
                }
            }
        }
    }

    for child in stack {
        let name = child.name.clone().unwrap_or_default();
        errors.push(ScopeError {
            message: format!(".subckt '{name}' has no matching .ends"),
            span: child.span,
        });
    }

    if errors.is_empty() {
        Ok(root)
    } else {
        Err(errors)
    }
}

pub fn build_scope_tree_from_tagged(
    tagged: &[(crate::include::source_map::FileId, Statement)],
) -> Result<Scope, Vec<ScopeError>> {
    let statements: Vec<Statement> = tagged.iter().map(|(_, s)| s.clone()).collect();
    build_scope_tree(&statements)
}

fn new_top_level() -> Scope {
    Scope {
        kind: ScopeKind::TopLevel,
        name: None,
        depth: 0,
        statements: Vec::new(),
        children: Vec::new(),
        span: 0..1,
    }
}

fn new_subckt_scope(name: String, depth: u32, span: LineSpan) -> Scope {
    Scope {
        kind: ScopeKind::Subckt,
        name: Some(name),
        depth,
        statements: Vec::new(),
        children: Vec::new(),
        span,
    }
}

pub fn flatten_scopes(scope: &Scope) -> Vec<&Scope> {
    let mut v = vec![scope];
    for child in &scope.children {
        v.extend(flatten_scopes(child));
    }
    v
}

pub fn collect_subckt_definitions(scope: &Scope) -> Vec<&Scope> {
    let mut v = Vec::new();
    if scope.kind == ScopeKind::Subckt {
        v.push(scope);
    }
    for child in &scope.children {
        v.extend(collect_subckt_definitions(child));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_subckt_stmt(name: &str, span: LineSpan) -> Statement {
        Statement::Subckt(Subckt {
            name: name.into(),
            nodes: vec![],
            params: vec![],
            span,
        })
    }

    fn make_ends(name: Option<&str>, span: LineSpan) -> Statement {
        Statement::Ends(name.map(String::from), span)
    }

    fn make_elem(name: &str) -> Statement {
        Statement::ElementInstance(ElementInstance {
            device_letter: 'R',
            name: name.into(),
            nodes: vec![],
            raw_params: vec![],
            subckt_name: None,
            span: 1..3,
        })
    }

    #[test]
    fn test_top_level_scope_with_no_subckts() {
        let stmts = vec![make_elem("R1")];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.kind, ScopeKind::TopLevel);
        assert!(tree.children.is_empty());
        assert_eq!(tree.statements.len(), 1);
    }

    #[test]
    fn test_single_subckt_produces_one_child_scope() {
        let stmts = vec![
            make_subckt_stmt("opamp", 1..2),
            make_elem("R1"),
            make_ends(Some("opamp"), 3..4),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].name.as_deref(), Some("opamp"));
        assert_eq!(tree.children[0].kind, ScopeKind::Subckt);
        assert_eq!(tree.children[0].statements.len(), 1);
    }

    #[test]
    fn test_nested_subckt_definitions_produce_nested_scopes() {
        let stmts = vec![
            make_subckt_stmt("outer", 1..2),
            make_elem("RTOP"),
            make_subckt_stmt("inner", 3..4),
            make_elem("RBOT"),
            make_ends(Some("inner"), 5..6),
            make_ends(Some("outer"), 7..8),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.children.len(), 1);
        let outer = &tree.children[0];
        assert_eq!(outer.name.as_deref(), Some("outer"));
        assert_eq!(outer.children.len(), 1);
        let inner = &outer.children[0];
        assert_eq!(inner.name.as_deref(), Some("inner"));
        assert_eq!(inner.statements.len(), 1);
        assert_eq!(outer.statements.len(), 1);
    }

    #[test]
    fn test_sibling_subckts_produce_sibling_scopes_not_nested() {
        let stmts = vec![
            make_subckt_stmt("a", 1..2),
            make_ends(Some("a"), 2..3),
            make_subckt_stmt("b", 3..4),
            make_ends(Some("b"), 4..5),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].name.as_deref(), Some("a"));
        assert_eq!(tree.children[1].name.as_deref(), Some("b"));
    }

    #[test]
    fn test_unterminated_subckt_returns_diagnostic_not_panic() {
        let stmts = vec![make_subckt_stmt("bad", 1..2)];
        let result = build_scope_tree(&stmts);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("no matching .ends"));
    }

    #[test]
    fn test_ends_without_open_subckt_returns_diagnostic_not_panic() {
        let stmts = vec![make_ends(Some("orphan"), 1..2)];
        let result = build_scope_tree(&stmts);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs[0].message.contains("without matching .subckt"));
    }

    #[test]
    fn test_ends_name_mismatch_produces_warning() {
        let stmts = vec![
            make_subckt_stmt("opamp", 1..2),
            make_elem("R1"),
            make_ends(Some("wrong"), 3..4),
        ];
        let result = build_scope_tree(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_document() {
        let result = build_scope_tree(&[]).unwrap();
        assert_eq!(result.kind, ScopeKind::TopLevel);
        assert!(result.statements.is_empty());
        assert!(result.children.is_empty());
    }
}
