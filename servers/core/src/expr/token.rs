//! Tokenizes raw expression text (a `.PARAM` value, a behavioral-source
//! equation, ...) into a flat [`Token`] stream — entry point [`tokenize`].
//! This runs before any of the dialect-specific expression parsers in
//! [`crate::expr`]; it does not itself apply operator precedence or
//! grammar structure, only lexical splitting, including dialect-sensitive
//! numeric unit-suffix parsing (`X` means ×1e6 in Xyce but is not a suffix
//! in ngspice, and vice versa for `a`/×1e-18 — see [`parse_scale_suffix`]).

use crate::dialect::Dialect;

/// A [`Token`] paired with its byte offset into the original input, for
/// error reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    /// The token itself.
    pub token: Token,
    /// The byte offset in the input where this token starts.
    pub offset: usize,
}

/// A single lexical token in a SPICE expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A numeric literal, with an optional unit-suffix string as written
    /// (e.g. `Some("k")` for `10k`).
    Number(f64, Option<String>),
    /// A bare identifier (parameter name, function name, or `V`/`I`/`N`
    /// reference name before it's recognized as such by the parser).
    Ident(String),
    /// A quoted string literal, with the surrounding quotes stripped.
    StringLit(String),
    /// An operator, kept as its literal source text (e.g. `"+"`, `"**"`).
    Op(String),
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `?` (ternary)
    Question,
    /// `:` (ternary, or Xyce's node-path separator)
    Colon,
    /// `=`
    Equals,
}

/// A lexical error: a malformed number, an unterminated string literal, or
/// similar. Carries the byte offset into the input where the problem was
/// found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenError {
    /// A human-readable description of the problem.
    pub message: String,
    /// The byte offset in the input where the problem was found.
    pub offset: usize,
}

/// Tokenize `input` into a flat [`SpannedToken`] stream. `dialect` affects
/// only numeric unit-suffix interpretation (see module docs); it does not
/// change which characters are recognized as operators/punctuation.
pub fn tokenize(input: &str, _dialect: Dialect) -> Result<Vec<SpannedToken>, TokenError> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut tokens = Vec::new();
    let dialect = _dialect;

    while pos < chars.len() {
        let c = chars[pos];
        let start = pos;

        if c.is_whitespace() {
            pos += 1;
            continue;
        }

        if c.is_ascii_digit()
            || (c == '.' && pos + 1 < chars.len() && chars[pos + 1].is_ascii_digit())
        {
            let (value, raw_suffix, new_pos) = parse_number(&chars, pos, dialect)?;
            tokens.push(SpannedToken {
                token: Token::Number(value, raw_suffix),
                offset: start,
            });
            pos = new_pos;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let (name, new_pos) = parse_ident(&chars, pos);
            tokens.push(SpannedToken {
                token: Token::Ident(name),
                offset: start,
            });
            pos = new_pos;
        } else if c == '"' {
            let (s, new_pos) = parse_string(&chars, pos)?;
            tokens.push(SpannedToken {
                token: Token::StringLit(s),
                offset: start,
            });
            pos = new_pos;
        } else {
            let (token, new_pos) = parse_operator(&chars, pos);
            tokens.push(SpannedToken {
                token,
                offset: start,
            });
            pos = new_pos;
        }
    }

    Ok(tokens)
}

fn parse_number(
    chars: &[char],
    start: usize,
    dialect: Dialect,
) -> Result<(f64, Option<String>, usize), TokenError> {
    let mut i = start;

    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }

    if i < chars.len() && chars[i] == '.' {
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }

    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        i += 1;
        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
            i += 1;
        }
        if i < chars.len() && chars[i].is_ascii_digit() {
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            return Err(TokenError {
                message: "malformed exponent in number".into(),
                offset: i,
            });
        }
    }

    let num_str: String = chars[start..i].iter().collect();
    let mut value: f64 = num_str.parse().map_err(|_| TokenError {
        message: format!("invalid number: {num_str}"),
        offset: start,
    })?;

    let (suffix_str, suffix_mult, mut i) = parse_scale_suffix(chars, i, dialect);
    value *= suffix_mult;

    while i < chars.len() && chars[i].is_ascii_alphabetic() {
        i += 1;
    }

    Ok((value, suffix_str, i))
}

