//! Response, state, and stability tests for the BASS-10 filters.

use crate::dsp::effects::prim::Rng;
use crate::dsp::ugen::bass_voice::{diode, ladder};

const SR: f32 = 48_000.0;

fn gain_db(mut process: impl FnMut(f32) -> f32, frequency: f32) -> f32 {
    let frames = (0.75 * SR) as usize;
    let settle = (0.25 * SR) as usize;
    let mut input_square = 0.0;
    let mut output_square = 0.0;
    for frame in 0..frames {
        let phase = std::f32::consts::TAU * frequency * frame as f32 / SR;
        let input = 0.01 * phase.sin();
        let output = process(input);
        if frame >= settle {
            input_square += input * input;
            output_square += output * output;
        }
    }
    20.0 * (output_square / input_square).sqrt().log10()
}

fn ladder_gain(cutoff: f32, res: f32, frequency: f32) -> f32 {
    let coeffs = ladder::LadderCoeffs::new(cutoff, res, 0.0, SR);
    let mut filter = ladder::Ladder::default();
    gain_db(|sample| filter.process(sample, &coeffs), frequency)
}

fn diode_gain(cutoff: f32, res: f32, frequency: f32) -> f32 {
    let coeffs = diode::DiodeCoeffs::new(cutoff, res, 0.0, SR);
    let mut filter = diode::Diode::default();
    gain_db(|sample| filter.process(sample, &coeffs), frequency)
}

#[test]
fn bass_filters_ladder_response_is_lowpass() {
    let low = ladder_gain(1000.0, 0.0, 62.5);
    let upper = ladder_gain(1000.0, 0.0, 250.0);
    let stop = ladder_gain(1000.0, 0.0, 4000.0);
    assert!(
        (upper - low).abs() <= 3.0,
        "250 Hz {upper} dB, 62.5 Hz {low} dB"
    );
    assert!(stop <= low - 30.0, "4 kHz {stop} dB, 62.5 Hz {low} dB");
}

#[test]
fn bass_filters_diode_response_is_lowpass() {
    let low = diode_gain(1000.0, 0.0, 62.5);
    let upper = diode_gain(1000.0, 0.0, 250.0);
    let stop = diode_gain(1000.0, 0.0, 4000.0);
    let mut previous_hz = 50.0;
    let mut previous_db = diode_gain(1000.0, 0.0, previous_hz);
    let corner_hz = (2..=160).find_map(|step| {
        let frequency = step as f32 * 25.0;
        let gain = diode_gain(1000.0, 0.0, frequency);
        if gain <= low - 3.0 {
            let fraction = ((low - 3.0) - previous_db) / (gain - previous_db);
            Some(previous_hz + fraction * (frequency - previous_hz))
        } else {
            previous_hz = frequency;
            previous_db = gain;
            None
        }
    });
    assert!(
        (upper - low).abs() <= 3.0,
        "250 Hz {upper} dB, 62.5 Hz {low} dB"
    );
    assert!(stop <= low - 20.0, "4 kHz {stop} dB, 62.5 Hz {low} dB");
    let corner_hz = corner_hz.expect("diode -3 dB corner in scan range");
    assert!(
        (800.0..=1200.0).contains(&corner_hz),
        "measured diode corner {corner_hz} Hz for 1000 Hz nominal cutoff"
    );
    println!("measured diode -3 dB corner: {corner_hz:.1} Hz");
}

#[test]
fn bass_filters_ladder_resonance_is_monotonic_and_exceeds_12_db() {
    let gains = [0.0, 0.25, 0.5, 0.75, 1.0].map(|res| ladder_gain(1000.0, res, 1000.0));
    assert!(gains.windows(2).all(|pair| pair[1] > pair[0]), "{gains:?}");
    assert!(gains[4] >= gains[0] + 12.0, "{gains:?}");
}

#[test]
fn bass_filters_diode_resonance_is_monotonic_and_exceeds_12_db() {
    let gains = [0.0, 0.25, 0.5, 0.75, 1.0].map(|res| diode_gain(1000.0, res, 1000.0));
    assert!(gains.windows(2).all(|pair| pair[1] > pair[0]), "{gains:?}");
    assert!(gains[4] >= gains[0] + 12.0, "{gains:?}");
}

#[test]
fn bass_filters_extreme_noise_is_finite_and_bounded_for_two_seconds() {
    for cutoff in [20.0, 20_000.0] {
        for res in [0.0, 1.0] {
            for drive in [0.0, 1.0] {
                let mut rng = Rng::new(0xB455_0010);
                let mut transistor = ladder::Ladder::default();
                let mut acid = diode::Diode::default();
                let ladder_coeffs = ladder::LadderCoeffs::new(cutoff, res, drive, SR);
                let diode_coeffs = diode::DiodeCoeffs::new(cutoff, res, drive, SR);
                for _ in 0..(2 * SR as usize) {
                    let input = 4.0 * rng.bipolar();
                    let a = transistor.process(input, &ladder_coeffs);
                    let b = acid.process(input, &diode_coeffs);
                    assert!(
                        a.is_finite() && a.abs() < 16.0,
                        "ladder {a} at {cutoff}/{res}/{drive}"
                    );
                    assert!(
                        b.is_finite() && b.abs() < 16.0,
                        "diode {b} at {cutoff}/{res}/{drive}"
                    );
                }
            }
        }
    }
}

