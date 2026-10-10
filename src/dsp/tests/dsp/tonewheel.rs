//! Pinned tonewheel port contract test owned by FM1V-00.

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::tonewheel;
use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

#[test]
fn tonewheel_ports_are_unique_and_within_max_ports() {
    const _: () = assert!(tonewheel::PORT_COUNT <= crate::dsp::ugen::MAX_PORTS);
    let expected: [(&str, usize, f32); 20] = [
        ("freq", tonewheel::port::FREQ, 440.0),
        ("cps", tonewheel::port::CPS, 0.5),
        ("onset-time", tonewheel::port::ONSET_TIME, 0.0),
        ("drawbar1", tonewheel::port::DRAWBAR1, 8.0),
        ("drawbar2", tonewheel::port::DRAWBAR2, 8.0),
        ("drawbar3", tonewheel::port::DRAWBAR3, 8.0),
        ("drawbar4", tonewheel::port::DRAWBAR4, 0.0),
        ("drawbar5", tonewheel::port::DRAWBAR5, 0.0),
        ("drawbar6", tonewheel::port::DRAWBAR6, 0.0),
        ("drawbar7", tonewheel::port::DRAWBAR7, 0.0),
        ("drawbar8", tonewheel::port::DRAWBAR8, 0.0),
        ("drawbar9", tonewheel::port::DRAWBAR9, 0.0),
        ("organ-click", tonewheel::port::ORGAN_CLICK, 0.3),
        ("organ-perc", tonewheel::port::ORGAN_PERC, 0.0),
        ("organ-perc-slow", tonewheel::port::ORGAN_PERC_SLOW, 0.0),
        ("organ-perc-soft", tonewheel::port::ORGAN_PERC_SOFT, 0.0),
        (
            "organ-perc-trigger",
            tonewheel::port::ORGAN_PERC_TRIGGER,
            1.0,
        ),
        ("organ-vibrato", tonewheel::port::ORGAN_VIBRATO, 0.0),
        ("gate-length", tonewheel::port::GATE_LENGTH, 4.0),
        ("velocity", tonewheel::port::VELOCITY, 1.0),
    ];
    assert_eq!(tonewheel::PORT_COUNT, expected.len());
    for (index, (name, port, default)) in expected.iter().enumerate() {
        assert_eq!(*port, index, "port index for {name}");
        assert_eq!(tonewheel::PORTS[index].0, *name, "port name at {index}");
        assert_eq!(
            tonewheel::PORTS[index].1,
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
    let mut result = std::array::from_fn(|index| {
        tonewheel::PORTS
            .get(index)
            .map_or(Inp::Val(0.0), |(_, value)| Inp::Val(*value))
    });
    for &(index, value) in overrides {
        result[index] = Inp::Val(value);
    }
    result
}

fn render_voice(
    overrides: &[(usize, f32)],
    frames: usize,
    sr: f32,
    block: usize,
    seed: u32,
) -> (Vec<f32>, NodeState) {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: 0,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed,
    };
    let ins = inputs(overrides);
    let mut state = NodeState::default();
    let mut mem = vec![0.0; tonewheel::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for chunk in output.chunks_mut(block) {
        tonewheel::render(&ins, &mut state, &mut mem, chunk, &kx);
    }
    (output, state)
}

fn amplitude(samples: &[f32], frequency: f32, sr: f32) -> f32 {
    let start = samples.len() / 4;
    let segment = &samples[start..];
    let mut real = 0.0;
    let mut imag = 0.0;
    for (index, sample) in segment.iter().enumerate() {
        let phase = std::f32::consts::TAU * frequency * index as f32 / sr;
        real += sample * phase.cos();
        imag -= sample * phase.sin();
    }
    2.0 * real.hypot(imag) / segment.len() as f32
}

fn band_energy(samples: &[f32], low_hz: f32, high_hz: f32, sr: f32) -> f32 {
    let mut energy = 0.0;
    for frequency in (low_hz as usize..=high_hz as usize).step_by(100) {
        let mut real = 0.0;
        let mut imag = 0.0;
        for (index, sample) in samples.iter().enumerate() {
            let phase = std::f32::consts::TAU * frequency as f32 * index as f32 / sr;
            real += sample * phase.cos();
            imag -= sample * phase.sin();
        }
        energy += real * real + imag * imag;
    }
    energy
}

#[test]
fn tonewheel_registration_888000000_peaks() {
    let (audio, _) = render_voice(
        &[
            (tonewheel::port::FREQ, 220.0),
            (tonewheel::port::CPS, 0.03),
            (tonewheel::port::GATE_LENGTH, 64.0),
        ],
        48_000,
        48_000.0,
        128,
        1,
    );
    for (frequency, expected) in [(110.0, 1.0), (329.6, 1.0), (220.0, 1.0)] {
        assert!(
            amplitude(&audio, frequency, 48_000.0) > 0.10 * expected,
            "missing {frequency} Hz component"
        );
    }
    assert!(amplitude(&audio, 440.0, 48_000.0) < amplitude(&audio, 220.0, 48_000.0) * 0.01);
}

#[test]
fn tonewheel_foldback_caps_partials() {
    let overrides = [
        (tonewheel::port::FREQ, 2093.0),
        (tonewheel::port::CPS, 0.03),
        (tonewheel::port::GATE_LENGTH, 64.0),
        (tonewheel::port::DRAWBAR4, 8.0),
        (tonewheel::port::DRAWBAR5, 8.0),
        (tonewheel::port::DRAWBAR6, 8.0),
        (tonewheel::port::DRAWBAR7, 8.0),
        (tonewheel::port::DRAWBAR8, 8.0),
        (tonewheel::port::DRAWBAR9, 8.0),
    ];
    let (audio, _) = render_voice(&overrides, 48_000, 48_000.0, 256, 2);
    for frequency in [5_993.0, 8_000.0, 10_000.0, 12_000.0] {
        assert!(
            amplitude(&audio, frequency, 48_000.0) < 1.0e-3,
            "unexpected high partial near {frequency} Hz"
        );
    }
    for ratio in [2.996_614_3, 4.0, 5.039_684_3, 5.993_228_4, 8.0] {
        let frequency = 2093.0 * ratio;
        assert!(
            amplitude(&audio, frequency, 48_000.0) < 1.0e-3,
            "unfolded partial remains near {frequency} Hz"
        );
    }
    for ratio in [2.996_614_3, 4.0, 5.039_684_3] {
        let frequency = 2093.0 * ratio / 2.0;
        assert!(
            amplitude(&audio, frequency, 48_000.0) > 0.05,
            "folded partial is missing near {frequency} Hz"
        );
    }
}

#[test]
fn tonewheel_drawbar_steps_are_3db() {
    let full = render_voice(&[(tonewheel::port::FREQ, 220.0)], 24_000, 48_000.0, 128, 3).0;
    let reduced = render_voice(
        &[
            (tonewheel::port::FREQ, 220.0),
            (tonewheel::port::DRAWBAR3, 7.0),
        ],
        24_000,
        48_000.0,
        128,
        3,
    )
    .0;
    let ratio = amplitude(&full, 220.0, 48_000.0) / amplitude(&reduced, 220.0, 48_000.0);
    let db = 20.0 * ratio.log10();
    assert!((db - 3.0).abs() < 0.1, "drawbar delta was {db} dB");
}

#[test]
fn tonewheel_percussion_second_and_third() {
    let silent_drawbars = [
        (tonewheel::port::DRAWBAR1, 0.0),
        (tonewheel::port::DRAWBAR2, 0.0),
        (tonewheel::port::DRAWBAR3, 0.0),
        (tonewheel::port::DRAWBAR9, 8.0),
    ];
    let mut second_overrides = silent_drawbars.to_vec();
    second_overrides.extend([
        (tonewheel::port::FREQ, 220.0),
        (tonewheel::port::ORGAN_PERC, 1.0),
    ]);
    let mut third_overrides = silent_drawbars.to_vec();
    third_overrides.extend([
        (tonewheel::port::FREQ, 220.0),
        (tonewheel::port::ORGAN_PERC, 2.0),
    ]);
    let second = render_voice(&second_overrides, 24_000, 48_000.0, 128, 4).0;
    let third = render_voice(&third_overrides, 24_000, 48_000.0, 128, 4).0;
    assert!(amplitude(&second, 440.0, 48_000.0) > amplitude(&second, 330.0, 48_000.0));
    assert!(amplitude(&third, 220.0 * 2.996_614_3, 48_000.0) > amplitude(&third, 440.0, 48_000.0));
    assert!(
        amplitude(&second, 1_760.0, 48_000.0) < 1.0e-3,
        "percussion did not mute drawbar 9"
    );
    let mut slow_overrides = silent_drawbars.to_vec();
    slow_overrides.extend([
        (tonewheel::port::FREQ, 220.0),
        (tonewheel::port::ORGAN_PERC, 1.0),
        (tonewheel::port::ORGAN_PERC_SLOW, 1.0),
    ]);
    let slow = render_voice(&slow_overrides, 24_000, 48_000.0, 128, 4).0;
    let fast_early = second[..200].iter().map(|x| x * x).sum::<f32>();
    let fast_late = second[12_000..12_200].iter().map(|x| x * x).sum::<f32>();
    let slow_late = slow[12_000..14_000].iter().map(|x| x * x).sum::<f32>();
    let decay_db = 10.0 * (fast_late / fast_early).log10();
    assert!(
        (decay_db + 60.0).abs() < 6.0,
        "fast percussion decayed {decay_db} dB at 250 ms"
    );
    assert!(slow_late > fast_late * 5.0);
}

#[test]
fn tonewheel_perc_trigger_zero_adds_nothing() {
    let plain = render_voice(
        &[(tonewheel::port::ORGAN_PERC, 0.0)],
        4_000,
        48_000.0,
        128,
        5,
    )
    .0;
    let muted = render_voice(
        &[
            (tonewheel::port::ORGAN_PERC, 1.0),
            (tonewheel::port::ORGAN_PERC_TRIGGER, 0.0),
        ],
        4_000,
        48_000.0,
        128,
        5,
    )
    .0;
    assert_eq!(plain, muted);
}

#[test]
fn tonewheel_click_zero_is_exact_bypass() {
    let zero_a = render_voice(
        &[(tonewheel::port::ORGAN_CLICK, 0.0)],
        4_000,
        48_000.0,
        128,
        6,
    )
    .0;
    let zero_b = render_voice(
        &[(tonewheel::port::ORGAN_CLICK, 0.0)],
        4_000,
        48_000.0,
        128,
        99,
    )
    .0;
    let click = render_voice(
        &[(tonewheel::port::ORGAN_CLICK, 1.0)],
        4_000,
        48_000.0,
        128,
        6,
    )
    .0;
    assert_eq!(zero_a, zero_b);
    assert_ne!(zero_a, click);
    assert!(click[..480]
        .iter()
        .zip(&zero_a[..480])
        .any(|(a, b)| (a - b).abs() > 1.0e-4));
    let click_only: Vec<f32> = click[..480]
        .iter()
        .zip(&zero_a[..480])
        .map(|(a, b)| a - b)
        .collect();
    assert!(band_energy(&click_only, 2_000.0, 5_000.0, 48_000.0) > 1.0);
}

#[test]
fn tonewheel_scanner_modulates_at_6_9hz() {
    let (dry, _) = render_voice(
        &[
            (tonewheel::port::FREQ, 1000.0),
            (tonewheel::port::DRAWBAR1, 0.0),
            (tonewheel::port::DRAWBAR2, 0.0),
            (tonewheel::port::DRAWBAR3, 8.0),
            (tonewheel::port::CPS, 0.03),
            (tonewheel::port::GATE_LENGTH, 64.0),
        ],
        96_000,
        48_000.0,
        128,
        7,
    );
    let (vibrato, _) = render_voice(
        &[
            (tonewheel::port::FREQ, 1000.0),
            (tonewheel::port::DRAWBAR1, 0.0),
            (tonewheel::port::DRAWBAR2, 0.0),
            (tonewheel::port::DRAWBAR3, 8.0),
            (tonewheel::port::CPS, 0.03),
            (tonewheel::port::GATE_LENGTH, 64.0),
            (tonewheel::port::ORGAN_VIBRATO, 3.0),
        ],
        96_000,
        48_000.0,
        128,
        7,
    );
    assert!(dry
        .iter()
        .zip(&vibrato)
        .skip(2_000)
        .any(|(a, b)| (a - b).abs() > 1.0e-3));
    let mut crossings = Vec::new();
    for index in 2_000..vibrato.len() {
        let before = vibrato[index - 1];
        let after = vibrato[index];
        if before <= 0.0 && after > 0.0 {
            let fraction = -before / (after - before);
            crossings.push(index as f32 - 1.0 + fraction);
        }
    }
    let mut frequencies = Vec::new();
    let mut times = Vec::new();
    for pair in crossings.windows(2) {
        frequencies.push(48_000.0 / (pair[1] - pair[0]));
        times.push((pair[1] + pair[0]) / (2.0 * 48_000.0));
    }
    let mean = frequencies.iter().sum::<f32>() / frequencies.len() as f32;
    let mut peak_hz = 0.0;
    let mut peak_power = 0.0;
    for step in 0..=32 {
        let candidate = 6.5 + step as f32 * 0.025;
        let mut real = 0.0;
        let mut imag = 0.0;
        for (&frequency, &time) in frequencies.iter().zip(&times) {
            let phase = std::f32::consts::TAU * candidate * time;
            real += (frequency - mean) * phase.cos();
            imag -= (frequency - mean) * phase.sin();
        }
        let power = real * real + imag * imag;
        if power > peak_power {
            peak_power = power;
            peak_hz = candidate;
        }
    }
    assert!(
        (peak_hz - 6.9).abs() <= 0.35,
        "scanner modulation peak {peak_hz} Hz"
    );
}

#[test]
fn tonewheel_gate_length_owns_note_off() {
    let (audio, state) = render_voice(
        &[
            (tonewheel::port::CPS, 0.5),
            (tonewheel::port::GATE_LENGTH, 4.0),
        ],
        25_000,
        48_000.0,
        128,
        8,
    );
    assert!(state.done());
    assert!(audio[24_250..].iter().all(|sample| *sample == 0.0));
    assert!(audio[..24_240].iter().any(|sample| sample.abs() > 1.0e-4));
    assert!(audio[23_900..24_000]
        .iter()
        .any(|sample| sample.abs() > 1.0e-3));
    assert!(audio[24_000..24_200]
        .iter()
        .any(|sample| sample.abs() > 1.0e-3));
    let sustained_peak = audio[23_800..24_000]
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    let release_tail_peak = audio[24_200..24_240]
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    assert!(release_tail_peak < sustained_peak * 0.25);
    assert!(audio[24_241..].iter().all(|sample| *sample == 0.0));
}

#[test]
fn tonewheel_block_size_independent_and_deterministic() {
    let a = render_voice(&[], 8_192, 48_000.0, 64, 9).0;
    let b = render_voice(&[], 8_192, 48_000.0, 256, 9).0;
    let c = render_voice(&[], 8_192, 48_000.0, 128, 9).0;
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn tonewheel_extreme_grid_is_finite_and_bounded() {
    for frequency in [20.0, 8_000.0] {
        let overrides = [
            (tonewheel::port::FREQ, frequency),
            (tonewheel::port::CPS, 50.0),
            (tonewheel::port::GATE_LENGTH, 64.0),
            (tonewheel::port::ORGAN_CLICK, 1.0),
            (tonewheel::port::ORGAN_PERC, 2.0),
            (tonewheel::port::ORGAN_PERC_SLOW, 1.0),
            (tonewheel::port::ORGAN_VIBRATO, 6.0),
            (tonewheel::port::VELOCITY, 1.0),
            (tonewheel::port::DRAWBAR1, 8.0),
            (tonewheel::port::DRAWBAR9, 8.0),
        ];
        let (audio, _) = render_voice(&overrides, 4_096, 96_000.0, 97, 11);
        assert!(audio
            .iter()
            .all(|sample| sample.is_finite() && sample.abs() <= 1.0));
    }
}

#[test]
fn tonewheel_render_does_not_allocate() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 0,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 12,
    };
    let ins = inputs(&[
        (tonewheel::port::ORGAN_CLICK, 1.0),
        (tonewheel::port::ORGAN_VIBRATO, 3.0),
    ]);
    let mut state = NodeState::default();
    let mut mem = vec![0.0; tonewheel::STATE_FLOATS];
    let mut out = [0.0; 256];
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        for _ in 0..8 {
            tonewheel::render(&ins, &mut state, &mut mem, &mut out, &kx);
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn tonewheel_state_covers_scanner_at_96khz() {
    assert_eq!(tonewheel::STATE_FLOATS, 9 + 292);
    let (audio, _) = render_voice(
        &[(tonewheel::port::ORGAN_VIBRATO, 3.0)],
        512,
        96_000.0,
        97,
        13,
    );
    assert!(audio.iter().all(|sample| sample.is_finite()));
}
