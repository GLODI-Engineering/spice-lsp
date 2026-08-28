//! `Statement::BlockInstance` — the first-class block/signal-domain statement grammar (`NAME
//! kind=... field=value ...`), which replaces the old convention of disguising these lines as
//! SPICE comments (`* NAME kind=...`) so this crate's own parser would skip them. Checks the
//! dispatch rule (a `kind=` token distinguishes a block line from a real element line, by shape,
//! not by any reserved letter), that real element lines are unaffected, and that block
//! statements nest correctly inside `.subckt` scopes and multi-file `.include` merges the same
//! way any other statement does.

use general_spice_core::ast::Statement;
use general_spice_core::dialect::Dialect;
use general_spice_core::lexer::preprocess;
use general_spice_core::parser;

fn parse_one(source: &str, dialect: Dialect) -> Statement {
    let processed = preprocess(source, dialect);
    let results = parser::parse(&processed, dialect);
    assert_eq!(results.len(), 1, "expected exactly one statement");
    results[0].clone().expect("statement should parse cleanly")
}

#[test]
fn a_block_line_parses_as_block_instance_not_element_instance() {
    let stmt = parse_one("MOD1 kind=pwm freq=100000 in=PIDTF\n", Dialect::Ngspice);
    match stmt {
        Statement::BlockInstance(b) => {
            assert_eq!(b.name, "MOD1");
            assert_eq!(
                b.fields,
                vec![
                    ("kind".to_string(), "pwm".to_string()),
                    ("freq".to_string(), "100000".to_string()),
                    ("in".to_string(), "PIDTF".to_string()),
                ]
            );
        }
        other => panic!("expected BlockInstance, got {other:?}"),
    }
}

#[test]
fn kind_field_can_appear_anywhere_after_the_name_not_just_second() {
    // The dispatch rule scans every field, not just the immediate second token -- field order
    // shouldn't matter (matches the existing kind=/field= grammar's own order-independence).
    let stmt = parse_one("MOD1 freq=100000 kind=pwm in=PIDTF\n", Dialect::Ngspice);
    assert!(matches!(stmt, Statement::BlockInstance(_)));
}

#[test]
fn a_python_list_field_value_is_captured_as_opaque_raw_text() {
    // This crate doesn't parse the [[1,2],[3,4]] shape itself -- it's not a lexer/token concept
    // here, just an ordinary value string with no internal whitespace, same treatment
    // Model::raw_params gets. Downstream (general-mna) interprets it.
    let stmt = parse_one(
        "FILTER kind=statespace a=[[0,1],[-1e6,-1400]] b=[0,1] c=[1e6,0] in=ERR\n",
        Dialect::Ngspice,
    );
    match stmt {
        Statement::BlockInstance(b) => {
            let a = b.fields.iter().find(|(k, _)| k == "a").unwrap();
            assert_eq!(a.1, "[[0,1],[-1e6,-1400]]");
        }
        other => panic!("expected BlockInstance, got {other:?}"),
    }
}

#[test]
fn real_element_lines_are_never_misdetected_as_block_instances() {
    // No real SPICE parameter is ever literally named "kind" -- confirm ordinary R/C/M-prefixed
    // element lines, including ones with plenty of key=value trailing params, still parse as
    // ElementInstance under both dialects.
    for (source, dialect) in [
        ("R1 1 2 100 tc1=0.001\n", Dialect::Ngspice),
        ("M1 d g s b nmos_model l=1u w=10u\n", Dialect::Ngspice),
        ("C1 3 0 1u\n", Dialect::Xyce),
    ] {
        let stmt = parse_one(source, dialect);
        assert!(
            matches!(stmt, Statement::ElementInstance(_)),
            "expected ElementInstance for {source:?}, got {stmt:?}"
        );
    }
}

#[test]
fn block_instance_line_parses_the_same_under_both_dialects() {
    for dialect in [Dialect::Ngspice, Dialect::Xyce] {
        let stmt = parse_one("HYST kind=hysteresis high=1.0 low=-1.0\n", dialect);
        assert!(matches!(stmt, Statement::BlockInstance(_)));
    }
}

#[test]
fn a_real_spice_comment_is_still_a_comment_not_a_block_instance() {
    // The old disguise convention (prefixing a block line with `*`) must keep working as an
    // ordinary comment for existing decks that haven't migrated yet -- this crate's lexer still
    // strips `*`-prefixed lines to nothing, same as always.
    let stmt = parse_one("* MOD1 kind=pwm freq=100000 in=PIDTF\n", Dialect::Ngspice);
    assert!(matches!(stmt, Statement::Comment(_)));
}

#[test]
fn block_instance_nests_inside_a_subckt_scope() {
    use general_spice_core::symbols::scope::build_scope_tree;

    let source = ".subckt buck vin vout\nMOD1 kind=pwm freq=100000 in=PIDTF\n.ends buck\n";
    let processed = preprocess(source, Dialect::Ngspice);
    let statements: Vec<Statement> = parser::parse(&processed, Dialect::Ngspice)
        .into_iter()
        .map(|r| r.unwrap())
        .collect();
    let scope = build_scope_tree(&statements).expect("scope tree should build cleanly");
    assert_eq!(scope.children.len(), 1);
    let buck = &scope.children[0];
    assert!(
        buck.statements
            .iter()
            .any(|s| matches!(s, Statement::BlockInstance(b) if b.name == "MOD1")),
        "MOD1 block instance should be nested inside the buck subckt scope"
    );
}

#[test]
fn a_double_quoted_field_value_may_contain_whitespace() {
    // A path through a directory with a space in its name (e.g. `kind=cscript lib=...`) has no
    // other way to survive whitespace tokenization -- found and fixed after a real doc-verify
    // run against a `lib=` path through such a directory produced a garbled parse error instead
    // of a clean one.
    let stmt = parse_one(
        r#"G1 kind=cscript lib="my libs/gain.so" in=SRC"#,
        Dialect::Ngspice,
    );
    match stmt {
        Statement::BlockInstance(b) => {
            assert_eq!(b.name, "G1");
            assert_eq!(
                b.fields,
                vec![
                    ("kind".to_string(), "cscript".to_string()),
                    ("lib".to_string(), "my libs/gain.so".to_string()),
                    ("in".to_string(), "SRC".to_string()),
                ]
            );
        }
        other => panic!("expected BlockInstance, got {other:?}"),
    }
}

#[test]
fn an_unquoted_field_value_is_unaffected_by_quote_support() {
    // Quote support must not change behavior for the common, no-whitespace-path case.
    let stmt = parse_one("G1 kind=cscript lib=gain.so in=SRC\n", Dialect::Ngspice);
    match stmt {
        Statement::BlockInstance(b) => {
            assert_eq!(
                b.fields,
                vec![
                    ("kind".to_string(), "cscript".to_string()),
                    ("lib".to_string(), "gain.so".to_string()),
                    ("in".to_string(), "SRC".to_string()),
                ]
            );
        }
        other => panic!("expected BlockInstance, got {other:?}"),
    }
}

#[test]
fn an_unterminated_quote_in_a_block_statement_is_a_clear_error() {
    let processed = preprocess(
        "G1 kind=cscript lib=\"unterminated in=SRC\n",
        Dialect::Ngspice,
    );
    let results = parser::parse(&processed, Dialect::Ngspice);
    assert_eq!(results.len(), 1);
    let err = results[0].clone().expect_err("expected a parse error");
    assert!(
        err.message.contains("unterminated"),
        "expected an 'unterminated' error, got: {}",
        err.message
    );
}
