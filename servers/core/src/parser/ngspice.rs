use super::NodeParamsResult;
use super::{ParseError, ParseResult};
use crate::ast::*;
use crate::dialect::{DeviceKind, Dialect};
use crate::lexer::ProcessedLine;

pub fn parse(lines: &[ProcessedLine]) -> Vec<ParseResult> {
    lines.iter().map(parse_line).collect()
}

fn parse_line(line: &ProcessedLine) -> ParseResult {
    let span = span_of(line);

    let trimmed = line.text.trim();
    if trimmed.is_empty() {
        return Ok(Statement::Comment(span));
    }

    if trimmed.starts_with('.') {
        parse_dot_command(trimmed, span)
    } else if trimmed
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        parse_element(trimmed, span)
    } else {
        Ok(Statement::Unrecognized(trimmed.to_string(), span))
    }
}

fn span_of(line: &ProcessedLine) -> LineSpan {
    let start = line.source_lines.first().copied().unwrap_or(1);
    let end = line.source_lines.last().copied().unwrap_or(1);
    start..end + 1
}

fn parse_dot_command(line: &str, span: LineSpan) -> ParseResult {
    let upper = line.to_uppercase();
    if upper.starts_with(".SUBCKT") {
        parse_subckt(line, span)
    } else if upper.starts_with(".ENDS") {
        parse_ends(line, span)
    } else if upper.starts_with(".MODEL") {
        parse_model(line, span)
    } else if upper.starts_with(".PARAM") {
        parse_param(line, span)
    } else if upper.starts_with(".GLOBAL_PARAM") {
        parse_global_param(line, span)
    } else if upper.starts_with(".GLOBAL") {
        parse_global(line, span)
    } else if upper.starts_with(".IC") {
        parse_ic(line, span)
    } else if upper.starts_with(".NODESET") {
        parse_nodeset(line, span)
    } else if upper.starts_with(".TEMP") {
        parse_temp(line, span)
    } else if upper.starts_with(".CSPARAM") {
        parse_csparam(line, span)
    } else if upper.starts_with(".OPTIONS") {
        parse_options_ngspice(line, span)
    } else if upper.starts_with(".FUNC") {
        parse_func(line, span)
    } else if upper.starts_with(".INCLUDE") || upper.starts_with(".INC") {
        parse_include(line, span)
    } else if upper.starts_with(".LIB") {
        parse_lib(line, span)
    } else if upper.starts_with(".AC") {
        Ok(Statement::Ac(rest_after(".AC", line), span))
    } else if upper.starts_with(".DC") {
        Ok(Statement::Dc(rest_after(".DC", line), span))
    } else if upper.starts_with(".OP") {
        Ok(Statement::Op(span))
    } else if upper.starts_with(".TRAN") {
        Ok(Statement::Tran(rest_after(".TRAN", line), span))
    } else if is_ngspice_analysis(&upper) {
        Ok(Statement::Analysis {
            keyword: first_word(&upper),
            dialect_tag: Some("ngspice".into()),
            raw_args: rest_after(&first_word(&upper), line),
            span,
        })
    } else {
        Ok(Statement::Unrecognized(line.to_string(), span))
    }
}

fn rest_after(keyword: &str, line: &str) -> String {
    if line.len() > keyword.len() {
        line[keyword.len()..].trim().to_string()
    } else {
        String::new()
    }
}

fn first_word(s: &str) -> String {
    s.split_whitespace().next().unwrap_or("").to_string()
}

fn is_ngspice_analysis(upper: &str) -> bool {
    let kw = upper.split_whitespace().next().unwrap_or("");
    matches!(
        kw,
        ".DISTO"
            | ".NOISE"
            | ".PZ"
            | ".SENS"
            | ".SP"
            | ".FOUR"
            | ".PROBE"
            | ".WIDTH"
            | ".MEASURE"
            | ".MEAS"
    )
}

