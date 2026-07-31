//! Turns preprocessed lines ([`crate::lexer::preprocess`]'s output) into
//! [`Statement`]s. Two entry points: [`parse`] for an arbitrary statement
//! fragment, [`parse_document`] for a complete real netlist file (handles
//! the mandatory title line). Dialect-specific parsing logic lives in
//! [`ngspice`] and [`xyce`]; this module dispatches to whichever one
//! matches the requested [`Dialect`] plus holds the shared [`ParseError`]/
//! [`ParseResult`] types.

use crate::ast::{LineSpan, Statement};
use crate::dialect::Dialect;
use crate::lexer::ProcessedLine;

/// ngspice-dialect statement parsing.
pub mod ngspice;
/// Xyce-dialect statement parsing.
pub mod xyce;

/// A statement that failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// A human-readable description of the problem.
    pub message: String,
    /// Where in the source the offending statement is.
    pub span: LineSpan,
}

/// The result of parsing one logical line: the recognized [`Statement`],
/// or a [`ParseError`] if it couldn't be parsed.
pub type ParseResult = Result<Statement, ParseError>;

/// The result of splitting a device/subcircuit instance's trailing text
/// into (nodes, raw parameters, optional subcircuit name), or a
/// [`ParseError`] if the split failed.
pub type NodeParamsResult =
    std::result::Result<(Vec<String>, Vec<String>, Option<String>), ParseError>;

/// Parse `lines` as a sequence of ordinary statements — no title-line
/// handling, every line is parsed as statement content. Use this for a
/// fragment of a netlist (a single device line, a `.subckt` body, an
/// included file's contents). For a whole real netlist file starting from
/// its true first physical line, use [`parse_document`] instead.
pub fn parse(lines: &[ProcessedLine], dialect: Dialect) -> Vec<ParseResult> {
    match dialect {
        Dialect::Ngspice => ngspice::parse(lines),
        Dialect::Xyce => xyce::parse(lines),
    }
}

/// Parse a complete netlist FILE, as opposed to an arbitrary statement
/// fragment. The first physical line of a real netlist is always the
/// title, treated as a comment regardless of its content, even if it
/// doesn't start with `*` and even if it looks like a device instance
/// (docs/GRAMMAR.md §1, confirmed for both dialects) — real-world impact
/// confirmed against ngspice's own test suite: a title line like "ex1a,
/// check lib processing" was silently misparsed as an E-device (Vcvs)
/// instance, since 'e' is a device-letter prefix.
///
/// [`parse`] itself intentionally does NOT apply this rule — it treats
/// every line as ordinary statement content, which is what every
/// statement-level unit test in `ngspice.rs`/`xyce.rs` (and any caller
/// parsing a fragment rather than a whole file) relies on. Use
/// `parse_document` specifically when `lines` represents an entire real
/// `.cir`/`.sp` file starting from its true first physical line — e.g. the
/// top-level entry point for a file a user opened in an editor, or
/// `include::graph`'s per-file resolution loop.
pub fn parse_document(lines: &[ProcessedLine], dialect: Dialect) -> Vec<ParseResult> {
    if lines.is_empty() {
        return Vec::new();
    }
    let title = &lines[0];
    let start = title.source_lines.first().copied().unwrap_or(1);
    let end = title.source_lines.last().copied().unwrap_or(1);
    let mut results = vec![Ok(Statement::Comment(start..end + 1))];
    results.extend(parse(&lines[1..], dialect));
    results
}

