use crate::ast::*;
use crate::dialect::Dialect;
use crate::expr::ast::Expr;
use crate::expr::ngspice_runtime;
use crate::expr::xyce;
use crate::symbols::scope::Scope;

#[derive(Debug, Clone)]
pub struct WiredRuntimeExpr {
    pub span: LineSpan,
    pub expr: Option<Expr>,
    pub error: Option<String>,
}

pub fn wire_runtime_expressions(scope_tree: &Scope, dialect: Dialect) -> Vec<WiredRuntimeExpr> {
    let mut results = Vec::new();
    wire_runtime_in_scope(scope_tree, dialect, &mut results);
    results
}

fn wire_runtime_in_scope(scope: &Scope, dialect: Dialect, results: &mut Vec<WiredRuntimeExpr>) {
    for stmt in &scope.statements {
        if let Statement::ElementInstance(ei) = stmt {
            let letter = ei.device_letter.to_ascii_uppercase();
            let is_behavioral = matches!(letter, 'B' | 'E' | 'G' | 'F' | 'H');

            if is_behavioral {
                for param in &ei.raw_params {
                    if let Some(rest) = param
                        .strip_prefix("V=")
                        .or_else(|| param.strip_prefix("I="))
                    {
                        let parsed = parse_runtime(rest, dialect);
                        results.push(WiredRuntimeExpr {
                            span: ei.span.clone(),
                            expr: parsed.0,
                            error: parsed.1,
                        });
                    }
                }
            }

            if matches!(letter, 'R' | 'C' | 'L') {
                for param in &ei.raw_params {
                    if let Some(rest) = param
                        .strip_prefix("R=")
                        .or_else(|| param.strip_prefix("C="))
                        .or_else(|| param.strip_prefix("L="))
                        .or_else(|| param.strip_prefix("Q="))
                    {
                        let parsed = parse_runtime(rest, dialect);
                        results.push(WiredRuntimeExpr {
                            span: ei.span.clone(),
                            expr: parsed.0,
                            error: parsed.1,
                        });
                    }
                }
            }
        }
    }

    for child in &scope.children {
        wire_runtime_in_scope(child, dialect, results);
    }
}

fn parse_runtime(text: &str, dialect: Dialect) -> (Option<Expr>, Option<String>) {
    match dialect {
        Dialect::Ngspice => match ngspice_runtime::parse_ngspice_runtime(text) {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e.message)),
        },
        Dialect::Xyce => match xyce::parse_xyce(text) {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e.message)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::scope;

    #[test]
    fn test_b_source_v_equals_parsed_ngspice() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::ElementInstance(ElementInstance {
                device_letter: 'B',
                name: "B1".into(),
                nodes: vec!["out".into(), "0".into()],
                raw_params: vec!["V=V(in)*2".into()],
                subckt_name: None,
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_runtime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
        assert!(results[0].expr.is_some());
    }

    #[test]
    fn test_b_source_i_equals_parsed_ngspice() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::ElementInstance(ElementInstance {
                device_letter: 'B',
                name: "B2".into(),
                nodes: vec!["out".into(), "0".into()],
                raw_params: vec!["I=V(in)/1000".into()],
                subckt_name: None,
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_runtime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
        assert!(results[0].expr.is_some());
    }

    #[test]
    fn test_behavioral_r_equals_parsed_like_b_source() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::ElementInstance(ElementInstance {
                device_letter: 'R',
                name: "R1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec!["R=V(1,2)/I(Vsense)".into()],
                subckt_name: None,
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_runtime_expressions(&scope, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_plain_passive_device_untouched_by_runtime_wiring() {
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::ElementInstance(ElementInstance {
                device_letter: 'R',
                name: "R1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec!["100".into()],
                subckt_name: None,
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_runtime_expressions(&scope, Dialect::Ngspice);
        assert!(results.is_empty());
    }
}
