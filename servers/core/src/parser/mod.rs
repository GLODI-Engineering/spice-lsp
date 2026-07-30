use crate::ast::{LineSpan, Statement};
use crate::dialect::Dialect;
use crate::lexer::ProcessedLine;

pub mod ngspice;
pub mod xyce;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: LineSpan,
}

pub type ParseResult = Result<Statement, ParseError>;

pub type NodeParamsResult =
    std::result::Result<(Vec<String>, Vec<String>, Option<String>), ParseError>;

pub fn parse(lines: &[ProcessedLine], dialect: Dialect) -> Vec<ParseResult> {
    match dialect {
        Dialect::Ngspice => ngspice::parse(lines),
        Dialect::Xyce => xyce::parse(lines),
    }
}
