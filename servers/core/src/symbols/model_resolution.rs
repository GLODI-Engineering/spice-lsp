//! Resolves device-instance model-name references against `.model`
//! definitions — [`resolve_model_references`] is the entry point.
//!
//! The tricky part this module handles: not every element instance
//! references a model at all. `R1 1 2 100` is a bare numeric value; only
//! `RMOD 3 7 RMODEL L=10u W=1u` actually names a model. Getting this
//! distinction wrong either misses real undefined-model errors or (worse)
//! false-positives on every ordinary resistor/capacitor/inductor in a
//! netlist — see [`model_name_from_params`].

use crate::ast::{LineSpan, Statement};
use crate::dialect::DeviceKind;
use crate::dialect::Dialect;
use crate::symbols::scope::Scope;

/// An undefined `.model` reference, found by [`resolve_model_references`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDiagnostic {
    /// A human-readable message naming the undefined model.
    pub message: String,
    /// Where in the source the referencing element instance is.
    pub span: LineSpan,
}

const NO_MODEL_DEVICES: &[DeviceKind] = &[
    DeviceKind::VoltageSource,
    DeviceKind::CurrentSource,
    DeviceKind::Vcvs,
    DeviceKind::Vccs,
    DeviceKind::Cccs,
    DeviceKind::Ccvs,
    DeviceKind::SubcircuitCall,
];

fn device_takes_model(kind: DeviceKind) -> bool {
    !NO_MODEL_DEVICES.contains(&kind)
}

fn model_name_from_params(raw_params: &[String]) -> Option<String> {
    for param in raw_params {
        if !param.contains('=') && !param.starts_with('(') {
            let is_numeric = param.chars().all(|c| {
                c.is_ascii_digit()
                    || c == '.'
                    || c == 'e'
                    || c == 'E'
                    || c == '-'
                    || c == '+'
                    || c == 'k'
                    || c == 'K'
                    || c == 'm'
                    || c == 'M'
                    || c == 'u'
                    || c == 'n'
                    || c == 'p'
                    || c == 'f'
                    || c == 'G'
                    || c == 'T'
                    || c == 'g'
                    || c == 't'
            });
            if !is_numeric && !param.starts_with("DC") && !param.starts_with("AC") {
                return Some(param.clone());
            }
        }
    }
    None
}

/// Resolve every element instance in `scope_tree` that plausibly
/// references a `.model` (per [`device_takes_model`] and
/// [`model_name_from_params`]'s bare-numeric-value heuristic) against the
/// `.model` definitions available to it, flagging undefined references.
/// Devices that structurally never take a model (`V`, `I`, `E`, `G`, `F`,
/// `H`, `X`) are skipped entirely, not run through model-name detection at
/// all. Model-binning suffixes (`basename.1`, `basename.2`, ...) resolve
/// against their base model name, matching
/// [`crate::symbols::uniqueness`]'s binning exemption.
pub fn resolve_model_references(scope_tree: &Scope, dialect: Dialect) -> Vec<ModelDiagnostic> {
    let mut diagnostics = Vec::new();
    let model_names: Vec<String> = collect_model_names(scope_tree);

    resolve_in_scope_model(scope_tree, dialect, &model_names, &mut diagnostics);
    diagnostics
}

fn collect_model_names(scope: &Scope) -> Vec<String> {
    let mut names = Vec::new();
    for stmt in &scope.statements {
        if let Statement::Model(model) = stmt {
            names.push(model.name.to_uppercase());
        }
    }
    for child in &scope.children {
        names.extend(collect_model_names(child));
    }
    names
}

