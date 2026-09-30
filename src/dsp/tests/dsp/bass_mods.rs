use crate::dsp::ugen::bass_voice::mods::{
    accent, clamp_cps, cutoff_octaves, gate_samples, initial_phase, rate_hz, settle_coeff,
    AccentOut, AmpEnv, AmpParams, FilterEnv, Glide, Lfo, LfoShape, ACCENT_GAIN,
};

#[test]
fn bass_mods_gate_length_uses_sixteenth_steps() {
    assert_eq!(gate_samples(1.0, 0.5, 48_000.0), 6_000);
    assert_eq!(gate_samples(0.55, 0.5625, 48_000.0), 2_933);
    assert_eq!(gate_samples(1.0, 0.0, 48_000.0), 100_000);
    assert_eq!(gate_samples(1_000.0, 0.5, 48_000.0), 384_000);
}

#[test]
fn bass_mods_timing_helpers_clamp_invalid_values() {
    assert_eq!(clamp_cps(f32::NAN), 0.5);
    assert_eq!(clamp_cps(0.0), 0.03);
    assert_eq!(clamp_cps(f32::INFINITY), 0.5);
    assert_eq!(gate_samples(f32::NAN, 0.5, 48_000.0), 6_000);
    assert!(settle_coeff(f32::NAN, 48_000.0).is_finite());
}

#[test]
fn bass_mods_settle_coefficient_reaches_one_percent() {
    let coeff = settle_coeff(0.1, 48_000.0);
    let after_100_ms = coeff.powi(4_800);
    assert!((after_100_ms - 0.01).abs() < 1.0e-5);
}

#[test]
fn bass_mods_amp_envelope_attacks_decays_and_releases() {
    let params = AmpParams {
        attack: 0.002,
        decay: 0.1,
        sustain: 0.5,
        release: 0.05,
    };
    let mut env = AmpEnv::default();
    env.start(false);
    let mut level = 0.0;
    for _ in 0..96 {
        level = env.next(&params, false, 48_000.0);
    }
    assert!((level - 1.0).abs() < 1.0e-6);
    for _ in 0..4_800 {
        level = env.next(&params, false, 48_000.0);
    }
    assert!((level - 0.5).abs() <= 0.0051);
    env.release();
    for _ in 0..(params.release * 48_000.0) as usize + 2 {
        level = env.next(&params, false, 48_000.0);
    }
    assert!(env.done());
    assert_eq!(level, 0.0);
}

#[test]
fn bass_mods_legato_amp_ramps_to_sustain_without_overshoot() {
    let params = AmpParams {
        attack: 0.1,
        decay: 1.0,
        sustain: 0.8,
        release: 0.1,
    };
    let mut env = AmpEnv::default();
    env.start(true);
    let mut level = 0.0;
    for _ in 0..97 {
        level = env.next(&params, true, 48_000.0);
        assert!(level <= params.sustain);
    }
    assert!(level >= 0.79);
    assert_eq!(env.stage, 1.0);
}

#[test]
fn bass_mods_filter_envelope_starts_and_decays_to_one_percent() {
    let mut env = FilterEnv::default();
    env.start(false);
    assert_eq!(env.value, 1.0);
    for _ in 0..9_600 {
        env.next(0.2, 48_000.0);
    }
    assert!((0.0095..=0.0105).contains(&env.value));
    env.start(true);
    assert_eq!(env.value, 0.0);
}

#[test]
fn bass_mods_accent_maps_octaves_decay_and_gain() {
    assert_eq!(
        accent(0.0, 1.0, 0.5),
        AccentOut {
            oct: 0.0,
            decay: 0.5,
            gain: 1.0,
        }
    );
    let full = accent(1.0, 1.0, 0.5);
    assert_eq!(full.oct, 2.0);
    assert!((full.decay - 0.2).abs() < 1.0e-6);
    assert_eq!(full.gain, 1.4);
    let low_res = accent(1.0, 0.0, 0.1);
    assert_eq!(low_res.oct, 1.0);
    assert_eq!(low_res.decay, 0.1);
    assert_eq!(accent(f32::NAN, f32::NAN, f32::NAN).gain, 1.0);
    assert_eq!(ACCENT_GAIN, 0.4);
}

#[test]
fn bass_mods_glide_settles_to_zero_and_clamps_start() {
    let mut glide = Glide::default();
    glide.start(-12.0);
    assert_eq!(glide.next(0.06, 48_000.0), -12.0);
    let mut value = 0.0;
    for _ in 0..2_880 {
        value = glide.next(0.06, 48_000.0);
    }
    assert!(value.abs() <= 0.12);
    glide.start(100.0);
    assert_eq!(glide.offset, 24.0);
    glide.start(f32::NAN);
    assert_eq!(glide.offset, 0.0);
}

