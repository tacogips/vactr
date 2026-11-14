//! TASK-007 criterion 6 (clock half, design 11.7): jittered synthetic 24-ppq
//! pulses drive a smoothed anchor while every logical position stays an
//! exact ratio; clock loss freewheels with `clock-lost`; `use-bpm` under
//! `:midi` is `clock-external`.

use super::MidiRig;
use crate::clock::clock::ClockSource;
use crate::host::caps::MidiInEvent;
use crate::sched::telemetry::PlayingEvent;
use crate::types::diag::DiagCode;
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;

/// External 24-ppq pulses from `t0` with a deterministic jitter of at most
/// `jitter` seconds.
pub(super) struct Pulses {
    t0: f64,
    period: f64,
    jitter: f64,
    k: u64,
    state: u64,
}

impl Pulses {
    pub(super) fn new(t0: f64, bpm: f64, jitter: f64) -> Self {
        Self {
            t0,
            period: 60.0 / (bpm * 24.0),
            jitter,
            k: 1,
            state: 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn noise(&mut self) -> f64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        #[allow(clippy::cast_precision_loss)]
        let u = (self.state >> 11) as f64 / (1u64 << 53) as f64;
        (2.0 * u - 1.0) * self.jitter
    }

    /// Queues every pulse due by `t`.
    pub(super) fn send_due(&mut self, rig: &MidiRig, t: f64) {
        loop {
            #[allow(clippy::cast_precision_loss)]
            let nominal = self.t0 + self.k as f64 * self.period;
            if nominal > t {
                break;
            }
            let time = nominal + self.noise();
            rig.send(MidiInEvent::Clock { time });
            self.k += 1;
        }
    }

    /// Skips the pulses due by `t` (a lost cable).
    pub(super) fn skip_to(&mut self, t: f64) {
        #[allow(clippy::cast_precision_loss)]
        while self.t0 + self.k as f64 * self.period <= t {
            self.k += 1;
        }
    }
}

/// Ticks every 10 ms to `end`, queueing due pulses before each tick.
pub(super) fn drive(rig: &mut MidiRig, pulses: &mut Pulses, end: f64) {
    let mut t = rig.clock.now();
    while t + 0.01 <= end + 1e-9 {
        t += 0.01;
        pulses.send_due(rig, t);
        rig.tick_at(t);
    }
}

/// A slaved rig playing four quarter notes a cycle, started (MIDI `Start`)
/// at host 0.01 s; telemetry before the start is discarded.
pub(super) fn started() -> MidiRig {
    let mut rig = MidiRig::new();
    rig.run("use-clock :midi");
    assert!(rig.rt.midi_clock().is_slave());
    assert_eq!(rig.rt.clock().source(), ClockSource::MidiClock);
    assert!(rig.run("s [:bd :bd :bd :bd] > d1").faults.is_empty());
    rig.tick_at(0.0);
    let _ = rig.rt.telemetry();
    rig.send(MidiInEvent::Start);
    rig.tick_at(0.01);
    rig
}

/// Telemetry of `d1`: `(beat, time)`.
pub(super) fn beats(ev: &[PlayingEvent]) -> Vec<(Ratio64, f64)> {
    let d1 = intern_kw("d1");
    ev.iter()
        .filter(|e| e.slot == d1)
        .map(|e| (e.beat, e.time))
        .collect()
}

#[test]
fn jittered_pulses_drive_a_smoothed_anchor_with_exact_positions() {
    let mut rig = started();
    let t0 = 0.01;
    let mut pulses = Pulses::new(t0, 100.0, 0.0015);
    drive(&mut rig, &mut pulses, 8.0);
    let bpm = rig.rt.midi_clock().smoothed_bpm().expect("smoothed");
    // 1.5 ms of jitter on a 25 ms period: the smoothed tempo stays within 2%.
    assert!((bpm - 100.0).abs() < 2.0, "smoothed tempo {bpm}");
    let got = beats(&rig.rt.telemetry());
    assert!(got.len() >= 12, "{got:?}");
    let quarter = 60.0 / 100.0;
    for (i, (beat, time)) in got.iter().enumerate() {
        // Exact logical positions: every quarter once, in order.
        assert_eq!(*beat, Ratio64::from_int(i64::try_from(i).unwrap()));
        #[allow(clippy::cast_precision_loss)]
        let nominal = t0 + i as f64 * quarter;
        let tol = if i < 4 { 0.03 } else { 0.004 };
        assert!(
            (time - nominal).abs() < tol,
            "beat {i}: {time} vs {nominal}"
        );
    }
    assert!(rig.diags_with(DiagCode::ClockLost).is_empty());
}

#[test]
fn clock_loss_freewheels_at_the_last_tempo_with_clock_lost() {
    let mut rig = started();
    let t0 = 0.01;
    let mut pulses = Pulses::new(t0, 120.0, 0.0005);
    drive(&mut rig, &mut pulses, 2.0);
    assert!(!rig.rt.midi_clock().is_lost());
    // The cable goes dead for one second.
    let sent_before = rig.ring_events();
    rig.run_to(3.0);
    pulses.skip_to(3.0);
    let lost = rig.diags_with(DiagCode::ClockLost);
    assert_eq!(lost.len(), 1, "one warning per loss");
    assert!(rig.rt.midi_clock().is_lost());
    let lost_at = rig
        .ticks
        .iter()
        .position(|t| t.diags.iter().any(|d| d.code == DiagCode::ClockLost))
        .unwrap();
    #[allow(clippy::cast_precision_loss)]
    let lost_time = lost_at as f64 * 0.01;
    // The last pulse arrived just before 2.0 s.
    assert!(
        (2.48..2.53).contains(&lost_time),
        "after 500 ms: {lost_time}"
    );
    assert!(
        rig.ring_events() > sent_before,
        "freewheeling keeps playing"
    );
    // Pulses return: re-synced, positions still exact and on time.
    drive(&mut rig, &mut pulses, 6.0);
    assert!(!rig.rt.midi_clock().is_lost());
    assert_eq!(rig.diags_with(DiagCode::ClockLost).len(), 1);
    let got = beats(&rig.rt.telemetry());
    for (i, (beat, time)) in got.iter().enumerate() {
        assert_eq!(*beat, Ratio64::from_int(i64::try_from(i).unwrap()));
        #[allow(clippy::cast_precision_loss)]
        let nominal = t0 + i as f64 * 0.5;
        assert!((time - nominal).abs() < 0.004, "beat {i}: {time}");
    }
    assert!(got.len() >= 11);
}

#[test]
fn use_bpm_under_midi_is_clock_external() {
    let mut rig = MidiRig::new();
    rig.run("use-clock :midi");
    let rep = rig.run("use-bpm 140");
    assert!(
        rep.diags.iter().any(|d| d.code == DiagCode::ClockExternal),
        "{:?}",
        rep.diags
    );
    assert_eq!(rig.rt.clock().tempo().bpm, Ratio64::from_int(120));
    rig.run("use-clock :internal");
    assert!(!rig.rt.midi_clock().is_slave());
    assert_eq!(rig.rt.clock().source(), ClockSource::Internal);
    let rep = rig.run("use-bpm 140");
    assert!(rep.diags.is_empty(), "{:?}", rep.diags);
    assert_eq!(rig.rt.clock().tempo().bpm, Ratio64::from_int(140));
}
