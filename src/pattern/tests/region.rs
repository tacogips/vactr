//! Sample-region goldens (design 10.1, vactrol-core TASK-006).

use std::rc::Rc;

use super::*;
use crate::clock::Tempo;
use crate::pattern::build::signal;
use crate::pattern::combinators::region::{chop, fit, loop_at, slice, splice, striate, SpeedFit};
use crate::pattern::combinators::time::fast;
use crate::pattern::eval::num_ratio;
use crate::pattern::pat::{PParam, SliceCuts};
use crate::pattern::signal::Sig;
use crate::vm::fail::FailCode;

/// `(begin, end)` region controls of an event.
pub(crate) fn region(e: &Event) -> (Ratio64, Ratio64) {
    let get = |n: &str| num_ratio(ctl_of(e, n).expect("region control")).unwrap();
    (get("begin"), get("end"))
}

pub(crate) fn speed_fit(e: &Event) -> SpeedFit {
    SpeedFit::from_value(ctl_of(e, "speed-fit").expect("speed-fit control")).unwrap()
}

fn idx(values: &[i32]) -> Rc<Pat> {
    pat(list(values.iter().map(|i| int(*i)).collect()))
}

#[test]
fn chop_8_cuts_each_event_into_contiguous_regions() {
    // s :break > chop 8
    let p = chop(s(kw("break")), PParam::int(8), None);
    let ev = sorted(run(&p, cycle(0)).events);
    assert_eq!(ev.len(), 8);
    for (i, e) in ev.iter().enumerate() {
        let i = i as i64;
        assert_eq!(e.whole, Some(span(r(i, 8), r(i + 1, 8))));
        assert_eq!(region(e), (r(i, 8), r(i + 1, 8)));
        assert!(e.is_onset());
    }
    // chop composes with an existing region: begin 0.25 > end 0.5 > chop 2.
    let sub = ctl(
        ctl(s(kw("break")), "begin", Value::Float(0.25)),
        "end",
        Value::Float(0.5),
    );
    let c2 = chop(sub, PParam::int(2), None);
    let ev = sorted(run(&c2, cycle(0)).events);
    assert_eq!(region(&ev[0]), (r(1, 4), r(3, 8)));
    assert_eq!(region(&ev[1]), (r(3, 8), r(1, 2)));
}

#[test]
fn striate_keeps_timing_and_interleaves_regions() {
    // Ten onsets per cycle: ordinals 0..9 play regions 0..7, 0, 1.
    let src = Rc::new(fast(s(kw("break")), PParam::int(10), None));
    let p = striate(Rc::clone(&src), PParam::int(8), None);
    let ev = sorted(run(&p, cycle(0)).events);
    let base = sorted(run(&src, cycle(0)).events);
    assert_eq!(timing(&ev), timing(&base), "timing unchanged");
    let regions: Vec<Ratio64> = ev.iter().map(|e| region(e).0).collect();
    let want: Vec<Ratio64> = [0, 1, 2, 3, 4, 5, 6, 7, 0, 1]
        .iter()
        .map(|i| r(*i, 8))
        .collect();
    assert_eq!(regions, want);
    // The ordinal is a rank inside the event's cycle, not the window.
    let late = sorted(run(&p, span(r(1, 2), r(3, 2))).events);
    assert_eq!(region(&late[0]).0, r(5, 8));
    assert_eq!(late[0].anchor(), r(1, 2));
    assert_eq!(region(&late[5]).0, r(0, 1));
    assert_eq!(late[5].anchor(), r(1, 1));
}

#[test]
fn slice_equal_and_manual() {
    // s :break > slice 8 [0 2 4 7]: the index list gives the structure.
    let p = slice(
        s(kw("break")),
        SliceCuts::Equal(PParam::int(8)),
        idx(&[0, 2, 4, 7]),
        None,
    );
    let ev = sorted(run(&p, cycle(0)).events);
    assert_eq!(
        ev.iter().map(Event::anchor).collect::<Vec<_>>(),
        vec![r(0, 1), r(1, 4), r(1, 2), r(3, 4)]
    );
    let regions: Vec<(Ratio64, Ratio64)> = ev.iter().map(region).collect();
    assert_eq!(
        regions,
        vec![
            (r(0, 1), r(1, 8)),
            (r(2, 8), r(3, 8)),
            (r(4, 8), r(5, 8)),
            (r(7, 8), r(1, 1))
        ]
    );
    // s :break > slice [0 0.31 0.5 0.8] [2 0]: manual starts, the last to 1.
    let pts: Vec<PParam> = [0.0f32, 0.31, 0.5, 0.8]
        .iter()
        .map(|x| PParam::Const(Value::Float(*x)))
        .collect();
    let m = slice(
        s(kw("break")),
        SliceCuts::Manual(pts.into_boxed_slice()),
        idx(&[2, 0]),
        None,
    );
    let ev = sorted(run(&m, cycle(0)).events);
    assert_eq!(region(&ev[0]), (r(1, 2), r(4, 5)));
    assert_eq!(region(&ev[1]), (r(0, 1), r(31, 100)));
    let last = slice(
        s(kw("break")),
        SliceCuts::Manual(
            vec![PParam::int(0), PParam::Const(Value::Float(0.5))].into_boxed_slice(),
        ),
        idx(&[1]),
        None,
    );
    assert_eq!(region(&run(&last, cycle(0)).events[0]), (r(1, 2), r(1, 1)));
    // A structured subject keeps its timing; the index is sampled at onsets.
    let st = slice(
        pat(list(vec![kw("a"), kw("b")])),
        SliceCuts::Equal(PParam::int(4)),
        idx(&[3, 1]),
        None,
    );
    let ev = sorted(run(&st, cycle(0)).events);
    assert_eq!(region(&ev[0]).0, r(3, 4));
    assert_eq!(region(&ev[1]).0, r(1, 4));
}

