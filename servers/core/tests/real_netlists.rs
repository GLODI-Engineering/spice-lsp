//! Conformance/robustness testing against real, independently-authored
//! netlists (not hand-written unit-test fixtures) — see
//! `tests/fixtures/NOTICE.md` for where these files came from.
//!
//! This is deliberately NOT a "must fully parse" test: real production
//! netlists exercise statement types beyond general_spice_core's current coverage
//! (see docs/GRAMMAR.md's phase plan), and that's expected. What this test
//! actually guards:
//!
//! 1. The full pipeline (lex -> parse -> scope-build -> uniqueness ->
//!    subckt/model resolution -> expression wiring) never panics on any
//!    real file, for either dialect.
//! 2. Every fixture file produces at least one statement (the lexer/parser
//!    never silently produces nothing for non-empty input).
//! 3. Aggregate statement-recognition coverage doesn't regress below a
//!    floor — this is a regression guard, not a correctness guarantee: a
//!    future change that badly breaks parsing (e.g. a dispatch-order bug
//!    that routes half of all statements to Unrecognized) will fail this
//!    test even though no individual assertion above would have caught it.

use std::fs;
use std::path::Path;

use general_spice_core::ast::Statement;
use general_spice_core::dialect::Dialect;
use general_spice_core::expr::{wiring_compiletime, wiring_runtime};
use general_spice_core::lexer;
use general_spice_core::parser;
use general_spice_core::symbols::{model_resolution, scope, subckt_resolution, uniqueness};

struct FileStats {
    path: String,
    total_statements: usize,
    unrecognized: usize,
    parse_errors: usize,
}

/// Run every stage of the pipeline against one file's contents. Panicking
/// anywhere in here is the failure mode this test exists to catch — no
/// `catch_unwind`, so a panic in any stage fails the test with a normal
/// Rust backtrace pointing at the offending file/dialect.
fn run_pipeline(dialect: Dialect, source: &str) -> Vec<Statement> {
    // These fixtures are whole real files (each with its own mandatory
    // title line per docs/GRAMMAR.md §1), not statement fragments — use
    // parse_document, matching how a real top-level file is meant to be
    // parsed. See parser::parse_document's doc comment for why this
    // differs from plain parser::parse.
    let processed = lexer::preprocess(source, dialect);
    let parsed = parser::parse_document(&processed, dialect);

    let mut statements = Vec::new();
    for stmt in parsed.iter().flatten() {
        statements.push(stmt.clone());
    }

    // Only run the symbol/expression stages if the statement stream is at
    // least structurally sound (build_scope_tree can fail on legitimately
    // malformed .subckt/.ends nesting in a real file — that's a valid
    // diagnostic outcome, not a bug, so we don't require it to succeed).
    if let Ok(tree) = scope::build_scope_tree(&statements) {
        let _ = uniqueness::check_unique_names(&tree);
        let _ = subckt_resolution::resolve_subckt_calls(&tree, dialect);
        let _ = model_resolution::resolve_model_references(&tree, dialect);
        let _ = wiring_compiletime::wire_compiletime_expressions(&tree, dialect);
        let _ = wiring_runtime::wire_runtime_expressions(&tree, dialect);
    }

    statements
}

fn analyze_file(dialect: Dialect, path: &Path) -> FileStats {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));

    let processed = lexer::preprocess(&source, dialect);
    let parsed = parser::parse_document(&processed, dialect);
    let parse_errors = parsed.iter().filter(|r| r.is_err()).count();

    let statements = run_pipeline(dialect, &source);

    let unrecognized = statements
        .iter()
        .filter(|s| matches!(s, Statement::Unrecognized(_, _)))
        .count();

    FileStats {
        path: path.display().to_string(),
        total_statements: statements.len(),
        unrecognized,
        parse_errors,
    }
}

fn run_fixture_dir(dialect: Dialect, dir: &Path) -> Vec<FileStats> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read fixture dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("cir"))
        .collect();
    entries.sort();

    assert!(
        !entries.is_empty(),
        "no .cir fixtures found in {} — fixture set may have been accidentally removed",
        dir.display()
    );

    entries.iter().map(|p| analyze_file(dialect, p)).collect()
}

fn summarize(dialect_name: &str, stats: &[FileStats]) {
    let total: usize = stats.iter().map(|s| s.total_statements).sum();
    let unrecognized: usize = stats.iter().map(|s| s.unrecognized).sum();
    let parse_errors: usize = stats.iter().map(|s| s.parse_errors).sum();
    let pct = if total > 0 {
        100.0 * unrecognized as f64 / total as f64
    } else {
        0.0
    };
    let file_count = stats.len();
    println!(
        "[{dialect_name}] {file_count} files, {total} statements, {unrecognized} unrecognized ({pct:.1}%), {parse_errors} parse errors"
    );
    for s in stats {
        if s.total_statements == 0 {
            println!("  ! zero statements parsed: {}", s.path);
        }
    }
}

#[test]
fn ngspice_fixtures_never_panic_and_mostly_parse() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ngspice");
    let stats = run_fixture_dir(Dialect::Ngspice, &dir);

    assert!(
        stats.len() >= 25,
        "expected at least 25 ngspice fixture files, found {}",
        stats.len()
    );

    for s in &stats {
        assert!(
            s.total_statements > 0,
            "fixture produced zero statements (lexer/parser silently ate non-empty input): {}",
            s.path
        );
    }

    summarize("ngspice", &stats);

    let total: usize = stats.iter().map(|s| s.total_statements).sum();
    let unrecognized: usize = stats.iter().map(|s| s.unrecognized).sum();
    let pct = 100.0 * unrecognized as f64 / total as f64;
    assert!(
        pct < 45.0,
        "ngspice unrecognized-statement rate regressed to {pct:.1}% (floor: 45%) — likely a real parsing regression, not just fixture drift"
    );
}

#[test]
fn xyce_fixtures_never_panic_and_mostly_parse() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xyce");
    let stats = run_fixture_dir(Dialect::Xyce, &dir);

    assert!(
        stats.len() >= 20,
        "expected at least 20 Xyce fixture files, found {}",
        stats.len()
    );

    for s in &stats {
        assert!(
            s.total_statements > 0,
            "fixture produced zero statements (lexer/parser silently ate non-empty input): {}",
            s.path
        );
    }

    summarize("xyce", &stats);

    let total: usize = stats.iter().map(|s| s.total_statements).sum();
    let unrecognized: usize = stats.iter().map(|s| s.unrecognized).sum();
    let pct = 100.0 * unrecognized as f64 / total as f64;
    assert!(
        pct < 45.0,
        "Xyce unrecognized-statement rate regressed to {pct:.1}% (floor: 45%) — likely a real parsing regression, not just fixture drift"
    );
}

#[test]
fn combined_fixture_count_is_at_least_fifty() {
    let ng_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ngspice");
    let xy_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xyce");

    let count = |dir: &Path| {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|e| e.to_str()) == Some("cir"))
            .count()
    };

    let total = count(&ng_dir) + count(&xy_dir);
    assert!(
        total >= 50,
        "combined ngspice+xyce fixture count is {total}, expected at least 50"
    );
}
