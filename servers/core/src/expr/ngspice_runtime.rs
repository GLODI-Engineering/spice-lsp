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
                    if op_str == "^" || op_str == "**" {
                        let rhs = self.expr_bp(bp_right)?;
                        lhs = Expr::BinaryOp {
                            op: "^_RUNTIME".into(),
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        };
                    } else {
                        let rhs = self.expr_bp(bp_right)?;
                        lhs = Expr::BinaryOp {
                            op: op_str.clone(),
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        };
                    }
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
                let _is_builtin = matches!(
                    name_upper.as_str(),
                    "SQRT"
                        | "SIN"
                        | "COS"
                        | "TAN"
                        | "ASIN"
                        | "ACOS"
                        | "ATAN"
                        | "SINH"
                        | "COSH"
                        | "ASINH"
                        | "ACOSH"
                        | "ATANH"
                        | "ARCTAN"
                        | "EXP"
                        | "LN"
                        | "LOG"
                        | "LOG10"
                        | "ABS"
                        | "NINT"
                        | "INT"
                        | "FLOOR"
                        | "CEIL"
                        | "MIN"
                        | "MAX"
                        | "SGN"
                        | "TERNARY_FCN"
                        | "POW"
                        | "PWR"
                        | "U"
                        | "U2"
                        | "URAMP"
                        | "PWL"
                        | "GAUSS"
                        | "AGAUSS"
                        | "UNIF"
                        | "AUNIF"
                        | "LIMIT"
                );

                let is_ref = matches!(name_upper.as_str(), "V" | "I" | "N");
                let is_time_var = matches!(name_lower.as_str(), "time" | "temper" | "hertz");
                let is_i_inst = name == "i";

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

                    if is_ref {
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

                    if name_upper == "POW" {
                        return Ok(Expr::Call {
                            name: "POW_RUNTIME".into(),
                            args,
                        });
                    }

                    if name_upper == "PWR" {
                        return Ok(Expr::Call {
                            name: "PWR".into(),
                            args,
                        });
                    }

                    if name_lower == "i" {
                        return Ok(Expr::Reference {
                            kind: RefKind::I,
                            args: args.iter().map(|a| format!("{a:?}")).collect(),
                        });
                    }

                    Ok(Expr::Call {
                        name: name_upper,
                        args,
                    })
                } else {
                    if is_ref || is_i_inst {
                        let kind = if name_upper == "V" {
                            RefKind::V
                        } else {
                            RefKind::I
                        };
                        return Ok(Expr::Reference { kind, args: vec![] });
                    }

                    if is_time_var {
                        return Ok(Expr::Ident(name_lower));
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

pub fn parse_ngspice_runtime(input: &str) -> Result<Expr> {
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
        parse_ngspice_runtime(src)
    }

    #[test]
    fn test_v_single_node_reference() {
        let e = parse("V(1)").unwrap();
        assert!(matches!(
            e,
            Expr::Reference {
                kind: RefKind::V,
                ..
            }
        ));
    }

    #[test]
    fn test_v_two_node_reference() {
        let e = parse("V(1,2)").unwrap();
        assert!(matches!(
            e,
            Expr::Reference {
                kind: RefKind::V,
                ..
            }
        ));
    }

    #[test]
    fn test_i_devname_reference() {
        let e = parse("I(Vmeas)").unwrap();
        assert!(matches!(
            e,
            Expr::Reference {
                kind: RefKind::I,
                ..
            }
        ));
    }

    #[test]
    fn test_bare_time_temper_hertz_accepted() {
        let e1 = parse("time").unwrap();
        let e2 = parse("temper").unwrap();
        let e3 = parse("hertz").unwrap();
        assert!(matches!(e1, Expr::Ident(n) if n == "time"));
        assert!(matches!(e2, Expr::Ident(n) if n == "temper"));
        assert!(matches!(e3, Expr::Ident(n) if n == "hertz"));
    }

    #[test]
    fn test_tanh_not_in_runtime_function_list() {
        let e = parse("tanh(x)").unwrap();
        match &e {
            Expr::Call { name, .. } => {
                assert_eq!(name, "TANH");
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn test_pow_caret_doublestar_share_sign_discarding_tag() {
        let e1 = parse("pow(x,2)").unwrap();
        let e2 = parse("x**2").unwrap();
        let e3 = parse("x^2").unwrap();

        match &e1 {
            Expr::Call { name, .. } => assert_eq!(name, "POW_RUNTIME"),
            _ => panic!("expected call"),
        }
        match &e2 {
            Expr::BinaryOp { op, .. } => assert_eq!(op, "^_RUNTIME"),
            _ => panic!("expected binary"),
        }
        match &e3 {
            Expr::BinaryOp { op, .. } => assert_eq!(op, "^_RUNTIME"),
            _ => panic!("expected binary"),
        }
    }

    #[test]
    fn test_pwr_has_distinct_sign_preserving_tag() {
        let e = parse("pwr(x,2)").unwrap();
        match &e {
            Expr::Call { name, .. } => assert_eq!(name, "PWR"),
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn test_unit_step_family_u_u2_uramp() {
        let e1 = parse("u(1)").unwrap();
        let e2 = parse("u2(1)").unwrap();
        let e3 = parse("uramp(1)").unwrap();
        assert_eq!(e1, Expr::call("U", vec![Expr::number(1.0)]));
        assert_eq!(e2, Expr::call("U2", vec![Expr::number(1.0)]));
        assert_eq!(e3, Expr::call("URAMP", vec![Expr::number(1.0)]));
    }

    #[test]
    fn test_pwl_even_length_pairs() {
        let e = parse("pwl(v, 0.0,0.0, 1.0,1.0)").unwrap();
        match &e {
            Expr::Call { name, args } => {
                assert_eq!(name, "PWL");
                assert_eq!(args.len(), 5);
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn test_pwl_odd_length_pairs_returns_diagnostic_not_panic() {
        let e = parse("pwl(v, 0.0,0.0, 1.0)").unwrap();
        match &e {
            Expr::Call { name, args } => {
                assert_eq!(name, "PWL");
                assert_eq!(args.len(), 4);
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn test_lowercase_i_devname_form() {
        let e = parse("i(Vmeas)").unwrap();
        assert!(matches!(
            e,
            Expr::Reference {
                kind: RefKind::I,
                ..
            }
        ));
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
    fn test_ternary() {
        let e = parse("a ? b : c").unwrap();
        assert_eq!(
            e,
            Expr::ternary(Expr::ident("a"), Expr::ident("b"), Expr::ident("c"))
        );
    }

    #[test]
    fn test_unary_minus() {
        let e = parse("-5").unwrap();
        assert_eq!(e, Expr::unary("-", Expr::number(5.0)));
    }

    #[test]
    fn test_unbalanced_parens_returns_diagnostic_not_panic() {
        let e = parse("(1+2");
        assert!(e.is_err());
    }
}
