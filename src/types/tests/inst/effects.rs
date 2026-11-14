//! One effect catalog, three positions (design 12.5): a pattern control, a
//! ugen in an `inst` chain, a unit of a `bus` chain.

use crate::dsp::effects::param_ctl;
use crate::dsp::graph::{EffectKind, UGenSpec};
use crate::host::wire::Ctl;
use crate::value::intern::intern_kw;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

use super::{input, name_at, out, Session};

#[test]
fn an_effect_in_all_three_positions() {
    let mut s = Session::new();
    // 1. A per-event pattern control (SuperDirt style).
    assert!(matches!(s.ok("s :bd > room 0.3"), Value::Pattern(_)));
    // 2. A ugen inside an `inst` chain: the subject is port 0, a constant
    //    parameter is in the effect's own table.
    s.ok("inst wet:\n\tsaw freq > plate mix: 0.3");
    let d = s.def("wet");
    let o = out(&d);
    let UGenSpec::Effect(fx) = &d.nodes[usize::from(o)] else {
        panic!("{:?}", d.nodes);
    };
    assert_eq!(fx.kind, EffectKind::Plate);
    assert_eq!(
        &*fx.params,
        [(
            param_ctl(EffectKind::Plate, "mix").expect("mix"),
            Ctl::Const(0.3)
        )]
    );
    assert_eq!(name_at(&d, input(&d, o, 0)), "saw");
    // 3. A unit of a bus chain.
    s.ok("bus :b:\n\tplate mix: 0.3");
    let r = s.reg.borrow();
    let b = r.bus(intern_kw("b")).expect("bus");
    assert_eq!(b.def.chain[0].kind, EffectKind::Plate);
    assert_eq!(
        &*b.def.chain[0].params,
        [(
            param_ctl(EffectKind::Plate, "mix").expect("mix"),
            Ctl::Const(0.3)
        )]
    );
}

#[test]
fn keyword_kinds_encode_as_their_index() {
    let mut s = Session::new();
    s.ok("bus :lofi:\n\tcodec kind: :gsm > radio kind: :sw");
    let r = s.reg.borrow();
    let chain = &r.bus(intern_kw("lofi")).expect("bus").def.chain;
    // R6: `kind` is effect-local, resolved per effect kind (it happens
    // to land at the same id for both here, but is not assumed to).
    let codec_kind = param_ctl(EffectKind::Codec, "kind").expect("kind");
    let radio_kind = param_ctl(EffectKind::Radio, "kind").expect("kind");
    assert_eq!(chain[0].params[0], (codec_kind, Ctl::Const(1.0)));
    assert_eq!(chain[1].params[0], (radio_kind, Ctl::Const(2.0)));
}

#[test]
fn an_unknown_keyword_or_parameter_fails_the_definition() {
    let mut s = Session::new();
    assert_eq!(s.fails("bus :x:\n\tcodec kind: :ogg"), FailCode::InstFailed);
    assert_eq!(s.fails("bus :y:\n\tplate wobble: 1"), FailCode::InstFailed);
    assert!(s.reg.borrow().bus(intern_kw("x")).is_none());
}

#[test]
fn an_effect_needs_a_subject_outside_a_bus() {
    let mut s = Session::new();
    assert_eq!(s.fails("let c compressor ratio: 4"), FailCode::Type);
    assert!(matches!(
        s.ok("let c compressor {sin-osc 440} ratio: 4\nc"),
        Value::UGen(_)
    ));
}

#[test]
fn analyzers_are_bus_units() {
    let mut s = Session::new();
    s.ok("bus :meters:\n\tlevel > spectrum");
    let r = s.reg.borrow();
    let chain = &r.bus(intern_kw("meters")).expect("bus").def.chain;
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].kind.name(), "level");
    assert_eq!(chain[1].kind.name(), "spectrum");
}

#[test]
fn lofi_composite_is_typed_and_lowers_in_bus_and_instrument() {
    let source="bus :dust:\n\tlofi tone: 4200 drive: 1.4 wow: 0.1 flutter: 0.02 bits: 14 rate: 26000 hiss: 0.03 crackle: 0.02 mix: 0.8\ninst soft:\n\tsin-osc freq > lofi tone: 5000 mix: 0.5";
    let checked = super::super::check_src(source);
    assert!(checked.diags.is_empty(), "{:?}", checked.diags);
    let mut session = Session::new();
    session.ok(source);
    let registry = session.reg.borrow();
    assert_eq!(
        registry.bus(intern_kw("dust")).unwrap().def.chain[0].kind,
        EffectKind::Lofi
    );
    let definition = session.def("soft");
    assert!(definition
        .nodes
        .iter()
        .any(|n| matches!(n,UGenSpec::Effect(fx) if fx.kind==EffectKind::Lofi)));
}
