use crate::clock::{Clock, ClockSource, MidiClockSync, Tempo, PULSES_PER_BEAT};
use crate::value::ratio::Ratio64;
use crate::vm::fail::FailCode;

fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}

#[test]
fn cps_from_bpm_and_beats_per_cycle() {
    assert_eq!(Tempo::default().cps().unwrap(), r(1, 2));
    let t = Tempo::new(Ratio64::from_int(90), Ratio64::from_int(3)).unwrap();
    assert_eq!(t.cps().unwrap(), r(1, 2));
    let t = Tempo::new(Ratio64::from_int(140), Ratio64::from_int(4)).unwrap();
    assert_eq!(t.cps().unwrap(), r(7, 12));
    assert_eq!(t.beats_at(r(3, 2)).unwrap(), Ratio64::from_int(6));
    assert!((Tempo::default().cycle_seconds().unwrap() - 2.0).abs() < 1e-12);
    assert!(Tempo::new(Ratio64::ZERO, Ratio64::ONE).is_err());
    assert!(Tempo::new(Ratio64::ONE, Ratio64::from_int(-1)).is_err());
}

#[test]
fn tempo_change_anchors_keep_continuity() {
    let mut c = Clock::new(ClockSource::Internal, Tempo::default(), 10.0).unwrap();
    // 120 bpm, 4 beats per cycle: 2 s per cycle.
    assert!((c.to_host(Ratio64::from_int(2)) - 14.0).abs() < 1e-12);
    let fast = Tempo::new(Ratio64::from_int(240), Ratio64::from_int(4)).unwrap();
    c.set_tempo(fast, Ratio64::from_int(2)).unwrap();
    // Past positions keep their host times; the change point is continuous.
    assert!((c.to_host(Ratio64::ONE) - 12.0).abs() < 1e-12);
    assert!((c.to_host(Ratio64::from_int(2)) - 14.0).abs() < 1e-12);
    // After the change, 1 s per cycle.
    assert!((c.to_host(Ratio64::from_int(3)) - 15.0).abs() < 1e-12);
    assert!((c.to_host(r(5, 2)) - 14.5).abs() < 1e-12);
    assert!((c.to_cycles(15.0) - 3.0).abs() < 1e-12);
    assert!((c.to_cycles(12.0) - 1.0).abs() < 1e-12);
    assert_eq!(c.tempo(), fast);
    assert_eq!(c.anchor_count(), 2);
    assert!((c.epoch_host() - 10.0).abs() < 1e-12);
}

#[test]
fn slaved_clock_rejects_use_bpm_and_link_is_unavailable() {
    let mut c = Clock::new(ClockSource::MidiClock, Tempo::default(), 0.0).unwrap();
    let err = c.set_tempo(Tempo::default(), Ratio64::ZERO).unwrap_err();
    assert_eq!(err.code, FailCode::Type);
    assert_eq!(ClockSource::from_name("midi"), Some(ClockSource::MidiClock));
    assert_eq!(ClockSource::from_name("link"), Some(ClockSource::Link));
    assert_eq!(
        ClockSource::from_name("internal"),
        Some(ClockSource::Internal)
    );
    assert_eq!(ClockSource::from_name("nope"), None);
    assert!(!ClockSource::Link.is_available());
    assert!(ClockSource::MidiClock.is_available());
}

/// A deterministic jitter in [-1.5 ms, 1.5 ms].
fn jitter(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let unit = (*state >> 11) as f64 / (1u64 << 53) as f64;
    (unit - 0.5) * 0.003
}

#[test]
fn midi_anchor_keeps_logical_positions_exact_under_jitter() {
    let mut c = Clock::new(ClockSource::MidiClock, Tempo::default(), 0.0).unwrap();
    let mut sync = MidiClockSync::new(0.1);
    // Pulses before Start are ignored.
    assert_eq!(sync.pulse(&mut c, 0.5).unwrap(), Ratio64::ZERO);
    sync.start(&mut c, 1.0).unwrap();
    // External tempo 150 bpm: one pulse every 60 / 150 / 24 s.
    let period = 60.0 / 150.0 / PULSES_PER_BEAT as f64;
    let mut state = 7u64;
    let mut last_anchor = f64::MIN;
    for k in 1..=192i64 {
        let host = 1.0 + k as f64 * period + jitter(&mut state);
        let pos = sync.pulse(&mut c, host).unwrap();
        // Exactly k / 96 cycles (24 pulses per beat, 4 beats per cycle).
        assert_eq!(pos, r(k, 96));
        assert_eq!(c.pos(), pos);
        let anchor = c.to_host(pos);
        assert!(anchor > last_anchor, "anchors move forward");
        last_anchor = anchor;
    }
    assert_eq!(c.pos(), Ratio64::from_int(2));
    let bpm = sync.smoothed_bpm().unwrap();
    assert!((bpm - 150.0).abs() < 3.0, "smoothed bpm {bpm}");
    let tempo_bpm = c.tempo().bpm.to_f64();
    assert!((tempo_bpm - 150.0).abs() < 3.0);
    // Stop freezes, Continue resumes from the frozen position.
    sync.stop();
    assert!(!sync.is_running());
    assert_eq!(sync.pulse(&mut c, 100.0).unwrap(), Ratio64::from_int(2));
    sync.resume(&c);
    assert_eq!(sync.pulse(&mut c, 101.0).unwrap(), r(193, 96));
    // Start restarts at cycle 0.
    sync.start(&mut c, 200.0).unwrap();
    assert_eq!(c.pos(), Ratio64::ZERO);
    assert_eq!(sync.pulse(&mut c, 200.02).unwrap(), r(1, 96));
}