#[test]
fn slice_faults_are_event_local() {
    // An index outside [0, count) fails that event only.
    let p = slice(
        s(kw("break")),
        SliceCuts::Equal(PParam::int(4)),
        idx(&[0, 9, 1]),
        None,
    );
    let res = run(&p, cycle(0));
    assert_eq!(res.events.len(), 2);
    assert_eq!(res.faults.len(), 1);
    assert_eq!(res.faults[0].code, FailCode::SliceIndex);
    assert_eq!(res.faults[0].origin.beat, Some(Ratio64::new(4, 3).unwrap()));
    // Unsorted or out-of-range dynamic manual points: bad-slice-points.
    for pts in [vec![0.5f32, 0.25], vec![0.0, 1.5]] {
        let cuts = SliceCuts::Manual(
            pts.iter()
                .map(|x| PParam::Const(Value::Float(*x)))
                .collect(),
        );
        let res = run(&slice(s(kw("break")), cuts, idx(&[0, 1]), None), cycle(0));
        assert!(res.events.is_empty());
        assert_eq!(res.faults.len(), 2);
        assert!(res
            .faults
            .iter()
            .all(|f| f.code == FailCode::BadSlicePoints));
    }
    let zero = slice(
        s(kw("break")),
        SliceCuts::Equal(PParam::int(0)),
        idx(&[0]),
        None,
    );
    assert_eq!(
        run(&zero, cycle(0)).faults[0].code,
        FailCode::BadSlicePoints
    );
}

#[test]
fn splice_loop_at_fit_mark_commit_time_speed() {
    // Mock bank: a 2 s sample; default tempo: 120 bpm, 4 beats = 2 s cycles.
    let cycle_s = Tempo::default().cycle_seconds().unwrap();
    assert!((cycle_s - 2.0).abs() < 1e-12);
    let sp = splice(
        s(kw("break")),
        SliceCuts::Equal(PParam::int(8)),
        idx(&[0, 2, 4, 7]),
        None,
    );
    let ev = sorted(run(&sp, cycle(0)).events);
    assert_eq!(region(&ev[1]), (r(2, 8), r(3, 8)));
    let fitv = speed_fit(&ev[1]);
    assert_eq!(
        fitv,
        SpeedFit::Splice {
            fraction: r(1, 8),
            cycles: r(1, 4)
        }
    );
    // slice seconds 0.25 / whole-span seconds 0.5.
    assert!((fitv.resolve(2.0, cycle_s) - 0.5).abs() < 1e-12);

    // s :break > loop-at 2: full region, loop, speed = 3 s / (2 * 2 s).
    let la = loop_at(s(kw("break")), PParam::int(2), None);
    let ev = run(&la, cycle(0)).events;
    assert_eq!(region(&ev[0]), (r(0, 1), r(1, 1)));
    assert_eq!(show(ctl_of(&ev[0], "loop").unwrap()), show(&int(1)));
    let lf = speed_fit(&ev[0]);
    assert_eq!(lf, SpeedFit::LoopAt { cycles: r(2, 1) });
    assert!((lf.resolve(3.0, cycle_s) - 0.75).abs() < 1e-12);

    // s [:a :b] > fit: speed = 1 s / (1/2 cycle * 2 s).
    let f = fit(pat(list(vec![kw("a"), kw("b")])), None);
    let ev = sorted(run(&f, cycle(0)).events);
    let ff = speed_fit(&ev[0]);
    assert_eq!(ff, SpeedFit::Fit { cycles: r(1, 2) });
    assert!((ff.resolve(1.0, cycle_s) - 1.0).abs() < 1e-12);
    // A faster tempo halves the cycle seconds and doubles the speed.
    let fast_tempo = Tempo::new(Ratio64::from_int(240), Ratio64::from_int(4)).unwrap();
    assert!((ff.resolve(1.0, fast_tempo.cycle_seconds().unwrap()) - 2.0).abs() < 1e-12);
}

#[test]
fn a_whole_less_source_is_an_event_local_fault_for_every_region_operator() {
    let sig = Rc::new(signal(Sig::Saw, None));
    let ops: Vec<Pat> = vec![
        chop(Rc::clone(&sig), PParam::int(2), None),
        striate(Rc::clone(&sig), PParam::int(8), None),
        slice(
            Rc::clone(&sig),
            SliceCuts::Equal(PParam::int(4)),
            konst(int(1)),
            None,
        ),
        splice(
            Rc::clone(&sig),
            SliceCuts::Equal(PParam::int(4)),
            konst(int(1)),
            None,
        ),
        loop_at(Rc::clone(&sig), PParam::int(2), None),
        fit(Rc::clone(&sig), None),
    ];
    for p in &ops {
        let res = run(p, cycle(0));
        assert!(res.events.is_empty());
        assert_eq!(res.faults.len(), 1);
        assert_eq!(res.faults[0].code, FailCode::NoWhole);
    }
}
