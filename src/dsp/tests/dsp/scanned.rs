//! Scanned synthesis contract and behavior tests.

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::{scanned, Inp, Kx, NodeState, MAX_PORTS};

fn inputs(overrides: &[(usize, f32)]) -> [Inp<'static>; MAX_PORTS] {
    let mut values = std::array::from_fn(|index| {
        scanned::PORTS
            .get(index)
            .map_or(Inp::Val(0.0), |(_, default)| Inp::Val(*default))
    });
    for &(index, value) in overrides {
        values[index] = Inp::Val(value);
    }
    values
}

fn render(scans: &[(usize, f32)], sr: f32, frames: usize, block_size: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: block_size,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 17,
    };
    let ins = inputs(scans);
    let mut state = NodeState::default();
    let mut memory = vec![0.0; scanned::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for block in output.chunks_mut(block_size) {
        scanned::render(&ins, &mut state, &mut memory, block, &kx);
    }
    output
}

fn centroid(samples: &[f32]) -> f32 {
    let magnitudes = crate::dsp::offline::spectrum(samples, 2048);
    let weighted = magnitudes
        .iter()
        .enumerate()
        .skip(1)
        .map(|(bin, magnitude)| bin as f32 * magnitude)
        .sum::<f32>();
    weighted / magnitudes.iter().sum::<f32>().max(1.0e-12)
}

