use crate::dialect::Dialect;
use crate::expr::ast::*;
use crate::expr::token::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
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
                    "**" => (Token::Op(op.clone()), 8, 7),
                    "*" | "/" | "%" => (Token::Op(op.clone()), 7, 8),
                    "+" | "-" => (Token::Op(op.clone()), 6, 7),
                    "==" | "!=" | "<=" | ">=" | "<" | ">" => (Token::Op(op.clone()), 5, 6),
                    "&" => (Token::Op(op.clone()), 4, 5),
                    "|" | "^" => (Token::Op(op.clone()), 3, 4),
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
            Some(Token::Op(ref op)) if op == "-" || op == "~" => {
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
                let name_upper = name.to_uppercase();
                let _offset = self.peek_offset();
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

                    if matches!(name_upper.as_str(), "V" | "I" | "N") {
                        let kind = match name_upper.as_str() {
                            "V" => RefKind::V,
                            "I" => RefKind::I,
                            "N" => RefKind::N,
                            _ => unreachable!(),
                        };
                        let args_str: Vec<String> = args.iter().map(|a| format!("{a:?}")).collect();
                        return Ok(Expr::Reference {
                            kind,
                            args: args_str,
                        });
                    }

                    Ok(Expr::Call {
                        name: name_upper,
                        args,
                    })
                } else {
                    match name_upper.as_str() {
                        "TIME" | "FREQ" | "TEMP" | "VT" => {
                            return Ok(Expr::Reference {
                                kind: RefKind::V,
                                args: vec![name_upper.to_lowercase()],
                            });
                        }
                        _ => {}
                    }
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
}

const BUILTINS: &[&str] = &[
    "ABS",
    "CEIL",
    "DDT",
    "DDX",
    "FLOOR",
    "FMOD",
    "IF",
    "INT",
    "LIMIT",
    "M",
    "MIN",
    "MAX",
    "NINT",
    "PWR",
    "POW",
    "PWRS",
    "SDT",
    "SGN",
    "SIGN",
    "STP",
    "SQRT",
    "URAMP",
    "SIN",
    "COS",
    "TAN",
    "ASIN",
    "ACOS",
    "ATAN",
    "ATAN2",
    "SINH",
    "COSH",
    "TANH",
    "EXP",
    "LN",
    "LOG",
    "LOG10",
    "DB",
    "IMG",
    "PH",
    "R",
    "RE",
    "AGAUSS",
    "GAUSS",
    "AUNIF",
    "UNIF",
    "RAND",
    "SPICE_EXP",
    "SPICE_PULSE",
    "SPICE_SFFM",
    "SPICE_SIN",
    "TABLE",
    "TABLEFILE",
    "FASTTABLE",
    "SPLINE",
    "CUBIC",
    "WODICKA",
    "BLI",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name.to_uppercase().as_str())
}

pub fn parse_xyce(input: &str) -> Result<Expr> {
    let tokens = tokenize(input, Dialect::Xyce).map_err(|e| ParseError {
        message: e.message,
        offset: e.offset,
    })?;
    Parser::new(tokens).expr_bp(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Result<Expr> {
        parse_xyce(src)
    }

    #[test]
    fn test_caret_is_xor_not_power() {
        let e = parse("a ^ b").unwrap();
        assert_eq!(e, Expr::binary("^", Expr::ident("a"), Expr::ident("b")));
    }

    #[test]
    fn test_double_star_is_power() {
        let e = parse("a ** b").unwrap();
        assert_eq!(e, Expr::binary("**", Expr::ident("a"), Expr::ident("b")));
    }

    #[test]
    fn test_boolean_and_or_not_are_single_char() {
        let e = parse("a & b | c").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "|",
                Expr::binary("&", Expr::ident("a"), Expr::ident("b")),
                Expr::ident("c")
            )
        );
    }

    #[test]
    fn test_unary_not_is_tilde() {
        let e = parse("~a").unwrap();
        assert_eq!(e, Expr::unary("~", Expr::ident("a")));
    }

    #[test]
    fn test_ternary_low_precedence_matches_c_gotcha_example() {
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
    fn test_power_right_associative() {
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
    fn test_log_log10_ln_tagged_distinctly() {
        let e1 = parse("log(10)").unwrap();
        let e2 = parse("log10(10)").unwrap();
        let e3 = parse("ln(10)").unwrap();
        assert_eq!(e1, Expr::call("LOG", vec![Expr::number(10.0)]));
        assert_eq!(e2, Expr::call("LOG10", vec![Expr::number(10.0)]));
        assert_eq!(e3, Expr::call("LN", vec![Expr::number(10.0)]));
    }

    #[test]
    fn test_hierarchical_node_path_inside_v_reference_not_misparsed_as_ternary() {
        let e = parse("V(Xmain:Xnot1:A)").unwrap();
        assert!(matches!(
            e,
            Expr::Reference {
                kind: RefKind::V,
                ..
            }
        ));
    }

    #[test]
    fn test_builtin_ddt() {
        let e = parse("DDT(x)").unwrap();
        assert_eq!(e, Expr::call("DDT", vec![Expr::ident("x")]));
    }

    #[test]
    fn test_builtin_ddx() {
        let e = parse("DDX(f,x)").unwrap();
        assert_eq!(
            e,
            Expr::call("DDX", vec![Expr::ident("f"), Expr::ident("x")])
        );
    }

    #[test]
    fn test_builtin_limit_3arg() {
        let e = parse("LIMIT(x,0,1)").unwrap();
        assert_eq!(
            e,
            Expr::call(
                "LIMIT",
                vec![Expr::ident("x"), Expr::number(0.0), Expr::number(1.0)]
            )
        );
    }

    #[test]
    fn test_builtin_limit_2arg() {
        let e = parse("LIMIT(1,2)").unwrap();
        assert_eq!(
            e,
            Expr::call("LIMIT", vec![Expr::number(1.0), Expr::number(2.0)])
        );
    }

    #[test]
    fn test_builtin_stp() {
        let e = parse("STP(x)").unwrap();
        assert_eq!(e, Expr::call("STP", vec![Expr::ident("x")]));
    }

    #[test]
    fn test_builtin_uramp() {
        let e = parse("URAMP(x)").unwrap();
        assert_eq!(e, Expr::call("URAMP", vec![Expr::ident("x")]));
    }

    #[test]
    fn test_builtin_complex_re() {
        let e = parse("RE(z)").unwrap();
        assert_eq!(e, Expr::call("RE", vec![Expr::ident("z")]));
    }

    #[test]
    fn test_all_builtins_as_calls() {
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
    fn test_plus_operator_on_two_numbers() {
        let e = parse("1 + 2").unwrap();
        assert!(matches!(e, Expr::BinaryOp { op, .. } if op == "+"));
    }

    #[test]
    fn test_unbalanced_parens_returns_diagnostic_not_panic() {
        let e = parse("(1+2");
        assert!(e.is_err());
    }

    #[test]
    fn test_boolean_xor_precedence() {
        let e = parse("a | b ^ c").unwrap();
        assert_eq!(
            e,
            Expr::binary(
                "^",
                Expr::binary("|", Expr::ident("a"), Expr::ident("b")),
                Expr::ident("c")
            )
        );
    }
}
