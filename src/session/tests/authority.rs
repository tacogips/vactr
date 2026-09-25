//! Write authority (design 14.4, 14.5.6 rules 1-7; TASK-009 criterion 7):
//! stale generations, unreconciled edits, edit invalidation through mapped
//! spans, composed change sets, base mismatches, superseded definitions and
//! latest-wins coalescing.

use super::support::{offset, site_at, stales, Rig};
use crate::ns::tweak::TweakId;
use crate::session::protocol::{EvalResultBody, StaleReason, StaleTarget, WireSite, WireValue};
use crate::value::value::Value;

const SRC: &str =
    "var g 0.5\ns :analog > note [60] > gain 0.4 > d1\ns :analog > note [67] > gain 0.3 > d2\n";

fn setup() -> (Rig, EvalResultBody) {
    let mut rig = Rig::new();
    let (r, _) = rig.eval(SRC, 1, 1);
    (rig, r)
}

fn reason(out: &[crate::session::protocol::ServerMsg]) -> Option<StaleReason> {
    stales(out).first().map(|s| s.reason)
}

fn slot_value(rig: &Rig, site: &WireSite) -> Value {
    let t = rig.s.evaluator().ns().tweaks().borrow();
    t.get(TweakId::new(site.id)).expect("site").slot.get()
}

#[test]
fn a_stale_form_gen_set_tweak_is_rejected() {
    let (mut rig, r) = setup();
    let mut site = site_at(&r, SRC, "0.4").clone();
    let current = site.form_gen;
    site.form_gen += 1;
    let out = rig.set_tweak(&site, 0.9, 1);
    let s = &stales(&out)[0];
    assert_eq!(s.target, StaleTarget::Id(site.id));
    assert_eq!(s.reason, StaleReason::StaleFormGen);
    assert_eq!(s.current_form_gen, Some(current));
    // After a re-eval the old generation's site is gone.
    let old = site_at(&r, SRC, "0.4").clone();
    rig.eval(SRC, 2, 1);
    assert_eq!(
        reason(&rig.set_tweak(&old, 0.9, 1)),
        Some(StaleReason::StaleFormGen)
    );
}

#[test]
fn a_debounce_race_write_is_unreconciled_until_the_matching_doc_changed() {
    let (mut rig, r) = setup();
    let site = site_at(&r, SRC, "0.3").clone();
    // Edited at epoch 2, the notification is still debounced.
    let out = rig.set_tweak(&site, 0.9, 2);
    assert_eq!(reason(&out), Some(StaleReason::UnreconciledEdit));
    // The doc-changed for epoch 2 (an edit elsewhere: a comment at the end).
    let end = u32::try_from(SRC.len()).expect("small");
    rig.doc_changed(2, 1, &[(end, end, 4)], &[(end, end + 4)], 2);
    assert!(rig.set_tweak(&site, 0.9, 2).is_empty(), "accepted now");
    rig.tick();
    assert_eq!(slot_value(&rig, &site).to_string(), "0.9");
}

#[test]
fn doc_changed_invalidates_intersecting_sites_only_and_rejects_a_delayed_write() {
    let (mut rig, r) = setup();
    let a = site_at(&r, SRC, "0.4").clone();
    let b = site_at(&r, SRC, "0.3").clone();
    // A write accepted now, applied at the tick...
    assert!(rig.set_tweak(&a, 0.8, 1).is_empty());
    // ...but `0.4` is retyped as `0.45` before the tick.
    let at = offset(SRC, "0.4", 0);
    rig.doc_changed(2, 1, &[(at, at + 3, 4)], &[(at, at + 4)], 2);
    let out = rig.tick();
    assert_eq!(reason(&out), Some(StaleReason::EditInvalidated), "{out:?}");
    assert_eq!(slot_value(&rig, &a).to_string(), "0.4", "never applied");
    assert_eq!(
        reason(&rig.set_tweak(&a, 0.8, 2)),
        Some(StaleReason::EditInvalidated)
    );
    // The unrelated site stays valid (its span shifted by one byte).
    assert!(rig.set_tweak(&b, 0.6, 2).is_empty());
    rig.tick();
    assert_eq!(slot_value(&rig, &b).to_string(), "0.6");
    let doc = rig.s.doc(super::support::DOC).expect("doc");
    let sb = doc.sites[&TweakId::new(b.id)];
    assert_eq!(sb.span.0, b.span.start + 1);
    // Rule 7: a re-eval brings fresh, valid sites.
    let src2 = SRC.replacen("0.4", "0.45", 1);
    let (r2, _) = rig.eval(&src2, 2, 2);
    let a2 = site_at(&r2, &src2, "0.45").clone();
    assert!(rig.set_tweak(&a2, 0.7, 2).is_empty());
}

