//! Tweak-site classification at compile time (design 13).

use crate::compile::{compile, CompileCx};
use crate::expand::{expand, ExpandCx};
use crate::ns::namespace::{FormGen, Namespace};
use crate::ns::tweak::{SiteOrigin, SiteTier};
use crate::reader::{read, AliasEnv};
use crate::vm::ops::Op;
use crate::vm::tests::{probe_prelude, FILE};

fn classify(src: &str, tweak_sites: bool) -> (Vec<(String, SiteTier, SiteOrigin)>, usize) {
    let ns = Namespace::new(probe_prelude());
    let r = read(src, FILE, &AliasEnv::new());
    let k = expand(&r.nodes[0], &mut ExpandCx::new(r.next_node_id())).expect("expand");
    let mut cx = CompileCx::new(&ns, FormGen::new(3));
    cx.tweak_sites = tweak_sites;
    let p = compile(&k, &mut cx).expect("compile");
    let loads = p
        .code
        .iter()
        .filter(|o| matches!(o, Op::LoadTweak(_)))
        .count();
    let sites = cx
        .sites
        .iter()
        .map(|s| {
            assert_eq!(s.form_gen, FormGen::new(3));
            assert_eq!(
                &src[s.span.start as usize..s.span.end as usize],
                s.initial.to_string()
            );
            (s.initial.to_string(), s.tier, s.origin)
        })
        .collect();
    (sites, loads)
}

#[test]
fn a_late_argument_literal_is_direct_and_loads_through_a_tweak_slot() {
    let (sites, loads) = classify("ctl :x 0.5", true);
    assert_eq!(
        sites,
        vec![("0.5".into(), SiteTier::Direct, SiteOrigin::PatternLiteral)]
    );
    assert_eq!(loads, 1);
}

#[test]
fn step_list_items_are_direct_and_computed_literals_reeval() {
    let (sites, _) = classify("ctl {ctl :x [1 [2 3]]} {+ 4 5}", true);
    let tiers: Vec<(String, SiteTier)> = sites.into_iter().map(|(v, t, _)| (v, t)).collect();
    assert_eq!(
        tiers,
        vec![
            ("1".into(), SiteTier::Direct),
            ("2".into(), SiteTier::Direct),
            ("3".into(), SiteTier::Direct),
            ("4".into(), SiteTier::Reeval),
            ("5".into(), SiteTier::Reeval),
        ]
    );
}

#[test]
fn value_positions_function_bodies_and_plain_calls_are_not_sites() {
    for src in [
        "ctl 1 :x",
        "print 1",
        "+ 1 2",
        "fn f k:\n\tctl :x 0.5",
        "map [1] {x -> ctl x 2}",
    ] {
        assert_eq!(classify(src, true), (vec![], 0), "{src}");
    }
}

#[test]
fn binding_and_inst_default_sites() {
    assert_eq!(
        classify("let bpm 120", true).0,
        vec![("120".into(), SiteTier::Reeval, SiteOrigin::Binding)]
    );
    assert_eq!(
        classify("var cutoff 800", true).0,
        vec![("800".into(), SiteTier::Direct, SiteOrigin::Binding)]
    );
    assert_eq!(
        classify("inst pluck freq amp = 0.5 pan = -1:\n\t+ amp pan", true).0,
        vec![
            ("0.5".into(), SiteTier::Direct, SiteOrigin::InstDefault),
            ("-1".into(), SiteTier::Direct, SiteOrigin::InstDefault),
        ]
    );
    // A fn header default is not a site.
    assert!(classify("fn f a b = 2:\n\t+ a b", true).0.is_empty());
}

#[test]
fn sites_off_compiles_literals_as_constants() {
    assert_eq!(classify("ctl :x 0.5", false), (vec![], 0));
    assert!(classify("var cutoff 800", false).0.is_empty());
}