#[test]
fn bass_filters_flush_zeros_tiny_states_after_silence() {
    let mut rng = Rng::new(17);
    let mut transistor = ladder::Ladder::default();
    let mut acid = diode::Diode::default();
    let ladder_coeffs = ladder::LadderCoeffs::new(100.0, 0.0, 0.3, SR);
    let diode_coeffs = diode::DiodeCoeffs::new(100.0, 0.0, 0.3, SR);
    for _ in 0..(SR as usize / 2) {
        let input = rng.bipolar();
        let _ = transistor.process(input, &ladder_coeffs);
        let _ = acid.process(input, &diode_coeffs);
    }
    for _ in 0..(2 * SR as usize) {
        let _ = transistor.process(0.0, &ladder_coeffs);
        let _ = acid.process(0.0, &diode_coeffs);
    }
    transistor.flush();
    acid.flush();
    let mut ladder_state = [1.0; ladder::Ladder::FLOATS];
    let mut diode_state = [1.0; diode::Diode::FLOATS];
    transistor.store(&mut ladder_state);
    acid.store(&mut diode_state);
    assert_eq!(ladder_state, [0.0; ladder::Ladder::FLOATS]);
    assert_eq!(diode_state, [0.0; diode::Diode::FLOATS]);
}

#[test]
fn bass_filters_nan_input_resets_both_filters() {
    let ladder_coeffs = ladder::LadderCoeffs::new(1000.0, 0.5, 0.2, SR);
    let diode_coeffs = diode::DiodeCoeffs::new(1000.0, 0.5, 0.2, SR);
    let mut transistor = ladder::Ladder::default();
    let mut acid = diode::Diode::default();
    assert_eq!(transistor.process(f32::NAN, &ladder_coeffs), 0.0);
    assert_eq!(acid.process(f32::NAN, &diode_coeffs), 0.0);
    assert!(transistor.process(0.1, &ladder_coeffs).is_finite());
    assert!(acid.process(0.1, &diode_coeffs).is_finite());
}

#[test]
fn bass_filters_memory_roundtrip_and_render_are_deterministic() {
    let ladder_coeffs = ladder::LadderCoeffs::new(1200.0, 0.65, 0.3, SR);
    let diode_coeffs = diode::DiodeCoeffs::new(1200.0, 0.65, 0.3, SR);
    let mut transistor = ladder::Ladder::default();
    let mut acid = diode::Diode::default();
    for input in [0.1, -0.7, 0.3, 0.0, 0.9] {
        let _ = transistor.process(input, &ladder_coeffs);
        let _ = acid.process(input, &diode_coeffs);
    }
    let mut ladder_memory = [0.0; ladder::Ladder::FLOATS];
    let mut diode_memory = [0.0; diode::Diode::FLOATS];
    transistor.store(&mut ladder_memory);
    acid.store(&mut diode_memory);
    assert_eq!(ladder::Ladder::load(&ladder_memory), transistor);
    assert_eq!(diode::Diode::load(&diode_memory), acid);

    let render_ladder = || {
        let mut filter = ladder::Ladder::default();
        (0..1024)
            .map(|i| filter.process((i as f32 * 0.17).sin(), &ladder_coeffs))
            .map(f32::to_bits)
            .collect::<Vec<_>>()
    };
    let render_diode = || {
        let mut filter = diode::Diode::default();
        (0..1024)
            .map(|i| filter.process((i as f32 * 0.17).sin(), &diode_coeffs))
            .map(f32::to_bits)
            .collect::<Vec<_>>()
    };
    assert_eq!(render_ladder(), render_ladder());
    assert_eq!(render_diode(), render_diode());
}

#[test]
fn bass_filters_drive_does_not_induce_self_oscillation() {
    let ladder_coeffs = ladder::LadderCoeffs::new(180.0, 0.9, 1.0, SR);
    let diode_coeffs = diode::DiodeCoeffs::new(320.0, 0.72, 1.0, SR);
    let mut transistor = ladder::Ladder::default();
    let mut acid = diode::Diode::default();
    let total = 2 * SR as usize;
    let tail_start = total - SR as usize / 2;
    let mut ladder_square = 0.0_f64;
    let mut diode_square = 0.0_f64;
    for n in 0..total {
        let input = if n < SR as usize {
            0.8 * (2.0 * (n as f32 * 55.0 / SR).fract() - 1.0)
        } else {
            0.0
        };
        let a = transistor.process(input, &ladder_coeffs);
        let b = acid.process(input, &diode_coeffs);
        if n >= tail_start {
            ladder_square += f64::from(a * a);
            diode_square += f64::from(b * b);
        }
    }
    let tail_len = (total - tail_start) as f64;
    for (case, square) in [
        ("ladder 180 Hz res 0.9 drive 1.0", ladder_square),
        ("diode 320 Hz res 0.72 drive 1.0", diode_square),
    ] {
        let rms = (square / tail_len).sqrt();
        assert!(rms < 1e-4, "{case}: tail RMS {rms}");
    }
}