fn parse_global(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".global".len()..].trim();
    let nodes: Vec<String> = rest.split_whitespace().map(|s| s.to_string()).collect();
    Ok(Statement::Global(nodes, span))
}

fn parse_ic(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".ic".len()..].trim();
    let assignments = parse_param_assignments(rest);
    Ok(Statement::Ic(assignments, span))
}

fn parse_nodeset(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".nodeset".len()..].trim();
    if rest.to_uppercase().starts_with("ALL=") {
        Ok(Statement::NodesetAll(
            rest["ALL=".len()..].to_string(),
            span,
        ))
    } else {
        let assignments = parse_param_assignments(rest);
        Ok(Statement::Nodeset(assignments, span))
    }
}

fn parse_temp(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".temp".len()..].trim();
    Ok(Statement::Temp(rest.to_string(), span))
}

fn parse_csparam(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".csparam".len()..].trim();
    let assignments = parse_param_assignments(rest);
    Ok(Statement::Csparam(assignments, span))
}

fn parse_global_param(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".global_param".len()..].trim();
    let assignments = parse_param_assignments(rest);
    Ok(Statement::GlobalParam(Param {
        assignments,
        span: span.clone(),
    }))
}

fn parse_options_ngspice(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".options".len()..].trim();
    let assignments = parse_option_assignments(rest);
    Ok(Statement::Options {
        package: None,
        assignments,
        span,
    })
}

fn parse_option_assignments(text: &str) -> Vec<(String, Option<String>)> {
    let mut result = Vec::new();
    for token in text.split_whitespace() {
        if let Some(eq_pos) = token.find('=') {
            let name = token[..eq_pos].to_string();
            let val = token[eq_pos + 1..].to_string();
            result.push((name, Some(val)));
        } else {
            result.push((token.to_string(), None));
        }
    }
    result
}

fn parse_element(line: &str, span: LineSpan) -> ParseResult {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(ParseError {
            message: "empty element line".into(),
            span,
        });
    }

    let first = tokens[0];
    let device_letter = first.chars().next().unwrap().to_ascii_uppercase();
    let name = first.to_string();

    let kind = Dialect::Ngspice.resolve_device_letter(device_letter);

    let (nodes, raw_params, subckt_name) = match kind {
        Some(k) => split_nodes_params(&tokens[1..], k, &span)?,
        None => {
            let params: Vec<String> = tokens[1..].iter().map(|s| s.to_string()).collect();
            (vec![], params, None)
        }
    };

    Ok(Statement::ElementInstance(ElementInstance {
        device_letter,
        name,
        nodes,
        raw_params,
        subckt_name,
        span,
    }))
}

fn split_nodes_params(tokens: &[&str], kind: DeviceKind, span: &LineSpan) -> NodeParamsResult {
    let min = kind.min_nodes();

    match kind {
        DeviceKind::SubcircuitCall => split_subcircuit_nodes_params(tokens, span),
        DeviceKind::MutualInductor => split_mutual_inductor(tokens, span),
        DeviceKind::Cccs | DeviceKind::Ccvs | DeviceKind::CurrentSwitch => {
            if tokens.len() < 3 {
                return Err(ParseError {
                    message: format!(
                        "controlling-source device {kind:?} requires at least 3 tokens (2 nodes + source name), got {}",
                        tokens.len()
                    ),
                    span: span.clone(),
                });
            }
            let nodes: Vec<String> = tokens[..2].iter().map(|s| s.to_string()).collect();
            let params: Vec<String> = tokens[2..].iter().map(|s| s.to_string()).collect();
            Ok((nodes, params, None))
        }
        _ => {
            if tokens.len() < min {
                return Err(ParseError {
                    message: format!(
                        "device {kind:?} requires at least {min} nodes, got {} tokens",
                        tokens.len()
                    ),
                    span: span.clone(),
                });
            }
            let nodes: Vec<String> = tokens[..min].iter().map(|s| s.to_string()).collect();
            let params: Vec<String> = tokens[min..].iter().map(|s| s.to_string()).collect();
            Ok((nodes, params, None))
        }
    }
}

