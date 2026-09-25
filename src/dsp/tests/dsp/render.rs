//! TASK-008 criterion 1: events pushed into the ring render over N buffers,
//! voice starts land on the exact sample frame, a late event starts at
//! frame 0 and is counted, a stale-generation event is dropped, and no
//! callback allocates.

use super::{chain, ctl, event, NativeRig, BLOCK, SR};
use crate::dsp::graph::UGenSpec;
use crate::host::wire::{CtlMsg, HostMsg, Release, SlotControl, SlotControlAck};
use crate::sched::slots::SlotId;

/// A constant-1 instrument: its left channel at pan 0 is exactly 1 while it
/// sounds, so the first non-zero sample is the start frame.
fn dc(rig: &mut NativeRig) {
    rig.install(&chain(1, vec![UGenSpec::Const(1.0)]));
    let _ = rig.step();
}

fn first_nonzero(x: &[f32]) -> Option<usize> {
    x.iter().position(|v| *v != 0.0)
}

#[test]
fn voice_starts_land_on_the_exact_frame() {
    for (k, frame) in [0usize, 1, 37, 127].into_iter().enumerate() {
        let mut r = NativeRig::native();
        dc(&mut r);
        let base = r.engine.now();
        #[allow(clippy::cast_precision_loss)]
        let t = base + (BLOCK * (k + 1) + frame) as f64 / f64::from(SR);
        r.send(event(1, t, &[(ctl::PAN, 0.0)]));
        let (l, rr) = r.run(k + 3);
        let at = first_nonzero(&l).expect("the voice sounds");
        assert_eq!(at, BLOCK * (k + 1) + frame, "start frame");
        assert_eq!(l[at], 1.0);
        assert!(rr.iter().all(|v| *v == 0.0), "pan 0 is hard left");
        assert_eq!(r.engine.counters().late, 0);
    }
}

#[test]
fn many_events_over_many_buffers() {
    let mut rig = NativeRig::native();
    dc(&mut rig);
    let base = rig.engine.now();
    // Four short voices, one per 4 blocks, each at a distinct frame.
    let frames = [5usize, 600, 1100, 1900];
    for f in frames {
        #[allow(clippy::cast_precision_loss)]
        let t = base + f as f64 / f64::from(SR);
        rig.send(event(
            1,
            t,
            &[
                (ctl::PAN, 0.0),
                (ctl::ATTACK, 0.0),
                (ctl::DECAY, 0.001),
                (ctl::RELEASE, 0.001),
            ],
        ));
    }
    let (l, _) = rig.run(20);
    for f in frames {
        assert_eq!(l[f], 1.0, "voice at frame {f}");
        assert_eq!(l[f - 1], 0.0, "silent just before frame {f}");
    }
    assert_eq!(rig.engine.counters().dropped, 0);
    assert_eq!(rig.engine.active_voices(), 0, "short voices ended");
}

#[test]
fn late_event_starts_at_frame_zero_and_is_counted() {
    let mut rig = NativeRig::native();
    dc(&mut rig);
    let _ = rig.run(4);
    let now = rig.engine.now();
    rig.send(event(1, now - 0.01, &[(ctl::PAN, 0.0)]));
    let (l, _) = rig.run(1);
    assert_eq!(l[0], 1.0, "a late event starts at frame 0");
    assert_eq!(rig.engine.counters().late, 1);
    let acks = rig.acks();
    assert!(acks
        .iter()
        .any(|m| matches!(m, HostMsg::Counters { late: 1, .. })));
}

#[test]
fn stale_generation_event_is_dropped_at_dequeue() {
    let mut rig = NativeRig::native();
    dc(&mut rig);
    let now = rig.engine.now();
    rig.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: now,
        release: Release::None,
    }));
    let _ = rig.step();
    let acks = rig.acks();
    assert!(acks.contains(&HostMsg::SlotControlAck(SlotControlAck {
        slot: SlotId::new(1),
        gen: 2
    })));
    let t = rig.engine.now() + 0.001;
    // gen 1 (stale) and gen 2 at the same time.
    rig.send(event(1, t, &[(ctl::PAN, 0.0)]));
    let mut fresh = event(1, t, &[(ctl::PAN, 1.0)]);
    fresh.gen = 2;
    rig.send(fresh);
    let (l, r) = rig.run(2);
    assert!(
        l.iter().all(|v| *v == 0.0),
        "the stale gen-1 event never sounds"
    );
    assert!(r.iter().any(|v| *v != 0.0), "the gen-2 event sounds");
    assert_eq!(rig.engine.counters().dropped, 1);
}

#[test]
fn old_generation_before_the_boundary_still_plays() {
    let mut rig = NativeRig::native();
    dc(&mut rig);
    let now = rig.engine.now();
    // A rebind whose boundary is 10 ms ahead: gen-1 events before it play.
    rig.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: now + 0.010,
        release: Release::None,
    }));
    rig.send(event(1, now + 0.001, &[(ctl::PAN, 0.0)]));
    rig.send(event(1, now + 0.020, &[(ctl::PAN, 1.0)]));
    let (l, r) = rig.run(8);
    assert!(l.iter().any(|v| *v != 0.0), "before the boundary: plays");
    assert!(
        r.iter().all(|v| *v == 0.0),
        "at/after the boundary: dropped"
    );
    assert_eq!(rig.engine.counters().dropped, 1);
}

#[test]
fn unknown_instrument_is_dropped_not_crashed() {
    let mut rig = NativeRig::native();
    rig.send(event(99, 0.0, &[]));
    let (l, _) = rig.run(2);
    assert!(l.iter().all(|v| *v == 0.0));
    assert_eq!(rig.engine.counters().dropped, 1);
}

#[test]
fn output_is_finite_for_a_nan_control() {
    let mut rig = NativeRig::native();
    rig.install(&chain(1, vec![UGenSpec::SinOsc]));
    let _ = rig.step();
    rig.send(event(
        1,
        0.0,
        &[(ctl::FREQ, f32::NAN), (ctl::AMP, f32::INFINITY)],
    ));
    let (l, r) = rig.run(4);
    assert!(l.iter().chain(&r).all(|v| v.is_finite()));
}
