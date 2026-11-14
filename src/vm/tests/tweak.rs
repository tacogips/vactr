//! Tweak sites (design 13; TASK-005 tweak criterion): `TweakId`s stamped
//! with their `FormGen`, `Direct`-tier writes heard without re-evaluation,
//! and the tier classification.

use super::*;
use crate::ns::tweak::{SiteOrigin, SiteTier};

fn sites(s: &Sess, gen: u64) -> Vec<(String, SiteTier, SiteOrigin)> {
    s.ns.tweaks()
        .borrow()
        .sites_of(FormGen::new(gen))
        .iter()
        .map(|x| (x.initial.to_string(), x.tier, x.origin))
        .collect()
}

#[test]
fn a_pattern_literal_site_gets_a_tweak_id_stamped_with_its_form_gen() {
    let mut s = Sess::probe();
    s.eval("ctl :x 0.5").expect("form");
    let table = s.ns.tweaks().borrow();
    let all: Vec<_> = table.iter().collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].form_gen, FormGen::new(s.gen));
    assert_eq!(all[0].tier, SiteTier::Direct);
    assert_eq!(all[0].origin, SiteOrigin::PatternLiteral);
    assert_eq!(all[0].initial.to_string(), "0.5");
}

#[test]
fn a_direct_write_is_heard_at_the_next_read_without_re_evaluation() {
    let mut s = Sess::probe();
    s.eval("let p ctl :x 0.5").expect("form");
    let gen = s.gen;
    let id = s.ns.tweaks().borrow().sites_of(FormGen::new(gen))[0].id;
    assert_eq!(s.show("+ {first p} 0"), "0.5");
    s.ns.tweaks()
        .borrow()
        .write(id, Value::Float(0.9))
        .expect("write");
    // Only the reading form runs; the owning form is not re-evaluated (no
    // new generation of its site exists).
    assert_eq!(s.show("+ {first p} 0"), "0.9");
    let table = s.ns.tweaks().borrow();
    assert_eq!(table.iter().count(), 1);
    assert_eq!(table.get(id).map(|x| x.form_gen), Some(FormGen::new(gen)));
}

#[test]
fn an_inst_default_is_a_direct_site() {
    let mut s = Sess::probe();
    s.eval("inst pluck freq amp = 0.5:\n\t+ amp 0")
        .expect("inst");
    let gen = s.gen;
    let site = s.ns.tweaks().borrow().sites_of(FormGen::new(gen))[0].clone();
    assert_eq!(
        (site.tier, site.origin),
        (SiteTier::Direct, SiteOrigin::InstDefault)
    );
    assert_eq!(s.show("pluck 440"), "0.5");
    s.ns.tweaks()
        .borrow()
        .write(site.id, Value::Float(0.8))
        .expect("write");
    assert_eq!(s.show("pluck 440"), "0.8");
}

#[test]
fn tiers_follow_the_form_structure() {
    let mut s = Sess::probe();
    s.eval("ctl :x [1 2]").expect("list");
    assert_eq!(
        sites(&s, s.gen),
        vec![
            ("1".into(), SiteTier::Direct, SiteOrigin::PatternLiteral),
            ("2".into(), SiteTier::Direct, SiteOrigin::PatternLiteral),
        ]
    );
    s.eval("ctl :x {* 0.5 2}").expect("computed");
    assert_eq!(
        sites(&s, s.gen),
        vec![
            ("0.5".into(), SiteTier::Reeval, SiteOrigin::PatternLiteral),
            ("2".into(), SiteTier::Reeval, SiteOrigin::PatternLiteral),
        ]
    );
    s.eval("let bpm 120").expect("let");
    assert_eq!(
        sites(&s, s.gen),
        vec![("120".into(), SiteTier::Reeval, SiteOrigin::Binding)]
    );
    s.eval("var cutoff 800").expect("var");
    assert_eq!(
        sites(&s, s.gen),
        vec![("800".into(), SiteTier::Direct, SiteOrigin::Binding)]
    );
    // Not sites: function bodies, non-domain calls, plain values.
    s.eval("fn f k:\n\tctl :x 0.5").expect("fn");
    assert!(sites(&s, s.gen).is_empty());
    s.eval("print 1").expect("print");
    assert!(sites(&s, s.gen).is_empty());
    s.eval("let xs [1 2]").expect("list binding");
    assert!(sites(&s, s.gen).is_empty());
}

#[test]
fn a_binding_site_is_the_binding_slot() {
    let mut s = Sess::probe();
    s.eval("var cutoff 800").expect("var");
    let site = s.ns.tweaks().borrow().sites_of(FormGen::new(s.gen))[0].clone();
    assert!(site.slot.same(
        &s.ns
            .session_slot(crate::value::intern::intern_sym("cutoff"))
            .expect("slot")
    ));
    s.ns.tweaks()
        .borrow()
        .write(site.id, Value::Int(400))
        .expect("write");
    assert_eq!(s.get_str("cutoff"), "400");
}

#[test]
fn a_tweak_write_rejects_a_non_number() {
    let mut s = Sess::probe();
    s.eval("ctl :x 0.5").expect("form");
    let id = s.ns.tweaks().borrow().sites_of(FormGen::new(s.gen))[0].id;
    let e =
        s.ns.tweaks()
            .borrow()
            .write(id, Value::str("x"))
            .unwrap_err();
    assert_eq!(e.code, crate::vm::fail::FailCode::Type);
}
