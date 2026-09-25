//! TASK-007 criterion 6 (transport and master, design 11.7): `Stop` freezes
//! the scheduler (staging cleared, `SlotControl` Natural to every sink),
//! `Continue` resumes from the frozen position, `Start` restarts at cycle 0;
//! `midi-clock-out` emits Start/Clock/Stop with the commit discipline.

use super::clock::{beats, drive, started, Pulses};
use super::MidiRig;
use crate::host::caps::MidiEvent;
use crate::host::caps::MidiInEvent;
use crate::host::testing::SinkCall;
use crate::host::wire::Release;
use crate::ns::stage::SlotKey;
use crate::types::diag::DiagCode;
use crate::value::ratio::Ratio64;

#[test]
fn stop_freezes_continue_resumes_and_start_restarts_at_cycle_0() {
    let mut rig = started();
    let mut pulses = Pulses::new(0.01, 120.0, 0.0);
    drive(&mut rig, &mut pulses, 2.9);
    let before = beats(&rig.rt.telemetry());
    assert!(before.len() >= 6);
    let gen = rig.rt.slot_gen(SlotKey::D(1)).unwrap();
    // Stop at 3.0 s.
    pulses.send_due(&rig, 3.0);
    rig.send(MidiInEvent::Stop);
    rig.tick_at(3.0);
    assert!(rig.rt.midi_clock().is_frozen());
    let frozen = rig.rt.clock().pos();
    let stop = rig.slot_controls().last().unwrap().1;
    assert_eq!(stop.release, Release::Natural);
    assert_eq!(stop.new_gen, gen + 1);
    assert!((stop.effective_time - 3.0).abs() < 1e-9);
    let slot = rig.rt.slots().get(SlotKey::D(1)).unwrap();
    assert!(
        slot.lanes().iter().all(|l| l.staging.is_empty()),
        "staging cleared"
    );
    // The same control reaches the MIDI and OSC sinks.
    assert!(rig
        .midi
        .calls()
        .iter()
        .any(|(_, c)| matches!(c, SinkCall::Control(c) if c.release == Release::Natural)));
    assert!(rig
        .osc
        .calls()
        .iter()
        .any(|(_, c)| matches!(c, SinkCall::Control(c) if c.release == Release::Natural)));
    // Frozen for one second: nothing is sent, the position holds, no loss.
    let sent = rig.ring_events();
    rig.run_to(4.0);
    assert_eq!(rig.ring_events(), sent, "the scheduler is frozen");
    assert_eq!(rig.rt.clock().pos(), frozen);
    assert!(rig.diags_with(DiagCode::ClockLost).is_empty());
    let _ = rig.rt.telemetry();
    // Continue at 4.01 s from the frozen position.
    rig.send(MidiInEvent::Continue);
    rig.tick_at(4.01);
    assert!(!rig.rt.midi_clock().is_frozen());
    let mut pulses = Pulses::new(4.01, 120.0, 0.0);
    drive(&mut rig, &mut pulses, 5.9);
    let after = beats(&rig.rt.telemetry());
    assert!(!after.is_empty());
    let frozen_beat = frozen.checked_mul(Ratio64::from_int(4)).unwrap();
    let first = after[0].0;
    assert!(first >= frozen_beat, "resumes from the frozen position");
    assert!(
        first.checked_sub(frozen_beat).unwrap() < Ratio64::ONE,
        "the first beat at or after it"
    );
    for (beat, time) in &after {
        let rel = beat.checked_sub(frozen_beat).unwrap().to_f64();
        let nominal = 4.01 + rel * 0.5;
        assert!((time - nominal).abs() < 0.004, "beat {beat:?}: {time}");
    }
    // Start at 6.0 s: every binding restarts at cycle 0 from now.
    pulses.send_due(&rig, 6.0);
    rig.send(MidiInEvent::Start);
    rig.tick_at(6.0);
    let restart = beats(&rig.rt.telemetry());
    assert_eq!(restart.first().map(|b| b.0), Some(Ratio64::ZERO));
    assert!((restart[0].1 - 6.0).abs() < 1e-9);
    assert_eq!(rig.rt.clock().pos(), Ratio64::ZERO);
}

#[test]
fn clock_master_emits_start_clock_and_stop_with_commit_discipline() {
    let mut rig = MidiRig::new();
    rig.run("midi-clock-out true");
    assert!(rig.rt.midi_clock().is_master());
    rig.run_to(1.0);
    rig.run("use-bpm 60");
    rig.run_to(3.0);
    let lead = rig.rt.commit_lead();
    let sent = rig.midi_sent();
    let (a0, MidiEvent::Start { time: start }) = sent[0] else {
        panic!("Start first: {:?}", sent[0]);
    };
    assert!(start >= a0 && start <= a0 + lead + 1e-9);
    let clocks: Vec<(f64, f64)> = sent[1..]
        .iter()
        .map(|(a, e)| match e {
            MidiEvent::Clock { time } => (*a, *time),
            other => panic!("only clocks: {other:?}"),
        })
        .collect();
    assert!(
        (clocks[0].1 - start).abs() < 1e-9,
        "the first pulse at Start"
    );
    for (arrival, time) in &clocks {
        // Sent within the commit horizon, never late.
        assert!(*time > arrival - 1e-9, "on time: {arrival} {time}");
        assert!(*time <= arrival + lead + 1e-9, "not before the horizon");
    }
    // 24 pulses a quarter: 1/48 s at 120 bpm, 1/24 s at 60 bpm.
    let gaps: Vec<f64> = clocks.windows(2).map(|w| w[1].1 - w[0].1).collect();
    assert!((gaps[0] - 1.0 / 48.0).abs() < 1e-9);
    assert!((gaps[gaps.len() - 1] - 1.0 / 24.0).abs() < 1e-9);
    // Pulses already inside the horizon keep their times: one transition gap.
    let odd = gaps
        .iter()
        .filter(|g| (*g - 1.0 / 48.0).abs() > 1e-9 && (*g - 1.0 / 24.0).abs() > 1e-9)
        .count();
    assert!(odd <= 1 && gaps.iter().all(|g| *g > 0.0), "{gaps:?}");
    let n = clocks.len();
    rig.run("midi-clock-out false");
    rig.run_to(4.0);
    let sent = rig.midi_sent();
    let MidiEvent::Stop { time } = sent[n + 1].1 else {
        panic!("Stop: {:?}", sent[n + 1]);
    };
    assert!((time - (3.0 + lead)).abs() < 1e-9);
    assert_eq!(sent.len(), n + 2, "no clock after Stop");
}
