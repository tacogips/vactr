//! Behavioral tests for the bass oscillators, feedback FM, and digital stages.

use crate::dsp::effects::prim::Rng;
use crate::dsp::offline::spectrum;
use crate::dsp::ugen::bass_voice::fm::{quantize, FmPair, Folder};
use crate::dsp::ugen::bass_voice::osc::{click_semis, detune_ratios, Osc, Wave};
use std::f32::consts::TAU;

const RATE: usize = 48_000;

fn render_wave(wave: Wave, frequency: f32, samples: usize) -> Vec<f32> {
    let mut osc = Osc::default();
    let inc = frequency / RATE as f32;
    (0..samples).map(|_| osc.next(wave, inc)).collect()
}

fn upward_crossings(samples: &[f32]) -> Vec<usize> {
    samples
        .windows(2)
        .enumerate()
        .filter_map(|(i, pair)| (pair[0] <= 0.0 && pair[1] > 0.0).then_some(i + 1))
        .collect()
}

fn alias_share(samples: &[f32], fundamental: f32) -> f32 {
    let mags = spectrum(samples, 4096);
    let bin_hz = RATE as f32 / 8192.0;
    let mut total = 0.0;
    let mut inharmonic = 0.0;
    for (bin, magnitude) in mags.iter().copied().enumerate().skip(1) {
        let frequency = bin as f32 * bin_hz;
        let power = magnitude * magnitude;
        total += power;
        let harmonic = (frequency / fundamental).round();
        if harmonic < 1.0
            || harmonic * fundamental > 24_000.0
            || (frequency - harmonic * fundamental).abs() > 2.0 * bin_hz
        {
            inharmonic += power;
        }
    }
    inharmonic / total
}

fn naive_wave(wave: Wave, frequency: f32, samples: usize) -> Vec<f32> {
    let increment = frequency / RATE as f32;
    let mut phase = 0.0_f32;
    (0..samples)
        .map(|_| {
            let sample = match wave {
                Wave::Saw => 2.0 * phase - 1.0,
                Wave::Tri => {
                    if phase < 0.5 {
                        -1.0 + 4.0 * phase
                    } else {
                        3.0 - 4.0 * phase
                    }
                }
                _ => 0.0,
            };
            phase = (phase + increment).rem_euclid(1.0);
            sample
        })
        .collect()
}

fn ideal_triangle(frequency: f32, samples: usize) -> Vec<f32> {
    let harmonics = (RATE as f32 / (2.0 * frequency)).floor() as usize;
    (0..samples)
        .map(|sample| {
            let phase = TAU as f64 * frequency as f64 * sample as f64 / RATE as f64;
            (1..=harmonics)
                .step_by(2)
                .map(|harmonic| {
                    let sign = if (harmonic - 1) / 2 % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let h = harmonic as f64;
                    sign * 8.0 / (std::f64::consts::PI.powi(2) * h * h) * (h * phase).sin()
                })
                .sum::<f64>() as f32
        })
        .collect()
}

fn centroid(samples: &[f32]) -> f32 {
    let mags = spectrum(samples, 4096);
    let mut weighted = 0.0;
    let mut sum = 0.0;
    for (bin, magnitude) in mags.iter().copied().enumerate().skip(1) {
        let hz = bin as f32 * RATE as f32 / 8192.0;
        weighted += hz * magnitude;
        sum += magnitude;
    }
    weighted / sum
}

#[test]
fn bass_sources_waveforms_are_finite_bounded_and_dc_free() {
    for wave in [Wave::Saw, Wave::Pulse, Wave::Square, Wave::Tri, Wave::Sine] {
        let samples = render_wave(wave, 110.0, RATE);
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        let peak = samples.iter().fold(0.0_f32, |value, x| value.max(x.abs()));
        assert!(samples.iter().all(|x| x.is_finite()), "{wave:?}");
        assert!(peak <= 1.2, "{wave:?} peak={peak}");
        assert!(mean.abs() < 0.02, "{wave:?} mean={mean}");
    }
}

#[test]
fn bass_sources_waveforms_have_110_hz_upward_crossing_period() {
    for wave in [Wave::Saw, Wave::Pulse, Wave::Square, Wave::Tri, Wave::Sine] {
        let samples = render_wave(wave, 110.0, RATE);
        let crossings = upward_crossings(&samples);
        assert!(
            crossings.len() >= 50,
            "{wave:?}: {} crossings",
            crossings.len()
        );
        let spans = crossings
            .windows(2)
            .take(50)
            .map(|pair| (pair[1] - pair[0]) as f32);
        let period = spans.sum::<f32>() / 50.0;
        assert!(
            (period - RATE as f32 / 110.0).abs() <= 1.0,
            "{wave:?}: {period}"
        );
    }
}

#[test]
fn bass_sources_polyblep_saw_reduces_aliasing_against_naive_saw() {
    let bandlimited = render_wave(Wave::Saw, 2950.0, RATE);
    let naive = naive_wave(Wave::Saw, 2950.0, RATE);
    let share = alias_share(&bandlimited, 2950.0);
    let naive_share = alias_share(&naive, 2950.0);
    assert!(
        share <= 0.5 * naive_share,
        "bandlimited={share}, naive={naive_share}"
    );
}

#[test]
fn bass_sources_polyblamp_triangle_reduces_aliasing_against_naive_triangle() {
    let bandlimited = render_wave(Wave::Tri, 2950.0, RATE);
    let naive = naive_wave(Wave::Tri, 2950.0, RATE);
    let ideal = ideal_triangle(2950.0, RATE);
    let share = alias_share(&bandlimited, 2950.0);
    let naive_share = alias_share(&naive, 2950.0);
    let ideal_share = alias_share(&ideal, 2950.0);
    assert!(
        share - ideal_share <= 0.5 * (naive_share - ideal_share),
        "bandlimited={share}, ideal={ideal_share}, naive={naive_share}"
    );
}

