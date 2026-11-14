//! Rebuild rules (vactr-core.md TASK-005 rebuild criterion, design 5.6
//! eligibility/effects/transactions/DAG, section 13 tiers and override
//! migration).

use super::reactive_basic::Harness;
use crate::ns::depgraph::FormState;
use crate::ns::stage::{SlotKey, StagedEffect};
use crate::ns::tweak::{SiteOrigin, SiteTier};
use crate::types::diag::DiagCode;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

#[test]
fn a_partially_superseded_multi_output_form_is_ineligible_and_cannot_steal_d1() {
    let mut h = Harness::new();
    h.ok("var root 60\nlet raised + root 7\n[{d1 {ctl raised 0.5}} {d2 raised}]");
    let a = h.ev.form_of_bind(SlotKey::D(2)).expect("A owns d2");
    assert_eq!(h.ev.form_of_bind(SlotKey::D(1)), Some(a));
    let gen = h.ev.graph().get(a).map(|r| r.gen).expect("gen");
    let sites = h.ev.ns().tweaks().borrow().sites_of(gen);
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert_eq!(h.ev.site_tier(sites[0].id), Some(SiteTier::Direct));
    // B replaces d1: A no longer owns its whole write set.
    h.ok("d1 7");
    assert!(!h.ev.graph().eligible(a));
    assert_eq!(h.ev.site_tier(sites[0].id), Some(SiteTier::Manual));
    h.take();
    h.ok("upd root 62");
    assert_eq!(h.get("raised"), "69");
    assert_eq!(
        h.ev.graph().get(a).map(|r| r.runs),
        Some(1),
        "A is not replayed"
    );
    let fx = h.take();
    assert!(Harness::binds(&fx).is_empty(), "{:?}", Harness::binds(&fx));
    assert_ne!(h.ev.form_of_bind(SlotKey::D(1)), Some(a), "d1 stays B's");
}

#[test]
fn a_form_that_ran_once_is_excluded_and_reports_manual() {
    let mut h = Harness::new();
    h.ok("var root 60\nlet raised + root 7");
    let out = h.run("[{d1 {ctl raised 0.5}} {once raised}]");
    assert!(out[0].value.is_ok());
    let f = h.ev.form_of_bind(SlotKey::D(1)).expect("f");
    assert!(h.ev.graph().get(f).is_some_and(|r| r.non_replayable));
    let gen = h.ev.graph().get(f).map(|r| r.gen).expect("gen");
    let site = h.ev.ns().tweaks().borrow().sites_of(gen)[0].clone();
    assert_eq!(h.ev.site_tier(site.id), Some(SiteTier::Manual));
    h.take();
    h.ok("upd root 61");
    let fx = h.take();
    assert!(
        !fx.iter().any(|e| matches!(e, StagedEffect::OneShot { .. })),
        "a historical once never replays"
    );
    assert!(Harness::binds(&fx).is_empty());
    assert_eq!(h.ev.graph().get(f).map(|r| r.runs), Some(1));
}

#[test]
fn a_bind_then_fail_rebuild_releases_nothing_and_keeps_the_old_binding() {
    let mut h = Harness::new();
    h.ok("var root 60\nlet raised + root 7\n[{d1 raised} {/ 1 {- raised 69}}]");
    let f = h.ev.form_of_bind(SlotKey::D(1)).expect("f");
    h.take();
    h.ok("upd root 62");
    assert!(matches!(
        h.ev.graph().get(f).map(|r| r.state.clone()),
        Some(FormState::Failed(e)) if e.code == FailCode::DivisionByZero
    ));
    let fx = h.take();
    assert!(
        Harness::binds(&fx).is_empty(),
        "the staged d1 bind was retracted"
    );
    assert_eq!(
        h.ev.form_of_bind(SlotKey::D(1)),
        Some(f),
        "ownership unchanged"
    );
}

#[test]
fn a_failed_standalone_form_rolls_back_its_namespace_writes() {
    // Control: the bare VM session (no transaction) keeps the partial write.
    let mut s = crate::vm::tests::Sess::new();
    s.eval("var v 1").expect("setup");
    assert!(s.eval("[{upd v 5} {/ 1 0}]").is_err());
    assert_eq!(s.get_str("v"), "5", "the upd ran before the failure");
    // The evaluator's whole-form transaction undoes it: no trigger, no pass.
    let mut h = Harness::new();
    h.ok("var v 1\nlet w + v 1");
    h.take();
    let out = h.run("[{upd v 5} {/ 1 0}]");
    assert_eq!(
        out[0].value.as_ref().map_err(|e| e.code).err(),
        Some(FailCode::DivisionByZero)
    );
    assert_eq!((h.get("v"), h.get("w")), ("1".into(), "2".into()));
    assert!(h.ev.last_pass().is_none());
    assert!(h.take().is_empty(), "nothing released");
    assert_eq!(h.show("+ v 1"), "2", "the session stays usable");
}

