//! The 13.5 PROPOSED grammar: classification by first token, pairs, and the
//! parse-level diagnostics.

use crate::directives::parse::{parse_directive, DirectiveBody, DirectiveKind, Pair};
use crate::reader::span::Span;

use super::{codes, FILE};

fn parse(text: &str) -> (crate::directives::parse::Directive, Vec<&'static str>) {
    let mut diags = Vec::new();
    let span = Span::new(FILE, 100, 100 + u32::try_from(text.len()).expect("short"));
    let d = parse_directive(text, span, &mut diags);
    let c = codes(&diags);
    (d, c)
}

#[test]
fn classification_by_first_token() {
    let (d, c) = parse("#@ midi ch: 1");
    assert_eq!(d.kind, DirectiveKind::Addressed);
    assert!(c.is_empty());
    let DirectiveBody::FileDefault { pairs } = d.body else {
        panic!("file default");
    };
    assert_eq!(pairs[0].pair, Pair::Ch(1));

    let (d, _) = parse("#@ hats.hpf cc: 30");
    assert_eq!(d.kind, DirectiveKind::Addressed);
    let DirectiveBody::Addressed {
        label,
        sel,
        ordinal,
        pairs,
        label_span,
        sel_span,
    } = d.body
    else {
        panic!("addressed");
    };
    assert_eq!((&*label, &*sel, ordinal), ("hats", "hpf", None));
    assert_eq!((label_span.start, label_span.end), (103, 107));
    assert_eq!((sel_span.start, sel_span.end), (108, 111));
    assert_eq!(pairs[0].pair, Pair::Cc(vec![Some(30)]));

    let (d, _) = parse("#@ hats.lpf.2 cc: 31");
    let DirectiveBody::Addressed { ordinal, .. } = d.body else {
        panic!("addressed ordinal");
    };
    assert_eq!(ordinal, Some(2));

    // An ordinal after a site name is positional, not a label reference.
    let (d, _) = parse("#@ lpf.2 cc: 30");
    assert_eq!(d.kind, DirectiveKind::Positional);
    let DirectiveBody::Positional(p) = d.body else {
        panic!("positional");
    };
    assert_eq!((&*p.sites[0].name, p.sites[0].ordinal), ("lpf", Some(2)));

    let (d, _) = parse("#@ lpf hpf cc: 74 71");
    assert_eq!(d.kind, DirectiveKind::Positional);
    let DirectiveBody::Positional(p) = d.body else {
        panic!("positional");
    };
    let names: Vec<&str> = p.sites.iter().map(|s| &*s.name).collect();
    assert_eq!(names, ["lpf", "hpf"]);
}

#[test]
fn label_definitions() {
    let (d, c) = parse("#@ name bass-filter");
    assert!(c.is_empty());
    assert_eq!(d.kind, DirectiveKind::Positional);
    let DirectiveBody::LabelDef { name, rest, .. } = d.body else {
        panic!("label def");
    };
    assert_eq!((&*name, rest), ("bass-filter", None));

    let (d, _) = parse("#@ bass-filter: lpf cc: 74 71");
    let DirectiveBody::LabelDef {
        name,
        name_span,
        rest,
    } = d.body
    else {
        panic!("short form");
    };
    assert_eq!(&*name, "bass-filter");
    assert_eq!((name_span.start, name_span.end), (103, 114));
    let rest = rest.expect("rest");
    assert_eq!(&*rest.sites[0].name, "lpf");
    assert_eq!(rest.pairs[0].pair, Pair::Cc(vec![Some(74), Some(71)]));
    // The head ends after `lpf`: pairs are rewritten after it.
    assert_eq!(d.head_end, 119);

    let (d, _) = parse("#@ hats:");
    assert!(matches!(d.body, DirectiveBody::LabelDef { rest: None, .. }));
}

#[test]
fn cc_skips_and_channel() {
    let (d, c) = parse("#@ ladder cc: 74 _ 30 ch: 5");
    assert!(c.is_empty());
    let DirectiveBody::Positional(p) = d.body else {
        panic!("positional");
    };
    assert_eq!(p.pairs[0].pair, Pair::Cc(vec![Some(74), None, Some(30)]));
    assert_eq!(p.pairs[1].pair, Pair::Ch(5));
}

#[test]
fn diagnostics() {
    assert_eq!(parse("#@ lpf range: 20 2000").1, ["reserved-key"]);
    assert_eq!(parse("#@ lpf curve: log").1, ["reserved-key"]);
    assert_eq!(parse("#@ lpf cc: 128").1, ["cc-out-of-range"]);
    assert_eq!(parse("#@ lpf cc: x").1, ["cc-out-of-range"]);
    assert_eq!(parse("#@ lpf ch: 0").1, ["cc-out-of-range"]);
    assert_eq!(parse("#@ lpf ch: 17").1, ["cc-out-of-range"]);
    assert_eq!(parse("#@ lpf cc: 1 cc: 2").1, ["duplicate-key"]);
    assert_eq!(parse("#@ cc: 1").1, ["unknown-directive-site"]);
    assert_eq!(
        parse("#@ range: 1 2").1,
        ["reserved-key", "unknown-directive-site"]
    );
    // A `#` token starts an ordinary comment inside the directive.
    let (d, c) = parse("#@ hats.hpf cc: 30   # later, by label");
    assert!(c.is_empty());
    let DirectiveBody::Addressed { pairs, .. } = d.body else {
        panic!("addressed");
    };
    assert_eq!(pairs[0].pair, Pair::Cc(vec![Some(30)]));
    // A blank directive is empty and silent.
    let (d, c) = parse("#@");
    assert!(c.is_empty());
    assert_eq!(d.body, DirectiveBody::Empty);
}
