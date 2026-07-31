//! Line-level preprocessing: strips comments, joins continuation lines
//! (`+`-prefixed, and ngspice's `\`-suffixed physical-line continuation),
//! and tracks which real source lines each resulting logical line came
//! from. This always runs before [`crate::parser::parse`]/
//! [`crate::parser::parse_document`] — the parser never sees raw,
//! un-preprocessed source. Entry point: [`preprocess`].

use crate::dialect::Dialect;

/// One logical (post-continuation-joining) line of source, plus the real
/// 1-based line number(s) it was assembled from — used to attribute
/// diagnostics back to the actual source line(s) a joined statement came
/// from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedLine {
    /// The joined, comment-stripped text of this logical line.
    pub text: String,
    /// The real 1-based source line number(s) this logical line was
    /// assembled from, in order. More than one entry when continuation
    /// lines (`+` or `\`) were joined into it.
    pub source_lines: Vec<usize>,
}

/// Split `source` into logical lines: full-line comments (`*`) become
/// empty [`ProcessedLine`]s (position preserved, not dropped), end-of-line
/// comments are stripped per-dialect (`$`/`//` for ngspice, `;` for Xyce),
/// and continuation lines are joined onto the previous logical line —
/// `+`-prefixed continuations in both dialects, plus ngspice's trailing-`\`
/// physical-line continuation (not honored on `.title`/`.lib`/`.include`
/// lines, since those may legitimately contain a literal trailing
/// backslash in a path or title).
pub fn preprocess(source: &str, dialect: Dialect) -> Vec<ProcessedLine> {
    let physical_lines: Vec<&str> = source.lines().collect();
    let mut result: Vec<ProcessedLine> = Vec::new();
    let mut i = 0;

    while i < physical_lines.len() {
        let line_num = i + 1;
        let raw = physical_lines[i];

        if raw.starts_with('*') {
            result.push(ProcessedLine {
                text: String::new(),
                source_lines: vec![line_num],
            });
            i += 1;
            continue;
        }

        let mut stripped = strip_eol_comment(raw, dialect);
        let mut source_lines = vec![line_num];
        if dialect == Dialect::Ngspice {
            loop {
                let trimmed = stripped.trim_end();
                if !trimmed.ends_with('\\') {
                    break;
                }
                if i + 1 >= physical_lines.len() {
                    stripped = trimmed[..trimmed.len() - 1].trim_end().to_string();
                    break;
                }
                if is_ngspice_continuation_exception(&stripped) {
                    break;
                }
                let without_backslash = trimmed[..trimmed.len() - 1].trim_end();
                i += 1;
                let next_line_num = i + 1;
                source_lines.push(next_line_num);
                let next_stripped = strip_eol_comment(physical_lines[i].trim(), dialect);
                stripped = format!("{without_backslash} {next_stripped}");
            }
        }

        let trimmed = stripped.trim().to_string();

        if let Some(rest) = trimmed.strip_prefix('+') {
            let continuation_text = rest.trim().to_string();
            if let Some(last) = result.last_mut() {
                if dialect == Dialect::Ngspice && is_ngspice_continuation_exception(&last.text) {
                    result.push(ProcessedLine {
                        text: continuation_text,
                        source_lines,
                    });
                } else {
                    if !last.text.is_empty() && !continuation_text.is_empty() {
                        last.text.push(' ');
                    }
                    last.text.push_str(&continuation_text);
                    last.source_lines.extend(source_lines);
                }
            } else {
                result.push(ProcessedLine {
                    text: continuation_text,
                    source_lines,
                });
            }
        } else {
            result.push(ProcessedLine {
                text: trimmed,
                source_lines,
            });
        }

        i += 1;
    }

    result
}

fn is_ngspice_continuation_exception(text: &str) -> bool {
    let upper = text.trim_start().to_uppercase();
    upper.starts_with(".TITLE") || upper.starts_with(".LIB") || upper.starts_with(".INCLUDE")
}

fn strip_eol_comment(line: &str, dialect: Dialect) -> String {
    match dialect {
        Dialect::Ngspice => strip_eol_comment_ngspice(line),
        Dialect::Xyce => strip_eol_comment_xyce(line),
    }
}

fn strip_eol_comment_ngspice(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut earliest: Option<usize> = None;

    for (i, &c) in chars.iter().enumerate() {
        if c == '$' && preceded_by_whitespace_or_start(&chars, i) {
            earliest = Some(earliest.map_or(i, |e| e.min(i)));
            break;
        }
    }

    let len = chars.len();
    for i in 0..len.saturating_sub(1) {
        if chars[i] == '/' && chars[i + 1] == '/' && preceded_by_whitespace_or_start(&chars, i) {
            earliest = Some(earliest.map_or(i, |e| e.min(i)));
            break;
        }
    }

    match earliest {
        Some(pos) => line[..pos].to_string(),
        None => line.to_string(),
    }
}

fn strip_eol_comment_xyce(line: &str) -> String {
    if let Some(pos) = line.find(';') {
        line[..pos].to_string()
    } else {
        line.to_string()
    }
}