/// Split the text after `.model` into (name, type, raw_params). Handles
/// both `.model NAME TYPE (params)` (whitespace-separated) and
/// `.model NAME TYPE(params)` (type and the opening paren glued together
/// with no space) — both are legal SPICE syntax and real-world netlists use
/// the glued form constantly (confirmed against real ngspice/Xyce
/// test-suite netlists: `.model D_IDEAL D(IS=1e-14 N=1 RS=10m)`). A naive
/// whitespace split on the first two tokens breaks the glued form, folding
/// the `(` into the type and leaving `raw_params` without its opening
/// paren.
pub(super) fn split_model_name_type_params(rest: &str) -> Option<(String, String, String)> {
    let rest = rest.trim();
    let mut it = rest.splitn(2, char::is_whitespace);
    let name = it.next()?.to_string();
    if name.is_empty() {
        return None;
    }
    let after_name = it.next().unwrap_or("").trim_start();
    if after_name.is_empty() {
        return None;
    }
    let type_end = after_name
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(after_name.len());
    if type_end == 0 {
        return None;
    }
    let model_type = after_name[..type_end].to_string();
    let raw_params = after_name[type_end..].trim().to_string();
    Some((name, model_type, raw_params))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::preprocess;

    // Regression tests for bugs found testing against real ngspice/Xyce
    // test-suite netlists (servers/core/tests/real_netlists.rs).

    #[test]
    fn test_split_model_name_type_params_glued_paren() {
        // `.model D_IDEAL D(IS=1e-14 N=1 RS=10m)` — type and the opening
        // paren glued together with no space, common in real netlists.
        let (name, model_type, raw_params) =
            split_model_name_type_params("D_IDEAL D(IS=1e-14 N=1 RS=10m)").unwrap();
        assert_eq!(name, "D_IDEAL");
        assert_eq!(model_type, "D");
        assert_eq!(raw_params, "(IS=1e-14 N=1 RS=10m)");
    }

    #[test]
    fn test_split_model_name_type_params_spaced_paren() {
        let (name, model_type, raw_params) =
            split_model_name_type_params("NPN1 NPN (BF=100)").unwrap();
        assert_eq!(name, "NPN1");
        assert_eq!(model_type, "NPN");
        assert_eq!(raw_params, "(BF=100)");
    }

    #[test]
    fn test_split_model_name_type_params_no_params() {
        let (name, model_type, raw_params) = split_model_name_type_params("DMOD D").unwrap();
        assert_eq!(name, "DMOD");
        assert_eq!(model_type, "D");
        assert_eq!(raw_params, "");
    }

    #[test]
    fn test_split_model_name_type_params_missing_type_returns_none() {
        assert!(split_model_name_type_params("DMOD").is_none());
        assert!(split_model_name_type_params("").is_none());
    }

    #[test]
    fn test_parse_document_strips_first_line_as_title_ngspice() {
        // "ex1a, check lib processing" was silently misparsed as an
        // E-device (Vcvs) instance under plain parse() — confirmed against
        // ngspice's own test suite. parse_document must not do that.
        let processed = preprocess("ex1a, check lib processing\nR1 1 2 100\n", Dialect::Ngspice);
        let results = parse_document(&processed, Dialect::Ngspice);
        assert_eq!(results.len(), 2);
        assert!(matches!(results[0], Ok(Statement::Comment(_))));
        assert!(matches!(results[1], Ok(Statement::ElementInstance(_))));
    }

    #[test]
    fn test_parse_document_strips_first_line_as_title_xyce() {
        let processed = preprocess("Test circuit title\nR1 1 2 100\n", Dialect::Xyce);
        let results = parse_document(&processed, Dialect::Xyce);
        assert_eq!(results.len(), 2);
        assert!(matches!(results[0], Ok(Statement::Comment(_))));
        assert!(matches!(results[1], Ok(Statement::ElementInstance(_))));
    }

    #[test]
    fn test_parse_document_empty_input() {
        assert!(parse_document(&[], Dialect::Ngspice).is_empty());
    }

    #[test]
    fn test_parse_still_treats_first_line_as_ordinary_statement() {
        // The plain (non-document) parse() entry point must NOT apply
        // title-line stripping — every existing statement-level unit test
        // across this crate relies on that. This test guards against
        // accidentally re-merging the two entry points.
        let processed = preprocess("R1 1 2 100\n", Dialect::Ngspice);
        let results = parse(&processed, Dialect::Ngspice);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], Ok(Statement::ElementInstance(_))));
    }
}
