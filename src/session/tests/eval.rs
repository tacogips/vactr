//! The eval pipeline (design 14.5.4; TASK-009 criterion 1, first half):
//! `eval-result` contents, set-tweak and set-var through the recording
//! host, `playing` provenance, and message ordering.

use super::support::{batches, site_at, Rig, DOC};
use crate::session::protocol::{
    ClientMsg, EvalBody, ServerMsg, WireOrigin, WireSpan, WireTier, WireValue,
};

const PROGRAM: &str = "var g 0.5\nlet speed 2\ns :analog > note [60 64] > gain g > d1   #@ lead:\nlet oops + 1 \"a\"\n";

#[test]
fn eval_result_carries_diagnostics_sites_and_the_directive_table() {
    let mut rig = Rig::new();
    let (r, rest) = rig.eval(PROGRAM, 4, 0);
    assert!(rest.is_empty(), "no pass: {rest:?}");
    assert_eq!((r.file.as_str(), r.doc_revision), (DOC, 4));
    assert_eq!(r.forms.len(), 4);
    // The checker's static diagnostic and the runtime failure of `oops`.
    assert!(r.diagnostics.iter().any(|d| d.code == "type-mismatch"));
    let oops = &r.forms[3];
    assert_eq!(oops.failure.as_ref().map(|f| f.code.as_str()), Some("type"));
    assert_eq!(r.forms[0].value.as_deref(), Some("0.5"));
    // The pattern form's span is its whole statement.
    let at = u32::try_from(PROGRAM.find("s :analog").expect("s")).expect("small");
    assert_eq!(r.forms[2].span.start, at);
    // Sites: tier, origin, form_gen.
    let g = site_at(&r, PROGRAM, "0.5");
    assert_eq!((g.tier, g.origin), (WireTier::Direct, WireOrigin::Binding));
    assert_eq!(g.form_gen, r.forms[0].form_gen);
    let speed = site_at(&r, PROGRAM, "2\n");
    assert_eq!(speed.tier, WireTier::Reeval);
    let n60 = site_at(&r, PROGRAM, "60");
    assert_eq!(
        (n60.tier, n60.origin, n60.value),
        (WireTier::Direct, WireOrigin::PatternLiteral, 60.0)
    );
    assert_eq!(n60.form_gen, r.forms[2].form_gen);
    // The directive table: the trailing label.
    assert_eq!(r.directives.entries.len(), 1);
    assert!(r.directives.entries[0].trailing);
    assert!(r.directives.labels.iter().any(|l| l.name == "lead"));
}

#[test]
fn set_var_and_set_tweak_change_state_heard_at_the_recording_host() {
    let mut rig = Rig::new();
    let src = "var g 0.5\ns :analog > note [60] > gain g > lpf 800 > d1\n";
    let r = rig.ok(src, 1);
    rig.run_to(1.0);
    let before = rig.sent().len();
    assert!(before >= 1, "the pattern plays");
    assert_eq!(rig.ctl(&rig.sent()[0], "gain"), Some(0.5));
    // set-var: applied at the next tick as `upd`.
    let out = rig.set_var("g", WireValue::Float(0.8), r.forms[0].form_gen, 0);
    assert!(out.is_empty(), "accepted: {out:?}");
    assert_eq!(
        rig.s
            .evaluator()
            .ns()
            .session_value("g")
            .map(|v| v.to_string())
            .as_deref(),
        Some("0.5")
    );
    rig.run_to(3.0);
    assert_eq!(
        rig.s
            .evaluator()
            .ns()
            .session_value("g")
            .map(|v| v.to_string())
            .as_deref(),
        Some("0.8")
    );
    let last = *rig.sent().last().expect("an event");
    assert_eq!(rig.ctl(&last, "gain"), Some(0.8));
    // set-tweak on the direct `800`: the next committed events hear it.
    let cut = site_at(&r, src, "800").clone();
    assert!(rig.set_tweak(&cut, 1200.0, 0).is_empty());
    let n = rig.sent().len();
    rig.run_to(5.0);
    let after: Vec<_> = rig.sent()[n..].to_vec();
    assert!(!after.is_empty());
    for e in &after {
        assert_eq!(
            rig.ctl(e, "cutoff").or_else(|| rig.ctl(e, "lpf")),
            Some(1200.0)
        );
    }
}

#[test]
fn playing_telemetry_carries_srcrefs_with_the_evals_revision() {
    let mut rig = Rig::new();
    let src = "s :analog > note [60 64] > d1\n";
    rig.ok(src, 7);
    let out = rig.run_to(1.2);
    let events: Vec<_> = out
        .iter()
        .filter_map(|m| match m {
            ServerMsg::Playing(p) => Some(p.events.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(!events.is_empty(), "telemetry flows to the subscriber");
    let at60 = WireSpan::new(
        u32::try_from(src.find("60").expect("60")).expect("small"),
        u32::try_from(src.find("60").expect("60") + 2).expect("small"),
    );
    let e = events
        .iter()
        .find(|e| e.src.as_ref().is_some_and(|s| s.span == at60))
        .expect("an event of the 60 step");
    let src_ref = e.src.as_ref().expect("src");
    assert_eq!((src_ref.file.as_str(), src_ref.doc_revision), (DOC, 7));
    assert_eq!(e.slot, "d1");
    // A later eval at revision 9 re-stamps the new form's events.
    rig.ok(src, 9);
    let out = rig.run_to(4.0);
    let revs: Vec<u64> = out
        .iter()
        .filter_map(|m| match m {
            ServerMsg::Playing(p) => Some(p.events.clone()),
            _ => None,
        })
        .flatten()
        .filter_map(|e| e.src.map(|s| s.doc_revision))
        .collect();
    assert_eq!(revs.last(), Some(&9));
}

#[test]
fn eval_result_precedes_the_bindings_batch_it_triggered() {
    let mut rig = Rig::new();
    rig.ok("var root 1\nlet left + root 1", 1);
    rig.clear();
    // Redefining `root` triggers a pass over `left`.
    let out = rig.send(ClientMsg::Eval(EvalBody {
        file: DOC.to_string(),
        code: "var root 5\nlet left + root 1".to_string(),
        span: Some(WireSpan::new(0, 10)),
        doc_revision: 2,
        edit_epoch: 0,
    }));
    let kinds: Vec<&str> = out.iter().map(ServerMsg::kind).collect();
    assert_eq!(kinds, vec!["eval-result", "bindings"]);
    let ServerMsg::EvalResult(r) = &out[0] else {
        unreachable!()
    };
    assert_eq!(r.forms.len(), 1, "only the form in span ran");
    let b = &batches(&out)[0];
    assert!(b.changed.iter().any(|c| c.name == "left" && c.value == "6"));
}

#[test]
fn a_reader_or_expander_error_in_one_form_does_not_stop_the_others() {
    let mut rig = Rig::new();
    let (r, _) = rig.eval("let a 1\nlet b [1 2\nlet c 3\nlet d {let}\nlet e 5\n", 1, 0);
    let values: Vec<Option<&str>> = r.forms.iter().map(|f| f.value.as_deref()).collect();
    assert!(values.contains(&Some("1")), "{values:?}");
    assert!(values.contains(&Some("5")), "{values:?}");
    assert!(r.diagnostics.iter().any(|d| d.severity == "error"));
    assert_eq!(
        rig.s
            .evaluator()
            .ns()
            .session_value("e")
            .map(|v| v.to_string())
            .as_deref(),
        Some("5")
    );
}