fn preceded_by_whitespace_or_start(chars: &[char], idx: usize) -> bool {
    idx == 0 || chars[idx - 1].is_whitespace()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_text(lines: &[ProcessedLine]) -> Vec<&str> {
        lines.iter().map(|l| l.text.as_str()).collect()
    }

    fn lines_sources(lines: &[ProcessedLine]) -> Vec<Vec<usize>> {
        lines.iter().map(|l| l.source_lines.clone()).collect()
    }

    #[test]
    fn test_ngspice_full_line_comment_preserves_line_numbers() {
        let src = "* comment 1\nR1 1 2 100\n* comment 2\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["", "R1 1 2 100", ""]);
        assert_eq!(lines_sources(&r), vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn test_ngspice_eol_comment_dollar_and_slashslash() {
        let src = "R1 1 2 100 $ resistor\nC1 3 0 1u // cap\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["R1 1 2 100", "C1 3 0 1u"]);
    }

    #[test]
    fn test_xyce_eol_comment_semicolon_only() {
        let src = "R1 1 2 100 ; comment\nC1 3 0 1u $ not a comment in xyce\n";
        let r = preprocess(src, Dialect::Xyce);
        assert_eq!(
            lines_text(&r),
            vec!["R1 1 2 100", "C1 3 0 1u $ not a comment in xyce"]
        );
    }

    #[test]
    fn test_plus_continuation_both_dialects() {
        for dialect in &[Dialect::Ngspice, Dialect::Xyce] {
            let src = "R1 1 2\n+ 100\nC1 3 0 1u\n";
            let r = preprocess(src, *dialect);
            assert_eq!(
                lines_text(&r),
                vec!["R1 1 2 100", "C1 3 0 1u"],
                "failed for {dialect:?}"
            );
            assert_eq!(
                lines_sources(&r),
                vec![vec![1, 2], vec![3]],
                "failed for {dialect:?}"
            );
        }
    }

    #[test]
    fn test_ngspice_backslash_continuation() {
        let src = "R1 1 2 \\\n 100\nC1 3 0 1u\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["R1 1 2 100", "C1 3 0 1u"]);
        assert_eq!(lines_sources(&r), vec![vec![1, 2], vec![3]]);
    }

    #[test]
    fn test_xyce_no_backslash_continuation() {
        let src = "R1 1 2 \\\n 100\n";
        let r = preprocess(src, Dialect::Xyce);
        assert_eq!(lines_text(&r), vec!["R1 1 2 \\", "100"]);
        assert_eq!(lines_sources(&r), vec![vec![1], vec![2]]);
    }

    #[test]
    fn test_ngspice_title_line_not_continued() {
        let src = ".title My Circuit \\\n extra\nR1 1 2 100\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(
            lines_text(&r),
            vec![".title My Circuit \\", "extra", "R1 1 2 100"]
        );

        let src2 = ".title My Circuit\n+ extra\n";
        let r2 = preprocess(src2, Dialect::Ngspice);
        assert_eq!(lines_text(&r2), vec![".title My Circuit", "extra"]);
    }

    #[test]
    fn test_empty_file() {
        let r = preprocess("", Dialect::Ngspice);
        assert!(r.is_empty());
        let r = preprocess("", Dialect::Xyce);
        assert!(r.is_empty());
    }

    #[test]
    fn test_file_with_only_comments() {
        let src = "* comment 1\n* comment 2\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["", ""]);
        assert_eq!(lines_sources(&r), vec![vec![1], vec![2]]);
    }

    #[test]
    fn test_continuation_on_very_first_line_no_panic() {
        let src = "+ 100\nR1 1 2\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["100", "R1 1 2"]);
    }

    #[test]
    fn test_multiple_backslash_continuations() {
        let src = "V1 \\\n 1 \\\n 0 \\\n DC 5\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["V1 1 0 DC 5"]);
        assert_eq!(lines_sources(&r), vec![vec![1, 2, 3, 4]]);
    }

    #[test]
    fn test_backslash_at_end_of_file_no_panic() {
        let src = "R1 1 2 \\\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["R1 1 2"]);
    }

    #[test]
    fn test_lib_line_not_continued_ngspice() {
        let src = ".lib ./models.lib cmos \\\n extra\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec![".lib ./models.lib cmos \\", "extra"]);
    }

    #[test]
    fn test_include_line_not_continued_ngspice() {
        let src = ".include ./parts.inc \\\n extra\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec![".include ./parts.inc \\", "extra"]);
    }

    #[test]
    fn test_plus_continuation_not_applied_to_title_ngspice() {
        let src = ".title My Circuit\n+ extra\nR1 1 2\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec![".title My Circuit", "extra", "R1 1 2"]);
    }

    #[test]
    fn test_x_spice_device_p() {
        let src = "R1 1 2\n+ 100k\nC1 3 0\n+ 1u\n";
        let r = preprocess(src, Dialect::Ngspice);
        assert_eq!(lines_text(&r), vec!["R1 1 2 100k", "C1 3 0 1u"]);
        assert_eq!(lines_sources(&r), vec![vec![1, 2], vec![3, 4]]);
    }
}
