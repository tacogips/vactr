//! Pinned hurdy_gurdy port contract test owned by FM1V-00.

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::{hurdy_gurdy, Inp, Kx, NodeState, MAX_PORTS};

#[test]
fn hurdy_gurdy_ports_are_unique_and_within_max_ports() {
    const _: () = assert!(hurdy_gurdy::PORT_COUNT <= crate::dsp::ugen::MAX_PORTS);
    let expected: [(&str, usize, f32); 16] = [
        ("freq", hurdy_gurdy::port::FREQ, 440.0),
        ("cps", hurdy_gurdy::port::CPS, 0.5),
        ("onset-time", hurdy_gurdy::port::ONSET_TIME, 0.0),
        ("gurdy-wheel", hurdy_gurdy::port::GURDY_WHEEL, 0.5),
        ("gurdy-pressure", hurdy_gurdy::port::GURDY_PRESSURE, 0.5),
        ("gurdy-melody", hurdy_gurdy::port::GURDY_MELODY, 1.0),
        ("gurdy-bourdon", hurdy_gurdy::port::GURDY_BOURDON, 0.6),
        ("gurdy-fifth", hurdy_gurdy::port::GURDY_FIFTH, 0.4),
        ("gurdy-trompette", hurdy_gurdy::port::GURDY_TROMPETTE, 0.5),
        ("gurdy-drone-key", hurdy_gurdy::port::GURDY_DRONE_KEY, 43.0),
        ("gurdy-buzz", hurdy_gurdy::port::GURDY_BUZZ, 0.6),
        (
            "gurdy-buzz-threshold",
            hurdy_gurdy::port::GURDY_BUZZ_THRESHOLD,
            0.5,
        ),
        ("gurdy-strokes", hurdy_gurdy::port::GURDY_STROKES, 0.0),
        (
            "gurdy-stroke-depth",
            hurdy_gurdy::port::GURDY_STROKE_DEPTH,
            0.5,
        ),
        ("gate-length", hurdy_gurdy::port::GATE_LENGTH, 8.0),
        ("velocity", hurdy_gurdy::port::VELOCITY, 1.0),
    ];
    assert_eq!(hurdy_gurdy::PORT_COUNT, expected.len());
    for (index, (name, port, default)) in expected.iter().enumerate() {
        assert_eq!(*port, index, "port index for {name}");
        assert_eq!(hurdy_gurdy::PORTS[index].0, *name, "port name at {index}");
        assert_eq!(
            hurdy_gurdy::PORTS[index].1,
            *default,
            "port default at {index}"
        );
        for (other_index, (other_name, _, _)) in expected.iter().enumerate() {
            if index != other_index {
                assert_ne!(name, other_name, "duplicate port name {name}");
            }
        }
    }
}

fn inputs(overrides: &[(usize, f32)]) -> [Inp<'static>; MAX_PORTS] {
    let mut result = [Inp::Val(0.0); MAX_PORTS];
    for (index, (_, value)) in hurdy_gurdy::PORTS.iter().enumerate() {
        result[index] = Inp::Val(*value);
    }
    for (index, value) in overrides {
        result[*index] = Inp::Val(*value);
    }
    result
}

fn render(
    overrides: &[(usize, f32)],
    sr: f32,
    frames: usize,
    block_size: usize,
) -> (Vec<f32>, NodeState) {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let inputs = inputs(overrides);
    let mut state = NodeState::default();
    let mut mem = vec![0.0; hurdy_gurdy::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for start in (0..frames).step_by(block_size) {
        let end = (start + block_size).min(frames);
        let kx = Kx {
            sr,
            gate: end - start,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 17,
        };
        hurdy_gurdy::render(&inputs, &mut state, &mut mem, &mut output[start..end], &kx);
    }
    (output, state)
}

fn pitch_hz(signal: &[f32], sr: f32, min_hz: f32, max_hz: f32) -> f32 {
    let min_lag = (sr / max_hz).floor() as usize;
    let max_lag = (sr / min_hz).ceil() as usize;
    let mut best = (f32::MIN, min_lag);
    for lag in min_lag..=max_lag {
        let corr = signal
            .iter()
            .take(signal.len().saturating_sub(lag))
            .zip(signal.iter().skip(lag))
            .map(|(a, b)| a * b)
            .sum::<f32>();
        if corr > best.0 {
            best = (corr, lag);
        }
    }
    sr / best.1 as f32
}

fn spectral_amplitude(signal: &[f32], sr: f32, hz: f32) -> f32 {
    let omega = std::f32::consts::TAU * hz / sr;
    let (sin, cos) = signal
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(s, c), (i, x)| {
            let phase = omega * i as f32;
            (s + x * phase.sin(), c + x * phase.cos())
        });
    2.0 * sin.hypot(cos) / signal.len().max(1) as f32
}

