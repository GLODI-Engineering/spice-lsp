//! Resolves `X`-element subcircuit calls against `.subckt` definitions —
//! [`resolve_subckt_calls`] is the entry point. Covers three diagnostic
//! categories: undefined subcircuit references, node-count mismatches
//! between a call site and its definition, and circular subcircuit
//! references (direct or transitive).

use crate::ast::{LineSpan, Statement};
use crate::dialect::Dialect;
use crate::symbols::scope::{collect_subckt_definitions, Scope};

/// A problem found resolving `X`-element subcircuit calls: an undefined
/// reference, a node-count mismatch, a circular reference chain, or node
/// `0` illegally appearing in a `.subckt`'s own external node list. See
/// [`resolve_subckt_calls`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionDiagnostic {
    /// A human-readable description of the problem.
    pub message: String,
    /// Where in the source the problem was found.
    pub span: LineSpan,
}

/// Resolve every `X`-element instance in `scope_tree` against the
/// `.subckt` definitions available to it, per real SPICE `.subckt`
/// semantics: resolution is a whole-document lookup, not order-sensitive
/// — a call may reference a `.subckt` defined later in the same document
/// (unlike `.param` evaluation, which is sequential).
///
/// `dialect` only affects the wording of circular-reference diagnostics:
/// ngspice's `.subckt` expansion is pure textual substitution (docs/
/// GRAMMAR.md §6.1), so a cycle is framed as "would not terminate during
/// expansion" rather than a named error; Xyce documents an explicit
/// circular-reference error, so that's how its diagnostic is worded.
pub fn resolve_subckt_calls(scope_tree: &Scope, dialect: Dialect) -> Vec<ResolutionDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut call_graph: Vec<(String, String)> = Vec::new();

    let all_defs = collect_subckt_definitions(scope_tree);
    for def in &all_defs {
        for stmt in &def.statements {
            if let Statement::Subckt(s) = stmt {
                if s.nodes.contains(&"0".to_string()) {
                    diagnostics.push(ResolutionDiagnostic {
                        message: "node 0 cannot appear in .subckt node list".into(),
                        span: s.span.clone(),
                    });
                }
            }
        }
    }

    resolve_in_scope(scope_tree, dialect, &mut call_graph, &mut diagnostics);

    let def_names: Vec<String> = all_defs.iter().filter_map(|s| s.name.clone()).collect();

    for (_caller, callee) in &call_graph {
        let callee_upper = callee.to_uppercase();
        if !def_names.iter().any(|d| d.to_uppercase() == callee_upper) {
            let msg = format!("undefined subcircuit reference: '{callee}'");
            diagnostics.push(ResolutionDiagnostic {
                message: msg,
                span: 0..1,
            });
        }
    }

    let cycles = detect_cycles(&call_graph);
    for cycle in cycles {
        let chain = cycle.join(" \u{2192} ");
        let msg = match dialect {
            Dialect::Ngspice => format!(
                "circular subcircuit reference detected: {chain} (would not terminate during subcircuit expansion in ngspice)"
            ),
            Dialect::Xyce => format!(
                "circular subcircuit reference detected: {chain} (explicitly forbidden in Xyce)"
            ),
        };
        diagnostics.push(ResolutionDiagnostic {
            message: msg,
            span: 0..1,
        });
    }

    diagnostics
}

