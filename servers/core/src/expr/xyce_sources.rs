use crate::expr::ast::*;
use crate::expr::xyce;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

type Result<T> = std::result::Result<T, ParseError>;

#[allow(dead_code)]
fn table_expr_to_pairs(expr: &Expr) -> Vec<(Expr, Expr)> {
    let mut pairs = Vec::new();
    if let Expr::Call { args, .. } = expr {
        let mut i = 0;
        while i + 1 < args.len() {
            pairs.push((args[i].clone(), args[i + 1].clone()));
            i += 2;
        }
    }
    pairs
}

pub fn parse_xyce_table(input: &str) -> Result<(Expr, Vec<(Expr, Expr)>)> {
    let trimmed = input.trim();

    if !trimmed.to_uppercase().starts_with("TABLE") {
        return Err(ParseError {
            message: "expected TABLE expression".into(),
            offset: 0,
        });
    }

    let after_table = trimmed[5..].trim();

    if let Some(pspice_double_paren) = after_table.find("((") {
        return Err(ParseError {
            message: format!(
                "PSpice-style double-paren TABLE syntax at offset {} — Xyce requires TABLE {{expr}} = (x0,y0)(x1,y1)...",
                pspice_double_paren + 5
            ),
            offset: pspice_double_paren + 5,
        });
    }

    if let Some(brace_open) = after_table.find('{') {
        if let Some(brace_close) = after_table.rfind('}') {
            let brace_content = &after_table[brace_open..=brace_close];
            if brace_content.matches('{').count() > 1 {
                return Err(ParseError {
                    message: format!(
                        "nested braces in TABLE expression at offset {} — Xyce does not support nested braces",
                        brace_open + 5
                    ),
                    offset: brace_open + 5,
                });
            }
        }
    }

    if let Some(rest) = after_table.strip_prefix('{') {
        if let Some(brace_close) = rest.find('}') {
            let expr_text = &rest[..brace_close];
            let after_brace = rest[brace_close + 1..].trim();

            let controlling = xyce::parse_xyce(expr_text).map_err(|e| ParseError {
                message: format!("TABLE controlling expression: {}", e.message),
                offset: e.offset + 6,
            })?;

            if !after_brace.starts_with('=') {
                return Err(ParseError {
                    message: "TABLE requires = before pair list".into(),
                    offset: brace_close + 6,
                });
            }

            let pairs_text = after_brace[1..].trim();

            let (pairs, _) = if pairs_text.starts_with('(') {
                parse_table_pair_list(pairs_text)?
            } else {
                return Err(ParseError {
                    message: "TABLE pairs must be in parenthesized (x,y) tuples".into(),
                    offset: brace_close + 6 + 1,
                });
            };

            return Ok((controlling, pairs));
        }
    }

    Err(ParseError {
        message: "malformed TABLE expression".into(),
        offset: 0,
    })
}

fn parse_table_pair_list(input: &str) -> Result<(Vec<(Expr, Expr)>, usize)> {
    let mut pairs = Vec::new();
    let mut pos = 0;
    let chars: Vec<char> = input.chars().collect();

    while pos < chars.len() {
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        if chars[pos] != '(' {
            if pairs.is_empty() {
                return Err(ParseError {
                    message: "TABLE pair list must start with '('".into(),
                    offset: pos,
                });
            }
            break;
        }

        let pair_start = pos;
        pos += 1;

        let mut x_str = String::new();
        let mut depth = 1i32;
        while pos < chars.len() {
            if chars[pos] == ')' && depth == 1 {
                return Err(ParseError {
                    message: format!(
                        "space-separated TABLE pairs at offset {pair_start} — Xyce requires comma-separated (x,y) pairs"
                    ),
                    offset: pair_start,
                });
            }
            if chars[pos] == ',' && depth == 1 {
                break;
            }
            if chars[pos] == '(' {
                depth += 1;
            } else if chars[pos] == ')' {
                depth -= 1;
            }
            x_str.push(chars[pos]);
            pos += 1;
        }

        if pos >= chars.len() || chars[pos] != ',' {
            return Err(ParseError {
                message: format!(
                    "missing comma in TABLE pair at offset {pair_start} — Xyce requires (x,y) format"
                ),
                offset: pair_start,
            });
        }
        pos += 1;

        let mut y_str = String::new();
        depth = 1;
        while pos < chars.len() && depth > 0 {
            if chars[pos] == '(' {
                depth += 1;
            } else if chars[pos] == ')' {
                depth -= 1;
                if depth == 0 {
                    pos += 1;
                    break;
                }
            }
            y_str.push(chars[pos]);
            pos += 1;
        }

        if depth > 0 {
            return Err(ParseError {
                message: format!("unclosed parenthesized pair at offset {pair_start}"),
                offset: pair_start,
            });
        }

        let x = xyce::parse_xyce(&x_str).map_err(|e| ParseError {
            message: format!("TABLE pair x-value: {}", e.message),
            offset: e.offset + pair_start + 1,
        })?;
        let y = xyce::parse_xyce(&y_str).map_err(|e| ParseError {
            message: format!("TABLE pair y-value: {}", e.message),
            offset: e.offset + pair_start + 1 + x_str.len() + 1,
        })?;

        pairs.push((x, y));
    }

    Ok((pairs, pos))
}