#[test]
fn a_diamond_over_a_var_rebuilds_each_shared_descendant_once() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet l + root 1\nlet r * root 2\nlet m + l r\nlet t + m l r\nd1 t");
    h.take();
    h.ok("upd root 2");
    assert_eq!(h.get("t"), "14");
    for n in ["l", "r", "m", "t"] {
        assert_eq!(h.ev.runs(n), 2, "{n} rebuilt once");
    }
    assert_eq!(
        h.trace(),
        vec!["commit l", "commit r", "commit m", "commit t", "commit ?"]
    );
    assert!(
        h.ev.last_pass().is_some_and(|p| p.diags.is_empty()),
        "a diamond is not a cycle"
    );
}

#[test]
fn a_true_back_edge_ends_with_the_cycle_diagnostic() {
    let mut h = Harness::new();
    h.ok("let a 1\nlet b + a 1");
    // Redefining `a` from `b` closes a genuine cycle a -> b -> a.
    let out = h.run("let a + b 1");
    assert!(out[0].value.is_ok());
    assert!(
        out[0]
            .diags
            .iter()
            .any(|d| d.code == DiagCode::DependencyCycle),
        "{:?}",
        out[0].diags
    );
    // The pass stopped; the previous coherent values stand.
    assert_eq!((h.get("a"), h.get("b")), ("3".into(), "2".into()));
    assert!(h.state("b").is_bad());
}

#[test]
fn a_reeval_tweak_write_re_evaluates_the_owning_form() {
    // A computed literal inside a Late argument is a `reeval` site.
    let mut h = Harness::new();
    h.ok("d1 {late-probe {* 0.5 2}}");
    let f = h.ev.form_of_bind(SlotKey::D(1)).expect("f");
    let gen = h.ev.graph().get(f).map(|r| r.gen).expect("gen");
    let sites = h.ev.ns().tweaks().borrow().sites_of(gen);
    assert_eq!(sites.len(), 2, "{sites:?}");
    assert!(sites
        .iter()
        .all(|s| s.tier == SiteTier::Reeval && s.origin == SiteOrigin::PatternLiteral));
    h.take();
    let report =
        h.ev.set_tweak(sites[0].id, gen, Value::Float(0.25))
            .expect("set-tweak")
            .expect("a pass ran");
    assert!(report.diags.is_empty());
    assert_eq!(
        h.ev.graph().get(f).map(|r| r.runs),
        Some(2),
        "the owner re-evaluated"
    );
    let fx = h.take();
    assert_eq!(Harness::binds(&fx).len(), 1, "the rebuilt form re-binds d1");
    assert!(fx
        .iter()
        .any(|e| matches!(e, StagedEffect::TweakRefresh(_))));
}

#[test]
fn repeated_controller_updates_on_a_computed_literal_converge_via_override_inheritance() {
    let mut h = Harness::new();
    h.ok("d1 {late-probe {* 0.5 2}}");
    let f = h.ev.form_of_bind(SlotKey::D(1)).expect("f");
    for v in [0.25_f32, 0.125, 0.75] {
        let gen = h.ev.graph().get(f).map(|r| r.gen).expect("gen");
        let site = h.ev.ns().tweaks().borrow().sites_of(gen)[0].clone();
        h.ev.set_tweak(site.id, gen, Value::Float(v))
            .expect("set-tweak");
        // The stale generation's site is rejected afterwards.
        assert_eq!(
            h.ev.set_tweak(site.id, gen, Value::Float(v))
                .unwrap_err()
                .code,
            FailCode::UndefinedName
        );
        let now = h.ev.graph().get(f).map(|r| r.gen).expect("gen");
        assert_ne!(now, gen, "a rebuild creates fresh tweak slots");
        let fresh = h.ev.ns().tweaks().borrow().sites_of(now);
        assert_eq!(fresh.len(), 2, "only the current generation's sites remain");
        assert_eq!(
            fresh[0].slot.get().to_string(),
            Value::Float(v).to_string(),
            "inherited, no snap-back"
        );
        assert_eq!(
            fresh[0].initial.to_string(),
            "0.5",
            "the source literal is unchanged"
        );
        assert_eq!(
            fresh[1].slot.get().to_string(),
            "2",
            "an untouched site keeps its literal"
        );
    }
    assert_eq!(h.ev.graph().get(f).map(|r| r.runs), Some(4));
}

#[test]
fn a_let_binding_site_write_rebuilds_its_dependents_not_the_let() {
    let mut h = Harness::new();
    h.ok("let base 60\nlet up d1 {+ base 7}");
    let site = h.ev.sites_of("base")[0].clone();
    assert_eq!(
        (site.origin, site.tier),
        (SiteOrigin::Binding, SiteTier::Reeval)
    );
    h.take();
    h.ev.set_tweak(site.id, site.form_gen, Value::Int(62))
        .expect("set-tweak");
    assert_eq!((h.get("base"), h.get("up")), ("62".into(), "69".into()));
    assert_eq!(
        h.ev.runs("base"),
        1,
        "the write is the new value: the let is not replayed"
    );
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "69".to_string())]
    );
    assert_eq!(
        Harness::batches(&fx)[0][0],
        ("base".to_string(), "62".to_string())
    );
    // Re-typing the let supersedes its form: the old site is retired.
    h.ok("let base 70");
    assert!(h.ev.ns().tweaks().borrow().get(site.id).is_none());
    assert_eq!(h.get("up"), "77");
}
