//! The dialect-agnostic AST: the [`Statement`] enum and the payload
//! structs for its more complex variants. This is the common core in the
//! "common core + per-dialect overlays" architecture (see the crate-level
//! docs) — nothing in this module knows which dialect (ngspice or Xyce) a
//! statement came from. Dialect-specific *parsing* lives in
//! [`crate::parser::ngspice`]/[`crate::parser::xyce`]; dialect-specific
//! *meaning* (e.g. what a given device letter is) lives in
//! [`crate::dialect`].

use std::ops::Range;

/// A 1-based, half-open range of source line numbers (`start..end`) that a
/// statement was parsed from. For a single-line statement this is
/// `line..line+1`. For a statement built from a `+`-continued or
/// backslash-continued logical line (see [`crate::lexer`]), it spans every
/// physical line that contributed to it.
///
/// When a document is the result of multi-file `.include`/`.lib`
/// resolution (see [`crate::include::graph`]), these are *virtual* line
/// numbers in a shared numbering space, not necessarily the line number
/// you'd see if you opened the original file directly — use
/// [`crate::include::source_map::SourceMap::resolve_virtual_line`] to map
/// a virtual line back to `(FileId, real_line)`.
pub type LineSpan = Range<usize>;

/// A parsed device/element instance line, e.g. `R1 1 2 100` or
/// `X1 in out opamp gain=10`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementInstance {
    /// The device-type letter, always uppercased (e.g. `'R'` for a
    /// resistor, `'X'` for a subcircuit call). Its *meaning* is
    /// dialect-dependent — resolve it via
    /// [`crate::dialect::Dialect::resolve_device_letter`], since some
    /// letters (`P`, `U`) mean different devices in ngspice vs. Xyce.
    pub device_letter: char,
    /// The full instance name as written, including the device-letter
    /// prefix (e.g. `"R1"`, not just `"1"`).
    pub name: String,
    /// The node names in the order they appear on the line, before any
    /// trailing parameters. How many leading tokens count as nodes (vs.
    /// parameters) is device-kind-dependent (see
    /// [`crate::dialect::DeviceKind::min_nodes`]) and was already resolved
    /// by the parser — this field is exactly the node list, not raw
    /// tokens.
    pub nodes: Vec<String>,
    /// Everything after the node list, as raw unparsed tokens — model
    /// name, numeric value, `key=value` parameters, or a behavioral-source
    /// expression fragment, depending on the device kind. See
    /// [`crate::expr::wiring_runtime`] for parsing the behavioral-source
    /// case into an expression tree.
    pub raw_params: Vec<String>,
    /// The subcircuit name for an `X` (subcircuit call) instance, e.g.
    /// `"opamp"` in `X1 in out opamp`. `None` for every other device type.
    pub subckt_name: Option<String>,
    /// Where in the source this instance was parsed from.
    pub span: LineSpan,
}

/// A block/signal-domain statement, e.g. `MOD1 kind=pwm freq=100000 in=PIDTF`. Distinguished
/// from an [`ElementInstance`] purely by shape, not by any reserved device letter: a real SPICE
/// element line never has a field literally named `kind`, so a line with a `kind=...` token
/// among its trailing fields is unambiguously this variant instead — see
/// [`crate::parser::ngspice::parse_line`]/[`crate::parser::xyce::parse_line`] for exactly where
/// that dispatch happens. This crate doesn't know what `kind=pid`/`kind=statespace`/etc. *mean*
/// — same as it doesn't know what a resistor means — it only recognizes the shape and captures
/// every field's raw text; a downstream builder (`general-mna`) interprets `kind` and the rest
/// of the fields (including any Python-list-literal field value like `a=[[1,2],[3,4]]`, kept
/// here as an opaque raw string, same treatment [`Model::raw_params`] gets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockInstance {
    /// The block's own name, as declared (e.g. `"MOD1"`).
    pub name: String,
    /// Every `key=value` field on the line, in source order, including `kind` itself (not
    /// pulled out separately — a downstream builder looks it up by key like any other field).
    /// Each value is raw, unparsed text.
    pub fields: Vec<(String, String)>,
    /// Where in the source this statement was parsed from.
    pub span: LineSpan,
}