#[test]
fn bass_sources_wave_index_rounds_clamps_and_handles_nonfinite() {
    assert_eq!(Wave::from_index(0.0), Wave::Saw);
    assert_eq!(Wave::from_index(3.6), Wave::Sine);
    assert_eq!(Wave::from_index(9.0), Wave::Sine);
    assert_eq!(Wave::from_index(-1.0), Wave::Saw);
    assert_eq!(Wave::from_index(f32::NAN), Wave::Saw);
}

#[test]
fn bass_sources_click_pitch_follows_exponential_decay() {
    assert_eq!(click_semis(0.0, 1.0), 24.0);
    assert!(click_semis(0.005, 1.0) < 1.0);
    for time in [0.0, 0.001, 1.0] {
        assert_eq!(click_semis(time, 0.0), 0.0);
    }
    assert_eq!(click_semis(f32::NAN, 1.0), 0.0);
}

#[test]
fn bass_sources_detune_ratios_are_symmetric_and_clamped() {
    let (low, high) = detune_ratios(0.5);
    assert!((low * high - 1.0).abs() < 1.0e-6);
    assert!(low < 1.0 && high > 1.0);
    assert_eq!(detune_ratios(-1.0), (1.0, 1.0));
    assert_eq!(detune_ratios(f32::NAN), (1.0, 1.0));
}

#[test]
fn bass_sources_fm_zero_index_matches_carrier_sine() {
    let mut pair = FmPair::default();
    let inc = 220.0 / RATE as f32;
    let mut phase = 0.0_f32;
    for _ in 0..1000 {
        let expected = (TAU * phase).sin();
        assert!((pair.next(inc, 1.0, 0.0, 0.0) - expected).abs() < 1.0e-5);
        phase = (phase + inc).rem_euclid(1.0);
    }
}

#[test]
fn bass_sources_fm_feedback_uses_average_of_previous_two_modulator_samples() {
    let mut pair = FmPair::default();
    let (inc, ratio, index, feedback) = (173.0 / RATE as f32, 1.75, 2.3, 0.8);
    let (mut mod_phase, mut car_phase, mut y1, mut y2) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    for _ in 0..2048 {
        let modulator = (TAU * mod_phase + 1.5 * feedback * (y1 + y2) * 0.5).sin();
        let expected = (TAU * car_phase + index * modulator).sin();
        assert!((pair.next(inc, ratio, index, feedback) - expected).abs() < 1.0e-5);
        y2 = y1;
        y1 = modulator;
        mod_phase = (mod_phase + inc * ratio).rem_euclid(1.0);
        car_phase = (car_phase + inc).rem_euclid(1.0);
    }
}

#[test]
fn bass_sources_fm_feedback_increases_spectral_centroid() {
    let render = |feedback| {
        let mut pair = FmPair::default();
        (0..RATE)
            .map(|_| pair.next(220.0 / RATE as f32, 1.0, 2.0, feedback))
            .collect::<Vec<_>>()
    };
    assert!(centroid(&render(0.8)) > centroid(&render(0.0)));
}

#[test]
fn bass_sources_fm_extreme_feedback_remains_finite_and_bounded() {
    let mut pair = FmPair::default();
    for _ in 0..2 * RATE {
        let sample = pair.next(2000.0 / RATE as f32, 7.0, 32.0, 1.0);
        assert!(sample.is_finite());
        assert!(sample.abs() <= 1.0 + 1.0e-6);
    }
}

#[test]
fn bass_sources_folder_zero_is_bit_exact_for_random_samples() {
    let mut rng = Rng::new(0xB455_0011);
    let mut folder = Folder::default();
    for _ in 0..10_000 {
        let input = rng.bipolar();
        assert_eq!(folder.process(input, 0.0).to_bits(), input.to_bits());
    }
}

#[test]
fn bass_sources_folder_is_finite_bounded_and_adds_harmonics() {
    let input: Vec<f32> = (0..RATE)
        .map(|i| (TAU * 100.0 * i as f32 / RATE as f32).sin())
        .collect();
    let mut folder = Folder::default();
    let output: Vec<f32> = input.iter().map(|&x| folder.process(x, 1.0)).collect();
    assert!(output.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    assert!(centroid(&output) > centroid(&input));
}

#[test]
fn bass_sources_quantizer_bypasses_and_uses_requested_levels() {
    for input in [-1.0_f32, -0.37, 0.0, 0.51, 1.0] {
        assert_eq!(quantize(input, 16.0).to_bits(), input.to_bits());
        assert_eq!(quantize(input, 20.0).to_bits(), input.to_bits());
    }
    for i in -100..=100 {
        let output = quantize(i as f32 / 100.0, 3.0);
        assert!((output * 4.0 - (output * 4.0).round()).abs() < 1.0e-6);
    }
}

#[test]
fn bass_sources_component_states_round_trip() {
    let oscillator = Osc::with_phase(2.375);
    let mut osc_state = [0.0; Osc::FLOATS];
    oscillator.store(&mut osc_state);
    assert_eq!(Osc::load(&osc_state), oscillator);

    let mut fm = FmPair::default();
    for _ in 0..24 {
        let _ = fm.next(0.013, 2.5, 4.0, 0.7);
    }
    let mut fm_state = [0.0; FmPair::FLOATS];
    fm.store(&mut fm_state);
    assert_eq!(FmPair::load(&fm_state), fm);

    let mut folder = Folder::default();
    let _ = folder.process(0.23, 0.6);
    let mut folder_state = [0.0; Folder::FLOATS];
    folder.store(&mut folder_state);
    assert_eq!(Folder::load(&folder_state), folder);
}
