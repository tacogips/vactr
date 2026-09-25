//! `bus :name:` and `master:` definitions lower to `BusDef` chains
//! (design-music section 5, design 12.5, 12.8.6); `bus :name` with a
//! pattern subject stays the routing control.

use crate::dsp::effects::param_ctl;
use crate::dsp::graph::{BusId, EffectKind};
use crate::host::caps::{GraphHandle, InstResolver};
use crate::host::wire::Ctl;
use crate::ns::stage::StagedEffect;
use crate::types::diag::DiagCode;
use crate::value::intern::intern_kw;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

use super::super::check_src;
use super::Session;

const DRUMS: &str = "bus :drums:
\tcompressor threshold: -18 ratio: 4 attack: 0.01 release: 0.1
\t\t> tape drive: 0.3
\t\t> plate size: 0.6 mix: 0.2";

const MASTER: &str = "master:
\tmultiband-compressor > limiter ceiling: -0.3";

#[test]
fn a_bus_lowers_to_a_three_unit_chain() {
    let mut s = Session::new();
    s.clear();
    s.ok(DRUMS);
    let r = s.reg.borrow();
    let b = r.bus(intern_kw("drums")).expect("drums");
    let kinds: Vec<EffectKind> = b.def.chain.iter().map(|u| u.kind).collect();
    assert_eq!(
        kinds,
        [EffectKind::Compressor, EffectKind::Tape, EffectKind::Plate]
    );
    // R6: bus/effect unit parameters are effect-local ids.
    let id = |n| param_ctl(EffectKind::Compressor, n).expect(n);
    assert_eq!(
        &*b.def.chain[0].params,
        [
            (id("threshold"), Ctl::Const(-18.0)),
            (id("ratio"), Ctl::Const(4.0)),
            (id("attack"), Ctl::Const(0.01)),
            (id("release"), Ctl::Const(0.1)),
        ]
    );
    let installs: Vec<_> = s
        .effects
        .borrow()
        .iter()
        .filter_map(|e| match e {
            StagedEffect::Install(GraphHandle::Bus { id, .. }) => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(installs, [b.id]);
}

#[test]
fn master_lowers_and_installs_as_the_root_bus() {
    let mut s = Session::new();
    s.clear();
    s.ok(MASTER);
    let r = s.reg.borrow();
    let m = r.master().expect("master");
    assert_eq!(m.id, BusId::new(0));
    let kinds: Vec<EffectKind> = m.def.chain.iter().map(|u| u.kind).collect();
    assert_eq!(
        kinds,
        [EffectKind::MultibandCompressor, EffectKind::Limiter]
    );
    assert!(s
        .effects
        .borrow()
        .iter()
        .any(|e| matches!(e, StagedEffect::Install(GraphHandle::Master(_)))));
}

#[test]
fn bus_and_master_are_definition_heads_for_the_checker() {
    let r = check_src(&format!("{DRUMS}\n{MASTER}"));
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    let r = check_src("s [:bd-haus :sn-dub] > bus :drums > d1");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
}

#[test]
fn a_pattern_subject_keeps_the_routing_control() {
    let mut s = Session::new();
    s.ok(DRUMS);
    let p = s.ok("s [:bd-haus :sn-dub] > bus :drums");
    assert!(matches!(p, Value::Pattern(_)));
    let q = s.query(&p, 0);
    assert_eq!(q.events.len(), 2, "{:?}", q.faults);
    let bus = q.events[0].controls.get(&intern_kw("bus")).cloned();
    assert!(
        matches!(bus, Some(Value::Keyword(k)) if k == intern_kw("drums")),
        "{bus:?}"
    );
}

#[test]
fn redefining_a_bus_keeps_its_id_and_a_failing_body_keeps_it() {
    let mut s = Session::new();
    s.ok(DRUMS);
    let id = s.reg.borrow().bus(intern_kw("drums")).expect("bus").id;
    s.ok("bus :drums:\n\tlimiter");
    let r = s.reg.borrow();
    let b = r.bus(intern_kw("drums")).expect("bus");
    assert_eq!((b.id, b.def.chain.len()), (id, 1));
    drop(r);
    assert_eq!(
        s.fails("bus :drums:\n\tplate size: {alt 1 2}"),
        FailCode::InstFailed
    );
    assert_eq!(
        s.reg
            .borrow()
            .bus(intern_kw("drums"))
            .expect("bus")
            .def
            .chain
            .len(),
        1
    );
}

#[test]
fn a_signal_parameter_is_a_control_cell() {
    let mut s = Session::new();
    s.ok("bus :swell:\n\tplate mix: sine");
    let r = s.reg.borrow();
    let unit = &r.bus(intern_kw("swell")).expect("bus").def.chain[0];
    assert!(matches!(unit.params[0].1, Ctl::Cell(_)), "{unit:?}");
    assert_eq!(r.signal_inputs().len(), 1);
}

#[test]
fn a_bus_chain_over_the_cap_is_graph_too_large() {
    let mut s = Session::new();
    let out = s.eval("bus :long:\n\treduce 0..260 {gain 1} {acc k -> gain acc 1}");
    let o = out.last().expect("a form");
    assert!(
        matches!(&o.value, Err(f) if f.code == FailCode::InstFailed),
        "{:?}",
        o.value
    );
    assert!(
        o.diags.iter().any(|d| d.code == DiagCode::GraphTooLarge),
        "{:?}",
        o.diags
    );
    // 200 units fit.
    s.ok("bus :ok:\n\treduce 0..199 {gain 1} {acc k -> gain acc 1}");
    assert_eq!(
        s.reg
            .borrow()
            .bus(intern_kw("ok"))
            .expect("bus")
            .def
            .chain
            .len(),
        200
    );
}
