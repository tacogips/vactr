//! ADDRESSED SELECTOR RESOLUTION (design 13.5) and the vocabulary's
//! mapping rules; directives never change evaluation.

use crate::directives::key::BindingKey;
use crate::ns::evaluator::Evaluator;
use crate::ns::load::NoopHost;
use crate::ns::namespace::Prelude;
use crate::ns::stage::RecordingSink;
use crate::reader::node::TriviaKind;
use crate::reader::{read, AliasEnv};

use super::{codes, find, shown, table, FILE, SPEC};

#[test]
fn addressed_parameter_selects_the_inst_parameter() {
    let (t, _) = table(SPEC);
    let r = t
        .resolved
        .iter()
        .find(|r| r.key == Some(BindingKey::param("analog", "cutoff")))
        .expect("analog.cutoff");
    // The header parameter `cutoff`, not a call site.
    assert_eq!(r.target_span, find(SPEC, "cutoff", 0));
    assert_eq!((r.param_index, r.cc, r.ch), (0, Some(1), Some(1)));
    assert_eq!(&*r.param_name, "cutoff");
}

#[test]
fn addressed_call_site_maps_onto_its_first_declared_parameter() {
    let src = "s [:hh] > hpf 2000 > d2                    #@ hats:\n#@ hats.hpf cc: 30\n";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    let hpf = find(src, "hpf", 0);
    let got: Vec<_> = t
        .resolved
        .iter()
        .map(|r| {
            (
                r.key.clone(),
                r.target_span,
                r.param_name.to_string(),
                r.param_index,
                r.cc,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                Some(BindingKey::site("hats", "hpf", 1, "cutoff")),
                hpf,
                "cutoff".to_string(),
                0,
                Some(30)
            ),
            (
                Some(BindingKey::site("hats", "hpf", 1, "q")),
                hpf,
                "q".to_string(),
                1,
                None
            ),
        ]
    );
}

#[test]
fn parameter_and_call_site_clash_is_ambiguous() {
    let src = "inst tone lpf: float = 300:\n\tsaw 220 > lpf lpf\n#@ tone.lpf cc: 3\n";
    let (t, diags) = table(src);
    assert_eq!(codes(&diags), ["ambiguous-selector"]);
    assert!(t.resolved.is_empty());
}

#[test]
fn repeated_call_sites_need_an_ordinal() {
    let src = "s [:hh] > lpf 800 > lpf 2000 > d2   #@ hats:
#@ hats.lpf cc: 1
#@ hats.lpf.2 cc: 31
s [:bd] > lpf 1 > lpf 2 > d3   #@ lpf.2 cc: 30
s [:sd] > lpf 1 > d4   #@ lpf.2 cc: 30
";
    let (t, diags) = table(src);
    assert_eq!(
        codes(&diags),
        ["ambiguous-selector", "unknown-directive-site"]
    );
    assert_eq!(
        shown(src, &t),
        [
            "hats.lpf.2.cutoff cc:31 ch:-",
            "hats.lpf.2.q cc:- ch:-",
            "@4:19.cutoff cc:30 ch:-",
            "@4:19.q cc:- ch:-",
        ]
    );
    // The ordinal selects the second `lpf` in source order.
    assert_eq!(t.resolved[0].target_span, find(src, "lpf", 1));
}

#[test]
fn cc_underscore_skips_and_fewer_numbers_leave_panel_only() {
    let src = "s [:hh] > ladder 900 0.3 > lpf 400 > d1\n#@ ladder cc: 74 _ 30\n#@ lpf cc: 74\n";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        shown(src, &t),
        [
            "@1:11.in cc:74 ch:-",
            "@1:11.cutoff cc:- ch:-",
            "@1:11.res cc:30 ch:-",
            "@1:28.cutoff cc:74 ch:-",
            "@1:28.q cc:- ch:-",
        ]
    );
}

#[test]
fn ch_overrides_the_file_default() {
    let src = "#@ midi ch: 1\ns [:hh] > lpf 1 > hpf 2 > d1\n#@ lpf cc: 74 ch: 5\n#@ hpf cc: 30\n";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        shown(src, &t),
        [
            "@2:11.cutoff cc:74 ch:5",
            "@2:11.q cc:- ch:-",
            "@2:19.cutoff cc:30 ch:1",
            "@2:19.q cc:- ch:-",
        ]
    );
}

#[test]
fn reserved_and_unknown() {
    let (t, diags) = table("s [:hh] > lpf 1 > d1\n#@ lpf range: 20 2000\n");
    assert_eq!(codes(&diags), ["reserved-key"]);
    // The site is still named: panel membership without a mapping.
    assert_eq!(t.resolved.len(), 2);
    let (_, diags) = table("s [:hh] > lpf 1 > d1\n#@ foo cc: 1\n");
    assert_eq!(codes(&diags), ["unknown-directive-site"]);
    let (_, diags) = table(SPEC.replace("analog.cutoff", "analog.bogus").as_str());
    assert_eq!(codes(&diags), ["unknown-parameter"]);
    let (_, diags) = table("s [:hh] > lpf 1 > d1   #@ hats:\n#@ hats.bogus cc: 1\n");
    assert_eq!(codes(&diags), ["unknown-directive-site"]);
    let (_, diags) = table("s [:hh] > lpf 1 > d1\n#@ nosuch.lpf cc: 1\n");
    assert_eq!(codes(&diags), ["unknown-label"]);
    // More CC numbers than parameters.
    let (_, diags) = table("s [:hh] > lpf 1 > d1\n#@ lpf cc: 1 2 3\n");
    assert_eq!(codes(&diags), ["unknown-parameter"]);
}

/// `src` with every directive blanked to spaces (spans stay identical).
fn stripped(src: &str) -> String {
    let r = read(src, FILE, &AliasEnv::new());
    let mut bytes = src.as_bytes().to_vec();
    for item in r
        .trivia
        .items
        .iter()
        .filter(|t| t.kind == TriviaKind::Directive)
    {
        for b in &mut bytes[item.span.start as usize..item.span.end as usize] {
            *b = b' ';
        }
    }
    String::from_utf8(bytes).expect("ascii")
}

/// Every form's printed result, in a fresh evaluator.
fn evaluate(src: &str) -> Vec<String> {
    let r = read(src, FILE, &AliasEnv::new());
    let mut ev = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    r.nodes
        .iter()
        .map(|n| match ev.eval_form(n).value {
            Ok(v) => format!("ok {v}"),
            Err(f) => format!("err {}", f.code),
        })
        .collect()
}

#[test]
fn directives_never_change_evaluation() {
    let src = "#@ midi ch: 1
let base 440
let twice {* base 2}
fn f a b:
\t* a b
f 3 4
inst pluck freq: float = 440:
\tsaw freq
\t\t> lpf 900
\t#@ lpf cc: 20
#@ freq cc: 21
s [:bd :sd] > lpf 800 res: 0.4 > hpf 120 > d1   #@ drums: lpf cc: 74 71
#@ drums.hpf cc: 30
twice
";
    let plain = stripped(src);
    assert_ne!(plain, src);
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(!t.resolved.is_empty());
    let with = read(src, FILE, &AliasEnv::new());
    let without = read(&plain, FILE, &AliasEnv::new());
    assert_eq!(with.nodes, without.nodes);
    assert_eq!(with.diags, without.diags);
    let a = evaluate(src);
    assert_eq!(a, evaluate(&plain));
    assert!(a.iter().any(|v| v == "ok 12"), "{a:?}");
    assert!(a.iter().any(|v| v == "ok 880"), "{a:?}");
}
