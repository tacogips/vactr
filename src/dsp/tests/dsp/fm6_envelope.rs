use crate::dsp::ugen::fm::envelope::{log_to_amp, Eg, EG_BLOCK, EG_FLOATS, FULL_SCALE_LEVEL};
use crate::dsp::ugen::fm::patch::{op, op_base, Fm6Patch};
use crate::dsp::ugen::fm::scaling::{
    feedback_gain, midi_key, op_freq_hz, op_outlevel, op_rate_scaling, scale_level,
    scale_out_level, scale_rate, scale_velocity, transpose_hz, velocity_midi,
};

const FULL_SCALE_OUTLEVEL: i32 = 127 << 5;

#[test]
fn fm6_eg_settles_to_l3_while_held() {
    let mut eg = Eg::new([99; 4], [99, 80, 60, 0], FULL_SCALE_OUTLEVEL, 0, 48_000.0);
    let mut level = 0;
    for _ in 0..(2 * 48_000 / EG_BLOCK) {
        level = eg.tick();
    }
    let l3 = (scale_out_level(60) >> 1) * 64 + FULL_SCALE_OUTLEVEL - 4_256;
    assert_eq!(level, l3 << 16);
}

#[test]
fn fm6_eg_releases_to_l4_after_key_off() {
    let mut eg = Eg::new([99; 4], [99, 90, 80, 0], FULL_SCALE_OUTLEVEL, 0, 48_000.0);
    for _ in 0..100 {
        eg.tick();
    }
    assert!(!eg.released_and_silent());
    eg.key_off();
    for _ in 0..(2 * 48_000 / EG_BLOCK) {
        if eg.released_and_silent() {
            break;
        }
        eg.tick();
    }
    assert!(eg.released_and_silent());
}

#[test]
fn fm6_eg_higher_rate_is_shorter() {
    let mut counts = [0; 4];
    for (index, rate) in [25, 50, 75, 99].into_iter().enumerate() {
        let mut eg = Eg::new(
            [rate, 0, 0, 0],
            [99, 0, 0, 0],
            FULL_SCALE_OUTLEVEL,
            0,
            48_000.0,
        );
        while eg.tick() < FULL_SCALE_LEVEL && counts[index] < 3_000 {
            counts[index] += 1;
        }
    }
    assert!(counts.windows(2).all(|pair| pair[0] > pair[1]));
}

#[test]
fn fm6_eg_timing_matches_msfa_formula() {
    // msfa qrate=(rate*41)>>6 and inc=(4+(qrate&3))<<(8+(qrate>>2));
    // iterating its jump-to-1716 and exponential approach to level 15 gives
    // 1902, 118, 7 and 1 blocks for rates 25, 50, 75 and 99 at 48 kHz.
    let cases: [(u8, i32); 4] = [(25, 1_902), (50, 118), (75, 7), (99, 1)];
    for (rate, expected_ticks) in cases {
        let mut eg = Eg::new(
            [rate, 0, 0, 0],
            [99, 0, 0, 0],
            FULL_SCALE_OUTLEVEL,
            0,
            48_000.0,
        );
        let ticks = (1..=expected_ticks + 1)
            .find(|_| eg.tick() == FULL_SCALE_LEVEL)
            .unwrap_or(expected_ticks + 2);
        assert!(
            (ticks - expected_ticks).abs() <= 1,
            "rate {rate}: {ticks} ticks"
        );
    }
    let mut rate_zero = Eg::new([0; 4], [99, 0, 0, 0], FULL_SCALE_OUTLEVEL, 0, 48_000.0);
    for _ in 0..1_000 {
        rate_zero.tick();
    }
    assert_ne!(rate_zero.tick(), FULL_SCALE_LEVEL);
}

#[test]
fn fm6_eg_store_load_round_trips_mid_segment() {
    let mut eg = Eg::new([38, 71, 22, 99], [90, 75, 45, 12], 3_900, 8, 48_000.0);
    for _ in 0..17 {
        eg.tick();
    }
    let mut state = [0.0; EG_FLOATS];
    eg.store(&mut state);
    assert_eq!(Eg::load(&state), eg);
    assert_eq!(EG_FLOATS, state.len());
}

#[test]
fn fm6_eg_sample_rate_independent() {
    let rates = [50, 70, 80, 99];
    let levels = [99, 70, 40, 0];
    let mut seconds = [0.0_f32; 3];
    for (index, sample_rate) in [44_100.0, 48_000.0, 96_000.0].into_iter().enumerate() {
        let mut eg = Eg::new(rates, levels, FULL_SCALE_OUTLEVEL, 0, sample_rate);
        let target = ((scale_out_level(40) >> 1) * 64 + FULL_SCALE_OUTLEVEL - 4_256) << 16;
        let mut frames = 0;
        while eg.tick() != target && frames < 3 * sample_rate as usize {
            frames += EG_BLOCK;
        }
        seconds[index] = frames as f32 / sample_rate;
    }
    assert!((seconds[0] - seconds[1]).abs() <= EG_BLOCK as f32 / 44_100.0);
    assert!((seconds[1] - seconds[2]).abs() <= EG_BLOCK as f32 / 48_000.0);
}