fn normalized_correlation(samples: &[f32], lag: usize) -> f32 {
    let mut cross = 0.0;
    let mut left_energy = 0.0;
    let mut right_energy = 0.0;
    for index in 0..(samples.len() - lag) {
        let left = samples[index];
        let right = samples[index + lag];
        cross += left * right;
        left_energy += left * left;
        right_energy += right * right;
    }
    cross / (left_energy * right_energy).sqrt().max(1.0e-12)
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

fn pitch_by_autocorrelation(samples: &[f32], sr: f32, expected: f32) -> (f32, f32) {
    let min_lag = (sr / (expected * 1.5)).floor() as usize;
    let max_lag = (sr / (expected / 1.5)).ceil() as usize;
    let mut best = (f32::NEG_INFINITY, min_lag);
    for lag in min_lag..=max_lag {
        let correlation = normalized_correlation(samples, lag);
        if correlation > best.0 {
            best = (correlation, lag);
        }
    }
    (sr / best.1 as f32, best.0)
}

#[test]
fn scanned_ports_are_unique_and_within_max_ports() {
    const _: () = assert!(scanned::PORT_COUNT <= crate::dsp::ugen::MAX_PORTS);
    let expected: [(&str, usize, f32); 7] = [
        ("freq", scanned::port::FREQ, 440.0),
        ("scan-stiffness", scanned::port::SCAN_STIFFNESS, 0.5),
        ("scan-damping", scanned::port::SCAN_DAMPING, 0.3),
        ("scan-centering", scanned::port::SCAN_CENTERING, 0.1),
        ("scan-hammer", scanned::port::SCAN_HAMMER, 0.3),
        ("scan-position", scanned::port::SCAN_POSITION, 0.5),
        ("scan-update", scanned::port::SCAN_UPDATE, 400.0),
    ];
    assert_eq!(scanned::PORT_COUNT, expected.len());
    for (index, (name, port, default)) in expected.iter().enumerate() {
        assert_eq!(*port, index, "port index for {name}");
        assert_eq!(scanned::PORTS[index].0, *name, "port name at {index}");
        assert_eq!(scanned::PORTS[index].1, *default, "port default at {index}");
        for (other_index, (other_name, _, _)) in expected.iter().enumerate() {
            if index != other_index {
                assert_ne!(name, other_name, "duplicate port name {name}");
            }
        }
    }
}

#[test]
fn scanned_pitch_matches_freq() {
    let sr = 48_000.0;
    let samples = render(&[(scanned::port::FREQ, 220.0)], sr, 24_000, 256);
    let analysis = &samples[4_800..];
    let (estimated, best_correlation) = pitch_by_autocorrelation(analysis, sr, 220.0);
    let half_period_lag = (sr / (2.0 * 220.0)).round() as usize;
    let half_period_correlation = normalized_correlation(analysis, half_period_lag);
    assert!(
        (estimated - 220.0).abs() <= 2.2,
        "estimated pitch {estimated}"
    );
    assert!(
        best_correlation >= 0.8,
        "best correlation {best_correlation} at estimated pitch {estimated}"
    );
    assert!(
        half_period_correlation < best_correlation - 0.3,
        "half-period correlation {half_period_correlation}, best {best_correlation} at estimated pitch {estimated}"
    );
}

#[test]
fn scanned_timbre_evolves() {
    let samples = render(
        &[
            (scanned::port::SCAN_HAMMER, 0.02),
            (scanned::port::SCAN_CENTERING, 0.0),
        ],
        48_000.0,
        24_000,
        256,
    );
    let early_window = &samples[..4_800];
    let late_window = &samples[19_200..24_000];
    let early_rms = rms(early_window);
    let late_rms = rms(late_window);
    assert!(
        late_rms >= 1.0e-3 * early_rms,
        "late RMS {late_rms} must remain at least 1e-3 of early RMS {early_rms}"
    );
    let early = centroid(early_window);
    let late = centroid(late_window);
    assert!(
        (early - late).abs() > 0.1 * early,
        "early centroid={early}, late centroid={late}, early RMS={early_rms}, late RMS={late_rms}, ratio={}",
        late_rms / early_rms
    );
}

#[test]
fn scanned_stiffness_speeds_evolution() {
    let sr = 48_000.0;
    let soft = render(&[(scanned::port::SCAN_STIFFNESS, 0.1)], sr, 12_000, 256);
    let stiff = render(&[(scanned::port::SCAN_STIFFNESS, 0.9)], sr, 12_000, 256);
    let early_soft = centroid(&soft[..4_800]);
    let late_soft = centroid(&soft[4_800..9_600]);
    let early_stiff = centroid(&stiff[..4_800]);
    let late_stiff = centroid(&stiff[4_800..9_600]);
    let soft_change = (late_soft - early_soft).abs();
    let stiff_change = (late_stiff - early_stiff).abs();
    assert!(
        stiff_change > soft_change,
        "soft={soft_change}, stiff={stiff_change}"
    );
}

#[test]
fn scanned_update_rate_changes_output() {
    let slow = render(
        &[(scanned::port::SCAN_UPDATE, 100.0)],
        48_000.0,
        12_000,
        256,
    );
    let fast = render(
        &[(scanned::port::SCAN_UPDATE, 1000.0)],
        48_000.0,
        12_000,
        256,
    );
    assert_ne!(slow, fast);
}

#[test]
fn scanned_coefficients_satisfy_stability_bound() {
    for stiffness in [0.0, 1.0] {
        for centering in [0.0, 1.0] {
            for damping in [0.0, 1.0] {
                let (k, c, d) = scanned::coefficients(stiffness, centering, damping);
                assert!(4.0 * k + c < 4.0 - 2.0 * d - 0.1);
            }
        }
    }
}

#[test]
fn scanned_stability_grid_is_finite_and_bounded() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for stiffness in [0.0, 1.0] {
            for centering in [0.0, 1.0] {
                for damping in [0.0, 1.0] {
                    for hammer in [0.0, 1.0] {
                        let samples = render(
                            &[
                                (
                                    scanned::port::FREQ,
                                    if sr == 96_000.0 { 8000.0 } else { 20.0 },
                                ),
                                (scanned::port::SCAN_STIFFNESS, stiffness),
                                (scanned::port::SCAN_DAMPING, damping),
                                (scanned::port::SCAN_CENTERING, centering),
                                (scanned::port::SCAN_HAMMER, hammer),
                                (scanned::port::SCAN_POSITION, hammer),
                                (scanned::port::SCAN_UPDATE, 400.0),
                            ],
                            sr,
                            (sr * 10.0) as usize,
                            256,
                        );
                        assert!(samples.iter().all(|sample| sample.is_finite()));
                        assert!(samples.iter().all(|sample| sample.abs() <= 1.0));
                    }
                }
            }
        }
    }
}

#[test]
fn scanned_block_size_independent_and_deterministic() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let args = [
            (scanned::port::FREQ, 330.0),
            (scanned::port::SCAN_UPDATE, 333.0),
        ];
        let one = render(&args, sr, 9_600, 64);
        let same = render(&args, sr, 9_600, 256);
        let replay = render(&args, sr, 9_600, 97);
        assert_eq!(one, same, "block size at {sr} Hz");
        assert_eq!(one, replay, "determinism at {sr} Hz");
    }
}

#[test]
fn scanned_render_does_not_allocate() {
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
        seed: 3,
    };
    let ins = inputs(&[]);
    let mut state = NodeState::default();
    let mut memory = [0.0; scanned::STATE_FLOATS];
    let mut output = [0.0; 256];
    scanned::render(&ins, &mut state, &mut memory, &mut output, &kx);
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        scanned::render(&ins, &mut state, &mut memory, &mut output, &kx);
    });
    assert_eq!(allocations, 0);
}
