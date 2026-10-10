//! Pinned vosim port contract test owned by FM1V-00.

use crate::dsp::ugen::vosim;

#[test]
fn vosim_ports_are_unique_and_within_max_ports() {
    const _: () = assert!(vosim::PORT_COUNT <= crate::dsp::ugen::MAX_PORTS);
    let expected: [(&str, usize, f32); 4] = [
        ("freq", vosim::port::FREQ, 440.0),
        ("vosim-formant", vosim::port::VOSIM_FORMANT, 900.0),
        ("vosim-pulses", vosim::port::VOSIM_PULSES, 3.0),
        ("vosim-decay", vosim::port::VOSIM_DECAY, 0.7),
    ];
    assert_eq!(vosim::PORT_COUNT, expected.len());
    for (index, (name, port, default)) in expected.iter().enumerate() {
        assert_eq!(*port, index, "port index for {name}");
        assert_eq!(vosim::PORTS[index].0, *name, "port name at {index}");
        assert_eq!(vosim::PORTS[index].1, *default, "port default at {index}");
        for (other_index, (other_name, _, _)) in expected.iter().enumerate() {
            if index != other_index {
                assert_ne!(name, other_name, "duplicate port name {name}");
            }
        }
    }
}

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

fn inputs(values: [f32; 4]) -> [Inp<'static>; MAX_PORTS] {
    let mut result = [Inp::Val(0.0); MAX_PORTS];
    for (input, value) in result.iter_mut().zip(values) {
        *input = Inp::Val(value);
    }
    result
}

fn render(values: [f32; 4], sample_rate: f32, frames: usize, block_size: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: sample_rate,
        gate: frames,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 17,
    };
    let ins = inputs(values);
    let mut state = NodeState::default();
    let mut memory = [0.0; vosim::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for block in output.chunks_mut(block_size) {
        vosim::render(&ins, &mut state, &mut memory, block, &kx);
    }
    output
}

fn autocorrelation_pitch(samples: &[f32], sample_rate: f32, expected: f32) -> f32 {
    let start = samples.len() / 4;
    let range = (sample_rate / expected * 0.98) as usize..=(sample_rate / expected * 1.02) as usize;
    let lag = range
        .max_by(|left, right| {
            let correlation = |lag: usize| {
                samples[start..samples.len() - lag]
                    .iter()
                    .zip(&samples[start + lag..])
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
            };
            correlation(*left).total_cmp(&correlation(*right))
        })
        .unwrap_or(1);
    sample_rate / lag as f32
}

fn strongest_region_centroid(samples: &[f32], sample_rate: f32, min_hz: f32) -> f32 {
    let count = 2048.min(samples.len());
    let mut best_bin = 0;
    let mut best_power = 0.0;
    let mut magnitudes = vec![0.0; count / 2];
    for (bin, slot) in magnitudes.iter_mut().enumerate().skip(1) {
        let hz = bin as f32 * sample_rate / count as f32;
        if hz < min_hz {
            continue;
        }
        let (mut real, mut imaginary) = (0.0, 0.0);
        for (index, value) in samples[..count].iter().enumerate() {
            let angle = std::f32::consts::TAU * bin as f32 * index as f32 / count as f32;
            let window =
                0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (count - 1) as f32).cos();
            real += value * window * angle.cos();
            imaginary -= value * window * angle.sin();
        }
        let magnitude = real.hypot(imaginary);
        *slot = magnitude;
        let power = magnitude * magnitude;
        if power > best_power {
            best_power = power;
            best_bin = bin;
        }
    }
    let first_bin = best_bin.saturating_sub(2).max(1);
    let last_bin = (best_bin + 2).min(count / 2 - 1);
    let mut weighted_frequency = 0.0;
    let mut total_magnitude = 0.0;
    for (bin, magnitude) in magnitudes
        .iter()
        .enumerate()
        .take(last_bin + 1)
        .skip(first_bin)
    {
        let hz = bin as f32 * sample_rate / count as f32;
        weighted_frequency += hz * magnitude;
        total_magnitude += magnitude;
    }
    weighted_frequency / total_magnitude.max(1.0e-12)
}