#[test]
fn fm6_scale_level_curves() {
    for curve in 0..=3 {
        assert_eq!(scale_level(60, 60, 0, 0, curve, curve), 0);
        let left: Vec<_> = (0..=60)
            .step_by(3)
            .map(|key| scale_level(key, 60, 80, 0, curve, 3))
            .collect();
        let right: Vec<_> = (77..=127)
            .step_by(3)
            .map(|key| scale_level(key, 60, 0, 80, 3, curve))
            .collect();
        if curve < 2 {
            assert!(left.windows(2).all(|pair| pair[0] <= pair[1]));
            assert!(right.windows(2).all(|pair| pair[0] >= pair[1]));
        } else {
            assert!(left.windows(2).all(|pair| pair[0] >= pair[1]));
            assert!(right.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }
}

#[test]
fn fm6_scale_rate_increases_with_key() {
    let values: Vec<_> = (0..=127).map(|key| scale_rate(key, 7)).collect();
    assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!((0..=127).all(|key| scale_rate(key, 0) == 0));
}

#[test]
fn fm6_velocity_sensitivity_zero_is_flat() {
    let mut patch = Fm6Patch::EMPTY;
    let base = op_base(1);
    patch.params[base + op::OUTPUT_LEVEL] = 80;
    patch.params[base + op::VELOCITY_SENS] = 0;
    let quiet = op_outlevel(&patch, 1, 69, 0);
    assert_eq!(quiet, op_outlevel(&patch, 1, 69, 64));
    assert_eq!(quiet, op_outlevel(&patch, 1, 69, 127));
    patch.params[base + op::VELOCITY_SENS] = 7;
    assert!(op_outlevel(&patch, 1, 69, 127) > op_outlevel(&patch, 1, 69, 0));
}

#[test]
fn fm6_op_freq_modes() {
    assert_eq!(op_freq_hz(440.0, 0, 0, 0, 7), 220.0);
    assert_eq!(op_freq_hz(440.0, 0, 2, 50, 7), 1_320.0);
    assert_eq!(op_freq_hz(440.0, 1, 1, 0, 7), 10.0);
    assert_eq!(op_freq_hz(880.0, 1, 1, 0, 0), 10.0);
    let low = op_freq_hz(440.0, 0, 1, 0, 0) / 440.0;
    let center = op_freq_hz(440.0, 0, 1, 0, 7) / 440.0;
    let high = op_freq_hz(440.0, 0, 1, 0, 14) / 440.0;
    assert!(low < center && center < high);
    assert!((high / center - 1.0).abs() < 0.01);
}

#[test]
fn fm6_feedback_gain_doubles() {
    assert_eq!(feedback_gain(0), 0.0);
    for value in 1..7 {
        assert_eq!(feedback_gain(value + 1), feedback_gain(value) * 2.0);
    }
}

#[test]
fn fm6_log_to_amp_full_scale_is_one_and_monotonic() {
    assert_eq!(log_to_amp(FULL_SCALE_LEVEL), 1.0);
    assert!(log_to_amp(FULL_SCALE_LEVEL - (1 << 16)) < 1.0);
    assert!(log_to_amp(FULL_SCALE_LEVEL + (1 << 16)) > 1.0);
    assert!((0..100).all(|step| {
        let lower = log_to_amp(FULL_SCALE_LEVEL + step * (1 << 16));
        let upper = log_to_amp(FULL_SCALE_LEVEL + (step + 1) * (1 << 16));
        lower <= upper
    }));
}

#[test]
fn fm6_scaling_inputs_are_clamped_and_invalid_frequency_is_finite() {
    assert_eq!(midi_key(440.0), 69);
    assert_eq!(midi_key(0.0), 69);
    assert_eq!(velocity_midi(f32::NAN), 127);
    assert_eq!(velocity_midi(0.5), 64);
    assert_eq!(scale_out_level(255), scale_out_level(99));
    assert_eq!(
        scale_level(-100, 60, 99, 99, 3, 3),
        scale_level(0, 60, 99, 99, 3, 3)
    );
    assert_eq!(op_rate_scaling(&Fm6Patch::EMPTY, 0, 60), 0);
    assert_eq!(scale_velocity(64, 0), 0);
    assert!((op_freq_hz(f32::NAN, 0, 1, 0, 7)).is_finite());
    assert_eq!(transpose_hz(440.0, 24), 440.0);
}
