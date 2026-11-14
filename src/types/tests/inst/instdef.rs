//! `inst` realizes ONCE to an `InstDef` (12.8.6): the node chain, the name
//! bound to `Sound::Inst`, cell-backed header defaults, the staged
//! `Install`, `inst-failed` keeping the previous definition, and
//! `graph-too-large`.

use std::sync::Arc;

use crate::dsp::build::SIGNAL_CTL_BASE;
use crate::host::caps::InstResolver;
use crate::host::wire::Ctl;
use crate::types::diag::DiagCode;
use crate::value::intern::intern_kw;
use crate::value::value::{Sound, Value};
use crate::vm::fail::FailCode;

use super::{input, kinds, name_at, out, param, Session};

const PLUCK: &str = "inst pluck freq: float = 440 amp: float = 0.5 cutoff: float = 2000:
\tsaw freq
\t\t> lpf cutoff
\t\t> * {env-perc 0.01 0.3}
\t\t> * amp";

#[test]
fn pluck_realizes_to_saw_lpf_env_amp() {
    let mut s = Session::new();
    s.ok(PLUCK);
    let d = s.def("pluck");
    assert_eq!(
        kinds(&d),
        [
            "param:freq",
            "saw",
            "param:cutoff",
            "lpf",
            "const:0.01",
            "const:0.3",
            "envperc",
            "mul",
            "param:amp",
            "mul"
        ]
    );
    let o = out(&d);
    let m1 = input(&d, o, 0);
    assert_eq!(name_at(&d, m1), "mul");
    assert_eq!(name_at(&d, input(&d, o, 1)), "param:amp");
    let m1 = m1.expect("mul");
    let lpf = input(&d, m1, 0);
    assert_eq!(name_at(&d, lpf), "lpf");
    assert_eq!(name_at(&d, input(&d, m1, 1)), "envperc");
    let lpf = lpf.expect("lpf");
    assert_eq!(name_at(&d, input(&d, lpf, 0)), "saw");
    assert_eq!(name_at(&d, input(&d, lpf, 1)), "param:cutoff");
    let saw = input(&d, lpf, 0).expect("saw");
    assert_eq!(name_at(&d, input(&d, saw, 0)), "param:freq");
}

