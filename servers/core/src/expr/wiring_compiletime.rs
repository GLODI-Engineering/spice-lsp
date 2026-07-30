use crate::ast::*;
use crate::dialect::Dialect;
use crate::expr::ast::Expr;
use crate::expr::ngspice_compiletime;
use crate::expr::xyce;
use crate::symbols::scope::Scope;

#[derive(Debug, Clone, PartialEq)]
pub struct WiredExpr {
    pub expr: Option<Expr>,
    pub error: Option<String>,
}

pub fn wire_compiletime_expressions(
    scope_tree: &Scope,
    dialect: Dialect,
) -> Vec<(LineSpan, Option<Expr>, Option<String>)> {
    let mut results = Vec::new();
    wire_in_scope(scope_tree, dialect, &mut results);
    results
}

fn wire_in_scope(
    scope: &Scope,
    dialect: Dialect,
    results: &mut Vec<(LineSpan, Option<Expr>, Option<String>)>,
) {
    for stmt in &scope.statements {
        match stmt {
            Statement::Param(param) => {
                for (_name, value) in &param.assignments {
                    let parsed = parse_expr(value, dialect);
                    results.push((param.span.clone(), parsed.0, parsed.1));
                }
            }
            Statement::GlobalParam(param) => {
                for (_name, value) in &param.assignments {
                    let parsed = parse_expr(value, dialect);
                    results.push((param.span.clone(), parsed.0, parsed.1));
                }
            }
            Statement::Func(func) => {
                let parsed = parse_expr(&func.body, dialect);
                results.push((func.span.clone(), parsed.0, parsed.1));
            }
            Statement::Subckt(subckt) => {
                for (_name, value) in &subckt.params {
                    if let Some(val) = value {
                        let parsed = parse_expr(val, dialect);
                        results.push((subckt.span.clone(), parsed.0, parsed.1));
                    }
                }
            }
            Statement::Model(model) => {
                if is_brace_expression(&model.raw_params) {
                    let inner = &model.raw_params[1..model.raw_params.len() - 1];
                    let parsed = parse_expr(inner, dialect);
                    results.push((model.span.clone(), parsed.0, parsed.1));
                }
            }
            _ => {}
        }
    }
    for child in &scope.children {
        wire_in_scope(child, dialect, results);
    }
}

fn parse_expr(text: &str, dialect: Dialect) -> (Option<Expr>, Option<String>) {
    let clean = strip_braces_and_quotes(text);
    match dialect {
        Dialect::Ngspice => match ngspice_compiletime::parse_ngspice_compiletime(&clean) {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e.message)),
        },
        Dialect::Xyce => match xyce::parse_xyce(&clean) {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e.message)),
        },
    }
}

fn strip_braces_and_quotes(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
        return inner.trim().to_string();
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
        return inner.trim().to_string();
    }
    s.to_string()
}

fn is_brace_expression(s: &str) -> bool {
    s.trim().starts_with('{') && s.trim().ends_with('}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::scope;

    #[test]
    fn test_param_assignment_gets_parsed_expr_ngspice() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Param(Param {
                assignments: vec![("x".into(), "1+2".into())],
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_compiletime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_some());
    }

    #[test]
    fn test_param_assignment_gets_parsed_expr_xyce() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Param(Param {
                assignments: vec![("y".into(), "a+b".into())],
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_compiletime_expressions(&scope, Dialect::Xyce);
        assert_eq!(results.len(), 1);
        assert!(results[0].1.is_some());
    }

    #[test]
    fn test_func_body_gets_parsed_expr() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Func(Func {
                name: "f".into(),
                args: vec!["x".into()],
                body: "x+1".into(),
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_compiletime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_model_brace_expression_parsed() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Model(Model {
                name: "M1".into(),
                model_type: "NMOS".into(),
                raw_params: "{BF=100}".into(),
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_compiletime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_model_plain_number_not_force_parsed_as_expression() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::Model(Model {
                name: "M1".into(),
                model_type: "NMOS".into(),
                raw_params: "BF=100".into(),
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_compiletime_expressions(&scope, Dialect::Ngspice);
        assert!(results.is_empty());
    }

    #[test]
    fn test_one_bad_param_does_not_block_rest_of_document() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![
                Statement::Param(Param {
                    assignments: vec![("bad".into(), "(".into())],
                    span: 1..2,
                }),
                Statement::Param(Param {
                    assignments: vec![("good".into(), "42".into())],
                    span: 3..4,
                }),
            ],
            children: vec![],
            span: 0..1,
        };
        let results = wire_compiletime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 2);
    }
}
