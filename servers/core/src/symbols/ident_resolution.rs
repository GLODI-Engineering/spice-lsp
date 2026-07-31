//! Resolves identifiers inside a parsed expression tree against a set of
//! known-defined parameter names, producing "undefined parameter"
//! diagnostics — [`resolve_expr_idents`] is the entry point. Built-in
//! function names, reserved special variables (`time`/`temper`/`hertz` in
//! ngspice; `TIME`/`FREQ`/`TEMP`/`VT`/`GMIN` in Xyce), and identifiers
//! that are really node/device names inside a `V()`/`I()`/`N()` reference
//! are all correctly excluded — see `resolve_in_expr`'s match arms.

use crate::dialect::Dialect;
use crate::expr::ast::*;
use crate::expr::ngspice_compiletime;
use crate::expr::ngspice_runtime;
use crate::expr::xyce;

/// An identifier inside an expression that doesn't resolve to any known
/// parameter, built-in function, or reserved special variable. Found by
/// [`resolve_expr_idents`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentDiagnostic {
    /// A human-readable message naming the undefined identifier.
    pub message: String,
    /// The undefined identifier itself.
    pub name: String,
}

/// Walk `expr` and flag every `Ident` that isn't in `defined_params`, a
/// known built-in function name for `dialect`, or a reserved special
/// variable for `dialect`. Identifiers inside a `V()`/`I()`/`N()`
/// [`Expr::Reference`] are node/device names, not parameter names, and are
/// never flagged here — resolving those against the circuit's node set is
/// a separate concern this function doesn't attempt.
pub fn resolve_expr_idents(
    expr: &Expr,
    dialect: Dialect,
    defined_params: &[String],
) -> Vec<IdentDiagnostic> {
    let mut diagnostics = Vec::new();
    resolve_in_expr(expr, dialect, defined_params, &mut diagnostics);
    diagnostics
}

fn resolve_in_expr(
    expr: &Expr,
    dialect: Dialect,
    defined_params: &[String],
    diagnostics: &mut Vec<IdentDiagnostic>,
) {
    match expr {
        Expr::Ident(name) => {
            let upper = name.to_uppercase();
            if is_builtin(&upper, dialect) {
                return;
            }
            if is_reserved_special(&upper, dialect) {
                return;
            }
            if defined_params.iter().any(|p| p.to_uppercase() == upper) {
                return;
            }
            diagnostics.push(IdentDiagnostic {
                message: format!("undefined parameter: '{name}'"),
                name: name.clone(),
            });
        }
        Expr::Call { name, args, .. } => {
            for arg in args {
                resolve_in_expr(arg, dialect, defined_params, diagnostics);
            }
            let _ = name;
        }
        Expr::BinaryOp { lhs, rhs, .. } => {
            resolve_in_expr(lhs, dialect, defined_params, diagnostics);
            resolve_in_expr(rhs, dialect, defined_params, diagnostics);
        }
        Expr::UnaryOp { operand, .. } => {
            resolve_in_expr(operand, dialect, defined_params, diagnostics);
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            resolve_in_expr(condition, dialect, defined_params, diagnostics);
            resolve_in_expr(then_branch, dialect, defined_params, diagnostics);
            resolve_in_expr(else_branch, dialect, defined_params, diagnostics);
        }
        Expr::Reference { .. } => {}
        Expr::Number(..) | Expr::StringLit(..) => {}
    }
}

fn is_builtin(name: &str, dialect: Dialect) -> bool {
    match dialect {
        Dialect::Ngspice => {
            ngspice_compiletime::is_builtin(name) || ngspice_runtime::is_builtin(name)
        }
        Dialect::Xyce => xyce::is_builtin(name),
    }
}

fn is_reserved_special(name: &str, dialect: Dialect) -> bool {
    let upper = name.to_uppercase();
    match dialect {
        Dialect::Ngspice => {
            matches!(upper.as_str(), "TIME" | "TEMPER" | "HERTZ")
        }
        Dialect::Xyce => {
            matches!(
                upper.as_str(),
                "TIME" | "FREQ" | "HERTZ" | "VT" | "TEMP" | "TEMPER" | "GMIN"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(src: &str, dialect: Dialect, params: &[&str]) -> Vec<IdentDiagnostic> {
        let expr = match dialect {
            Dialect::Ngspice => match ngspice_compiletime::parse_ngspice_compiletime(src) {
                Ok(e) => e,
                Err(_) => return vec![],
            },
            Dialect::Xyce => xyce::parse_xyce(src).unwrap(),
        };
        let defined: Vec<String> = params.iter().map(|s| s.to_string()).collect();
        resolve_expr_idents(&expr, dialect, &defined)
    }

    #[test]
    fn test_undefined_param_ident_flagged_ngspice() {
        let diags = resolve("unknown_var + 1", Dialect::Ngspice, &["x"]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined"));
        assert_eq!(diags[0].name, "unknown_var");
    }

    #[test]
    fn test_undefined_param_ident_flagged_xyce() {
        let diags = resolve("unknown_var + 1", Dialect::Xyce, &["x"]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined"));
    }

    #[test]
    fn test_builtin_function_name_never_flagged() {
        let diags_ng = resolve("sin(x)", Dialect::Ngspice, &["x"]);
        assert!(diags_ng.is_empty());

        let diags_xy = resolve("SQRT(x)", Dialect::Xyce, &["x"]);
        assert!(diags_xy.is_empty());
    }

    #[test]
    fn test_reference_arguments_skipped_not_flagged_as_undefined_param() {
        let expr = Expr::Reference {
            kind: RefKind::V,
            args: vec!["unknown_node".into()],
        };
        let diags = resolve_expr_idents(&expr, Dialect::Ngspice, &[]);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_reserved_special_variables_never_flagged() {
        let diags = resolve("TEMP + 1", Dialect::Xyce, &[]);
        assert!(diags.is_empty());

        let diags = resolve("FREQ + 1", Dialect::Xyce, &[]);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_subckt_formal_param_default_resolves_as_defined() {
        let diags = resolve("gain * 2", Dialect::Ngspice, &["gain"]);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_deeply_nested_valid_reference_resolves() {
        let diags = resolve("sin(cos(x))", Dialect::Ngspice, &["x"]);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_edge_case_pure_literal_no_diagnostics() {
        let diags = resolve("42 + 3.14", Dialect::Ngspice, &[]);
        assert!(diags.is_empty());
    }
}
