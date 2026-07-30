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

            // B sources: V= and I= forms
            if letter == 'B' {
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

            // E/G/F/H sources: TABLE/POLY forms + VALUE form
            if matches!(letter, 'E' | 'G' | 'F' | 'H') && !ei.raw_params.is_empty() {
                let first = ei.raw_params[0].to_uppercase();
                let joined = ei.raw_params.join(" ");

                if first == "TABLE" {
                    if dialect == Dialect::Xyce {
                        match crate::expr::xyce_sources::parse_xyce_table(&joined) {
                            Ok(_) => {
                                results.push(WiredRuntimeExpr {
                                    span: ei.span.clone(),
                                    expr: None,
                                    error: None,
                                });
                            }
                            Err(e) => {
                                results.push(WiredRuntimeExpr {
                                    span: ei.span.clone(),
                                    expr: None,
                                    error: Some(e.message),
                                });
                            }
                        }
                    } else {
                        let parsed = parse_runtime(&joined, dialect);
                        results.push(WiredRuntimeExpr {
                            span: ei.span.clone(),
                            expr: parsed.0,
                            error: parsed.1,
                        });
                    }
                } else if first.starts_with("POLY") {
                    let parsed = parse_runtime(&joined, dialect);
                    results.push(WiredRuntimeExpr {
                        span: ei.span.clone(),
                        expr: parsed.0,
                        error: parsed.1,
                    });
                } else if first == "VALUE" {
                    // VALUE = {expr} form
                    let rest = ei.raw_params[1..].join(" ");
                    if let Some(eq_text) = rest.strip_prefix("=") {
                        let parsed = parse_runtime(eq_text.trim(), dialect);
                        results.push(WiredRuntimeExpr {
                            span: ei.span.clone(),
                            expr: parsed.0,
                            error: parsed.1,
                        });
                    }
                } else {
                    // Plain form: V= / I= on E/G/F/H
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
            }

            // Behavioral R=/C=/L=/Q= on passives
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

    #[test]
    fn test_e_source_table_form_produces_wired_expression() {
        // Xyce-style: ET2 2 0 TABLE {V(ANODE,CATHODE)} = (0,0) (30,1)
        let scope = Scope {
            kind: scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: vec![Statement::ElementInstance(ElementInstance {
                device_letter: 'E',
                name: "ET2".into(),
                nodes: vec!["2".into(), "0".into()],
                raw_params: vec![
                    "TABLE".into(),
                    "{V(ANODE,CATHODE)}".into(),
                    "=".into(),
                    "(0,0)".into(),
                    "(30,1)".into(),
                ],
                subckt_name: None,
                span: 1..2,
            })],
            children: vec![],
            span: 0..1,
        };
        let results = wire_runtime_expressions(&scope, Dialect::Xyce);
        assert!(
            !results.is_empty(),
            "TABLE-form E-source should produce at least one wired expression"
        );
    }
}