fn parse_scale_suffix(
    chars: &[char],
    pos: usize,
    dialect: Dialect,
) -> (Option<String>, f64, usize) {
    if pos >= chars.len() {
        return (None, 1.0, pos);
    }

    let remaining: Vec<char> = chars[pos..].to_vec();

    if remaining.len() >= 3 {
        let prefix: String = remaining[..3].iter().collect();
        if prefix == "mil" {
            return (Some("mil".into()), 25.4e-6, pos + 3);
        }
        if prefix == "Meg" || prefix == "MEG" {
            return (Some("Meg".into()), 1e6, pos + 3);
        }
    }

    let c = chars[pos];
    match c {
        'T' | 't' => (Some(c.to_string()), 1e12, pos + 1),
        'G' | 'g' => (Some(c.to_string()), 1e9, pos + 1),
        'K' | 'k' => (Some(c.to_string()), 1e3, pos + 1),
        'M' | 'm' => (Some("m".into()), 1e-3, pos + 1),
        'U' | 'u' => (Some(c.to_string()), 1e-6, pos + 1),
        'N' | 'n' => (Some(c.to_string()), 1e-9, pos + 1),
        'P' | 'p' => (Some(c.to_string()), 1e-12, pos + 1),
        'F' | 'f' => (Some(c.to_string()), 1e-15, pos + 1),
        'A' | 'a' => {
            if dialect == Dialect::Ngspice {
                (Some(c.to_string()), 1e-18, pos + 1)
            } else {
                (None, 1.0, pos)
            }
        }
        'X' | 'x' => {
            if dialect == Dialect::Xyce {
                (Some(c.to_string()), 1e6, pos + 1)
            } else {
                (None, 1.0, pos)
            }
        }
        _ => (None, 1.0, pos),
    }
}

fn parse_ident(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start;
    while i < chars.len()
        && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == ':')
    {
        i += 1;
    }
    let name: String = chars[start..i].iter().collect();
    (name, i)
}

fn parse_string(chars: &[char], start: usize) -> Result<(String, usize), TokenError> {
    let mut i = start + 1;
    let mut s = String::new();
    while i < chars.len() {
        if chars[i] == '"' {
            return Ok((s, i + 1));
        }
        s.push(chars[i]);
        i += 1;
    }
    Err(TokenError {
        message: "unterminated string literal".into(),
        offset: start,
    })
}