fn peak_in_band(signal: &[f32], sr: f32, start_hz: f32, end_hz: f32) -> (f32, f32) {
    let mut best = (0.0, 0.0);
    let mut hz = start_hz;
    while hz <= end_hz {
        let amplitude = spectral_amplitude(signal, sr, hz);
        if amplitude > best.1 {
            best = (hz, amplitude);
        }
        hz += 0.1;
    }
    best
}

fn high_band_energy(signal: &[f32], sr: f32) -> f32 {
    let coefficient = (-std::f32::consts::TAU * 3_000.0 / sr).exp();
    let mut low = 0.0;
    let mut energy = 0.0;
    for sample in signal {
        low = coefficient * low + (1.0 - coefficient) * sample;
        let high = sample - low;
        energy += high * high;
    }
    energy / signal.len().max(1) as f32
}

fn high_band_rms_envelope(signal: &[f32], sr: f32, chunk_size: usize) -> Vec<f32> {
    let coefficient = (-std::f32::consts::TAU * 3_000.0 / sr).exp();
    let mut low = 0.0;
    signal
        .chunks_exact(chunk_size)
        .map(|chunk| {
            let energy = chunk
                .iter()
                .map(|sample| {
                    low = coefficient * low + (1.0 - coefficient) * sample;
                    let high = sample - low;
                    high * high
                })
                .sum::<f32>();
            (energy / chunk.len() as f32).sqrt()
        })
        .collect()
}

fn centered_variance(signal: &[f32]) -> f32 {
    let mean = signal.iter().sum::<f32>() / signal.len().max(1) as f32;
    signal
        .iter()
        .map(|sample| (sample - mean).powi(2))
        .sum::<f32>()
        / signal.len().max(1) as f32
}

fn normalized_autocorrelation(signal: &[f32], lag: usize) -> f32 {
    let mean = signal.iter().sum::<f32>() / signal.len().max(1) as f32;
    let paired = signal.len().saturating_sub(lag);
    let (numerator, left_energy, right_energy) = signal
        .iter()
        .take(paired)
        .zip(signal.iter().skip(lag))
        .fold((0.0, 0.0, 0.0), |(sum, left, right), (a, b)| {
            let centered_a = a - mean;
            let centered_b = b - mean;
            (
                sum + centered_a * centered_b,
                left + centered_a * centered_a,
                right + centered_b * centered_b,
            )
        });
    numerator / (left_energy * right_energy).sqrt().max(f32::MIN_POSITIVE)
}

#[test]
fn gurdy_bowed_chanterelle_pitch_tracks_freq() {
    let (signal, _) = render(
        &[
            (hurdy_gurdy::port::FREQ, 330.0),
            (hurdy_gurdy::port::GURDY_WHEEL, 0.5),
            (hurdy_gurdy::port::GURDY_PRESSURE, 0.5),
            (hurdy_gurdy::port::GURDY_BOURDON, 0.0),
            (hurdy_gurdy::port::GURDY_FIFTH, 0.0),
            (hurdy_gurdy::port::GURDY_TROMPETTE, 0.0),
            (hurdy_gurdy::port::GATE_LENGTH, 64.0),
        ],
        48_000.0,
        48_000,
        128,
    );
    let frequency = pitch_hz(&signal[14_400..], 48_000.0, 300.0, 360.0);
    assert!((frequency - 330.0).abs() <= 3.3, "measured {frequency} Hz");
}

#[test]
fn gurdy_no_wheel_no_sustain() {
    let (signal, _) = render(
        &[(hurdy_gurdy::port::GURDY_WHEEL, 0.0)],
        48_000.0,
        48_000,
        128,
    );
    let rms = (signal[9_600..].iter().map(|x| x * x).sum::<f32>() / 38_400.0).sqrt();
    assert!(rms < 1.0e-5, "unexpected RMS {rms}");
}

#[test]
fn gurdy_drones_sound_at_key_and_fifth() {
    let (signal, _) = render(
        &[
            (hurdy_gurdy::port::GURDY_MELODY, 0.0),
            (hurdy_gurdy::port::GURDY_DRONE_KEY, 43.0),
            (hurdy_gurdy::port::GURDY_BOURDON, 1.0),
            (hurdy_gurdy::port::GURDY_FIFTH, 1.0),
            (hurdy_gurdy::port::GURDY_TROMPETTE, 0.0),
            (hurdy_gurdy::port::GATE_LENGTH, 64.0),
        ],
        48_000.0,
        48_000,
        128,
    );
    let window = &signal[12_000..];
    let bourdon = peak_in_band(window, 48_000.0, 85.0, 115.0);
    let fifth = peak_in_band(window, 48_000.0, 125.0, 170.0);
    assert!((bourdon.0 - 98.0).abs() <= 0.98, "bourdon peak {bourdon:?}");
    assert!((fifth.0 - 146.8).abs() <= 1.468, "fifth peak {fifth:?}");
    assert!(bourdon.1 > 0.001 && fifth.1 > 0.001);
}

