use crate::ast::{LineSpan, Statement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    TopLevel,
    Subckt,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub kind: ScopeKind,
    pub name: Option<String>,
    pub depth: u32,
    pub statements: Vec<Statement>,
    pub children: Vec<Scope>,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeError {
    pub message: String,
    pub span: LineSpan,
}

pub fn build_scope_tree(statements: &[Statement]) -> Result<Scope, Vec<ScopeError>> {
    let mut errors = Vec::new();
    let mut root = new_top_level();
    let mut stack: Vec<Scope> = Vec::new();

    for stmt in statements {
        match stmt {
            Statement::Subckt(subckt) => {
                let child = new_subckt_scope(
                    subckt.name.clone(),
                    stack.len() as u32 + 1,
                    subckt.span.clone(),
                );
                stack.push(child);
            }
            Statement::Ends(name, span) => match stack.pop() {
                Some(mut child) => {
                    if let Some(ends_name) = name {
                        let child_name = child.name.clone().unwrap_or_default();
                        if ends_name.to_uppercase() != child_name.to_uppercase() {
                            errors.push(ScopeError {
                                    message: format!(
                                        ".ends name '{ends_name}' does not match .subckt name '{child_name}'"
                                    ),
                                    span: span.clone(),
                                });
                        }
                    }
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(child);
                    } else {
                        child.depth = 1;
                        root.children.push(child);
                    }
                }
                None => {
                    errors.push(ScopeError {
                        message: ".ends without matching .subckt".into(),
                        span: span.clone(),
                    });
                }
            },
            other => {
                if let Some(current) = stack.last_mut() {
                    current.statements.push(other.clone());
                } else {
                    root.statements.push(other.clone());
                }
            }
        }
    }

    for child in stack {
        let name = child.name.clone().unwrap_or_default();
        errors.push(ScopeError {
            message: format!(".subckt '{name}' has no matching .ends"),
            span: child.span,
        });
    }

    if errors.is_empty() {
        Ok(root)
    } else {
        Err(errors)
    }
}

pub fn build_scope_tree_from_tagged(
    tagged: &[(crate::include::source_map::FileId, Statement)],
) -> Result<Scope, Vec<ScopeError>> {
    let statements: Vec<Statement> = tagged.iter().map(|(_, s)| s.clone()).collect();
    build_scope_tree(&statements)
}

fn new_top_level() -> Scope {
    Scope {
        kind: ScopeKind::TopLevel,
        name: None,
        depth: 0,
        statements: Vec::new(),
        children: Vec::new(),
        span: 0..1,
    }
}

fn new_subckt_scope(name: String, depth: u32, span: LineSpan) -> Scope {
    Scope {
        kind: ScopeKind::Subckt,
        name: Some(name),
        depth,
        statements: Vec::new(),
        children: Vec::new(),
        span,
    }
}

pub fn flatten_scopes(scope: &Scope) -> Vec<&Scope> {
    let mut v = vec![scope];
    for child in &scope.children {
        v.extend(flatten_scopes(child));
    }
    v
}