#[test]
fn an_insertion_then_an_edit_to_the_shifted_site_uses_composed_mapping() {
    let (mut rig, r) = setup();
    let a = site_at(&r, SRC, "0.4").clone();
    let b = site_at(&r, SRC, "0.3").clone();
    // rev 2: a comment line inserted at the top shifts everything by 9.
    rig.doc_changed(2, 1, &[(0, 0, 9)], &[(0, 9)], 2);
    // rev 3: `0.3` retyped at its SHIFTED offset.
    let at = offset(SRC, "0.3", 0) + 9;
    rig.doc_changed(3, 2, &[(at, at + 3, 3)], &[(at, at + 3)], 3);
    assert_eq!(
        reason(&rig.set_tweak(&b, 0.9, 3)),
        Some(StaleReason::EditInvalidated),
        "the shifted site was edited"
    );
    assert!(
        rig.set_tweak(&a, 0.9, 3).is_empty(),
        "the other site is intact"
    );
    let doc = rig.s.doc(super::support::DOC).expect("doc");
    assert_eq!(doc.sites[&TweakId::new(a.id)].span.0, a.span.start + 9);
    assert_eq!(doc.rev, 3);
}

#[test]
fn a_mismatched_base_revision_invalidates_the_whole_file() {
    let (mut rig, r) = setup();
    let a = site_at(&r, SRC, "0.4").clone();
    let b = site_at(&r, SRC, "0.3").clone();
    rig.doc_changed(5, 4, &[(0, 0, 1)], &[(0, 1)], 2);
    assert_eq!(
        reason(&rig.set_tweak(&a, 0.9, 2)),
        Some(StaleReason::EditInvalidated)
    );
    assert_eq!(
        reason(&rig.set_tweak(&b, 0.9, 2)),
        Some(StaleReason::EditInvalidated)
    );
    let g = r.forms[0].form_gen;
    assert_eq!(
        reason(&rig.set_var("g", WireValue::Float(0.1), g, 2)),
        Some(StaleReason::EditInvalidated)
    );
}

#[test]
fn a_superseded_defining_form_gen_set_var_is_rejected() {
    let (mut rig, r) = setup();
    let old = r.forms[0].form_gen;
    let (r2, _) = rig.eval(SRC, 2, 1);
    let new = r2.forms[0].form_gen;
    assert_ne!(old, new);
    let out = rig.set_var("g", WireValue::Float(0.1), old, 1);
    let s = &stales(&out)[0];
    assert_eq!(s.target, StaleTarget::Name("g".to_string()));
    assert_eq!(s.reason, StaleReason::SupersededDefinition);
    assert_eq!(s.current_form_gen, Some(new));
    assert!(rig.set_var("g", WireValue::Float(0.1), new, 1).is_empty());
    // An edit of `var g`'s line invalidates the definition.
    rig.doc_changed(3, 2, &[(6, 9, 3)], &[(6, 9)], 2);
    assert_eq!(
        reason(&rig.set_var("g", WireValue::Float(0.2), new, 2)),
        Some(StaleReason::EditInvalidated)
    );
}

#[test]
fn writes_within_one_tick_coalesce_latest_wins() {
    let (mut rig, r) = setup();
    let a = site_at(&r, SRC, "0.4").clone();
    let g = r.forms[0].form_gen;
    for v in [0.1, 0.2, 0.9] {
        assert!(rig.set_tweak(&a, v, 1).is_empty());
    }
    for v in [0.25, 0.75] {
        assert!(rig.set_var("g", WireValue::Float(v), g, 1).is_empty());
    }
    assert_eq!(rig.s.pending.len(), 2, "one per target");
    rig.tick();
    assert!(rig.s.pending.is_empty());
    assert_eq!(slot_value(&rig, &a).to_string(), "0.9");
    assert_eq!(
        rig.s
            .evaluator()
            .ns()
            .session_value("g")
            .map(|v| v.to_string())
            .as_deref(),
        Some("0.75")
    );
}

#[test]
fn compose_sets_matches_mapping_through_both() {
    use crate::session::authority::compose_sets;
    use crate::session::changes::{map_through, Change, ChangeSet, Mapped};
    let c = |f, t, n| Change {
        from: f,
        to: t,
        insert_len: n,
    };
    let first = ChangeSet::new(vec![c(0, 0, 9), c(20, 25, 2)]).expect("valid");
    let later = ChangeSet::new(vec![c(12, 12, 4), c(40, 41, 0)]).expect("valid");
    let composed = compose_sets(&first, &later);
    let both = [first, later];
    for (s, e) in [
        (1, 2),
        (5, 8),
        (10, 12),
        (14, 18),
        (30, 34),
        (35, 38),
        (50, 60),
    ] {
        let direct = map_through(&both, s, e);
        let one = composed.map_span(s, e);
        if let Mapped::Moved { .. } = one {
            assert_eq!(one, direct, "{s}..{e}");
        } else {
            assert_eq!(
                direct,
                Mapped::Touched,
                "{s}..{e} touched only if truly touched"
            );
        }
    }
}