fn split_subcircuit_nodes_params(tokens: &[&str], span: &LineSpan) -> NodeParamsResult {
    let param_start = tokens.iter().position(|t| t.contains('='));
    let subckt_name_idx = match param_start {
        Some(p) if p > 0 => p.saturating_sub(1),
        Some(_) => 0,
        None if !tokens.is_empty() => tokens.len() - 1,
        None => 0,
    };

    if tokens.is_empty() || (subckt_name_idx == 0 && param_start != Some(0)) {
        return Err(ParseError {
            message: "subcircuit call has no nodes or subcircuit name".into(),
            span: span.clone(),
        });
    }

    let subckt_name = match tokens.get(subckt_name_idx) {
        Some(&s) if !s.contains('=') => Some(s.to_string()),
        _ => None,
    };

    let nodes: Vec<String> = tokens[..subckt_name_idx]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let params: Vec<String> = tokens[subckt_name_idx + 1..]
        .iter()
        .map(|s| s.to_string())
        .collect();
    Ok((nodes, params, subckt_name))
}

fn split_mutual_inductor(tokens: &[&str], span: &LineSpan) -> NodeParamsResult {
    if tokens.len() < 2 {
        return Err(ParseError {
            message: format!(
                "mutual inductor K requires at least 2 inductor names + coupling, got {} tokens",
                tokens.len()
            ),
            span: span.clone(),
        });
    }
    let last = tokens.len() - 1;
    let inductors: Vec<String> = tokens[..last].iter().map(|s| s.to_string()).collect();
    let params: Vec<String> = vec![tokens[last].to_string()];
    Ok((inductors, params, None))
}

fn parse_subckt(line: &str, span: LineSpan) -> ParseResult {
    let after_keyword = line[".subckt".len()..].trim();
    if after_keyword.is_empty() {
        return Err(ParseError {
            message: ".subckt missing name".into(),
            span,
        });
    }

    let tokens: Vec<&str> = after_keyword.split_whitespace().collect();
    let name = tokens[0].to_string();
    let remaining = &tokens[1..];

    if remaining.iter().any(|t| t.to_uppercase() == "PARAMS:") {
        return Err(ParseError {
            message: "PARAMS: keyword is not valid ngspice syntax (Xyce/HSPICE only)".into(),
            span,
        });
    }

    let (nodes, params) = split_node_list_and_param_defaults(remaining);

    Ok(Statement::Subckt(Subckt {
        name,
        nodes,
        params,
        span,
    }))
}

fn split_node_list_and_param_defaults(
    tokens: &[&str],
) -> (Vec<String>, Vec<(String, Option<String>)>) {
    let mut nodes = Vec::new();
    let mut params = Vec::new();
    let mut in_params = false;

    for token in tokens {
        if !in_params && token.contains('=') {
            in_params = true;
        }
        if in_params {
            if let Some(eq_pos) = token.find('=') {
                let ident = token[..eq_pos].to_string();
                let value = token[eq_pos + 1..].to_string();
                params.push((ident, Some(value)));
            } else {
                params.push((token.to_string(), None));
            }
        } else {
            nodes.push(token.to_string());
        }
    }

    (nodes, params)
}

fn parse_ends(line: &str, span: LineSpan) -> ParseResult {
    let after = line[".ends".len()..].trim();
    let name = if after.is_empty() {
        None
    } else {
        Some(after.to_string())
    };
    Ok(Statement::Ends(name, span))
}