/// A `.subckt` definition, e.g. `.subckt opamp in+ in- out gain=100`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subckt {
    /// The subcircuit's name, as declared after `.subckt`.
    pub name: String,
    /// The external node (port) list, in declared order.
    pub nodes: Vec<String>,
    /// Formal parameter defaults declared after the node list, as
    /// `(name, default_value)` pairs. `default_value` is `None` for a bare
    /// name with no `=value` (rare, but syntactically possible in some
    /// dialect variants). Values are captured as raw text, not parsed
    /// expressions — see [`crate::expr::wiring_compiletime`] for turning
    /// them into an [`crate::expr::Expr`] tree.
    pub params: Vec<(String, Option<String>)>,
    /// Where in the source this `.subckt` line was parsed from (just the
    /// `.subckt` line itself, not the body up to `.ends`).
    pub span: LineSpan,
}

/// A `.model` definition, e.g. `.model D_IDEAL D(IS=1e-14 N=1)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// The model's name, as declared after `.model`.
    pub name: String,
    /// The model type (e.g. `"D"`, `"NPN"`, `"NMOS"`). Note that the same
    /// type name doesn't always mean the same compact model across
    /// dialects/`LEVEL`s — see `docs/GRAMMAR.md` §6.2.
    pub model_type: String,
    /// The parameter list as raw text (e.g. `"(IS=1e-14 N=1)"` or, for
    /// Xyce's no-parentheses form, `"level=3 a1=0.17"`), not individually
    /// parsed. See [`crate::expr::wiring_compiletime`] for parsing a
    /// brace-delimited parameter value into an expression tree.
    pub raw_params: String,
    /// Where in the source this `.model` line was parsed from.
    pub span: LineSpan,
}

/// A `.param` or `.global_param` statement (both share this shape — see
/// [`Statement::Param`] vs. [`Statement::GlobalParam`]), e.g.
/// `.param x=1 y=2*x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Each `name=value` assignment on the line, in source order. `value`
    /// is raw expression text, not yet parsed — see
    /// [`crate::expr::wiring_compiletime`].
    pub assignments: Vec<(String, String)>,
    /// Where in the source this `.param`/`.global_param` line was parsed
    /// from.
    pub span: LineSpan,
}

/// A `.func` definition, e.g. `.func gain(x) {x*2}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Func {
    /// The function's name.
    pub name: String,
    /// The formal argument names, in declared order.
    pub args: Vec<String>,
    /// The function body as raw expression text (the part inside
    /// `{...}`/`'...'`/after `=`, with the delimiters already stripped).
    /// See [`crate::expr::wiring_compiletime`] for parsing it.
    pub body: String,
    /// Where in the source this `.func` line was parsed from.
    pub span: LineSpan,
}

