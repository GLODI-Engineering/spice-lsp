//! ngspice's compile-time expression grammar — the subset used in
//! `.PARAM` definitions, `.model` parameter expressions, and `.func`
//! bodies. Distinct from [`crate::expr::ngspice_runtime`], which covers
//! ngspice's behavioral-source (`B`-element, `E`/`G` POLY) expression
//! grammar; the two differ in which built-in functions/variables are
//! available (e.g. `time`/`temper`/`hertz` are runtime-only). Entry point:
//! [`parse_ngspice_compiletime`].

use crate::dialect::Dialect;
use crate::expr::ast::*;
use crate::expr::token::*;

/// An expression that failed to parse under ngspice's compile-time
/// grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// A human-readable description of the problem.
    pub message: String,
    /// The byte offset in the input where the problem was found.
    pub offset: usize,
}

type Result<T> = std::result::Result<T, ParseError>;

struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<SpannedToken>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|st| &st.token)
    }

    fn peek_offset(&self) -> usize {
        self.tokens.get(self.pos).map(|st| st.offset).unwrap_or(0)
    }

    fn advance(&mut self) -> Option<SpannedToken> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        match self.peek() {
            Some(t) if t == expected => {
                self.advance();
                Ok(())
            }
            Some(_) => Err(ParseError {
                message: format!("expected {expected:?}"),
                offset: self.peek_offset(),
            }),
            None => Err(ParseError {
                message: format!("expected {expected:?}, got end of input"),
                offset: 0,
            }),
        }
    }

    fn expr_bp(&mut self, min_bp: i32) -> Result<Expr> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let (op, bp_left, bp_right) = match self.peek() {
                Some(Token::Question) => (Token::Question, 1, 1),
                Some(Token::Op(op)) => match op.as_str() {
                    "**" | "^" => (Token::Op(op.clone()), 8, 7),
                    "*" | "/" | "%" | "\\" => (Token::Op(op.clone()), 7, 8),
                    "+" | "-" => (Token::Op(op.clone()), 6, 7),
                    "==" | "!=" | "<=" | ">=" | "<" | ">" => (Token::Op(op.clone()), 5, 6),
                    "&&" => (Token::Op(op.clone()), 4, 5),
                    "||" => (Token::Op(op.clone()), 3, 4),
                    _ => break,
                },
                Some(Token::Colon) => break,
                _ => break,
            };

            if bp_left < min_bp {
                break;
            }

            match op {
                Token::Question => {
                    self.advance();
                    let then_branch = self.expr_bp(0)?;
                    self.expect(&Token::Colon)?;
                    let else_branch = self.expr_bp(1)?;
                    lhs = Expr::Ternary {
                        condition: Box::new(lhs),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    };
                }
                Token::Op(ref op_str) => {
                    self.advance();
                    let rhs = self.expr_bp(bp_right)?;
                    lhs = Expr::BinaryOp {
                        op: op_str.clone(),
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                _ => unreachable!(),
            }
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr> {
        match self.peek().cloned() {
            Some(Token::Op(ref op)) if op == "-" || op == "!" => {
                let op = op.clone();
                self.advance();
                let operand = self.expr_bp(9)?;
                Ok(Expr::UnaryOp {
                    op,
                    operand: Box::new(operand),
                })
            }
            Some(Token::Number(v, s)) => {
                self.advance();
                Ok(Expr::Number(v, s))
            }
            Some(Token::Ident(ref name)) => {
                let name_lower = name.to_lowercase();
                let name_upper = name.to_uppercase();

                let offset = self.peek_offset();

                if matches!(name_upper.as_str(), "V" | "I" | "N") {
                    self.advance();
                    if self.peek() == Some(&Token::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        loop {
                            match self.peek() {
                                Some(Token::RParen) => {
                                    self.advance();
                                    break;
                                }
                                Some(Token::Comma) => {
                                    self.advance();
                                }
                                _ => {
                                    args.push(self.parse_reference_arg()?);
                                }
                            }
                        }
                        let kind = match name_upper.as_str() {
                            "V" => RefKind::V,
                            "I" => RefKind::I,
                            "N" => RefKind::N,
                            _ => unreachable!(),
                        };
                        return Err(ParseError {
                            message: format!(
                                "{kind}(...) reference is not allowed in compile-time expressions"
                            ),
                            offset,
                        });
                    }
                    match name_lower.as_str() {
                        "time" | "temper" | "hertz" => {
                            return Err(ParseError {
                                message: format!(
                                    "'{name}' is not allowed in compile-time expressions"
                                ),
                                offset,
                            });
                        }
                        _ => {}
                    }
                    return Ok(match name_upper.as_str() {
                        "V" => Expr::Reference {
                            kind: RefKind::V,
                            args: vec![],
                        },
                        "I" => Expr::Reference {
                            kind: RefKind::I,
                            args: vec![],
                        },
                        _ => Expr::Ident(name.clone()),
                    });
                }

                if matches!(name_lower.as_str(), "time" | "temper" | "hertz") {
                    self.advance();
                    return Err(ParseError {
                        message: format!("'{name}' is not allowed in compile-time expressions"),
                        offset,
                    });
                }

                self.advance();

                if self.peek() == Some(&Token::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        args.push(self.expr_bp(0)?);
                        while self.peek() == Some(&Token::Comma) {
                            self.advance();
                            args.push(self.expr_bp(0)?);
                        }
                    }
                    self.expect(&Token::RParen)?;
                    Ok(Expr::Call {
                        name: name_upper,
                        args,
                    })
                } else {
                    Ok(Expr::Ident(name.clone()))
                }
            }
            Some(Token::StringLit(ref s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::StringLit(s))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.expr_bp(0)?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(Token::LBrace) => {
                self.advance();
                let expr = self.expr_bp(0)?;
                self.expect(&Token::RBrace)?;
                Ok(expr)
            }
            Some(tok) => Err(ParseError {
                message: format!("unexpected token {tok:?}"),
                offset: self.peek_offset(),
            }),
            None => Err(ParseError {
                message: "unexpected end of input".into(),
                offset: 0,
            }),
        }
    }

    fn parse_reference_arg(&mut self) -> Result<String> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                Ok(name)
            }
            Some(Token::Number(v, _)) => {
                self.advance();
                Ok(v.to_string())
            }
            Some(tok) => Err(ParseError {
                message: format!("expected reference argument, got {tok:?}"),
                offset: self.peek_offset(),
            }),
            None => Err(ParseError {
                message: "expected reference argument, got end of input".into(),
                offset: 0,
            }),
        }
    }
}