#[test]
fn bass_mods_lfo_shape_order_matches_control_enum() {
    assert_eq!(LfoShape::from_index(0), LfoShape::Sine);
    assert_eq!(LfoShape::from_index(1), LfoShape::Tri);
    assert_eq!(LfoShape::from_index(2), LfoShape::Saw);
    assert_eq!(LfoShape::from_index(3), LfoShape::Ramp);
    assert_eq!(LfoShape::from_index(4), LfoShape::Square);
    assert_eq!(LfoShape::from_index(5), LfoShape::Random);
}

#[test]
fn bass_mods_synced_lfo_periods_match_common_divisions() {
    for cycles_per_beat in [1.0, 2.0, 4.0, 8.0, 3.0, 6.0, 12.0] {
        let hz = rate_hz(cycles_per_beat, true, 0.5625);
        let expected = (48_000.0 / hz).round() as usize;
        let mut lfo = Lfo::default();
        lfo.start(0.0, LfoShape::Saw, 10);
        let mut previous_wrap = None;
        let mut measured = Vec::new();
        for sample in 0..=expected * 4 + 2 {
            let before = lfo.cycles(48_000.0);
            lfo.next(LfoShape::Saw, hz, 10, 48_000.0);
            if lfo.cycles(48_000.0) > before {
                if let Some(previous) = previous_wrap.replace(sample) {
                    measured.push(sample - previous);
                }
            }
        }
        assert!(
            measured.len() >= 3,
            "rate {cycles_per_beat} wrapped too few times"
        );
        assert!(
            measured.iter().all(|period| period.abs_diff(expected) <= 1),
            "rate {cycles_per_beat}: {measured:?}, expected {expected}"
        );
    }
}

#[test]
fn bass_mods_free_lfo_period_and_rate_are_independent_of_tempo() {
    assert_eq!(rate_hz(3.2, false, 0.5), 3.2);
    let mut lfo = Lfo::default();
    lfo.start(0.0, LfoShape::Saw, 1);
    let mut wrap: Option<usize> = None;
    for sample in 0..=15_002 {
        let before = lfo.cycles(48_000.0);
        lfo.next(LfoShape::Saw, 3.2, 1, 48_000.0);
        if lfo.cycles(48_000.0) > before {
            wrap = Some(sample + 1);
            break;
        }
    }
    assert!(
        wrap.unwrap_or_default().abs_diff(15_000) <= 1,
        "observed period: {wrap:?}"
    );
}

#[test]
fn bass_mods_synced_lfo_stays_drift_free_over_long_playback() {
    let mut lfo = Lfo::default();
    lfo.start(0.0, LfoShape::Saw, 1);
    for _ in 0..5_760_000 {
        lfo.next(LfoShape::Saw, rate_hz(4.0, true, 0.5625), 1, 48_000.0);
    }
    assert_eq!(lfo.cycles(48_000.0), 270.0);
    assert!(lfo.phase(48_000.0).min(1.0 - lfo.phase(48_000.0)) <= 1.0e-6);
}

#[test]
fn bass_mods_lfo_split_count_preserves_phase_and_carries() {
    let mut lfo = Lfo::load(&[0.1, 0.0, 12_345.0, 3.0, 2.25, 0.0]);
    let count = 3.0_f64 * Lfo::COUNT_SPLIT as f64 + 12_345.0;
    let expected = (0.1_f64 + count * 2.25 / 48_000.0).rem_euclid(1.0) as f32;
    assert!((lfo.phase(48_000.0) - expected).abs() < 1.0e-6);

    for _ in 0..(Lfo::COUNT_SPLIT as usize - 12_345) {
        lfo.next(LfoShape::Saw, 2.25, 1, 48_000.0);
    }
    assert_eq!(lfo.count_hi, 4.0);
    assert_eq!(lfo.count_lo, 0.0);
}

#[test]
fn bass_mods_lfo_rate_change_reanchors_without_phase_jump() {
    let mut lfo = Lfo::default();
    lfo.start(0.23, LfoShape::Saw, 1);
    for _ in 0..1_000 {
        lfo.next(LfoShape::Saw, 2.0, 1, 48_000.0);
    }
    let before = lfo.phase(48_000.0);
    let previous_cycles = lfo.cycles(48_000.0);
    lfo.next(LfoShape::Saw, 8.0, 1, 48_000.0);
    let expected = (before as f64 + 8.0 / 48_000.0).rem_euclid(1.0) as f32;
    assert!((lfo.phase(48_000.0) - expected).abs() < 1.0e-7);
    assert!(lfo.cycles(48_000.0) >= previous_cycles);
    for _ in 0..8 {
        let before = lfo.phase(48_000.0);
        lfo.next(LfoShape::Saw, 8.0, 1, 48_000.0);
        let expected = (before as f64 + 8.0 / 48_000.0).rem_euclid(1.0) as f32;
        assert!((lfo.phase(48_000.0) - expected).abs() < 1.0e-7);
    }
}