pub fn collect_subckt_definitions(scope: &Scope) -> Vec<&Scope> {
    let mut v = Vec::new();
    if scope.kind == ScopeKind::Subckt {
        v.push(scope);
    }
    for child in &scope.children {
        v.extend(collect_subckt_definitions(child));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_subckt_stmt(name: &str, span: LineSpan) -> Statement {
        Statement::Subckt(Subckt {
            name: name.into(),
            nodes: vec![],
            params: vec![],
            span,
        })
    }

    fn make_ends(name: Option<&str>, span: LineSpan) -> Statement {
        Statement::Ends(name.map(String::from), span)
    }

    fn make_elem(name: &str) -> Statement {
        Statement::ElementInstance(ElementInstance {
            device_letter: 'R',
            name: name.into(),
            nodes: vec![],
            raw_params: vec![],
            subckt_name: None,
            span: 1..3,
        })
    }

    #[test]
    fn test_top_level_scope_with_no_subckts() {
        let stmts = vec![make_elem("R1")];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.kind, ScopeKind::TopLevel);
        assert!(tree.children.is_empty());
        assert_eq!(tree.statements.len(), 1);
    }

    #[test]
    fn test_single_subckt_produces_one_child_scope() {
        let stmts = vec![
            make_subckt_stmt("opamp", 1..2),
            make_elem("R1"),
            make_ends(Some("opamp"), 3..4),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].name.as_deref(), Some("opamp"));
        assert_eq!(tree.children[0].kind, ScopeKind::Subckt);
        assert_eq!(tree.children[0].statements.len(), 1);
    }

    #[test]
    fn test_nested_subckt_definitions_produce_nested_scopes() {
        let stmts = vec![
            make_subckt_stmt("outer", 1..2),
            make_elem("RTOP"),
            make_subckt_stmt("inner", 3..4),
            make_elem("RBOT"),
            make_ends(Some("inner"), 5..6),
            make_ends(Some("outer"), 7..8),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.children.len(), 1);
        let outer = &tree.children[0];
        assert_eq!(outer.name.as_deref(), Some("outer"));
        assert_eq!(outer.children.len(), 1);
        let inner = &outer.children[0];
        assert_eq!(inner.name.as_deref(), Some("inner"));
        assert_eq!(inner.statements.len(), 1);
        assert_eq!(outer.statements.len(), 1);
    }

    #[test]
    fn test_sibling_subckts_produce_sibling_scopes_not_nested() {
        let stmts = vec![
            make_subckt_stmt("a", 1..2),
            make_ends(Some("a"), 2..3),
            make_subckt_stmt("b", 3..4),
            make_ends(Some("b"), 4..5),
        ];
        let tree = build_scope_tree(&stmts).unwrap();
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].name.as_deref(), Some("a"));
        assert_eq!(tree.children[1].name.as_deref(), Some("b"));
    }

    #[test]
    fn test_unterminated_subckt_returns_diagnostic_not_panic() {
        let stmts = vec![make_subckt_stmt("bad", 1..2)];
        let result = build_scope_tree(&stmts);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("no matching .ends"));
    }

    #[test]
    fn test_ends_without_open_subckt_returns_diagnostic_not_panic() {
        let stmts = vec![make_ends(Some("orphan"), 1..2)];
        let result = build_scope_tree(&stmts);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs[0].message.contains("without matching .subckt"));
    }

    #[test]
    fn test_ends_name_mismatch_produces_warning() {
        let stmts = vec![
            make_subckt_stmt("opamp", 1..2),
            make_elem("R1"),
            make_ends(Some("wrong"), 3..4),
        ];
        let result = build_scope_tree(&stmts);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_document() {
        let result = build_scope_tree(&[]).unwrap();
        assert_eq!(result.kind, ScopeKind::TopLevel);
        assert!(result.statements.is_empty());
        assert!(result.children.is_empty());
    }

    // --- CORE-25: multi-file tagged input ---

    #[test]
    fn test_subckt_defined_in_included_file_resolves_from_includer() {
        use crate::include::source_map::FileId;

        // Simulates: file B (included) defines .subckt opamp; file A (the
        // includer) calls X1 ... opamp. build_scope_tree_from_tagged must
        // splice both into one tree where the X-call's enclosing scope can
        // still see the subckt definition, exactly as CORE-16 expects.
        let file_a = FileId::new_dummy();
        let file_b = FileId::new_dummy();
        let tagged = vec![
            (file_b, make_subckt_stmt("opamp", 1..2)),
            (file_b, make_ends(Some("opamp"), 2..3)),
            (
                file_a,
                Statement::ElementInstance(ElementInstance {
                    device_letter: 'X',
                    name: "X1".into(),
                    nodes: vec![],
                    raw_params: vec!["opamp".into()],
                    subckt_name: Some("opamp".into()),
                    span: 3..4,
                }),
            ),
        ];
        let tree = build_scope_tree_from_tagged(&tagged).unwrap();
        let subckt_names: Vec<String> = collect_subckt_definitions(&tree)
            .iter()
            .filter_map(|s| s.name.clone())
            .collect();
        assert!(
            subckt_names.contains(&"opamp".to_string()),
            "opamp subckt from the included file must be present in the merged tree"
        );
        assert!(
            tree.statements
                .iter()
                .any(|s| matches!(s, Statement::ElementInstance(ei) if ei.name == "X1")),
            "X1 from the includer file must be present in the merged tree"
        );
    }

    #[test]
    fn test_cross_file_subckt_resolution_is_order_independent() {
        // Corrects a premise in this epic's original acceptance bullet
        // ("test_ordering_constraint_included_file_cannot_see_later_
        // includer_content"): that bullet assumed .subckt resolution is
        // forward-only / order-sensitive across the include boundary. It
        // is not — per CORE-16's own implementation
        // (subckt_resolution.rs: collect_subckt_definitions() gathers
        // every .subckt in the whole tree up front, then resolve_in_scope
        // looks up X-calls against that full set regardless of textual
        // position) and per docs/GRAMMAR.md, only `.param` evaluation is
        // confirmed sequential/order-sensitive for ngspice — `.subckt`
        // resolution is a whole-document post-parse lookup in both
        // dialects, exactly like within a single file. This test verifies
        // the actual (correct) behavior: an included file's X-call CAN
        // resolve against a .subckt defined later in the includer file.
        use crate::include::source_map::FileId;
        use crate::symbols::subckt_resolution::resolve_subckt_calls;

        let file_a = FileId::new_dummy();
        let file_b = FileId::new_dummy();
        let tagged = vec![
            // file_b (included) calls a subckt that only file_a (the
            // includer) defines, and defines it AFTER the .include point.
            (
                file_b,
                Statement::ElementInstance(ElementInstance {
                    device_letter: 'X',
                    name: "X1".into(),
                    nodes: vec![],
                    raw_params: vec!["late_defined".into()],
                    subckt_name: Some("late_defined".into()),
                    span: 1..2,
                }),
            ),
            (file_a, make_subckt_stmt("late_defined", 2..3)),
            (file_a, make_ends(Some("late_defined"), 3..4)),
        ];
        let tree = build_scope_tree_from_tagged(&tagged).unwrap();
        let diags = resolve_subckt_calls(&tree, crate::dialect::Dialect::Ngspice);
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("undefined subcircuit")),
            "X1 should resolve against late_defined regardless of textual position: {diags:?}"
        );
    }

    #[test]
    fn test_duplicate_name_across_two_files_caught_with_both_file_locations() {
        // Real end-to-end pipeline: CORE-24's resolve_includes() assigns
        // each file a disjoint virtual-line-number block (SourceMap::
        // assign_offset) and remaps every statement's span into that
        // shared space before this function ever sees them, so a
        // duplicate-name diagnostic's span alone is enough to recover
        // which file it came from via SourceMap::resolve_virtual_line —
        // no `file` field needed on Statement/Scope/ScopeError at all.
        //
        // Uses .model (not .subckt) duplicates: a `.subckt` statement is
        // consumed into the Scope *tree structure* itself by
        // build_scope_tree() (it never appears in any scope.statements
        // list), so check_unique_names()'s .subckt branch can never fire
        // against the real pipeline's output — that is a separate,
        // pre-existing integration gap in CORE-15, out of scope for this
        // fix (see the CORE-15 gotcha note). .model statements are not
        // special-cased by build_scope_tree() and do reach
        // check_unique_names() correctly, so they exercise exactly what
        // this epic is actually responsible for: file-identity threading.
        use crate::dialect::Dialect;
        use crate::include::graph::resolve_includes;
        use crate::include::resolve::FakeFileSystem;
        use crate::include::source_map::SourceMap;
        use crate::symbols::uniqueness::check_unique_names;
        use std::path::Path;

        let mut fs = FakeFileSystem::new();
        fs.insert("/top.cir", ".model dup npn (bf=100)\n.include /sub.cir\n");
        fs.insert("/sub.cir", ".model dup npn (bf=50)\n");

        let mut sm = SourceMap::new();
        let (tagged, include_diags) =
            resolve_includes(Path::new("/top.cir"), &fs, Dialect::Ngspice, &mut sm);
        assert!(include_diags.is_empty(), "unexpected: {include_diags:?}");

        let tree = build_scope_tree_from_tagged(&tagged).unwrap();
        let diags = check_unique_names(&tree);
        assert_eq!(diags.len(), 1, "expected exactly one duplicate diagnostic");

        let first_loc = sm.resolve_virtual_line(diags[0].first_span.start);
        let dup_loc = sm.resolve_virtual_line(diags[0].duplicate_span.start);
        assert!(first_loc.is_some(), "first definition's file must resolve");
        assert!(
            dup_loc.is_some(),
            "duplicate definition's file must resolve"
        );
        assert_ne!(
            first_loc.unwrap().0,
            dup_loc.unwrap().0,
            "the two 'dup' model definitions must resolve to two DIFFERENT files, not the same one"
        );
    }
}