const BUILTINS: &[&str] = &[
    "SQRT",
    "SIN",
    "COS",
    "TAN",
    "ASIN",
    "ACOS",
    "ATAN",
    "SINH",
    "COSH",
    "TANH",
    "ASINH",
    "ACOSH",
    "ATANH",
    "ARCTAN",
    "EXP",
    "LN",
    "LOG",
    "LOG10",
    "ABS",
    "NINT",
    "INT",
    "FLOOR",
    "CEIL",
    "POW",
    "PWR",
    "MIN",
    "MAX",
    "SGN",
    "TERNARY_FCN",
    "GAUSS",
    "AGAUSS",
    "UNIF",
    "AUNIF",
    "LIMIT",
    "VAR",
    "VEC",
];

/// Returns whether `name` (case-insensitive) is a built-in function name in
/// ngspice's compile-time expression grammar.
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name.to_uppercase().as_str())
}

/// Tokenizes and parses `input` as an ngspice compile-time expression
/// (a `.PARAM`/`.model`/`.func` expression body).
pub fn parse_ngspice_compiletime(input: &str) -> Result<Expr> {
    let tokens = tokenize(input, Dialect::Ngspice).map_err(|e| ParseError {
        message: e.message,
        offset: e.offset,
    })?;
    Parser::new(tokens).expr_bp(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Result<Expr> {
        parse_ngspice_compiletime(src)
    }

    #[test]
    fn test_number() {
        let e = parse("42").unwrap();
        assert_eq!(e, Expr::Number(42.0, None));
    }

    #[test]
    fn test_ident() {
        let e = parse("x").unwrap();
        assert_eq!(e, Expr::Ident("x".into()));
    }

    #[test]
    fn test_addition() {
        let e = parse("1+2").unwrap();
        assert_eq!(e, Expr::binary("+", Expr::number(1.0), Expr::number(2.0)));
    }

    #[test]
    fn test_multiplication_higher_than_addition() {
        let e = parse("2+3*4").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "+",
                Expr::number(2.0),
                Expr::binary("*", Expr::number(3.0), Expr::number(4.0))
            )
        );
    }

    #[test]
    fn test_subtraction_left_assoc() {
        let e = parse("10-3-2").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "-",
                Expr::binary("-", Expr::number(10.0), Expr::number(3.0)),
                Expr::number(2.0)
            )
        );
    }

    #[test]
    fn test_division_higher_than_subtraction() {
        let e = parse("10-6/2").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "-",
                Expr::number(10.0),
                Expr::binary("/", Expr::number(6.0), Expr::number(2.0))
            )
        );
    }

    #[test]
    fn test_power_higher_than_multiplication() {
        let e = parse("2*3**2").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "*",
                Expr::number(2.0),
                Expr::binary("**", Expr::number(3.0), Expr::number(2.0))
            )
        );
    }

    #[test]
    fn test_double_star_and_caret_both_mean_power() {
        let e1 = parse("2**3").unwrap();
        let e2 = parse("2^3").unwrap();
        assert!(matches!(e1, Expr::BinaryOp { op, .. } if op == "**"));
        assert!(matches!(e2, Expr::BinaryOp { op, .. } if op == "^"));
    }

    #[test]
    fn test_power_is_right_associative() {
        let e = parse("2**3**2").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "**",
                Expr::number(2.0),
                Expr::binary("**", Expr::number(3.0), Expr::number(2.0))
            )
        );
    }

    #[test]
    fn test_comparison_precedence() {
        let e = parse("a==b && c<d").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "&&",
                Expr::binary("==", Expr::ident("a"), Expr::ident("b")),
                Expr::binary("<", Expr::ident("c"), Expr::ident("d"))
            )
        );
    }

    #[test]
    fn test_boolean_and_above_or() {
        let e = parse("a && b || c").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "||",
                Expr::binary("&&", Expr::ident("a"), Expr::ident("b")),
                Expr::ident("c")
            )
        );
    }

    #[test]
    fn test_ternary_lowest_precedence() {
        let e = parse("1+a==b?1:0+1").unwrap();
        let condition = Expr::binary(
            "==",
            Expr::binary("+", Expr::number(1.0), Expr::ident("a")),
            Expr::ident("b"),
        );
        let else_branch = Expr::binary("+", Expr::number(0.0), Expr::number(1.0));
        assert_eq!(e, Expr::ternary(condition, Expr::number(1.0), else_branch));
    }

    #[test]
    fn test_unary_minus() {
        let e = parse("-5").unwrap();
        assert_eq!(e, Expr::unary("-", Expr::number(5.0)));
    }

    #[test]
    fn test_unary_not() {
        let e = parse("!x").unwrap();
        assert_eq!(e, Expr::unary("!", Expr::ident("x")));
    }

    #[test]
    fn test_parenthesized_expression() {
        let e = parse("(2+3)*4").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "*",
                Expr::binary("+", Expr::number(2.0), Expr::number(3.0)),
                Expr::number(4.0)
            )
        );
    }

    #[test]
    fn test_brace_expression() {
        let e = parse("{2+3}").unwrap();
        assert_eq!(e, Expr::binary("+", Expr::number(2.0), Expr::number(3.0)));
    }

    #[test]
    fn test_function_call_single_arg() {
        let e = parse("sin(x)").unwrap();
        assert_eq!(e, Expr::call("SIN", vec![Expr::ident("x")]));
    }

    #[test]
    fn test_function_call_two_args() {
        let e = parse("pow(x,2)").unwrap();
        assert_eq!(
            e,
            Expr::call("POW", vec![Expr::ident("x"), Expr::number(2.0)])
        );
    }

    #[test]
    fn test_function_call_no_args() {
        let e = parse("var()").unwrap();
        assert_eq!(e, Expr::call("VAR", vec![]));
    }

    #[test]
    fn test_builtin_sqrt() {
        let e = parse("sqrt(4)").unwrap();
        assert_eq!(e, Expr::call("SQRT", vec![Expr::number(4.0)]));
    }

    #[test]
    fn test_builtin_log() {
        let e = parse("log(x)").unwrap();
        assert_eq!(e, Expr::call("LOG", vec![Expr::ident("x")]));
    }

    #[test]
    fn test_builtin_min() {
        let e = parse("min(a,b)").unwrap();
        assert_eq!(
            e,
            Expr::call("MIN", vec![Expr::ident("a"), Expr::ident("b")])
        );
    }

    #[test]
    fn test_builtin_gauss() {
        let e = parse("gauss(1,2)").unwrap();
        assert_eq!(
            e,
            Expr::call("GAUSS", vec![Expr::number(1.0), Expr::number(2.0)])
        );
    }

    #[test]
    fn test_v_reference_rejected_in_compiletime_context() {
        let e = parse("V(1,2)");
        assert!(e.is_err());
        let err = e.unwrap_err();
        assert!(err.message.contains("not allowed"));
    }

    #[test]
    fn test_i_reference_rejected() {
        let e = parse("I(Vmeas)");
        assert!(e.is_err());
    }

    #[test]
    fn test_bare_time_identifier_rejected_in_compiletime_context() {
        let e = parse("time");
        assert!(e.is_err());
    }

    #[test]
    fn test_bare_temper_rejected() {
        let e = parse("temper");
        assert!(e.is_err());
    }

    #[test]
    fn test_bare_hertz_rejected() {
        let e = parse("hertz");
        assert!(e.is_err());
    }

    #[test]
    fn test_unbalanced_parens_returns_diagnostic_not_panic() {
        let e = parse("(1+2");
        assert!(e.is_err());
    }

    #[test]
    fn test_suffix_resolved_to_numeric_value() {
        let e = parse("1k").unwrap();
        assert!(matches!(e, Expr::Number(1000.0, _)));
    }

    #[test]
    fn test_integer_divide_operator() {
        let e = parse("10\\3").unwrap();
        assert_eq!(e, Expr::binary("\\", Expr::number(10.0), Expr::number(3.0)));
    }

    #[test]
    fn test_all_builtins_recognized_as_calls() {
        for name in BUILTINS {
            let src = format!("{name}(1)");
            let e = parse(&src);
            if let Ok(expr) = e {
                assert!(
                    matches!(&expr, Expr::Call { name: n, .. } if n == *name),
                    "builtin {name} not recognized as call"
                );
            }
        }
    }

    #[test]
    fn test_nested_calls() {
        let e = parse("sqrt(sin(x))").unwrap();
        assert_eq!(
            e,
            Expr::call("SQRT", vec![Expr::call("SIN", vec![Expr::ident("x")])])
        );
    }

    #[test]
    fn test_complex_expression() {
        let e = parse("a+b*c/d-e**f").unwrap();
        assert!(matches!(e, Expr::BinaryOp { .. }));
    }
}