#[test]
fn bass_mods_lfo_initial_phase_handles_transport_offset_in_f64() {
    assert_eq!(initial_phase(false, 0.0, 2.0, 10.25), 0.5);
    assert_eq!(initial_phase(true, 1.25, 2.0, 10.25), 0.25);
    let phase = initial_phase(false, 0.0, 50.0, 3_599.99);
    assert!(phase.is_finite() && (0.0..1.0).contains(&phase));
}

#[test]
fn bass_mods_random_lfo_is_seeded_and_sample_and_hold() {
    let mut first = Lfo::default();
    let mut same = Lfo::default();
    let mut other = Lfo::default();
    first.start(0.0, LfoShape::Random, 1);
    same.start(0.0, LfoShape::Random, 1);
    other.start(0.0, LfoShape::Random, 2);
    let mut differs = false;
    for _ in 0..8 {
        let a = first.next(LfoShape::Random, 1.0, 1, 48_000.0);
        let b = same.next(LfoShape::Random, 1.0, 1, 48_000.0);
        let c = other.next(LfoShape::Random, 1.0, 2, 48_000.0);
        assert_eq!(a, b);
        assert!((0.0..=1.0).contains(&a));
        differs |= a != c;
    }
    assert!(differs);
}

#[test]
fn bass_mods_square_lfo_smoothing_prevents_click_sized_steps() {
    let mut lfo = Lfo::default();
    lfo.start(0.0, LfoShape::Square, 1);
    let mut previous = lfo.smooth;
    let mut largest_change = 0.0_f32;
    for _ in 0..12_000 {
        let current = lfo.next(LfoShape::Square, 8.0, 1, 48_000.0);
        largest_change = largest_change.max((current - previous).abs());
        previous = current;
    }
    assert!(largest_change < 0.02, "largest step was {largest_change}");
}

#[test]
fn bass_mods_cutoff_octaves_map_signed_depth() {
    assert_eq!(cutoff_octaves(0.8, 1.0), 4.0);
    assert_eq!(cutoff_octaves(-1.0, 1.0), -5.0);
    assert_eq!(cutoff_octaves(2.0, 2.0), 5.0);
}

#[test]
fn bass_mods_component_states_round_trip_through_float_memory() {
    let amp = AmpEnv {
        level: 0.35,
        stage: 2.0,
    };
    let mut amp_data = [0.0; AmpEnv::FLOATS];
    amp.store(&mut amp_data);
    assert_eq!(AmpEnv::load(&amp_data), amp);

    let filter = FilterEnv { value: 0.42 };
    let mut filter_data = [0.0; FilterEnv::FLOATS];
    filter.store(&mut filter_data);
    assert_eq!(FilterEnv::load(&filter_data), filter);

    let glide = Glide { offset: -11.5 };
    let mut glide_data = [0.0; Glide::FLOATS];
    glide.store(&mut glide_data);
    assert_eq!(Glide::load(&glide_data), glide);

    let lfo = Lfo {
        anchor_phase: 0.25,
        anchor_cycles: 123.0,
        count_lo: 7.0,
        count_hi: 2.0,
        rate: 1.25,
        smooth: 0.6,
    };
    let mut lfo_data = [0.0; Lfo::FLOATS];
    lfo.store(&mut lfo_data);
    assert_eq!(Lfo::load(&lfo_data), lfo);
}

#[test]
fn bass_mods_non_finite_parameters_do_not_escape_bounds() {
    let params = AmpParams {
        attack: f32::NAN,
        decay: f32::INFINITY,
        sustain: f32::NAN,
        release: f32::NAN,
    };
    let mut env = AmpEnv::default();
    env.start(false);
    for _ in 0..64 {
        assert!((0.0..=1.0).contains(&env.next(&params, false, f32::NAN)));
    }
    let mut lfo = Lfo::default();
    lfo.start(f32::NAN, LfoShape::Sine, 0);
    assert!((0.0..=1.0).contains(&lfo.next(LfoShape::Sine, f32::NAN, 0, f32::NAN)));
}