#[test]
fn gurdy_buzz_rises_above_threshold() {
    let settings = [
        (hurdy_gurdy::port::GURDY_MELODY, 0.0),
        (hurdy_gurdy::port::GURDY_BOURDON, 0.0),
        (hurdy_gurdy::port::GURDY_FIFTH, 0.0),
        (hurdy_gurdy::port::GURDY_TROMPETTE, 1.0),
        (hurdy_gurdy::port::GURDY_BUZZ, 1.0),
        (hurdy_gurdy::port::GURDY_BUZZ_THRESHOLD, 0.5),
        (hurdy_gurdy::port::GATE_LENGTH, 64.0),
    ];
    let low_settings = [
        settings.as_slice(),
        &[(hurdy_gurdy::port::GURDY_WHEEL, 0.3)],
    ]
    .concat();
    let high_settings = [
        settings.as_slice(),
        &[(hurdy_gurdy::port::GURDY_WHEEL, 0.9)],
    ]
    .concat();
    let (low, _) = render(&low_settings, 48_000.0, 48_000, 128);
    let (high, _) = render(&high_settings, 48_000.0, 48_000, 128);
    let low_energy = high_band_energy(&low[9_600..], 48_000.0);
    let high_energy = high_band_energy(&high[9_600..], 48_000.0);
    assert!(
        high_energy > low_energy * 4.0,
        "low={low_energy}, high={high_energy}"
    );
}

#[test]
fn gurdy_strokes_are_tempo_locked() {
    let shared = [
        (hurdy_gurdy::port::CPS, 0.5),
        (hurdy_gurdy::port::GURDY_BUZZ, 1.0),
        (hurdy_gurdy::port::GURDY_WHEEL, 0.6),
        (hurdy_gurdy::port::GURDY_STROKE_DEPTH, 1.0),
        (hurdy_gurdy::port::GATE_LENGTH, 64.0),
    ];
    let stroked_settings = [
        shared.as_slice(),
        &[(hurdy_gurdy::port::GURDY_STROKES, 4.0)],
    ]
    .concat();
    let steady_settings = [
        shared.as_slice(),
        &[(hurdy_gurdy::port::GURDY_STROKES, 0.0)],
    ]
    .concat();
    let (stroked, _) = render(&stroked_settings, 48_000.0, 144_000, 128);
    let (steady, _) = render(&steady_settings, 48_000.0, 144_000, 128);
    let envelope = high_band_rms_envelope(&stroked, 48_000.0, 240);
    let steady_envelope = high_band_rms_envelope(&steady, 48_000.0, 240);
    let measured = &envelope[60..540];
    let steady_measured = &steady_envelope[60..540];
    assert!(
        centered_variance(measured) > centered_variance(steady_measured) * 2.0,
        "strokes did not modulate buzz envelope: stroked variance {}, steady variance {}",
        centered_variance(measured),
        centered_variance(steady_measured)
    );

    let mut best = (f32::MIN, 1usize);
    for lag in 60..=140 {
        let correlation = normalized_autocorrelation(measured, lag);
        if correlation > best.0 {
            best = (correlation, lag);
        }
    }
    let seconds = best.1 as f32 * 0.005;
    assert!((seconds - 0.5).abs() <= 0.01, "period {seconds} s");
}