fn parse_model(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".model".len()..].trim();
    let tokens: Vec<&str> = rest.splitn(3, |c: char| c.is_whitespace()).collect();

    if tokens.len() < 2 {
        return Err(ParseError {
            message: ".model requires name and type".into(),
            span,
        });
    }

    let name = tokens[0].to_string();
    let model_type = tokens[1].to_string();
    let raw_params = if tokens.len() > 2 {
        tokens[2].to_string()
    } else {
        String::new()
    };

    Ok(Statement::Model(Model {
        name,
        model_type,
        raw_params,
        span,
    }))
}

fn parse_param(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".param".len()..].trim();
    let assignments = parse_param_assignments(rest);
    Ok(Statement::Param(Param { assignments, span }))
}

fn parse_param_assignments(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_single_quote = false;
    let mut in_brace = false;

    for ch in text.chars() {
        match ch {
            '\'' => in_single_quote = !in_single_quote,
            '{' => in_brace = true,
            '}' => in_brace = false,
            ' ' if depth == 0 && !in_single_quote && !in_brace => {
                if let Some((ident, value)) = try_split_assignment(&current) {
                    result.push((ident, value));
                }
                current.clear();
                continue;
            }
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        current.push(ch);
    }

    if !current.is_empty() {
        if let Some((ident, value)) = try_split_assignment(&current) {
            result.push((ident, value));
        }
    }

    result
}

fn try_split_assignment(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(eq_pos) = text.find('=') {
        let ident = text[..eq_pos].trim().to_string();
        let value = text[eq_pos + 1..].trim().to_string();
        Some((ident, value))
    } else {
        Some((text.to_string(), String::new()))
    }
}

fn parse_func(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".func".len()..].trim();

    let (name_args, body) = if let Some(brace_open) = rest.find('{') {
        let brace_close = rest.rfind('}').unwrap_or(rest.len());
        let na = rest[..brace_open].trim();
        let b = rest[brace_open + 1..brace_close].trim();
        (na, b.to_string())
    } else if let Some(eq_pos) = rest.find('=') {
        let na = rest[..eq_pos].trim();
        let b = rest[eq_pos + 1..].trim();
        (na, b.to_string())
    } else {
        return Err(ParseError {
            message: ".func requires a body in {braces} or after =".into(),
            span,
        });
    };

    let (name, args) = if let Some(paren_open) = name_args.find('(') {
        let paren_close = name_args.rfind(')').unwrap_or(name_args.len());
        let n = name_args[..paren_open].trim().to_string();
        let a: Vec<String> = name_args[paren_open + 1..paren_close]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        (n, a)
    } else {
        (name_args.to_string(), vec![])
    };

    Ok(Statement::Func(Func {
        name,
        args,
        body,
        span,
    }))
}

fn parse_include(line: &str, span: LineSpan) -> ParseResult {
    let keyword_len = if line.to_uppercase().starts_with(".INCLUDE") {
        ".include".len()
    } else {
        ".inc".len()
    };
    let rest = line[keyword_len..].trim();
    Ok(Statement::Include(rest.to_string(), span))
}

