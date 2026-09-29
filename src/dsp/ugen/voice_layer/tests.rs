use super::*;

fn close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual:?}, expected={expected:?}, tolerance={tolerance:?}"
    );
}

#[test]
fn decay_terms_clamp_and_replace_non_finite_inputs() {
    let at_zero = decay_terms(0.0, 0.0);
    close(at_zero.0, 0.05, 1e-9);
    close(at_zero.1, 0.005 - 0.05, 1e-9);

    let at_one = decay_terms(1.0, 1.0);
    close(at_one.0, 0.05 / 256.0, 1e-9);
    close(at_one.1, 0.005 * 2.0_f32.powi(-5) - 0.05 / 256.0, 1e-9);

    assert_eq!(decay_terms(f32::NAN, 0.5), decay_terms(0.5, 0.5));
    assert_eq!(decay_terms(0.5, f32::INFINITY), decay_terms(0.5, 0.5));
}

#[test]
fn level_and_amount_shaping_match_the_contract() {
    close(compress_level(0.0), 0.0, 0.0);
    close(compress_level(1.0), 1.0, 0.0);
    close(compress_level(0.5), 0.8125, 1e-6);
    close(compress_level(-1.0), 0.0, 0.0);
    close(compress_level(f32::NAN), 0.0, 0.0);

    close(shaped_amount(0.0), 0.0, 0.0);
    close(shaped_amount(1.0), 0.9975, 1e-6);
    close(shaped_amount(-1.0), -0.9975, 1e-6);
    close(shaped_amount(0.03), 1.05 * 0.03 * 0.05, 1e-7);
}

#[test]
fn mode_attack_gain_and_clip_helpers_match_boundaries() {
    assert_eq!(LpgMode::from_control(0.0), LpgMode::Off);
    assert_eq!(LpgMode::from_control(0.49), LpgMode::Off);
    assert_eq!(LpgMode::from_control(f32::NAN), LpgMode::Off);
    assert_eq!(LpgMode::from_control(1.0), LpgMode::Ping);
    assert_eq!(LpgMode::from_control(1.6), LpgMode::Level);
    assert_eq!(LpgMode::from_control(2.0), LpgMode::Level);

    close(ping_attack(440.0), 24.0 * 440.0 / 48_000.0, 0.0);
    close(ping_attack(f32::NAN), 24.0 * 440.0 / 48_000.0, 0.0);
    assert_eq!(post_gain(-0.7), 1.0);
    assert_eq!(post_gain(0.7), 0.7);
    assert_eq!(clip_unit(f32::NAN), 0.0);
    assert_eq!(clip_unit(f32::INFINITY), 1.0);
    assert_eq!(clip_unit(f32::NEG_INFINITY), -1.0);
}

#[test]
fn control_clock_tracks_reference_and_fractional_rates() {
    let mut clock = ControlClock { carry: 0.0 };
    assert!((0..1000).all(|_| clock.next_len(48_000.0) == 12));

    let mut clock = ControlClock { carry: 0.0 };
    assert!((0..1000).all(|_| clock.next_len(96_000.0) == 24));

    let mut clock = ControlClock { carry: 0.0 };
    let lengths: Vec<_> = (0..4000).map(|_| clock.next_len(44_100.0)).collect();
    assert!(lengths.iter().all(|&len| len == 11 || len == 12));
    assert!((lengths.iter().sum::<usize>() as f32 - 44_100.0).abs() <= 1.0);
}

#[test]
fn decay_envelope_triggers_and_follows_short_decay() {
    let mut envelope = DecayEnvelope { value: 0.25 };
    envelope.trigger();
    assert_eq!(envelope.value(), 1.0);
    envelope.process(0.1);
    close(envelope.value(), 0.8, 1e-7);
}

#[test]
fn ping_reaches_full_opening_then_decays_to_done() {
    let (short, tail) = decay_terms(0.5, 0.5);
    let mut envelope = VactrolEnvelope::new();
    assert_eq!(envelope.state, 0.0);
    assert_eq!(envelope.gain, 1.0);
    assert_eq!(envelope.frequency, 0.5);
    assert_eq!(envelope.hf_bleed, 0.0);
    envelope.trigger();
    for _ in 0..3 {
        envelope.process_ping(0.25, short, tail, 0.5);
    }
    assert_eq!(envelope.state, 0.75);
    assert!(envelope.ramp_up);
    // The peak block clears ramp_up before the level is chosen, so the first
    // decay step (coefficient short_decay because s^4 == 1) runs in that block.
    envelope.process_ping(0.25, short, tail, 0.5);
    assert!(!envelope.ramp_up);
    assert_eq!(envelope.state, 1.0 - short);
    assert_eq!(envelope.gain, envelope.state);

    let mut prior = envelope.state;
    for _ in 0..20_000 {
        envelope.process_ping(0.25, short, tail, 0.5);
        assert!(envelope.state <= prior);
        prior = envelope.state;
        if envelope.is_done() {
            break;
        }
    }
    assert!(envelope.is_done());
    assert!(envelope.state < DONE_FLOOR);
}