#[test]
fn gurdy_buzz_zero_is_exact_bypass() {
    let shared = [
        (hurdy_gurdy::port::GURDY_STROKES, 4.0),
        (hurdy_gurdy::port::GURDY_WHEEL, 0.9),
        (hurdy_gurdy::port::GURDY_STROKE_DEPTH, 0.8),
        (hurdy_gurdy::port::GATE_LENGTH, 64.0),
        (hurdy_gurdy::port::GURDY_TROMPETTE, 1.0),
    ];
    let silent_buzz = [
        shared.as_slice(),
        &[
            (hurdy_gurdy::port::GURDY_BUZZ, 0.0),
            (hurdy_gurdy::port::GURDY_BUZZ_THRESHOLD, 0.0),
        ],
    ]
    .concat();
    let silent_buzz_high_threshold = [
        shared.as_slice(),
        &[
            (hurdy_gurdy::port::GURDY_BUZZ, 0.0),
            (hurdy_gurdy::port::GURDY_BUZZ_THRESHOLD, 1.0),
        ],
    ]
    .concat();
    let audible_buzz = [
        shared.as_slice(),
        &[
            (hurdy_gurdy::port::GURDY_BUZZ, 0.8),
            (hurdy_gurdy::port::GURDY_BUZZ_THRESHOLD, 0.0),
        ],
    ]
    .concat();
    let (without, _) = render(&silent_buzz, 48_000.0, 4_096, 128);
    let (without_threshold, _) = render(&silent_buzz_high_threshold, 48_000.0, 4_096, 128);
    let (with, _) = render(&audible_buzz, 48_000.0, 4_096, 128);
    assert_eq!(without, without_threshold);
    assert_ne!(without, with);
    assert!(with.iter().all(|x| x.is_finite()));
}

#[test]
fn gurdy_gate_length_owns_note_off_and_finishes() {
    let (signal, state) = render(
        &[
            (hurdy_gurdy::port::GATE_LENGTH, 0.05),
            (hurdy_gurdy::port::CPS, 1.0),
        ],
        48_000.0,
        96_000,
        128,
    );
    assert!(signal.iter().any(|x| x.abs() > 1.0e-5));
    assert!(
        state.done(),
        "voice did not finish after its gate and decay: tail={:?}, gate={:?}",
        state.s[2],
        state.s[1]
    );
    let last_audible = signal.iter().rposition(|x| x.abs() > 1.0e-5).unwrap_or(0);
    assert!(last_audible > 150, "gate suppressed the note immediately");
    assert!(
        last_audible < signal.len() - 128,
        "tail did not reach finish"
    );
    assert!(signal[last_audible + 128..].iter().all(|x| *x == 0.0));
}

#[test]
fn gurdy_block_size_independent_and_deterministic() {
    let settings = [
        (hurdy_gurdy::port::GURDY_STROKES, 3.0),
        (hurdy_gurdy::port::GURDY_WHEEL, 0.7),
        (hurdy_gurdy::port::GATE_LENGTH, 64.0),
    ];
    let (a, _) = render(&settings, 48_000.0, 8_192, 64);
    let (b, _) = render(&settings, 48_000.0, 8_192, 256);
    let (again, _) = render(&settings, 48_000.0, 8_192, 64);
    assert_eq!(a, b);
    assert_eq!(a, again);
}

#[test]
fn gurdy_extreme_grid_is_finite_and_bounded() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for frequency in [20.0, 2_000.0] {
            for wheel in [0.0, 1.0] {
                let (signal, _) = render(
                    &[
                        (hurdy_gurdy::port::FREQ, frequency),
                        (hurdy_gurdy::port::CPS, 50.0),
                        (hurdy_gurdy::port::GURDY_WHEEL, wheel),
                        (hurdy_gurdy::port::GURDY_PRESSURE, 1.0),
                        (hurdy_gurdy::port::GURDY_MELODY, 1.0),
                        (hurdy_gurdy::port::GURDY_BOURDON, 1.0),
                        (hurdy_gurdy::port::GURDY_FIFTH, 1.0),
                        (hurdy_gurdy::port::GURDY_TROMPETTE, 1.0),
                        (hurdy_gurdy::port::GURDY_DRONE_KEY, 72.0),
                        (hurdy_gurdy::port::GURDY_BUZZ, 1.0),
                        (hurdy_gurdy::port::GURDY_BUZZ_THRESHOLD, 0.0),
                        (hurdy_gurdy::port::GURDY_STROKES, 16.0),
                        (hurdy_gurdy::port::GURDY_STROKE_DEPTH, 1.0),
                        (hurdy_gurdy::port::GATE_LENGTH, 64.0),
                        (hurdy_gurdy::port::VELOCITY, 1.0),
                    ],
                    sr,
                    2_048,
                    97,
                );
                assert!(signal.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            }
        }
    }
}

#[test]
fn gurdy_state_fits_test_budget() {
    const _: () = assert!(hurdy_gurdy::STATE_FLOATS <= 24_000);
    assert_eq!(hurdy_gurdy::STATE_FLOATS, 19_223);
}

#[test]
fn gurdy_render_does_not_allocate() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 128,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 23,
    };
    let inputs = inputs(&[]);
    let mut state = NodeState::default();
    let mut memory = [0.0; hurdy_gurdy::STATE_FLOATS];
    let mut output = [0.0; 128];
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        hurdy_gurdy::render(&inputs, &mut state, &mut memory, &mut output, &kx);
    });
    assert_eq!(allocations, 0);
}
