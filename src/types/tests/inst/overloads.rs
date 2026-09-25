//! The DSP name overload set (B2, 12.8.6): a pattern subject keeps the
//! control or signal; a ugen subject, a numeric argument in an `inst` body,
//! or no subject in a `bus` body selects the ugen or effect.

use crate::dsp::graph::EffectKind;
use crate::value::intern::intern_kw;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

use super::super::{assert_clean, last_type};
use super::{input, kinds, name_at, out, Session};

#[test]
fn lpf_on_a_pattern_stays_the_control() {
    let mut s = Session::new();
    assert!(matches!(s.ok("s :bd > lpf 800"), Value::Pattern(_)));
    assert!(matches!(s.ok("s :bd > room 0.3"), Value::Pattern(_)));
    assert!(matches!(
        s.ok("s :bd > gain 0.4 > pan 0.2"),
        Value::Pattern(_)
    ));
    assert_clean("s :bd > lpf 800 > d1");
    assert_clean("s :bd > room 0.3 > d1");
}

#[test]
fn saw_then_lpf_in_an_inst_builds_saw_lpf() {
    let mut s = Session::new();
    s.ok("inst k cutoff: float = 900:\n\tsaw freq > lpf cutoff");
    let d = s.def("k");
    let lpf = out(&d);
    assert_eq!(name_at(&d, Some(lpf)), "lpf");
    assert_eq!(name_at(&d, input(&d, lpf, 0)), "saw");
    // A numeric argument inside the body selects the ugen too.
    s.ok("inst k2:\n\tsaw 220 > lpf 800 > * 0.5");
    let k = kinds(&s.def("k2"));
    assert_eq!(&k[..3], ["const:220", "saw", "const:800"]);
}

#[test]
fn zero_argument_saw_and_tri_stay_signals() {
    let mut s = Session::new();
    assert!(matches!(s.ok("saw"), Value::Signal(_)));
    assert!(matches!(s.ok("tri"), Value::Signal(_)));
    assert!(matches!(s.ok("range saw 0 1"), Value::Pattern(_)));
    // Outside a body a number does not make the signal callable.
    assert_eq!(s.fails("saw 440"), FailCode::NotCallable);
    assert_eq!(last_type("saw"), "signal");
}

#[test]
fn room_on_a_pattern_is_the_control_and_in_a_bus_the_effect() {
    let mut s = Session::new();
    s.ok("bus :verb:\n\troom 0.4");
    s.ok("bus :hall:\n\tplate size: 0.6");
    let r = s.reg.borrow();
    let verb = r.bus(intern_kw("verb")).expect("verb");
    assert_eq!(verb.def.chain[0].kind, EffectKind::Room);
    let hall = r.bus(intern_kw("hall")).expect("hall");
    assert_eq!(hall.def.chain[0].kind, EffectKind::Plate);
}

#[test]
fn shared_effect_names_on_a_ugen_subject_are_effects() {
    let mut s = Session::new();
    s.ok("inst k:\n\tsaw freq > gain 0.5 > pan 0.3 > saturate drive: 0.2");
    let k = kinds(&s.def("k"));
    for fx in ["fx:gain", "fx:pan", "fx:saturate"] {
        assert!(k.contains(&fx.to_string()), "{fx}: {k:?}");
    }
    // `saturate` on a texture is still the visual transform.
    assert!(matches!(s.ok("osc 10 > saturate 2"), Value::Tex(_)));
}

#[test]
fn range_scales_a_ugen_and_a_signal_as_before() {
    let mut s = Session::new();
    s.ok("inst k:\n\tsin-osc {range {sin-osc 2} 200 400}");
    let k = kinds(&s.def("k"));
    assert!(k.iter().filter(|x| *x == "sinosc").count() == 2, "{k:?}");
    assert!(matches!(s.ok("range sine 200 400"), Value::Pattern(_)));
}

#[test]
fn arithmetic_on_a_ugen_builds_nodes() {
    let mut s = Session::new();
    s.ok("inst k:\n\t- {sin-osc 440} 0.5");
    let d = s.def("k");
    let o = out(&d);
    assert_eq!(name_at(&d, Some(o)), "add");
    assert_eq!(name_at(&d, input(&d, o, 0)), "sinosc");
    assert_eq!(name_at(&d, input(&d, o, 1)), "mul");
    assert_eq!(last_type("* {sin-osc 440} 0.5"), "ugen");
    assert_eq!(last_type("* 0.5 {sin-osc 440}"), "ugen");
    assert_eq!(last_type("lpf {sin-osc 440} 800"), "ugen");
    assert_clean("let x * 0.5 {sin-osc 440}");
    // Numbers keep their arithmetic.
    assert_eq!(s.ok("* 2 3").to_string(), "6");
}