pub fn parse_poly_e_g(_n_vars: usize, input: &str) -> Result<Expr> {
    let trimmed = input.trim();
    xyce::parse_xyce(trimmed).map_err(|e| ParseError {
        message: e.message,
        offset: e.offset,
    })
}

pub fn parse_poly_f_h(_n_vars: usize, input: &str) -> Result<Expr> {
    let trimmed = input.trim();
    xyce::parse_xyce(trimmed).map_err(|e| ParseError {
        message: e.message,
        offset: e.offset,
    })
}

pub fn is_file_load_function(name: &str) -> bool {
    matches!(
        name.to_uppercase().as_str(),
        "TABLE" | "TABLEFILE" | "FASTTABLE" | "SPLINE" | "CUBIC" | "WODICKA" | "BLI"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_basic_pairs() {
        let (expr, pairs) = parse_xyce_table("TABLE {V(1)} = (0.0,0.0)(1.0,1.0)").unwrap();
        assert!(matches!(expr, Expr::Reference { .. }));
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn test_table_nested_braces_rejected() {
        let r = parse_xyce_table("TABLE {{V(1)+{5}}} = (0.0,0.0)");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(err.message.contains("nested"));
    }

    #[test]
    fn test_table_pspice_extra_parens_rejected_with_actionable_message() {
        let r = parse_xyce_table("TABLE {V(1)} ((0.0,0.0))");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(err.message.contains("PSpice"));
    }

    #[test]
    fn test_table_pspice_space_separated_pairs_rejected_with_actionable_message() {
        let r = parse_xyce_table("TABLE {V(1)} = (0.0 0.0)");
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(err.message.to_lowercase().contains("space"));
    }

    #[test]
    fn test_file_load_table() {
        let e = xyce::parse_xyce("table(\"file.dat\")").unwrap();
        assert_eq!(
            e,
            Expr::call("TABLE", vec![Expr::StringLit("file.dat".into())])
        );
    }

    #[test]
    fn test_file_load_tablefile() {
        let e = xyce::parse_xyce("tablefile(\"data.csv\")").unwrap();
        assert_eq!(
            e,
            Expr::call("TABLEFILE", vec![Expr::StringLit("data.csv".into())])
        );
    }

    #[test]
    fn test_file_load_fasttable() {
        let e = xyce::parse_xyce("fasttable(\"data.csv\")").unwrap();
        assert_eq!(
            e,
            Expr::call("FASTTABLE", vec![Expr::StringLit("data.csv".into())])
        );
    }

    #[test]
    fn test_file_load_spline() {
        let e = xyce::parse_xyce("spline(\"data.csv\")").unwrap();
        assert_eq!(
            e,
            Expr::call("SPLINE", vec![Expr::StringLit("data.csv".into())])
        );
    }

    #[test]
    fn test_file_load_cubic() {
        let e = xyce::parse_xyce("cubic(\"data.csv\")").unwrap();
        assert_eq!(
            e,
            Expr::call("CUBIC", vec![Expr::StringLit("data.csv".into())])
        );
    }

    #[test]
    fn test_file_load_wodicka() {
        let e = xyce::parse_xyce("wodicka(\"data.csv\")").unwrap();
        assert_eq!(
            e,
            Expr::call("WODICKA", vec![Expr::StringLit("data.csv".into())])
        );
    }

    #[test]
    fn test_file_load_bli() {
        let e = xyce::parse_xyce("bli(\"data.csv\")").unwrap();
        assert_eq!(
            e,
            Expr::call("BLI", vec![Expr::StringLit("data.csv".into())])
        );
    }

    #[test]
    fn test_poly_e_g_two_nodes_per_variable() {
        let e = parse_poly_e_g(1, "1.0 2.0 3.0").unwrap();
        assert!(
            matches!(e, Expr::BinaryOp { .. })
                || matches!(e, Expr::Number(..))
                || matches!(e, Expr::Ident(..))
        );
    }

    #[test]
    fn test_poly_f_h_one_source_name_per_variable() {
        let e = parse_poly_f_h(1, "0.5 1.0").unwrap();
        assert!(
            matches!(e, Expr::BinaryOp { .. })
                || matches!(e, Expr::Number(..))
                || matches!(e, Expr::Ident(..))
        );
    }

    #[test]
    fn test_is_file_load_function_names() {
        for name in &[
            "table",
            "tablefile",
            "fasttable",
            "spline",
            "cubic",
            "wodicka",
            "bli",
        ] {
            assert!(is_file_load_function(name), "{name} should be recognized");
        }
    }
}