#[test]
fn level_envelope_converges_and_decay_control_changes_release_time() {
    let (short, tail) = decay_terms(0.5, 0.5);
    let mut envelope = VactrolEnvelope::new();
    for _ in 0..50 {
        envelope.process_lp(0.8, short, tail, 0.5);
    }
    close(envelope.state, 0.8, 1e-4);

    let mut faster = envelope;
    let mut slower = envelope;
    let (fast_short, fast_tail) = decay_terms(0.0, 0.5);
    let (slow_short, slow_tail) = decay_terms(1.0, 0.5);
    for _ in 0..100 {
        faster.process_lp(0.0, fast_short, fast_tail, 0.5);
        slower.process_lp(0.0, slow_short, slow_tail, 0.5);
    }
    assert!(slower.state > faster.state);
}

#[test]
fn low_pass_gate_settles_bleeds_and_is_partition_invariant() {
    let mut gate = LowPassGate {
        prev_gain: 1.0,
        ..LowPassGate::default()
    };
    gate.begin(1.0, 0.3, 0.0, 12, 48_000.0);
    let mut output = 0.0;
    for _ in 0..600 {
        output = gate.tick(1.0);
    }
    close(output, 1.0, 1e-3);

    let mut bleed = LowPassGate {
        prev_gain: 0.75,
        ..LowPassGate::default()
    };
    bleed.begin(0.75, 0.3, 1.0, 12, 48_000.0);
    for _ in 0..12 {
        let x = 0.25;
        assert_eq!(bleed.tick(x), x * 0.75);
    }

    let mut whole = LowPassGate::default();
    let mut split = whole;
    whole.begin(0.9, 0.13, 0.2, 12, 48_000.0);
    split.begin(0.9, 0.13, 0.2, 12, 48_000.0);
    let input = [
        0.2, -0.3, 0.6, 0.1, -0.7, 0.4, 0.3, -0.2, 0.9, 0.0, 0.5, -0.1,
    ];
    let whole_out: Vec<_> = input.iter().map(|&x| whole.tick(x)).collect();
    let split_out: Vec<_> = input[..6]
        .iter()
        .chain(&input[6..])
        .map(|&x| split.tick(x))
        .collect();
    assert_eq!(whole_out, split_out);
    assert_eq!(whole, split);
}

#[test]
fn limiter_has_reference_gain_bounded_output_and_rate_consistent_attack() {
    let mut quiet = PostLimiter::new();
    close(quiet.process(1.0, 48_000.0, 0.1), 0.08, 1e-6);

    let mut first_peak = PostLimiter::new();
    first_peak.process(1.0, 48_000.0, 4.0);
    assert_eq!(first_peak.peak, 0.5 + 0.05 * 3.5);

    let mut limiter = PostLimiter::new();
    let mut output = 0.0;
    for _ in 0..20_000 {
        output = limiter.process(1.0, 48_000.0, 4.0);
    }
    assert!(output.abs() <= 0.801);

    fn half_way_samples(sr: f32) -> usize {
        let mut limiter = PostLimiter::new();
        for sample in 1..1000 {
            limiter.process(1.0, sr, 4.0);
            if limiter.peak >= 2.25 {
                return sample;
            }
        }
        panic!("limiter did not reach its half-way peak");
    }
    let at_48 = half_way_samples(48_000.0) as f32 / 48_000.0;
    let at_96 = half_way_samples(96_000.0) as f32 / 96_000.0;
    assert!((at_48 - at_96).abs() <= 0.001);
}

#[test]
fn every_state_round_trips_exactly_and_reports_its_serialized_size() {
    macro_rules! round_trip {
        ($type:ty, $state:expr, $expected:expr) => {{
            let state = $state;
            let mut saved = vec![0.0; <$type>::FLOATS];
            state.store(&mut saved);
            assert_eq!(<$type>::FLOATS, $expected);
            assert_eq!(state, <$type>::load(&saved));
            assert_eq!(saved.len(), <$type>::FLOATS);
        }};
    }

    round_trip!(ControlClock, ControlClock { carry: 0.375 }, 1);
    round_trip!(DecayEnvelope, DecayEnvelope { value: 0.625 }, 1);
    round_trip!(
        VactrolEnvelope,
        VactrolEnvelope {
            state: 0.1,
            gain: 0.2,
            frequency: 0.3,
            hf_bleed: 0.4,
            ramp_up: true,
        },
        5
    );
    round_trip!(
        LowPassGate,
        LowPassGate {
            value: 0.1,
            increment: 0.2,
            g: 0.3,
            h: 0.4,
            bleed: 0.5,
            s1: 0.6,
            s2: 0.7,
            prev_gain: 0.8,
        },
        8
    );
    round_trip!(PostLimiter, PostLimiter { peak: 0.75 }, 1);
}