fn resolve_in_scope_model(
    scope: &Scope,
    dialect: Dialect,
    model_names: &[String],
    diagnostics: &mut Vec<ModelDiagnostic>,
) {
    for stmt in &scope.statements {
        if let Statement::ElementInstance(ei) = stmt {
            let device_kind = dialect.resolve_device_letter(ei.device_letter);

            if let Some(kind) = device_kind {
                if device_takes_model(kind) {
                    if let Some(model_name) = model_name_from_params(&ei.raw_params) {
                        let upper = model_name.to_uppercase();
                        let found = model_names.iter().any(|m| {
                            m == &upper
                                || (is_binning_suffix(m) && get_binning_base(m) == Some(&upper))
                                || (is_binning_suffix(&upper)
                                    && get_binning_base(&upper) == Some(m))
                        });

                        if !found {
                            diagnostics.push(ModelDiagnostic {
                                message: format!("undefined model reference: '{model_name}'"),
                                span: ei.span.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    for child in &scope.children {
        resolve_in_scope_model(child, dialect, model_names, diagnostics);
    }
}

fn is_binning_suffix(name: &str) -> bool {
    if let Some(dot_pos) = name.rfind('.') {
        let suffix = &name[dot_pos + 1..];
        suffix.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

fn get_binning_base(name: &str) -> Option<&str> {
    name.rfind('.').map(|pos| &name[..pos])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn scope_with(models: Vec<Model>, elements: Vec<ElementInstance>) -> Scope {
        let stmts: Vec<Statement> = models
            .into_iter()
            .map(Statement::Model)
            .chain(elements.into_iter().map(Statement::ElementInstance))
            .collect();
        Scope {
            kind: crate::symbols::scope::ScopeKind::TopLevel,
            name: None,
            depth: 0,
            statements: stmts,
            children: Vec::new(),
            span: 0..1,
        }
    }

    fn model_def(name: &str) -> Model {
        Model {
            name: name.into(),
            model_type: "NPN".into(),
            raw_params: String::new(),
            span: 1..2,
        }
    }

    fn transistor(name: &str, model: &str) -> ElementInstance {
        ElementInstance {
            device_letter: 'Q',
            name: name.into(),
            nodes: vec!["c".into(), "b".into(), "e".into()],
            raw_params: vec![model.into()],
            subckt_name: None,
            span: 3..4,
        }
    }

    #[allow(dead_code)]
    fn diode(name: &str, model: &str) -> ElementInstance {
        ElementInstance {
            device_letter: 'D',
            name: name.into(),
            nodes: vec!["a".into(), "c".into()],
            raw_params: vec![model.into()],
            subckt_name: None,
            span: 5..6,
        }
    }

    #[test]
    fn test_device_with_model_resolves_to_definition() {
        let scope = scope_with(
            vec![model_def("NPN_FAST")],
            vec![transistor("Q1", "NPN_FAST")],
        );
        let diags = resolve_model_references(&scope, Dialect::Ngspice);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_undefined_model_reference_flagged() {
        let scope = scope_with(vec![], vec![transistor("Q1", "MISSING")]);
        let diags = resolve_model_references(&scope, Dialect::Ngspice);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined model"));
    }

    #[test]
    fn test_bare_passive_value_produces_no_model_diagnostic() {
        let r = ElementInstance {
            device_letter: 'R',
            name: "R1".into(),
            nodes: vec!["1".into(), "2".into()],
            raw_params: vec!["100".into()],
            subckt_name: None,
            span: 1..2,
        };
        let scope = scope_with(vec![], vec![r]);
        let diags = resolve_model_references(&scope, Dialect::Ngspice);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_same_device_type_with_and_without_model_distinguished() {
        let with_model = ElementInstance {
            device_letter: 'R',
            name: "RMOD".into(),
            nodes: vec!["3".into(), "7".into()],
            raw_params: vec!["RMODEL".into(), "L=10u".into(), "W=1u".into()],
            subckt_name: None,
            span: 1..2,
        };
        let scope = scope_with(vec![], vec![with_model]);
        let diags = resolve_model_references(&scope, Dialect::Ngspice);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined"));
    }

    #[test]
    fn test_model_binning_suffix_resolves_correctly() {
        let scope = scope_with(
            vec![
                Model {
                    name: "NMOS.1".into(),
                    model_type: "NMOS".into(),
                    raw_params: String::new(),
                    span: 1..2,
                },
                Model {
                    name: "NMOS.2".into(),
                    model_type: "NMOS".into(),
                    raw_params: String::new(),
                    span: 3..4,
                },
            ],
            vec![ElementInstance {
                device_letter: 'M',
                name: "M1".into(),
                nodes: vec!["d".into(), "g".into(), "s".into(), "b".into()],
                raw_params: vec!["NMOS.1".into()],
                subckt_name: None,
                span: 5..6,
            }],
        );
        let diags = resolve_model_references(&scope, Dialect::Ngspice);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_devices_without_model_slot_skipped_entirely() {
        let v = ElementInstance {
            device_letter: 'V',
            name: "V1".into(),
            nodes: vec!["1".into(), "0".into()],
            raw_params: vec!["DC".into(), "5".into()],
            subckt_name: None,
            span: 1..2,
        };
        let scope = scope_with(vec![], vec![v]);
        let diags = resolve_model_references(&scope, Dialect::Ngspice);
        assert!(diags.is_empty());
    }
}