#[test]
fn vosim_fundamental_matches_freq() {
    let samples = render([220.0, 900.0, 3.0, 0.7], 48_000.0, 48_000, 256);
    let pitch = autocorrelation_pitch(&samples, 48_000.0, 220.0);
    assert!((pitch - 220.0).abs() / 220.0 < 0.01, "pitch {pitch}");
}

#[test]
fn vosim_formant_region_centroid_tracks_control() {
    for formant in [800.0, 1_600.0] {
        let samples = render([110.0, formant, 8.0, 1.0], 48_000.0, 16_384, 256);
        let centroid = strongest_region_centroid(&samples[2_048..], 48_000.0, 300.0);
        assert!(
            (centroid - formant).abs() / formant < 0.15,
            "formant {formant}, strongest-region centroid {centroid}"
        );
    }
}

#[test]
fn vosim_single_pulse_ignores_decay() {
    let quiet_decay = render([220.0, 900.0, 1.0, 0.2], 48_000.0, 4_096, 128);
    let loud_decay = render([220.0, 900.0, 1.0, 0.9], 48_000.0, 4_096, 128);
    assert_eq!(quiet_decay, loud_decay);
}

#[test]
fn vosim_pulse_width_clamps_to_period() {
    const _: () = assert!(vosim::STATE_FLOATS <= 8);
    let samples = render([8_000.0, 100.0, 8.0, 0.8], 48_000.0, 4_800, 64);
    assert!(samples
        .iter()
        .all(|sample| sample.is_finite() && sample.abs() <= 1.0));
    let maximum_difference = samples[4_000..]
        .iter()
        .zip(&samples[4_006..])
        .map(|(left, right)| (left - right).abs())
        .fold(0.0, f32::max);
    assert!(
        maximum_difference < 1.0e-4,
        "six-sample period difference {maximum_difference}"
    );
    let settled = &samples[4_000..];
    let peak_to_peak = settled.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        - settled.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(
        peak_to_peak > 0.3,
        "clamped pulse train peak-to-peak {peak_to_peak}"
    );
}

#[test]
fn vosim_dc_blocker_centres_output() {
    let samples = render([220.0, 900.0, 3.0, 0.7], 48_000.0, 48_000, 256);
    let settled = &samples[24_000..];
    let mean = settled.iter().copied().sum::<f32>() / settled.len() as f32;
    let minimum = settled.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(mean.abs() < 1.0e-2, "settled output mean {mean}");
    assert!(minimum < -0.05, "settled output minimum {minimum}");
}

#[test]
fn vosim_block_size_independent_and_deterministic() {
    let values = [173.0, 1_370.0, 5.0, 0.63];
    let small_blocks = render(values, 48_000.0, 8_192, 64);
    let large_blocks = render(values, 48_000.0, 8_192, 256);
    assert_eq!(small_blocks, large_blocks);
    assert_eq!(small_blocks, render(values, 48_000.0, 8_192, 128));
}

#[test]
fn vosim_extreme_grid_is_finite_and_bounded() {
    for frequency in [f32::NAN, f32::INFINITY, 20.0, 8_000.0] {
        for formant in [f32::NEG_INFINITY, 100.0, 8_000.0] {
            for pulses in [f32::NAN, -100.0, 1.0, 8.0, 100.0] {
                let samples = render(
                    [frequency, formant, pulses, f32::INFINITY],
                    44_100.0,
                    1_024,
                    97,
                );
                assert!(
                    samples
                        .iter()
                        .all(|sample| sample.is_finite() && sample.abs() <= 1.0),
                    "frequency {frequency}, formant {formant}, pulses {pulses}"
                );
            }
        }
    }
}

#[test]
fn vosim_render_does_not_allocate() {
    const _: () = assert!(vosim::STATE_FLOATS <= 8);
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 256,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 0,
    };
    let ins = inputs([220.0, 900.0, 3.0, 0.7]);
    let mut state = NodeState::default();
    let mut memory = [0.0; vosim::STATE_FLOATS];
    let mut output = [0.0; 256];
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        for _ in 0..16 {
            vosim::render(&ins, &mut state, &mut memory, &mut output, &kx);
        }
    });
    assert_eq!(allocations, 0);
}
