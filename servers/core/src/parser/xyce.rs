use super::NodeParamsResult;
use super::{ParseError, ParseResult};
use crate::ast::*;
use crate::dialect::{DeviceKind, Dialect};
use crate::lexer::ProcessedLine;

pub fn parse(lines: &[ProcessedLine]) -> Vec<ParseResult> {
    let mut results = Vec::new();
    let mut subckt_depth = 0u32;
    for line in lines {
        let result = parse_line(line, subckt_depth);
        let trimmed = line.text.trim();
        let upper = trimmed.to_uppercase();
        if upper.starts_with(".SUBCKT") {
            subckt_depth += 1;
        } else if upper.starts_with(".ENDS") {
            subckt_depth = subckt_depth.saturating_sub(1);
        }
        results.push(result);
    }
    results
}

fn parse_line(line: &ProcessedLine, subckt_depth: u32) -> ParseResult {
    let span = span_of(line);
    let trimmed = line.text.trim();

    if trimmed.is_empty() {
        return Ok(Statement::Comment(span));
    }

    if trimmed.starts_with('.') {
        parse_dot_command(trimmed, span, subckt_depth)
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

fn parse_dot_command(line: &str, span: LineSpan, subckt_depth: u32) -> ParseResult {
    let upper = line.to_uppercase();
    if upper.starts_with(".SUBCKT") {
        parse_subckt(line, span)
    } else if upper.starts_with(".ENDS") {
        parse_ends(line, span)
    } else if upper.starts_with(".GLOBAL_PARAM") {
        parse_global_param(line, span, subckt_depth)
    } else if upper.starts_with(".GLOBAL") {
        parse_global(line, span)
    } else if upper.starts_with(".IC") {
        parse_ic(line, span)
    } else if upper.starts_with(".NODESET") {
        parse_nodeset_xyce(line, span)
    } else if upper.starts_with(".TEMP") {
        parse_temp(line, span)
    } else if upper.starts_with(".CSPARAM") {
        Ok(Statement::CsparamInfo(
            "ngspice-only extension".into(),
            span,
        ))
    } else if upper.starts_with(".OPTIONS") {
        parse_options_xyce(line, span, subckt_depth)
    } else if upper.starts_with(".PARAM") {
        parse_param(line, span)
    } else if upper.starts_with(".MODEL") {
        parse_model(line, span)
    } else if upper.starts_with(".FUNC") {
        parse_func(line, span)
    } else if upper.starts_with(".INCLUDE")
        || upper.starts_with(".INC")
        || upper.starts_with(".INCL")
    {
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
    } else if is_xyce_analysis(&upper) {
        Ok(Statement::Analysis {
            keyword: first_word(&upper),
            dialect_tag: Some("xyce".into()),
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
fn is_xyce_analysis(upper: &str) -> bool {
    let kw = upper.split_whitespace().next().unwrap_or("");
    matches!(
        kw,
        ".STEP"
            | ".HB"
            | ".LIN"
            | ".SAMPLING"
            | ".EMBEDDEDSAMPLING"
            | ".PCE"
            | ".DATA"
            | ".SENS"
            | ".RESULT"
            | ".SAVE"
            | ".PREPROCESS"
            | ".FFT"
            | ".MEASURE"
            | ".MEAS"
            | ".DISTO"
            | ".NOISE"
            | ".PZ"
            | ".SP"
            | ".FOUR"
            | ".PROBE"
            | ".WIDTH"
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
fn parse_nodeset_xyce(line: &str, span: LineSpan) -> ParseResult {
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
fn parse_options_xyce(line: &str, span: LineSpan, subckt_depth: u32) -> ParseResult {
    if subckt_depth > 0 {
        return Err(ParseError {
            message: ".OPTIONS is top-level only in Xyce, cannot appear inside a .subckt".into(),
            span,
        });
    }
    let rest = line[".options".len()..].trim();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(ParseError {
            message: ".OPTIONS requires a package keyword in Xyce (e.g. DEVICE)".into(),
            span,
        });
    }
    let first = tokens[0];
    let is_known_pkg = matches!(
        first.to_uppercase().as_str(),
        "DEVICE"
            | "TIMEINT"
            | "NONLIN"
            | "LINSOL"
            | "OUTPUT"
            | "PARSER"
            | "SENSITIVITY"
            | "HBINT"
            | "MEASURE"
    );
    if is_known_pkg {
        let package = Some(first.to_string());
        let assignments = parse_option_assignments(&tokens[1..].join(" "));
        Ok(Statement::Options {
            package,
            assignments,
            span,
        })
    } else {
        Err(ParseError {
            message: ".OPTIONS requires a package keyword in Xyce (e.g. DEVICE)".into(),
            span,
        })
    }
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

    let kind = if device_letter == 'Y' {
        resolve_y_type(&name)
    } else {
        Dialect::Xyce.resolve_device_letter(device_letter)
    };

    let (nodes, raw_params, subckt_name) = match kind {
        Some(DeviceKind::Bjt) => {
            let (n, p, _) = split_bjt_nodes_params_xyce(&tokens[1..], &span)?;
            (n, p, None)
        }
        Some(k) => split_nodes_params_xyce(&tokens[1..], k, &span)?,
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

fn resolve_y_type(name: &str) -> Option<DeviceKind> {
    let upper = name.to_uppercase();
    if upper.starts_with("YACC") {
        Some(DeviceKind::YAcc)
    } else if upper.starts_with("YDELAY") {
        Some(DeviceKind::YDelay)
    } else if upper.starts_with("YLIN") {
        Some(DeviceKind::YLin)
    } else if upper.starts_with("YMEMRISTOR") {
        Some(DeviceKind::YMemristor)
    } else if upper.starts_with("YTRANSLINE") || upper.starts_with("YTRANS") {
        Some(DeviceKind::YTransline)
    } else if upper.starts_with("YPDE") {
        Some(DeviceKind::YPde)
    } else {
        None
    }
}

fn split_nodes_params_xyce(tokens: &[&str], kind: DeviceKind, span: &LineSpan) -> NodeParamsResult {
    let min = kind.min_nodes();

    match kind {
        DeviceKind::SubcircuitCall => split_subcircuit_nodes_params(tokens, span),
        DeviceKind::MutualInductor => split_mutual_inductor(tokens, span),
        DeviceKind::Cccs | DeviceKind::Ccvs | DeviceKind::CurrentSwitch => {
            if tokens.len() < 3 {
                return Err(ParseError {
                    message: format!(
                        "device {kind:?} requires at least 3 tokens (2 nodes + source name), got {}",
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

fn split_bjt_nodes_params_xyce(tokens: &[&str], span: &LineSpan) -> NodeParamsResult {
    if tokens.len() < 3 {
        return Err(ParseError {
            message: format!(
                "BJT requires at least 3 nodes (c b e), got {} tokens",
                tokens.len()
            ),
            span: span.clone(),
        });
    }

    let mut nodes: Vec<String> = tokens[..3].iter().map(|s| s.to_string()).collect();
    let mut remaining = &tokens[3..];

    if let Some(&first_rest) = remaining.first() {
        if first_rest.starts_with('[') && first_rest.ends_with(']') {
            let inner = &first_rest[1..first_rest.len() - 1];
            nodes.push(inner.to_string());
            remaining = &remaining[1..];
        } else if first_rest.chars().all(|c| c.is_ascii_digit()) {
            nodes.push(first_rest.to_string());
            remaining = &remaining[1..];
        }
    }

    let params: Vec<String> = remaining.iter().map(|s| s.to_string()).collect();
    Ok((nodes, params, None))
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
            message: "mutual inductor K requires at least 2 inductor names + coupling".into(),
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

    let effective: Vec<&str> = remaining
        .iter()
        .filter(|t| t.to_uppercase() != "PARAMS:")
        .copied()
        .collect();

    let (nodes, params) = split_node_list_and_param_defaults(&effective);

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

    let model_type = tokens[1].to_string();
    let raw_params = if tokens.len() > 2 {
        tokens[2].to_string()
    } else {
        String::new()
    };

    if let Err(msg) = validate_xyce_model_params(&raw_params) {
        return Err(ParseError { message: msg, span });
    }

    Ok(Statement::Model(Model {
        name: tokens[0].to_string(),
        model_type,
        raw_params,
        span,
    }))
}

fn validate_xyce_model_params(raw: &str) -> Result<(), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return Err(
            "Xyce .model parameters must be enclosed in a single pair of parentheses".into(),
        );
    }

    let inner = &trimmed[1..trimmed.len() - 1];

    if inner.contains(',') {
        return Err(
            "Xyce .model rejects comma-separated parameter lists (PSpice-incompatible)".into(),
        );
    }

    let open_count = trimmed.matches('(').count();
    let close_count = trimmed.matches(')').count();
    if open_count != 1 || close_count != 1 {
        return Err("Xyce .model rejects PSpice-style partial/nested parenthesization".into());
    }

    Ok(())
}

fn parse_param(line: &str, span: LineSpan) -> ParseResult {
    let rest = line[".param".len()..].trim();
    let assignments = parse_param_assignments(rest);
    Ok(Statement::Param(Param { assignments, span }))
}

fn parse_global_param(line: &str, span: LineSpan, subckt_depth: u32) -> ParseResult {
    if subckt_depth > 0 {
        return Err(ParseError {
            message:
                ".global_param is only allowed at top-level circuit scope, not inside a .subckt"
                    .into(),
            span,
        });
    }

    let rest = line[".global_param".len()..].trim();
    let assignments = parse_param_assignments(rest);
    Ok(Statement::GlobalParam(Param { assignments, span }))
}

fn parse_param_assignments(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
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
    let upper = line.to_uppercase();
    let keyword_len = if upper.starts_with(".INCLUDE") {
        ".include".len()
    } else if upper.starts_with(".INCL") {
        ".incl".len()
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
        let lines = preprocess(src, Dialect::Xyce);
        parse(&lines)
    }

    fn ok_statements(results: Vec<ParseResult>) -> Vec<Statement> {
        results.into_iter().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn test_xyce_p_device_is_port_not_ngspice_cpl() {
        let s = ok_statements(parsed("P1 1 0 port=1 Z0=50\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'P');
        assert_eq!(ei.nodes, vec!["1", "0"]);
        assert_eq!(ei.raw_params, vec!["port=1", "Z0=50"]);
    }

    #[test]
    fn test_xyce_bjt_bracketed_substrate_node() {
        let s = ok_statements(parsed("Q6 VC 4 11 [SUB] LAXPNP\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Q');
        assert_eq!(ei.nodes, vec!["VC", "4", "11", "SUB"]);
        assert_eq!(ei.raw_params, vec!["LAXPNP"]);
    }

    #[test]
    fn test_xyce_bjt_no_substrate_node_still_parses() {
        let s = ok_statements(parsed("Q1 c b e 2N3904\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.nodes, vec!["c", "b", "e"]);
        assert_eq!(ei.raw_params, vec!["2N3904"]);
    }

    #[test]
    fn test_xyce_bjt_with_numeric_substrate() {
        let s = ok_statements(parsed("Q2 1 2 3 4 NPN_MOD\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.nodes, vec!["1", "2", "3", "4"]);
        assert_eq!(ei.raw_params, vec!["NPN_MOD"]);
    }

    #[test]
    fn test_yacc_device() {
        let s = ok_statements(parsed("YACC1 n1 n2 n3 m=1\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
        assert_eq!(ei.name, "YACC1");
    }

    #[test]
    fn test_ydelay_device() {
        let s = ok_statements(parsed("YDELAY1 1 2 3 4 td=1n\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
        assert_eq!(ei.name, "YDELAY1");
    }

    #[test]
    fn test_ylin_device() {
        let s = ok_statements(parsed("YLIN1 1 0 2 0\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
        assert_eq!(ei.name, "YLIN1");
    }

    #[test]
    fn test_ymemristor_device() {
        let s = ok_statements(parsed("YMEMRISTOR1 1 2 model=m\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
        assert_eq!(ei.name, "YMEMRISTOR1");
    }

    #[test]
    fn test_ytransline_device() {
        let s = ok_statements(parsed("ytransline1 1 2 3 4\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
    }

    #[test]
    fn test_ypde_device() {
        let s = ok_statements(parsed("YPDE1 1 2\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'Y');
        assert_eq!(ei.name, "YPDE1");
    }

    #[test]
    fn test_xyce_r_device() {
        let s = ok_statements(parsed("R1 1 2 100\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.nodes, vec!["1", "2"]);
        assert_eq!(ei.raw_params, vec!["100"]);
    }

    #[test]
    fn test_subckt_accepts_optional_params_keyword() {
        let s = ok_statements(parsed(
            ".subckt opamp in+ in- out PARAMS: gain=100 bw=10k\n",
        ));
        assert_eq!(
            s[0],
            Statement::Subckt(Subckt {
                name: "opamp".into(),
                nodes: vec!["in+".into(), "in-".into(), "out".into()],
                params: vec![
                    ("gain".into(), Some("100".into())),
                    ("bw".into(), Some("10k".into()))
                ],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_subckt_without_params_keyword() {
        let s = ok_statements(parsed(".subckt buffer in out\n"));
        assert_eq!(
            s[0],
            Statement::Subckt(Subckt {
                name: "buffer".into(),
                nodes: vec!["in".into(), "out".into()],
                params: vec![],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_subckt_bare_param_defaults() {
        let s = ok_statements(parsed(".subckt inv in out vdd=5\n"));
        assert_eq!(
            s[0],
            Statement::Subckt(Subckt {
                name: "inv".into(),
                nodes: vec!["in".into(), "out".into()],
                params: vec![("vdd".into(), Some("5".into()))],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_model_rejects_partial_parens() {
        let results = parsed(".model NPN NPN (BF=100) (IS=1e-16)\n");
        assert!(results[0].is_err());
        let err = results[0].as_ref().unwrap_err();
        assert!(err.message.contains("partial") || err.message.contains("parenthesization"));
    }

    #[test]
    fn test_model_rejects_comma_separated_params() {
        let results = parsed(".model NPN NPN (BF=100, IS=1e-16)\n");
        assert!(results[0].is_err());
        let err = results[0].as_ref().unwrap_err();
        assert!(err.message.contains("comma"));
    }

    #[test]
    fn test_model_accepts_valid_xyce_params() {
        let s = ok_statements(parsed(".model NPN_MOD NPN (BF=200 IS=1e-14)\n"));
        assert_eq!(
            s[0],
            Statement::Model(Model {
                name: "NPN_MOD".into(),
                model_type: "NPN".into(),
                raw_params: "(BF=200 IS=1e-14)".into(),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_global_param_inside_subckt_is_diagnostic() {
        let src = ".subckt block a b\n.global_param G=10\n.ends\n";
        let results = parsed(src);
        let stmts: Vec<_> = results.into_iter().collect();
        assert!(stmts[0].is_ok());
        assert!(stmts[1].is_err());
        let err = stmts[1].as_ref().unwrap_err();
        assert!(err.message.contains("top-level") || err.message.contains("global_param"));
        assert!(stmts[2].is_ok());
    }

    #[test]
    fn test_global_param_at_top_level_is_ok() {
        let s = ok_statements(parsed(".global_param G=10\n"));
        assert_eq!(
            s[0],
            Statement::GlobalParam(Param {
                assignments: vec![("G".into(), "10".into())],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_global_param_distinct_from_param() {
        let src = ".param a=5\n.global_param b=10\n";
        let s = ok_statements(parsed(src));
        match &s[0] {
            Statement::Param(_) => {}
            other => panic!("expected Param, got {other:?}"),
        }
        match &s[1] {
            Statement::GlobalParam(_) => {}
            other => panic!("expected GlobalParam, got {other:?}"),
        }
    }

    #[test]
    fn test_x_device() {
        let s = ok_statements(parsed("X1 1 2 3 4 opamp gain=20\n"));
        let ei = match &s[0] {
            Statement::ElementInstance(ei) => ei,
            _ => panic!("expected element instance"),
        };
        assert_eq!(ei.device_letter, 'X');
        assert_eq!(ei.nodes, vec!["1", "2", "3", "4"]);
        assert_eq!(ei.raw_params, vec!["gain=20"]);
    }

    #[test]
    fn test_func_with_equals() {
        let s = ok_statements(parsed(".func add(a,b) = {a+b}\n"));
        assert_eq!(
            s[0],
            Statement::Func(Func {
                name: "add".into(),
                args: vec!["a".into(), "b".into()],
                body: "a+b".into(),
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_param_multiple_assignments() {
        let s = ok_statements(parsed(".param x=1 y=2 z=3\n"));
        assert_eq!(
            s[0],
            Statement::Param(Param {
                assignments: vec![
                    ("x".into(), "1".into()),
                    ("y".into(), "2".into()),
                    ("z".into(), "3".into())
                ],
                span: 1..2,
            })
        );
    }

    #[test]
    fn test_ends_with_name() {
        let s = ok_statements(parsed(".ends opamp\n"));
        assert_eq!(s[0], Statement::Ends(Some("opamp".into()), 1..2));
    }

    #[test]
    fn test_include() {
        let s = ok_statements(parsed(".incl ./models.inc\n"));
        assert_eq!(s[0], Statement::Include("./models.inc".into(), 1..2));
    }

    #[test]
    fn test_lib() {
        let s = ok_statements(parsed(".lib ./libs.lib typical\n"));
        assert_eq!(
            s[0],
            Statement::Lib("./libs.lib".into(), Some("typical".into()), 1..2)
        );
    }
}
