use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64, Option<String>),
    Ident(String),
    StringLit(String),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    BinaryOp {
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnaryOp {
        op: String,
        operand: Box<Expr>,
    },
    Ternary {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Reference {
        kind: RefKind,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    V,
    I,
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
    pub fn number(v: f64) -> Self {
        Expr::Number(v, None)
    }

    pub fn number_with_suffix(v: f64, s: &str) -> Self {
        Expr::Number(v, Some(s.to_string()))
    }

    pub fn ident(name: &str) -> Self {
        Expr::Ident(name.to_string())
    }

    pub fn call(name: &str, args: Vec<Expr>) -> Self {
        Expr::Call {
            name: name.to_string(),
            args,
        }
    }

    pub fn binary(op: &str, lhs: Expr, rhs: Expr) -> Self {
        Expr::BinaryOp {
            op: op.to_string(),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    pub fn unary(op: &str, operand: Expr) -> Self {
        Expr::UnaryOp {
            op: op.to_string(),
            operand: Box::new(operand),
        }
    }

    pub fn ternary(condition: Expr, then_branch: Expr, else_branch: Expr) -> Self {
        Expr::Ternary {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        }
    }

    pub fn v_ref(args: Vec<String>) -> Self {
        Expr::Reference {
            kind: RefKind::V,
            args,
        }
    }

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