fn parse_operator(chars: &[char], start: usize) -> (Token, usize) {
    let c = chars[start];
    let has_next = start + 1 < chars.len();

    let two_char = if has_next {
        let pair: String = chars[start..start + 2].iter().collect();
        match pair.as_str() {
            "**" | "==" | "!=" | "<=" | ">=" | "&&" | "||" => Some(pair),
            _ => None,
        }
    } else {
        None
    };

    if let Some(op) = two_char {
        return (Token::Op(op), start + 2);
    }

    match c {
        '(' => (Token::LParen, start + 1),
        ')' => (Token::RParen, start + 1),
        '{' => (Token::LBrace, start + 1),
        '}' => (Token::RBrace, start + 1),
        ',' => (Token::Comma, start + 1),
        '?' => (Token::Question, start + 1),
        ':' => (Token::Colon, start + 1),
        '=' => (Token::Equals, start + 1),
        '!' | '+' | '-' | '*' | '/' | '%' | '\\' | '^' | '~' | '&' | '|' | '<' | '>' | '\'' => {
            (Token::Op(c.to_string()), start + 1)
        }
        other => (Token::Op(other.to_string()), start + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_kinds(tokens: &[SpannedToken]) -> Vec<Token> {
        tokens.iter().map(|t| t.token.clone()).collect()
    }

    fn tok(src: &str, d: Dialect) -> Vec<Token> {
        tokenize(src, d).map(|ts| token_kinds(&ts)).unwrap()
    }

    #[test]
    fn test_number_bare() {
        let t = tok("42", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(42.0, None)]);
    }

    #[test]
    fn test_number_decimal() {
        let t = tok("1.234", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(1.234, None)]);
    }

    #[test]
    fn test_number_leading_dot() {
        let t = tok(".5", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(0.5, None)]);
    }

    #[test]
    fn test_number_scientific() {
        let t = tok("1e-3", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(0.001, None)]);
    }

    #[test]
    fn test_number_suffix_t() {
        let t = tok("1T", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(1e12, Some("T".into()))]);
    }

    #[test]
    fn test_number_suffix_g() {
        let t = tok("2G", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(2e9, Some("G".into()))]);
    }

    #[test]
    fn test_number_suffix_meg() {
        let t = tok("5Meg", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(5e6, Some("Meg".into()))]);
    }

    #[test]
    fn test_number_suffix_meg_upper() {
        let t = tok("5MEG", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(5e6, Some("Meg".into()))]);
    }

    #[test]
    fn test_number_suffix_k_lowercase() {
        let t = tok("10k", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(10000.0, Some("k".into()))]);
    }

    #[test]
    fn test_number_suffix_k_uppercase() {
        let t = tok("10K", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(10000.0, Some("K".into()))]);
    }

    #[test]
    fn test_number_suffix_mil() {
        let t = tok("1mil", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(25.4e-6, Some("mil".into()))]);
    }

    #[test]
    fn test_number_suffix_m_milli() {
        let t = tok("100m", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(0.1, Some("m".into()))]);
    }

    #[test]
    fn test_number_suffix_u() {
        let t = tok("4.7u", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(0.0000047, Some("u".into()))]);
    }

    #[test]
    fn test_number_suffix_n() {
        let t = tok("10n", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(1e-8, Some("n".into()))]);
    }

    #[test]
    fn test_number_suffix_p() {
        let t = tok("100p", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(1e-10, Some("p".into()))]);
    }

    #[test]
    fn test_number_suffix_f() {
        let t = tok("1f", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(1e-15, Some("f".into()))]);
    }

    #[test]
    fn test_number_suffix_a_ngspice() {
        let t = tok("1a", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Number(1e-18, Some("a".into()))]);
    }

    #[test]
    fn test_number_suffix_a_xyce_default() {
        let t = tok("10a", Dialect::Xyce);
        match &t[0] {
            Token::Number(v, _) => assert_eq!(*v, 10.0),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_number_suffix_x_xyce() {
        let t = tok("1X", Dialect::Xyce);
        assert_eq!(t, vec![Token::Number(1e6, Some("X".into()))]);
    }

    #[test]
    fn test_number_suffix_x_ngspice() {
        let t = tok("1X", Dialect::Ngspice);
        match &t[0] {
            Token::Number(v, _) => assert_eq!(*v, 1.0),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_decorative_trailing_letters() {
        let t = tok("10V", Dialect::Ngspice);
        assert_eq!(t.len(), 1);
        match &t[0] {
            Token::Number(v, _) => assert_eq!(*v, 10.0),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_number_10hz() {
        let t = tok("10Hz", Dialect::Ngspice);
        match &t[0] {
            Token::Number(v, _) => assert_eq!(*v, 10.0),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_number_10volts() {
        let t = tok("10Volts", Dialect::Ngspice);
        match &t[0] {
            Token::Number(v, _) => assert_eq!(*v, 10.0),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn test_multi_char_operators_not_split() {
        let t = tok("** == != <= >= && ||", Dialect::Ngspice);
        assert_eq!(
            t,
            vec![
                Token::Op("**".into()),
                Token::Op("==".into()),
                Token::Op("!=".into()),
                Token::Op("<=".into()),
                Token::Op(">=".into()),
                Token::Op("&&".into()),
                Token::Op("||".into()),
            ]
        );
    }

    #[test]
    fn test_ternary_no_space_before_question_mark() {
        let t = tok("a ? b : c", Dialect::Ngspice);
        assert_eq!(
            t,
            vec![
                Token::Ident("a".into()),
                Token::Question,
                Token::Ident("b".into()),
                Token::Colon,
                Token::Ident("c".into()),
            ]
        );
    }

    #[test]
    fn test_ternary_with_spaces() {
        let t = tok("a ? b : c", Dialect::Ngspice);
        assert_eq!(
            t,
            vec![
                Token::Ident("a".into()),
                Token::Question,
                Token::Ident("b".into()),
                Token::Colon,
                Token::Ident("c".into()),
            ]
        );
    }

    #[test]
    fn test_function_call_vs_grouped_subexpression() {
        let t = tok("V(1,2)", Dialect::Ngspice);
        assert_eq!(
            t,
            vec![
                Token::Ident("V".into()),
                Token::LParen,
                Token::Number(1.0, None),
                Token::Comma,
                Token::Number(2.0, None),
                Token::RParen,
            ]
        );

        let t = tok("x*(y)", Dialect::Ngspice);
        assert_eq!(
            t,
            vec![
                Token::Ident("x".into()),
                Token::Op("*".into()),
                Token::LParen,
                Token::Ident("y".into()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let t = tok("\"file.dat\"", Dialect::Ngspice);
        assert_eq!(t, vec![Token::StringLit("file.dat".into())]);
    }

    #[test]
    fn test_string_literal_in_tablefile_call() {
        let t = tok("tablefile(\"data.csv\")", Dialect::Xyce);
        assert_eq!(
            t,
            vec![
                Token::Ident("tablefile".into()),
                Token::LParen,
                Token::StringLit("data.csv".into()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_unterminated_string_literal_returns_diagnostic_not_panic() {
        let r = tokenize("\"unterminated", Dialect::Ngspice);
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn test_empty_expression() {
        let t = tok("", Dialect::Ngspice);
        assert!(t.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let t = tok("   \t\n", Dialect::Ngspice);
        assert!(t.is_empty());
    }

    #[test]
    fn test_ident_with_underscore() {
        let t = tok("my_param", Dialect::Ngspice);
        assert_eq!(t, vec![Token::Ident("my_param".into())]);
    }

    #[test]
    fn test_single_char_operators() {
        let t = tok("+ - * / % \\ ^ & | ~ ! < > '", Dialect::Ngspice);
        let ops: Vec<String> = t
            .iter()
            .filter_map(|tok| match tok {
                Token::Op(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            ops,
            vec!["+", "-", "*", "/", "%", "\\", "^", "&", "|", "~", "!", "<", ">", "'"]
        );
    }

    #[test]
    fn test_parens_and_braces() {
        let t = tok("({1+2})", Dialect::Ngspice);
        assert_eq!(
            t,
            vec![
                Token::LParen,
                Token::LBrace,
                Token::Number(1.0, None),
                Token::Op("+".into()),
                Token::Number(2.0, None),
                Token::RBrace,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_hierarchical_node_path_colon() {
        let t = tok("Xmain:Xnot1:A", Dialect::Xyce);
        assert_eq!(t, vec![Token::Ident("Xmain:Xnot1:A".into())]);
    }
}
