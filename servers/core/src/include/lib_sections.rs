use crate::lexer::ProcessedLine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDiagnostic {
    pub message: String,
    pub section: String,
}

pub fn extract_lib_section(
    lines: &[ProcessedLine],
    section_name: &str,
) -> Result<Vec<ProcessedLine>, SectionDiagnostic> {
    let target = section_name.to_uppercase();
    let mut in_section = false;
    let mut found = false;
    let mut result = Vec::new();

    for line in lines {
        let trimmed = line.text.trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with(".LIB ") {
            let rest = upper[4..].trim().to_string();
            if rest == target {
                in_section = true;
                found = true;
                continue;
            }
        } else if upper.starts_with(".ENDL ") {
            let rest = upper[5..].trim().to_string();
            if rest == target {
                in_section = false;
                continue;
            }
        }

        if in_section {
            result.push(line.clone());
        }
    }

    if !found {
        return Err(SectionDiagnostic {
            message: format!("section '{section_name}' not found in library file"),
            section: section_name.to_string(),
        });
    }

    if in_section {
        return Err(SectionDiagnostic {
            message: format!("unterminated section '{section_name}' (no matching .ENDL)"),
            section: section_name.to_string(),
        });
    }

    Ok(result)
}

pub fn extract_whole_file(lines: &[ProcessedLine]) -> Vec<ProcessedLine> {
    lines.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::Dialect;
    use crate::lexer::preprocess;

    fn preproc(src: &str) -> Vec<ProcessedLine> {
        preprocess(src, Dialect::Ngspice)
            .into_iter()
            .filter(|l| !l.text.trim().is_empty())
            .collect()
    }

    #[test]
    fn test_extracts_matching_section_only() {
        let src =
            ".lib cmos\nM1 d g s b NMOS\n.endl cmos\n.lib bipolar\nQ1 c b e NPN\n.endl bipolar\n";
        let lines = preproc(src);
        let section = extract_lib_section(&lines, "cmos").unwrap();
        assert_eq!(section.len(), 1);
        assert!(section[0].text.contains("M1"));
    }

    #[test]
    fn test_multiple_sections_do_not_leak_into_each_other() {
        let src = ".lib a\nR1 1 2 100\n.endl a\n.lib b\nR2 3 4 200\n.endl b\n";
        let lines = preproc(src);
        let a = extract_lib_section(&lines, "a").unwrap();
        let b = extract_lib_section(&lines, "b").unwrap();
        assert!(a[0].text.contains("R1"));
        assert!(b[0].text.contains("R2"));
    }

    #[test]
    fn test_unknown_section_name_flagged() {
        let src = ".lib a\nR1 1 2 100\n.endl a\n";
        let lines = preproc(src);
        let result = extract_lib_section(&lines, "missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_no_section_argument_returns_whole_file() {
        let src = "R1 1 2 100\nC1 3 0 1u\n";
        let lines = preproc(src);
        let whole = extract_whole_file(&lines);
        assert_eq!(whole.len(), 2);
    }

    #[test]
    fn test_unterminated_section_returns_diagnostic_not_panic() {
        let src = ".lib bad\nR1 1 2 100\n";
        let lines = preproc(src);
        let result = extract_lib_section(&lines, "bad");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn test_case_insensitive_section_name_matching() {
        let src = ".lib CMOS\nM1 d g s b NMOS\n.endl cmos\n";
        let lines = preproc(src);
        let section = extract_lib_section(&lines, "CMOS").unwrap();
        assert_eq!(section.len(), 1);
    }
}
