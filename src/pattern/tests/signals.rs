//! Signals at exact phases; input cells read at query time.

use std::rc::Rc;

use super::*;
use crate::pattern::build::signal;
use crate::pattern::combinators::music::range;
use crate::pattern::eval::{AnalyzerId, HostSig, InputCells, QueryCtx};
use crate::pattern::pat::PParam;
use crate::pattern::query::query;
use crate::pattern::signal::Sig;

fn at(sig: &Sig, t: Ratio64, cells: &InputCells) -> Value {
    let mut vm = StubVm::new();
    let cx = QueryCtx::new(&mut vm, cells, 42);
    sig.value_at(t, &cx).unwrap()
}

fn f(v: &Value) -> f64 {
    match v {
        Value::Float64(x) => *x,
        other => panic!("not a float64: {other:?}"),
    }
}

#[test]
fn waveforms_at_exact_phases() {
    let cells = InputCells::new();
    let phases = [r(0, 1), r(1, 4), r(1, 2), r(3, 4)];
    let table: [(Sig, [f64; 4]); 4] = [
        (Sig::Sine, [0.5, 1.0, 0.5, 0.0]),
        (Sig::Saw, [0.0, 0.25, 0.5, 0.75]),
        (Sig::Tri, [0.0, 0.5, 1.0, 0.5]),
        (Sig::Square, [0.0, 0.0, 1.0, 1.0]),
    ];
    for (sig, want) in &table {
        for (t, w) in phases.iter().zip(want) {
            assert_eq!(f(&at(sig, *t, &cells)), *w, "{sig:?} at {t}");
            // One cycle later, the same phase.
            let later = t.checked_add(Ratio64::from_int(3)).unwrap();
            assert_eq!(f(&at(sig, later, &cells)), *w);
        }
    }
    assert_eq!(show(&at(&Sig::Cycle, r(5, 2), &cells)), show(&int(2)));
    assert_eq!(f(&at(&Sig::Phase, r(5, 2), &cells)), 0.5);
    // 120 bpm, 4 beats per cycle: beat = 4 * cycles; time = 2 s per cycle.
    assert_eq!(f(&at(&Sig::Beat, r(1, 2), &cells)), 2.0);
    assert_eq!(f(&at(&Sig::Time, r(3, 2), &cells)), 3.0);
}

#[test]
fn random_signals_are_pure() {
    let cells = InputCells::new();
    for sig in [Sig::Rand, Sig::Perlin] {
        let a = f(&at(&sig, r(3, 7), &cells));
        assert_eq!(a, f(&at(&sig, r(3, 7), &cells)));
        assert!((0.0..=1.0).contains(&a));
    }
    let v = at(&Sig::IRand(8), r(1, 3), &cells);
    let Value::Int(i) = v else {
        panic!("irand is an int")
    };
    assert!((0..8).contains(&i));
    // Perlin interpolates between the integer points.
    let p0 = f(&at(&Sig::Perlin, r(2, 1), &cells));
    let r0 = f(&at(&Sig::Rand, r(2, 1), &cells));
    assert_eq!(p0, r0);
}

#[test]
fn cc_analyzer_and_host_signals_read_their_cells() {
    let mut cells = InputCells::new();
    let cc = Sig::Cc {
        controller: 74,
        channel: 1,
    };
    assert_eq!(f(&at(&cc, r(0, 1), &cells)), 0.0, "an unset cell reads 0");
    cells.set_cc(1, 74, 0.25);
    assert_eq!(f(&at(&cc, r(0, 1), &cells)), 0.25);
    cells.set_cc(1, 74, 0.75);
    assert_eq!(f(&at(&cc, r(0, 1), &cells)), 0.75, "read at query time");
    let an = Sig::Analyzer(AnalyzerId::new(3));
    cells.set_analyzer(AnalyzerId::new(3), 0.5);
    assert_eq!(f(&at(&an, r(0, 1), &cells)), 0.5);
    cells.set_host(HostSig::Fft(0), 0.125);
    assert_eq!(f(&at(&Sig::Host(HostSig::Fft(0)), r(0, 1), &cells)), 0.125);
    let mr = Sig::MapRange(Rc::new(cc), 200.0, 2000.0);
    assert_eq!(f(&at(&mr, r(0, 1), &cells)), 1550.0);
}

#[test]
fn a_cc_control_is_sampled_per_query() {
    // s [:bd :sd] > lpf {range {cc 74} 200 2000}
    let cc = Rc::new(signal(
        Sig::Cc {
            controller: 74,
            channel: 1,
        },
        None,
    ));
    let lpf = Value::Pattern(Rc::new(range(
        cc,
        PParam::int(200),
        PParam::int(2000),
        None,
    )));
    let p = ctl(s(list(vec![kw("bd"), kw("sd")])), "lpf", lpf);
    let mut cells = InputCells::new();
    let mut vm = StubVm::new();
    cells.set_cc(1, 74, 0.25);
    let a = {
        let mut cx = QueryCtx::new(&mut vm, &cells, 42);
        query(&p, cycle(0), &mut cx)
    };
    assert_eq!(a.events.len(), 2);
    assert_eq!(f(ctl_of(&a.events[0], "lpf").unwrap()), 650.0);
    cells.set_cc(1, 74, 1.0);
    let b = {
        let mut cx = QueryCtx::new(&mut vm, &cells, 42);
        query(&p, cycle(0), &mut cx)
    };
    assert_eq!(f(ctl_of(&b.events[0], "lpf").unwrap()), 2000.0);
}
