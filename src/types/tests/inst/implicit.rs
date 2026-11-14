//! Implicit control names (B2, 12.8.6): inside an `inst` or `bus` body a
//! free control-table name is a `Param` node; everywhere else it is still
//! `undefined-name`, and a session binding wins.

use crate::types::diag::DiagCode;
use crate::vm::fail::FailCode;

use super::super::check_src;
use super::{input, kinds, name_at, out, Session};

#[test]
fn control_names_in_a_body_are_params() {
    let mut s = Session::new();
    s.ok("inst k:\n\tsaw freq\n\t\t> * {env-perc attack release}\n\t\t> * amp");
    let d = s.def("k");
    let k = kinds(&d);
    for p in ["param:freq", "param:attack", "param:release", "param:amp"] {
        assert!(k.contains(&p.to_string()), "{p}: {k:?}");
    }
    // `note`, fed to `rate:`, is a control name too, as a named-argument
    // value. `n` (like `bank`) is a voice-level resource-selection
    // argument of `sample-play` (R2a, serial repair of the BE-FINAL STOP
    // finding): it is skipped before it becomes a node — the scheduler
    // selects it directly (`n` picks the bank index) — so it is never a
    // `param:n` node.
    s.ok("inst k2 bank:\n\tsample-play bank n: n rate: note");
    let k = kinds(&s.def("k2"));
    assert!(!k.contains(&"param:n".to_string()), "{k:?}");
    assert!(k.contains(&"param:note".to_string()), "{k:?}");
}

#[test]
fn outside_a_body_the_name_is_undefined() {
    let mut s = Session::new();
    assert_eq!(s.fails("let x decay"), FailCode::UndefinedName);
    assert_eq!(s.fails("let y cutoff"), FailCode::UndefinedName);
    let r = check_src("let x decay");
    assert!(
        r.diags.iter().any(|d| d.code == DiagCode::UndefinedName),
        "{:?}",
        r.diags
    );
}

#[test]
fn a_session_binding_wins_over_the_control() {
    let mut s = Session::new();
    s.ok("let cutoff 800\ninst k:\n\tsaw 440 > lpf cutoff");
    let d = s.def("k");
    let lpf = out(&d);
    assert_eq!(name_at(&d, input(&d, lpf, 1)), "const:800");
}

#[test]
fn a_call_head_is_never_a_control() {
    // `lpf` and `delay` are control names but here they are called.
    let mut s = Session::new();
    s.ok("inst k:\n\tsaw 440 > lpf 900 > delay 0.2 0.5");
    let k = kinds(&s.def("k"));
    assert!(k.contains(&"lpf".to_string()), "{k:?}");
    assert!(k.contains(&"delay".to_string()), "{k:?}");
}

#[test]
fn a_bus_body_takes_control_names_too() {
    // On a bus a control name starts the unit from the control's default.
    let mut s = Session::new();
    s.ok("bus :wet:\n\tplate mix: room");
    let r = s.reg.borrow();
    let b = r.bus(crate::value::intern::intern_kw("wet")).expect("bus");
    assert_eq!(b.def.chain.len(), 1);
}
