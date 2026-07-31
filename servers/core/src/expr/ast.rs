//! The dialect-agnostic expression AST ([`Expr`]) shared by every
//! expression sub-grammar in this crate (ngspice compile-time/runtime,
//! Xyce). Each dialect's tokenizer/parser in [`crate::expr`] produces this
//! same tree, so downstream code (symbol resolution, diagnostics) never
//! needs to branch on which dialect an expression came from.

use std::fmt;

/// A parsed SPICE expression, as it appears in `.PARAM` definitions,
/// behavioral-source (`B`/`E`/`G`) equations, and `.model` parameter
/// expressions. Operators are kept as their literal source text (`op:
/// String`, not a fixed enum) because the same operator character can mean
/// different things in different dialects — e.g. `^` is exponentiation in
/// one dialect and XOR-like in another — so resolving what an operator
/// *means* is deliberately left to dialect-specific code downstream rather
/// than baked into this shared tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A numeric literal, with an optional SPICE unit suffix as written in
    /// the source (`"k"`, `"u"`, `"Meg"`, ...) — the suffix is kept
    /// separately rather than pre-applied, since suffix tables differ
    /// slightly by dialect.
    Number(f64, Option<String>),
    /// A bare identifier — a parameter name, a built-in constant, or a
    /// reserved special variable (`time`, `TEMP`, ...), disambiguated later
    /// by symbol resolution, not by the parser.
    Ident(String),
    /// A quoted string literal.
    StringLit(String),
    /// A function call, e.g. `sin(x)` or `pwl(time, 0, 0, 1u, 5)`.
    Call {
        /// The function name as written (case as in source).
        name: String,
        /// The argument expressions, in order.
        args: Vec<Expr>,
    },
    /// A binary operator expression, e.g. `a + b`.
    BinaryOp {
        /// The operator's literal source text (e.g. `"+"`, `"^"`).
        op: String,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// A unary operator expression, e.g. `-x`.
    UnaryOp {
        /// The operator's literal source text (e.g. `"-"`).
        op: String,
        /// The operand.
        operand: Box<Expr>,
    },
    /// A C-style ternary conditional, `cond ? then : else`.
    Ternary {
        /// The condition expression.
        condition: Box<Expr>,
        /// The value if `condition` is true (non-zero).
        then_branch: Box<Expr>,
        /// The value if `condition` is false (zero).
        else_branch: Box<Expr>,
    },
    /// A `V()`/`I()`/`N()`-style reference to a node voltage, branch
    /// current, or named net — the `args` are node/device names, not
    /// sub-expressions, since they resolve against the circuit's
    /// node/device namespace rather than the expression namespace.
    Reference {
        /// Which kind of reference this is.
        kind: RefKind,
        /// The node/device name argument(s), as written.
        args: Vec<String>,
    },
}

/// Which kind of quantity an [`Expr::Reference`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// A node voltage reference, `V(node)` or `V(node1, node2)`.
    V,
    /// A branch current reference, `I(device)`.
    I,
    /// A named-net reference, `N(name)` (Xyce).
    N,
}

impl fmt::Display for RefKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefKind::V => write!(f, "V"),
            RefKind::I => write!(f, "I"),
            RefKind::N => write!(f, "N"),
        }
    }
}

impl Expr {
    /// Builds a bare numeric literal with no unit suffix.
    pub fn number(v: f64) -> Self {
        Expr::Number(v, None)
    }

    /// Builds a numeric literal with a SPICE unit suffix (`"k"`, `"u"`, ...).
    pub fn number_with_suffix(v: f64, s: &str) -> Self {
        Expr::Number(v, Some(s.to_string()))
    }

    /// Builds a bare identifier expression.
    pub fn ident(name: &str) -> Self {
        Expr::Ident(name.to_string())
    }

    /// Builds a function-call expression.
    pub fn call(name: &str, args: Vec<Expr>) -> Self {
        Expr::Call {
            name: name.to_string(),
            args,
        }
    }

    /// Builds a binary-operator expression.
    pub fn binary(op: &str, lhs: Expr, rhs: Expr) -> Self {
        Expr::BinaryOp {
            op: op.to_string(),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// Builds a unary-operator expression.
    pub fn unary(op: &str, operand: Expr) -> Self {
        Expr::UnaryOp {
            op: op.to_string(),
            operand: Box::new(operand),
        }
    }

    /// Builds a ternary conditional expression.
    pub fn ternary(condition: Expr, then_branch: Expr, else_branch: Expr) -> Self {
        Expr::Ternary {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        }
    }

    /// Builds a `V()` node-voltage reference expression.
    pub fn v_ref(args: Vec<String>) -> Self {
        Expr::Reference {
            kind: RefKind::V,
            args,
        }
    }

    /// Builds an `I()` branch-current reference expression.
    pub fn i_ref(args: Vec<String>) -> Self {
        Expr::Reference {
            kind: RefKind::I,
            args,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_number() {
        let e = Expr::number(42.0);
        assert_eq!(e, Expr::Number(42.0, None));
    }

    #[test]
    fn test_expr_number_with_suffix() {
        let e = Expr::number_with_suffix(1000.0, "k");
        assert_eq!(e, Expr::Number(1000.0, Some("k".into())));
    }

    #[test]
    fn test_expr_ident() {
        let e = Expr::ident("time");
        assert_eq!(e, Expr::Ident("time".into()));
    }

    #[test]
    fn test_expr_call() {
        let e = Expr::call("sin", vec![Expr::ident("x")]);
        assert_eq!(
            e,
            Expr::Call {
                name: "sin".into(),
                args: vec![Expr::Ident("x".into())]
            }
        );
    }

    #[test]
    fn test_expr_binary() {
        let e = Expr::binary("+", Expr::number(1.0), Expr::number(2.0));
        assert!(matches!(e, Expr::BinaryOp { op, .. } if op == "+"));
    }

    #[test]
    fn test_expr_unary() {
        let e = Expr::unary("-", Expr::number(5.0));
        assert!(matches!(e, Expr::UnaryOp { op, .. } if op == "-"));
    }

    #[test]
    fn test_expr_ternary() {
        let e = Expr::ternary(Expr::number(1.0), Expr::number(2.0), Expr::number(3.0));
        assert!(matches!(e, Expr::Ternary { .. }));
    }

    #[test]
    fn test_expr_v_ref() {
        let e = Expr::v_ref(vec!["1".into(), "2".into()]);
        assert_eq!(
            e,
            Expr::Reference {
                kind: RefKind::V,
                args: vec!["1".into(), "2".into()]
            }
        );
    }

    #[test]
    fn test_expr_i_ref() {
        let e = Expr::i_ref(vec!["Vmeas".into()]);
        assert_eq!(
            e,
            Expr::Reference {
                kind: RefKind::I,
                args: vec!["Vmeas".into()]
            }
        );
    }
}