fn parse_lib(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".lib".len()..].trim();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let filename = tokens.first().map(|s| s.to_string()).unwrap_or_default();
    let section = tokens.get(1).map(|s| s.to_string());
    Ok(Statement::Lib(filename, section, span))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::preprocess;

    fn parsed(src: &str) -> Vec<ParseResult> {
        let lines = preprocess(src, Dialect::Ngspice);
        parse(&lines)
    }

    fn ok_statements(results: Vec<ParseResult>) -> Vec<Statement> {
        results.into_iter().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn test_r_device() {
        let s = ok_statements(parsed("R1 1 2 100\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'R',
                name: "R1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec!["100".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_c_device() {
        let s = ok_statements(parsed("C1 3 0 1u ic=2\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'C',
                name: "C1".into(),
                nodes: vec!["3".into(), "0".into()],
                raw_params: vec!["1u".into(), "ic=2".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_l_device() {
        let s = ok_statements(parsed("L1 5 6 10m\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'L',
                name: "L1".into(),
                nodes: vec!["5".into(), "6".into()],
                raw_params: vec!["10m".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_v_device() {
        let s = ok_statements(parsed("V1 1 0 DC 5\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'V',
                name: "V1".into(),
                nodes: vec!["1".into(), "0".into()],
                raw_params: vec!["DC".into(), "5".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_i_device() {
        let s = ok_statements(parsed("I1 2 0 AC 1m 0\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'I',
                name: "I1".into(),
                nodes: vec!["2".into(), "0".into()],
                raw_params: vec!["AC".into(), "1m".into(), "0".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_d_device() {
        let s = ok_statements(parsed("D1 1 2 1N4148\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'D',
                name: "D1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec!["1N4148".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_q_device() {
        let s = ok_statements(parsed("Q1 c b e 2N3904\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'Q',
                name: "Q1".into(),
                nodes: vec!["c".into(), "b".into(), "e".into()],
                raw_params: vec!["2N3904".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_j_device() {
        let s = ok_statements(parsed("J1 d g s 2N3819\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'J',
                name: "J1".into(),
                nodes: vec!["d".into(), "g".into(), "s".into()],
                raw_params: vec!["2N3819".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_m_device() {
        let s = ok_statements(parsed("M1 d g s b NMOS L=1u W=10u\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'M',
                name: "M1".into(),
                nodes: vec!["d".into(), "g".into(), "s".into(), "b".into()],
                raw_params: vec!["NMOS".into(), "L=1u".into(), "W=10u".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_x_device() {
        let s = ok_statements(parsed("X1 1 2 3 opamp gain=10\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'X',
                name: "X1".into(),
                nodes: vec!["1".into(), "2".into(), "3".into()],
                raw_params: vec!["gain=10".into()],
                subckt_name: Some("opamp".into()),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_x_device_no_params() {
        let s = ok_statements(parsed("X1 1 2 opamp\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'X',
                name: "X1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec![],
                subckt_name: Some("opamp".into()),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_ngspice_a_device_xspice_code_model() {
        let s = ok_statements(parsed("A1 %vd [1 2] adc_bridge\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'A',
                name: "A1".into(),
                nodes: vec!["%vd".into()],
                raw_params: vec!["[1".into(), "2]".into(), "adc_bridge".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_ngspice_u_device_resolves_ngspice_meaning() {
        let s = ok_statements(parsed("U1 1 2 3 urc_model\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'U',
                name: "U1".into(),
                nodes: vec!["1".into(), "2".into(), "3".into()],
                raw_params: vec!["urc_model".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_malformed_element_line_returns_diagnostic_not_panic() {
        let results = parsed("R1 1\n");
        assert!(results[0].is_err());
    }

    #[test]
    fn test_ngspice_p_device() {
        let s = ok_statements(parsed("P1 1 2 3 4 5 6 cpl_model\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'P');
        assert_eq!(ei.nodes, vec!["1"]);
        assert_eq!(ei.raw_params, vec!["2", "3", "4", "5", "6", "cpl_model"]);
    }

    #[test]
    fn test_ngspice_y_device() {
        let s = ok_statements(parsed("Y1 1 2 txl_model\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
    }

    #[test]
    fn test_e_device() {
        let s = ok_statements(parsed("E1 5 0 1 0 10\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'E',
                name: "E1".into(),
                nodes: vec!["5".into(), "0".into(), "1".into(), "0".into()],
                raw_params: vec!["10".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_g_device() {
        let s = ok_statements(parsed("G1 3 0 1 2 0.1\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'G',
                name: "G1".into(),
                nodes: vec!["3".into(), "0".into(), "1".into(), "2".into()],
                raw_params: vec!["0.1".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_f_device() {
        let s = ok_statements(parsed("F1 1 2 Vmeas 100\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'F',
                name: "F1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec!["Vmeas".into(), "100".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_h_device() {
        let s = ok_statements(parsed("H1 3 4 Vsense 50\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'H',
                name: "H1".into(),
                nodes: vec!["3".into(), "4".into()],
                raw_params: vec!["Vsense".into(), "50".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_b_device() {
        let s = ok_statements(parsed("B1 out 0 V=V(in)*2\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'B',
                name: "B1".into(),
                nodes: vec!["out".into(), "0".into()],
                raw_params: vec!["V=V(in)*2".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_s_device() {
        let s = ok_statements(parsed("S1 1 2 3 0 smodel\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'S',
                name: "S1".into(),
                nodes: vec!["1".into(), "2".into(), "3".into(), "0".into()],
                raw_params: vec!["smodel".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_w_device() {
        let s = ok_statements(parsed("W1 1 2 Vctrl wmodel\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'W',
                name: "W1".into(),
                nodes: vec!["1".into(), "2".into()],
                raw_params: vec!["Vctrl".into(), "wmodel".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_t_device() {
        let s = ok_statements(parsed("T1 1 0 2 0 Z0=50 TD=1n\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'T',
                name: "T1".into(),
                nodes: vec!["1".into(), "0".into(), "2".into(), "0".into()],
                raw_params: vec!["Z0=50".into(), "TD=1n".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_k_device() {
        let s = ok_statements(parsed("K1 L1 L2 0.99\n"));
        assert_eq!(
            s[0],
            Statement::ElementInstance(ElementInstance {
                device_letter: 'K',
                name: "K1".into(),
                nodes: vec!["L1".into(), "L2".into()],
                raw_params: vec!["0.99".into()],
                subckt_name: None,
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_subckt_rejects_params_keyword() {
        let results = parsed(".subckt opamp in+ in- out PARAMS: gain=100\n");
        assert!(results[0].is_err());
        let err = results[0].as_ref().unwrap_err();
        assert!(err.message.contains("PARAMS:"));
    }

    #[test]
    fn test_subckt_bare_ident_value_defaults() {
        let s = ok_statements(parsed(".subckt opamp in+ in- out gain=100\n"));
        assert_eq!(
            s[0],
            Statement::Subckt(Subckt {
                name: "opamp".into(),
                nodes: vec!["in+".into(), "in-".into(), "out".into()],
                params: vec![("gain".into(), Some("100".into()))],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_subckt_nodes_only_no_params() {
        let s = ok_statements(parsed(".subckt nand a b out\n"));
        assert_eq!(
            s[0],
            Statement::Subckt(Subckt {
                name: "nand".into(),
                nodes: vec!["a".into(), "b".into(), "out".into()],
                params: vec![],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_ends() {
        let s = ok_statements(parsed(".ends\n"));
        assert_eq!(s[0], Statement::Ends(None, 1..2));

        let s = ok_statements(parsed(".ends opamp\n"));
        assert_eq!(s[0], Statement::Ends(Some("opamp".into()), 1..2));
    }

    #[test]
    fn test_model_captures_type_and_raw_params() {
        let s = ok_statements(parsed(".model NPN_FAST NPN (BF=200 IS=1e-14)\n"));
        assert_eq!(
            s[0],
            Statement::Model(Model {
                name: "NPN_FAST".into(),
                model_type: "NPN".into(),
                raw_params: "(BF=200 IS=1e-14)".into(),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_model_no_params() {
        let s = ok_statements(parsed(".model DMOD D\n"));
        assert_eq!(
            s[0],
            Statement::Model(Model {
                name: "DMOD".into(),
                model_type: "D".into(),
                raw_params: String::new(),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_param_single() {
        let s = ok_statements(parsed(".param a=5\n"));
        assert_eq!(
            s[0],
            Statement::Param(Param {
                assignments: vec![("a".into(), "5".into())],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_param_multiple_assignments_one_line() {
        let s = ok_statements(parsed(".param a=1 b=2 c=3\n"));
        assert_eq!(
            s[0],
            Statement::Param(Param {
                assignments: vec![
                    ("a".into(), "1".into()),
                    ("b".into(), "2".into()),
                    ("c".into(), "3".into())
                ],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_param_with_expression() {
        let s = ok_statements(parsed(".param rload={Vout/Iout}\n"));
        assert_eq!(
            s[0],
            Statement::Param(Param {
                assignments: vec![("rload".into(), "{Vout/Iout}".into())],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_func() {
        let s = ok_statements(parsed(".func myfunc(a,b) {a+b*2}\n"));
        assert_eq!(
            s[0],
            Statement::Func(Func {
                name: "myfunc".into(),
                args: vec!["a".into(), "b".into()],
                body: "a+b*2".into(),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_func_no_args() {
        let s = ok_statements(parsed(".func pi {3.14159}\n"));
        assert_eq!(
            s[0],
            Statement::Func(Func {
                name: "pi".into(),
                args: vec![],
                body: "3.14159".into(),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_include() {
        let s = ok_statements(parsed(".include ./models.lib\n"));
        assert_eq!(s[0], Statement::Include("./models.lib".into(), 1..2));
    }

    #[test]
    fn test_lib() {
        let s = ok_statements(parsed(".lib ./libs/cmos.lib typical\n"));
        assert_eq!(
            s[0],
            Statement::Lib("./libs/cmos.lib".into(), Some("typical".into()), 1..2)
        );
    }

    #[test]
    fn test_lib_no_section() {
        let s = ok_statements(parsed(".lib models.lib\n"));
        assert_eq!(s[0], Statement::Lib("models.lib".into(), None, 1..2));
    }

    #[test]
    fn test_comment_lines() {
        let src = "* full line comment\nR1 1 2 100\n";
        let results = parsed(src);
        let stmts: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(stmts[0], Statement::Comment(1..2));
        assert!(matches!(stmts[1], Statement::ElementInstance(_)));
    }

    #[test]
    fn test_unrecognized_statement() {
        let s = ok_statements(parsed(".unknown_directive foo bar\n"));
        assert!(matches!(s[0], Statement::Unrecognized(_, _)));
    }

    // --- CORE-28: .GLOBAL/.IC/.NODESET/.TEMP/.CSPARAM ---

    #[test]
    fn test_global_multiple_nodes() {
        let s = ok_statements(parsed(".global vdd vss\n"));
        assert_eq!(
            s[0],
            Statement::Global(vec!["vdd".into(), "vss".into()], 1..2)
        );
    }

    #[test]
    fn test_ic_multiple_assignments() {
        let s = ok_statements(parsed(".ic v(1)=5 v(2)=0\n"));
        match &s[0] {
            Statement::Ic(assignments, _) => {
                assert_eq!(assignments.len(), 2);
            }
            other => panic!("expected Ic, got {other:?}"),
        }
    }

    #[test]
    fn test_nodeset_multiple_assignments() {
        let s = ok_statements(parsed(".nodeset v(3)=1.2\n"));
        assert!(matches!(s[0], Statement::Nodeset(_, _)));
    }

    #[test]
    fn test_nodeset_all_equals_value() {
        let s = ok_statements(parsed(".nodeset all=0\n"));
        assert_eq!(s[0], Statement::NodesetAll("0".into(), 1..2));
    }

    #[test]
    fn test_temp_single_value() {
        let s = ok_statements(parsed(".temp 27\n"));
        assert_eq!(s[0], Statement::Temp("27".into(), 1..2));
    }

    #[test]
    fn test_csparam_ngspice_no_diagnostic() {
        let results = parsed(".csparam x=5\n");
        assert!(results[0].is_ok());
        assert!(matches!(results[0], Ok(Statement::Csparam(_, _))));
    }

    // --- CORE-29: .OPTIONS ---

    #[test]
    fn test_ngspice_options_flat_namespace_bare_and_valued_flags() {
        let s = ok_statements(parsed(".options reltol=1e-3 acct\n"));
        match &s[0] {
            Statement::Options {
                package,
                assignments,
                ..
            } => {
                assert_eq!(*package, None);
                assert_eq!(assignments.len(), 2);
            }
            other => panic!("expected Options, got {other:?}"),
        }
    }

    // --- CORE-30: common analysis statements ---

    #[test]
    fn test_ac_dec_form() {
        let s = ok_statements(parsed(".ac dec 10 1 1meg\n"));
        assert_eq!(s[0], Statement::Ac("dec 10 1 1meg".into(), 1..2));
    }

    #[test]
    fn test_dc_single_sweep() {
        let s = ok_statements(parsed(".dc V1 0 5 0.1\n"));
        assert_eq!(s[0], Statement::Dc("V1 0 5 0.1".into(), 1..2));
    }

    #[test]
    fn test_dc_nested_sweep() {
        let s = ok_statements(parsed(".dc V1 0 5 1 V2 0 1 0.5\n"));
        assert_eq!(s[0], Statement::Dc("V1 0 5 1 V2 0 1 0.5".into(), 1..2));
    }

    #[test]
    fn test_op_no_args() {
        let s = ok_statements(parsed(".op\n"));
        assert_eq!(s[0], Statement::Op(1..2));
    }

    #[test]
    fn test_tran_basic_form() {
        let s = ok_statements(parsed(".tran 1n 100n\n"));
        assert_eq!(s[0], Statement::Tran("1n 100n".into(), 1..2));
    }

    #[test]
    fn test_tran_with_uic() {
        let s = ok_statements(parsed(".tran 1n 100n uic\n"));
        assert_eq!(s[0], Statement::Tran("1n 100n uic".into(), 1..2));
    }

    // --- CORE-31: dialect-specific analysis/output statements ---

    #[test]
    fn test_ngspice_only_disto_recognized() {
        let s = ok_statements(parsed(".disto dec 10 1k 100k\n"));
        assert!(matches!(s[0], Statement::Analysis { .. }));
    }

    #[test]
    fn test_ngspice_only_pz_recognized() {
        let s = ok_statements(parsed(".pz 1 2 3 4 cur pol\n"));
        assert!(matches!(s[0], Statement::Analysis { .. }));
    }

    #[test]
    #[ignore = "CORE-31 blocked: is_ngspice_analysis() has no Xyce-only-keyword check at all, so .STEP/.HB/etc fall through to Unrecognized under ngspice with zero explanation instead of a 'this is Xyce-only' diagnostic. See progress.core.yaml CORE-31 blocked note."]
    fn test_xyce_only_keyword_flagged_under_ngspice() {
        // .STEP is Xyce-only per docs/GRAMMAR.md §7 — ngspice has no native
        // .STEP (emulated via .control+alter+run). It must not be silently
        // accepted as a recognized Analysis statement here.
        let results = parsed(".step Vin 0 5 1\n");
        match &results[0] {
            Ok(Statement::Analysis { .. }) => {
                panic!(".STEP was silently accepted under ngspice as a recognized Analysis statement, but it is Xyce-only per docs/GRAMMAR.md §7")
            }
            Ok(Statement::Unrecognized(_, _)) => {
                panic!(".STEP fell through to Unrecognized under ngspice with no diagnostic explaining it is Xyce-only")
            }
            Ok(other) => panic!("unexpected: {other:?}"),
            Err(_) => {} // acceptable: rejected with a diagnostic
        }
    }

    #[test]
    fn test_measure_both_spellings_captured_as_raw_statement() {
        let s1 = ok_statements(parsed(".measure tran vout1 max v(1)\n"));
        let s2 = ok_statements(parsed(".meas tran vout1 max v(1)\n"));
        assert!(matches!(s1[0], Statement::Analysis { .. }));
        assert!(matches!(s2[0], Statement::Analysis { .. }));
    }
}