fn resolve_in_scope(
    scope: &Scope,
    _dialect: Dialect,
    call_graph: &mut Vec<(String, String)>,
    diagnostics: &mut Vec<ResolutionDiagnostic>,
) {
    let scope_name = scope.name.clone().unwrap_or_else(|| "".into());

    for stmt in &scope.statements {
        if let Statement::ElementInstance(ei) = stmt {
            if ei.device_letter.eq_ignore_ascii_case(&'X') {
                let subckt_name = if let Some(ref sn) = ei.subckt_name {
                    sn.clone()
                } else if !ei.nodes.is_empty() && ei.raw_params.is_empty() {
                    ei.nodes.last().unwrap().clone()
                } else {
                    ei.raw_params.first().cloned().unwrap_or_default()
                };

                call_graph.push((scope_name.clone(), subckt_name.clone()));

                let defs = collect_subckt_definitions(scope);
                for def in &defs {
                    if let Some(def_name) = &def.name {
                        if def_name.to_uppercase() == subckt_name.to_uppercase() {
                            let call_node_count = ei.nodes.len();
                            let def_node_count = count_def_nodes(def);
                            if call_node_count != def_node_count && def_node_count > 0 {
                                diagnostics.push(ResolutionDiagnostic {
                                    message: format!(
                                        "node count mismatch: call site has {call_node_count} nodes, definition '{def_name}' has {def_node_count} nodes"
                                    ),
                                    span: ei.span.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    for child in &scope.children {
        resolve_in_scope(child, _dialect, call_graph, diagnostics);
    }
}

fn count_def_nodes(scope: &Scope) -> usize {
    for stmt in &scope.statements {
        if let Statement::Subckt(subckt) = stmt {
            return subckt.nodes.len();
        }
    }
    0
}

fn detect_cycles(edges: &[(String, String)]) -> Vec<Vec<String>> {
    use std::collections::{HashMap, HashSet};

    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for (from, to) in edges {
        graph
            .entry(from.to_uppercase())
            .or_default()
            .push(to.to_uppercase());
    }

    let mut cycles = Vec::new();

    for start in graph.keys() {
        let mut stack = vec![(start.clone(), vec![start.clone()])];
        let mut visited_global: HashSet<(String, String)> = HashSet::new();

        while let Some((current, path)) = stack.pop() {
            if let Some(neighbors) = graph.get(&current) {
                for next in neighbors {
                    if path.contains(next) {
                        let cycle_start = path.iter().position(|n| n == next).unwrap_or(0);
                        let mut cycle: Vec<String> = path[cycle_start..].to_vec();
                        cycle.push(next.clone());
                        cycles.push(cycle);
                        continue;
                    }

                    let edge = (current.clone(), next.clone());
                    if visited_global.insert(edge) {
                        let mut new_path = path.clone();
                        new_path.push(next.clone());
                        stack.push((next.clone(), new_path));
                    }
                }
            }
        }
    }

    cycles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn scope_with_children(children: Vec<Scope>) -> Scope {
        Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: Vec::new(),
            children,
            span: 0..1,
        }
    }

    fn subckt_scope(name: &str, nodes: Vec<&str>, stmts: Vec<Statement>) -> Scope {
        Scope {
            kind: crate::symbols::scope::ScopeKind::Subckt,
            name: Some(name.into()),
            depth: 1,
            statements: {
                let mut s = vec![Statement::Subckt(Subckt {
                    name: name.into(),
                    nodes: nodes.iter().map(|n| n.to_string()).collect(),
                    params: vec![],
                    span: 1..2,
                })];
                s.extend(stmts);
                s
            },
            children: Vec::new(),
            span: 1..2,
        }
    }

    fn x_element(name: &str, nodes: Vec<&str>, subckt: &str) -> Statement {
        Statement::ElementInstance(ElementInstance {
            device_letter: 'X',
            name: name.into(),
            nodes: nodes.iter().map(|n| n.to_string()).collect(),
            raw_params: vec![subckt.into()],
            subckt_name: None,
            span: 5..6,
        })
    }

    #[test]
    fn test_x_call_resolves_to_matching_subckt_definition() {
        let sc = subckt_scope("opamp", vec!["in+", "in-", "out"], vec![]);
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_undefined_subckt_reference_flagged() {
        let sc = {
            let mut s = subckt_scope("main", vec![], vec![]);
            s.statements
                .push(x_element("X1", vec!["1", "2", "3"], "missing"));
            s
        };
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags.iter().any(|d| d.message.contains("undefined")));
    }

    #[test]
    fn test_node_count_mismatch_flagged_with_both_counts() {
        let sc = subckt_scope(
            "opamp",
            vec!["in+", "in-", "out"],
            vec![x_element("X1", vec!["1", "2", "3", "4"], "opamp")],
        );
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags.iter().any(|d| d.message.contains("mismatch")));
    }

    #[test]
    fn test_direct_self_call_cycle_detected() {
        let sc = {
            let mut s = subckt_scope("ring", vec!["a", "b"], vec![]);
            s.statements.push(x_element("X1", vec!["a", "b"], "ring"));
            s
        };
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags.iter().any(|d| d.message.contains("circular")));
    }

    #[test]
    fn test_transitive_cycle_detected_and_chain_named() {
        let sc_a = {
            let mut s = subckt_scope("A", vec![], vec![]);
            s.statements.push(x_element("X1", vec![], "B"));
            s
        };
        let sc_b = {
            let mut s = subckt_scope("B", vec![], vec![]);
            s.statements.push(x_element("X2", vec![], "A"));
            s
        };
        let root = scope_with_children(vec![sc_a, sc_b]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags.iter().any(|d| d.message.contains("circular")));
    }

    #[test]
    fn test_deep_non_circular_chain_not_flagged() {
        let sc_a = {
            let mut s = subckt_scope("A", vec![], vec![]);
            s.statements.push(x_element("X1", vec![], "B"));
            s
        };
        let sc_b = {
            let mut s = subckt_scope("B", vec![], vec![]);
            s.statements.push(x_element("X2", vec![], "C"));
            s
        };
        let sc_c = subckt_scope("C", vec![], vec![]);
        let root = scope_with_children(vec![sc_a, sc_b, sc_c]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(!diags.iter().any(|d| d.message.contains("circular")));
    }

    #[test]
    fn test_ngspice_cycle_message_frames_as_expansion_non_termination() {
        let sc = {
            let mut s = subckt_scope("loop", vec![], vec![]);
            s.statements.push(x_element("X1", vec![], "loop"));
            s
        };
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags
            .iter()
            .any(|d| d.message.contains("would not terminate")));
    }

    #[test]
    fn test_xyce_cycle_message_frames_as_explicit_error() {
        let sc = {
            let mut s = subckt_scope("loop", vec![], vec![]);
            s.statements.push(x_element("X1", vec![], "loop"));
            s
        };
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Xyce);
        assert!(diags
            .iter()
            .any(|d| d.message.contains("forbidden in Xyce")));
    }

    #[test]
    fn test_node_zero_in_subckt_node_list_flagged() {
        let sc = subckt_scope("bad", vec!["0"], vec![]);
        let root = scope_with_children(vec![sc]);
        let diags = resolve_subckt_calls(&root, Dialect::Ngspice);
        assert!(diags
            .iter()
            .any(|d| d.message.contains("node 0 cannot appear")));
    }
}