#[test]
fn the_name_is_bound_to_the_instrument_sound() {
    let mut s = Session::new();
    s.ok(PLUCK);
    let id = s
        .reg
        .borrow()
        .id_of(intern_kw("pluck"))
        .expect("registered");
    match s.ev.ns().session_value("pluck") {
        Some(Value::Sound(snd)) => assert_eq!(*snd, Sound::Inst(id)),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        s.ok("pluck").to_string(),
        format!("(sound inst {})", id.get())
    );
    // `s pluck` and `s :pluck` both play it.
    let r = s.reg.borrow().route(&Sound::Inst(id));
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn header_defaults_that_are_tweak_sites_are_cells() {
    let mut s = Session::new();
    s.ok(PLUCK);
    let d = s.def("pluck");
    for name in ["freq", "amp", "cutoff"] {
        assert!(
            matches!(param(&d, name), Some(Ctl::Cell(_))),
            "{name}: {:?}",
            param(&d, name)
        );
    }
    let cells = s.reg.borrow().default_cells(d.id);
    let values: Vec<String> = cells
        .iter()
        .map(|(_, slot)| slot.get().to_string())
        .collect();
    assert_eq!(values, ["440", "0.5", "2000"]);
    // A default that is not a site is a constant.
    s.ok("inst k2 wave: keyword = :tri amp: float = 0.5:\n\tvco wave 440");
    assert_eq!(param(&s.def("k2"), "wave"), Some(Ctl::Const(3.0)));
}

#[test]
fn install_is_released_once_when_the_form_succeeds() {
    let mut s = Session::new();
    s.clear();
    s.ok(PLUCK);
    assert_eq!(s.installs(), 1);
    // Redefinition reuses the id and installs again.
    let id = s.def("pluck").id;
    s.ok("inst pluck freq:\n\tsin-osc freq");
    assert_eq!(s.def("pluck").id, id);
    assert_eq!(s.installs(), 2);
}

#[test]
fn a_failing_body_keeps_the_previous_definition() {
    let mut s = Session::new();
    s.ok(PLUCK);
    let before = s.def("pluck");
    s.clear();
    assert_eq!(
        s.fails("inst pluck freq:\n\tnot-a-ugen freq"),
        FailCode::InstFailed
    );
    assert!(Arc::ptr_eq(&before, &s.def("pluck")));
    assert_eq!(s.installs(), 0);
    match s.ev.ns().session_value("pluck") {
        Some(Value::Sound(snd)) => assert_eq!(*snd, Sound::Inst(before.id)),
        other => panic!("{other:?}"),
    }
    // A body that builds no unit generator fails the same way.
    assert_eq!(s.fails("inst text:\n\t\"hello\""), FailCode::InstFailed);
    assert!(s.reg.borrow().id_of(intern_kw("text")).is_none());
}

#[test]
fn more_than_256_nodes_is_graph_too_large() {
    let mut s = Session::new();
    let consts: Vec<String> = (1..=130).map(|k| k.to_string()).collect();
    let src = format!("inst big freq:\n\t+ {{saw freq}} {}", consts.join(" "));
    let out = s.eval(&src);
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
    assert!(s.reg.borrow().id_of(intern_kw("big")).is_none());
    // 120 constants fit.
    let consts: Vec<String> = (1..=120).map(|k| k.to_string()).collect();
    s.ok(&format!(
        "inst ok freq:\n\t+ {{saw freq}} {}",
        consts.join(" ")
    ));
}

#[test]
fn a_signal_input_is_a_control_cell() {
    let mut s = Session::new();
    s.ok("inst wob:\n\tphase-distortion freq shape: {range sine 0 1}");
    let d = s.def("wob");
    let sig = d
        .params
        .iter()
        .find(|(c, _)| c.get() >= SIGNAL_CTL_BASE)
        .expect("a signal parameter");
    let Ctl::Cell(cell) = sig.1 else {
        panic!("{sig:?}");
    };
    let inputs = s.reg.borrow().signal_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].cell, cell);
}

/// DDRUM-002A: an exhausted instrument-default cell pool no longer drops a
/// tweak-site header default silently. The instrument still installs, its
/// one default frozen at the current value, but the defining form reports
/// a `cell-capacity` diagnostic.
#[test]
fn exhausted_default_cells_report_cell_capacity_not_silence() {
    let mut s = Session::new();
    for _ in 0..crate::ns::insts::INST_CELL_COUNT {
        s.reg.borrow_mut().alloc_cell().expect("a free cell");
    }
    let out = s.eval("inst pluck2 cutoff: float = 2000:\n\tsaw freq > lpf cutoff > * amp");
    let o = out.last().expect("a form");
    assert!(o.value.is_ok(), "{:?}", o.value);
    assert!(
        o.diags.iter().any(|d| d.code == DiagCode::CellCapacity),
        "{:?}",
        o.diags
    );
    assert!(s.reg.borrow().id_of(intern_kw("pluck2")).is_some());
}

/// The same exhaustion, for a signal input (`dsp::build::signal_cell`):
/// the input still installs as a frozen `Const(0.0)`, with a diagnostic.
#[test]
fn exhausted_default_cells_freeze_a_signal_input_with_a_diagnostic() {
    let mut s = Session::new();
    for _ in 0..crate::ns::insts::INST_CELL_COUNT {
        s.reg.borrow_mut().alloc_cell().expect("a free cell");
    }
    let out = s.eval("inst wob:\n\tphase-distortion freq shape: {range sine 0 1}");
    let o = out.last().expect("a form");
    assert!(o.value.is_ok(), "{:?}", o.value);
    assert!(
        o.diags.iter().any(|d| d.code == DiagCode::CellCapacity),
        "{:?}",
        o.diags
    );
    let d = s.def("wob");
    let sig = d
        .params
        .iter()
        .find(|(c, _)| c.get() >= SIGNAL_CTL_BASE)
        .expect("a signal parameter");
    assert_eq!(sig.1, Ctl::Const(0.0));
}