/// One parsed netlist statement. This is the common core shared by both
/// dialects — see the module-level docs for what that means in practice.
///
/// Every variant carries a [`LineSpan`] so a diagnostic or hover feature
/// can always point back at source text, even for the variants (like
/// [`Statement::Unrecognized`]) that don't otherwise model the statement's
/// meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// A device/element instance line (`R1 1 2 100`, `X1 in out opamp`, ...).
    ElementInstance(ElementInstance),
    /// A block/signal-domain statement (`MOD1 kind=pwm freq=100000 in=PIDTF`) — see
    /// [`BlockInstance`]'s own doc comment for the dispatch rule that distinguishes this from
    /// [`Statement::ElementInstance`].
    BlockInstance(BlockInstance),
    /// A `.subckt` definition line (the body up to `.ends` is represented
    /// by the surrounding statements/scope structure — see
    /// [`crate::symbols::scope`] — not nested inside this variant).
    Subckt(Subckt),
    /// An `.ends` line. The `Option<String>` is the subcircuit name if
    /// given (`.ends opamp`), `None` for a bare `.ends`.
    Ends(Option<String>, LineSpan),
    /// A `.model` definition line.
    Model(Model),
    /// A `.param` line (top-level or subcircuit-local — scope is resolved
    /// later, see [`crate::symbols::ngspice_param_scope`]/
    /// [`crate::symbols::xyce_param_scope`], not encoded in this variant).
    Param(Param),
    /// A Xyce `.global_param` line. Structurally identical to
    /// [`Statement::Param`] but with stricter scoping rules — see
    /// `docs/GRAMMAR.md` §4.2 and [`crate::symbols::xyce_param_scope`].
    GlobalParam(Param),
    /// A `.func` definition line.
    Func(Func),
    /// An `.include`/`.inc`/`.incl` line. The `String` is the raw filename
    /// argument as written (not yet resolved to a real path — see
    /// [`crate::include::resolve`]).
    Include(String, LineSpan),
    /// A `.lib` line: `Lib(filename, section, span)`. `section` is `None`
    /// for a bare `.lib filename` (whole-file include), `Some(name)` for
    /// `.lib filename section` (splice only the matching
    /// `.lib section ... .endl section` block — see
    /// [`crate::include::lib_sections`]).
    Lib(String, Option<String>, LineSpan),
    /// A full-line comment (`*...`), a blank line, or (per docs/GRAMMAR.md
    /// §1) the mandatory first-line title of a whole document — see
    /// [`crate::parser::parse_document`].
    Comment(LineSpan),
    /// A line whose statement type isn't (yet) modeled by this crate,
    /// captured verbatim as raw text so no information is lost. This is
    /// also how `.control`/`.endc` block bodies (a separate csh-like
    /// scripting language, deliberately not parsed as netlist statements —
    /// see `docs/GRAMMAR.md` §6) come through.
    Unrecognized(String, LineSpan),
    /// A `.global` line; the `Vec<String>` is the declared global node
    /// names.
    Global(Vec<String>, LineSpan),
    /// An `.ic` line; each pair is a `(node_expr, value)` assignment, e.g.
    /// `("v(1)", "5")` for `.ic v(1)=5`.
    Ic(Vec<(String, String)>, LineSpan),
    /// A `.nodeset` line (the non-`all=` form); same pair shape as
    /// [`Statement::Ic`].
    Nodeset(Vec<(String, String)>, LineSpan),
    /// The ngspice-specific `.nodeset all=value` form; the `String` is the
    /// raw value.
    NodesetAll(String, LineSpan),
    /// A `.temp` line; the `String` is the raw temperature value.
    Temp(String, LineSpan),
    /// The ngspice-only `.csparam` line; same assignment-pair shape as
    /// [`Statement::Param`]'s assignments.
    Csparam(Vec<(String, String)>, LineSpan),
    /// A `.options` line. Structurally divergent between dialects (docs/
    /// GRAMMAR.md §6.4): ngspice uses one flat namespace (`package` is
    /// always `None`); Xyce requires a package keyword (`package` is
    /// `Some("DEVICE")`, `Some("TIMEINT")`, etc.).
    Options {
        /// The Xyce package keyword (`None` under ngspice, which has no
        /// package concept).
        package: Option<String>,
        /// Each `name`/`name=value` entry, in source order. A bare name
        /// with no `=value` has `None` for its value.
        assignments: Vec<(String, Option<String>)>,
        /// Where in the source this `.options` line was parsed from.
        span: LineSpan,
    },
    /// Emitted in place of [`Statement::Csparam`] when a `.csparam` line
    /// is encountered under Xyce, where it isn't valid syntax — the
    /// `String` is an explanatory note (currently `"ngspice-only
    /// extension"`), not the statement content.
    CsparamInfo(String, LineSpan),
    /// An `.ac` line; the `String` is the raw argument text (sweep type,
    /// points, frequency range), not semantically parsed.
    Ac(String, LineSpan),
    /// A `.dc` line; the `String` is the raw argument text.
    Dc(String, LineSpan),
    /// An `.op` line (no arguments).
    Op(LineSpan),
    /// A `.tran` line; the `String` is the raw argument text. Note: the
    /// first argument means something different in each dialect (ngspice:
    /// print interval; Xyce: initial timestep) — see `docs/GRAMMAR.md` §7.
    /// This variant doesn't disambiguate that for you; the caller needs to
    /// know the document's dialect.
    Tran(String, LineSpan),
    /// A dialect-specific analysis/output statement not covered by the
    /// common variants above (e.g. ngspice's `.pz`/`.noise`/`.probe`,
    /// Xyce's `.step`/`.hb`/`.data`, `.measure`/`.meas` in either
    /// dialect). Captured structurally rather than deeply modeled — see
    /// `docs/GRAMMAR.md` §7 for the full statement-by-statement reference.
    Analysis {
        /// The statement's dot-keyword, uppercased (e.g. `".STEP"`).
        keyword: String,
        /// Which dialect this keyword was recognized under
        /// (`Some("ngspice")` or `Some("xyce")`), when relevant.
        dialect_tag: Option<String>,
        /// The raw argument text after the keyword.
        raw_args: String,
        /// Where in the source this line was parsed from.
        span: LineSpan,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_instance_construction_roundtrip() {
        let ei = ElementInstance {
            device_letter: 'R',
            name: "R1".to_string(),
            nodes: vec!["1".to_string(), "2".to_string()],
            raw_params: vec!["100".to_string(), "tc1=0.001".to_string()],
            subckt_name: None,
            span: 1..2,
        };
        assert_eq!(ei.device_letter, 'R');
        assert_eq!(ei.name, "R1");
        assert_eq!(ei.nodes, vec!["1", "2"]);
        assert_eq!(ei.raw_params, vec!["100", "tc1=0.001"]);
        assert_eq!(ei.span, 1..2);
    }

    #[test]
    fn test_statement_variants_carry_line_span() {
        let span = 3..5;

        let s = Statement::ElementInstance(ElementInstance {
            device_letter: 'C',
            name: "C1".into(),
            nodes: vec!["3".into(), "0".into()],
            raw_params: vec!["1u".into()],
            subckt_name: None,
            span: span.clone(),
        });
        match &s {
            Statement::ElementInstance(ei) => assert_eq!(ei.span, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Subckt(Subckt {
            name: "opamp".into(),
            nodes: vec!["in+".into(), "in-".into(), "out".into()],
            params: vec![],
            span: span.clone(),
        });
        match &s {
            Statement::Subckt(sc) => assert_eq!(sc.span, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Ends(Some("opamp".into()), span.clone());
        match &s {
            Statement::Ends(name, s) => {
                assert_eq!(*name, Some("opamp".to_string()));
                assert_eq!(*s, span);
            }
            _ => panic!("wrong variant"),
        }

        let s = Statement::Model(Model {
            name: "NPN".into(),
            model_type: "NPN".into(),
            raw_params: "(BF=100 IS=1e-16)".into(),
            span: span.clone(),
        });
        match &s {
            Statement::Model(m) => assert_eq!(m.span, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Param(Param {
            assignments: vec![("a".into(), "5".into())],
            span: span.clone(),
        });
        match &s {
            Statement::Param(p) => assert_eq!(p.span, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::GlobalParam(Param {
            assignments: vec![("g".into(), "3".into())],
            span: span.clone(),
        });
        match &s {
            Statement::GlobalParam(p) => assert_eq!(p.span, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Func(Func {
            name: "f".into(),
            args: vec!["x".into()],
            body: "x+1".into(),
            span: span.clone(),
        });
        match &s {
            Statement::Func(f) => assert_eq!(f.span, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Include("models.lib".into(), span.clone());
        match &s {
            Statement::Include(_, s) => assert_eq!(*s, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Lib("parts.lib".into(), Some("cmos".into()), span.clone());
        match &s {
            Statement::Lib(_, _, s) => assert_eq!(*s, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Comment(span.clone());
        match &s {
            Statement::Comment(s) => assert_eq!(*s, span),
            _ => panic!("wrong variant"),
        }

        let s = Statement::Unrecognized("???".into(), span.clone());
        match &s {
            Statement::Unrecognized(_, s) => assert_eq!(*s, span),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_unrecognized_preserves_raw_text() {
        let raw = "this is not valid spice at all #@!";
        let s = Statement::Unrecognized(raw.to_string(), 1..2);
        assert_eq!(s, Statement::Unrecognized(raw.to_string(), 1..2));
    }
}
