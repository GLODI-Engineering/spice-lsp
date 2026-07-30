use crate::dialect::Dialect;
use crate::expr::ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
}

pub fn lint_caret_dialect_confusion(expr: &Expr, dialect: Dialect) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    lint_caret_recursive(expr, dialect, &mut diags);
    diags
}

fn lint_caret_recursive(expr: &Expr, dialect: Dialect, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::BinaryOp { op, lhs, rhs, .. } => {
            if op == "^" {
                let (meaning, other_meaning) = match dialect {
                    Dialect::Ngspice => ("power", "boolean XOR"),
                    Dialect::Xyce => ("boolean XOR", "power"),
                };
                diags.push(Diagnostic {
                    severity: Severity::Info,
                    message: format!(
                        "operator ^ means {meaning} in {:?} dialect (note: it means {other_meaning} in the other dialect)",
                        dialect
                    ),
                });
            }
            lint_caret_recursive(lhs, dialect, diags);
            lint_caret_recursive(rhs, dialect, diags);
        }
        Expr::UnaryOp { operand, .. } => {
            lint_caret_recursive(operand, dialect, diags);
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            lint_caret_recursive(condition, dialect, diags);
            lint_caret_recursive(then_branch, dialect, diags);
            lint_caret_recursive(else_branch, dialect, diags);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                lint_caret_recursive(arg, dialect, diags);
            }
        }
        Expr::Reference { .. } | Expr::Number(..) | Expr::Ident(..) | Expr::StringLit(..) => {}
    }
}

pub fn lint_log_dialect_confusion(expr: &Expr, dialect: Dialect) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    lint_log_recursive(expr, dialect, &mut diags);
    diags
}

fn lint_log_recursive(expr: &Expr, dialect: Dialect, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Call { name, args, .. } => {
            match name.as_str() {
                "LOG" => {
                    let (base, other_base) = match dialect {
                        Dialect::Ngspice => ("natural log (base e)", "base-10 log"),
                        Dialect::Xyce => ("base-10 log", "natural log (base e)"),
                    };
                    let mut msg = format!(
                        "log() means {base} in {:?} dialect (it would be {other_base} in the other dialect)",
                        dialect
                    );
                    if dialect == Dialect::Xyce {
                        msg.push_str(" — under -hspice-ext math, log() becomes natural log");
                    }
                    diags.push(Diagnostic {
                        severity: Severity::Info,
                        message: msg,
                    });
                }
                "LOG10" => {
                    let msg = match dialect {
                        Dialect::Ngspice => {
                            "log10() is base-10 log in ngspice (same as Xyce default)".into()
                        }
                        Dialect::Xyce => {
                            "log10() is base-10 log in Xyce (note: log() is also base-10 by default)".into()
                        }
                    };
                    diags.push(Diagnostic {
                        severity: Severity::Info,
                        message: msg,
                    });
                }
                "LN" => {
                    let msg = match dialect {
                        Dialect::Ngspice => "ln() is natural log in ngspice (same as log())".into(),
                        Dialect::Xyce => {
                            "ln() is natural log in Xyce — use this for base-e instead of log()"
                                .into()
                        }
                    };
                    diags.push(Diagnostic {
                        severity: Severity::Info,
                        message: msg,
                    });
                }
                _ => {}
            }
            for arg in args {
                lint_log_recursive(arg, dialect, diags);
            }
        }
        Expr::BinaryOp { lhs, rhs, .. } => {
            lint_log_recursive(lhs, dialect, diags);
            lint_log_recursive(rhs, dialect, diags);
        }
        Expr::UnaryOp { operand, .. } => {
            lint_log_recursive(operand, dialect, diags);
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            lint_log_recursive(condition, dialect, diags);
            lint_log_recursive(then_branch, dialect, diags);
            lint_log_recursive(else_branch, dialect, diags);
        }
        Expr::Reference { .. } | Expr::Number(..) | Expr::Ident(..) | Expr::StringLit(..) => {}
    }
}

pub fn lint_xyce_ternary_colon_collision(expr: &Expr) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    lint_ternary_recursive(expr, &mut diags);
    diags
}

fn lint_ternary_recursive(expr: &Expr, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            if let Expr::Ident(_) = &**then_branch {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "bare identifier before ':' in ternary — add a space or wrap in parens to avoid ambiguity with hierarchical node paths".into(),
                });
            }
            lint_ternary_recursive(condition, diags);
            lint_ternary_recursive(then_branch, diags);
            lint_ternary_recursive(else_branch, diags);
        }
        Expr::BinaryOp { lhs, rhs, .. } => {
            lint_ternary_recursive(lhs, diags);
            lint_ternary_recursive(rhs, diags);
        }
        Expr::UnaryOp { operand, .. } => {
            lint_ternary_recursive(operand, diags);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                lint_ternary_recursive(arg, diags);
            }
        }
        Expr::Reference { .. } | Expr::Number(..) | Expr::Ident(..) | Expr::StringLit(..) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caret_lint(src: &str, dialect: Dialect) -> Vec<Diagnostic> {
        let expr = crate::expr::ngspice_compiletime::parse_ngspice_compiletime(src).unwrap();
        lint_caret_dialect_confusion(&expr, dialect)
    }

    fn log_lint(src: &str, dialect: Dialect) -> Vec<Diagnostic> {
        let expr = crate::expr::ngspice_compiletime::parse_ngspice_compiletime(src).unwrap();
        lint_log_dialect_confusion(&expr, dialect)
    }

    #[test]
    fn test_caret_lint_fires_with_correct_meaning_per_dialect() {
        let diags = caret_lint("a^b", Dialect::Ngspice);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("power"));

        let diags = caret_lint("a^b", Dialect::Xyce);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("XOR"));
    }

    #[test]
    fn test_log_lint_states_base_per_dialect() {
        let diags = log_lint("log(x)", Dialect::Ngspice);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("natural"));

        let diags = log_lint("log(x)", Dialect::Xyce);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("base-10"));
    }

    #[test]
    fn test_log_lint_notes_hspice_ext_math_caveat_for_xyce() {
        let diags = log_lint("log(x)", Dialect::Xyce);
        assert!(diags[0].message.contains("hspice-ext math"));
    }

    #[test]
    fn test_xyce_ternary_colon_collision_flagged_on_bare_identifier() {
        let expr = Expr::ternary(Expr::ident("a"), Expr::ident("b"), Expr::ident("c"));
        let diags = lint_xyce_ternary_colon_collision(&expr);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("bare identifier"));
    }

    #[test]
    fn test_xyce_ternary_colon_collision_not_flagged_on_hierarchical_node_path() {
        let vref = Expr::Reference {
            kind: RefKind::V,
            args: vec!["Xmain:Xnot1:A".into()],
        };
        let expr = Expr::ternary(Expr::ident("a"), vref, Expr::ident("c"));
        let diags = lint_xyce_ternary_colon_collision(&expr);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_xyce_ternary_colon_collision_not_flagged_when_parenthesized() {
        let expr = Expr::ternary(
            Expr::ident("a"),
            Expr::binary("+", Expr::ident("b"), Expr::number(1.0)),
            Expr::ident("c"),
        );
        let diags = lint_xyce_ternary_colon_collision(&expr);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_lints_fire_on_dialect_correct_unambiguous_expression() {
        let diags = caret_lint("a+b*c", Dialect::Ngspice);
        assert!(diags.is_empty());

        let diags = log_lint("a+b*c", Dialect::Ngspice);
        assert!(diags.is_empty());
    }
}
