use std::ops::Range;

pub type LineSpan = Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementInstance {
    pub device_letter: char,
    pub name: String,
    pub nodes: Vec<String>,
    pub raw_params: Vec<String>,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subckt {
    pub name: String,
    pub nodes: Vec<String>,
    pub params: Vec<(String, Option<String>)>,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub model_type: String,
    pub raw_params: String,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub assignments: Vec<(String, String)>,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Func {
    pub name: String,
    pub args: Vec<String>,
    pub body: String,
    pub span: LineSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    ElementInstance(ElementInstance),
    Subckt(Subckt),
    Ends(Option<String>, LineSpan),
    Model(Model),
    Param(Param),
    GlobalParam(Param),
    Func(Func),
    Include(String, LineSpan),
    Lib(String, Option<String>, LineSpan),
    Comment(LineSpan),
    Unrecognized(String, LineSpan),
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
