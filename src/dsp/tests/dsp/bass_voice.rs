//! Bass voice port contract tests owned by BASS-00.

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::{bass_voice, Inp, Kx, NodeState, MAX_PORTS};

#[test]
fn bass_voice_ports_are_unique_and_within_max_ports() {
    assert_eq!(bass_voice::PORT_COUNT, 32);
    const _: () = assert!(bass_voice::PORT_COUNT <= MAX_PORTS);
    assert_eq!(bass_voice::port::FREQ, 0);
    assert_eq!(bass_voice::port::CPS, 2);
    assert_eq!(bass_voice::port::GATE_LENGTH, 21);
    assert_eq!(bass_voice::port::CLICK_LEVEL, 31);

    let port_names: [(usize, &str); 32] = [
        (bass_voice::port::FREQ, "freq"),
        (bass_voice::port::MODE, "mode"),
        (bass_voice::port::CPS, "cps"),
        (bass_voice::port::ONSET_TIME, "onset-time"),
        (bass_voice::port::WAVE, "wave"),
        (bass_voice::port::CUTOFF, "cutoff"),
        (bass_voice::port::RES, "res"),
        (bass_voice::port::DRIVE, "drive"),
        (bass_voice::port::DETUNE, "detune"),
        (bass_voice::port::RATIO, "ratio"),
        (bass_voice::port::INDEX, "index"),
        (bass_voice::port::AMP_ATTACK, "amp-attack"),
        (bass_voice::port::AMP_DECAY, "amp-decay"),
        (bass_voice::port::SUSTAIN, "sustain"),
        (bass_voice::port::RELEASE, "release"),
        (bass_voice::port::LFO_WAVE, "lfo-wave"),
        (bass_voice::port::LFO_RATE, "lfo-rate"),
        (bass_voice::port::LFO_DEPTH, "lfo-depth"),
        (bass_voice::port::LFO_OFFSET, "lfo-offset"),
        (bass_voice::port::LFO_RETRIGGER, "lfo-retrigger"),
        (bass_voice::port::LFO_SYNC, "lfo-sync"),
        (bass_voice::port::GATE_LENGTH, "gate-length"),
        (bass_voice::port::ENV_MOD, "env-mod"),
        (bass_voice::port::ENV_DECAY, "env-decay"),
        (bass_voice::port::ACCENT, "accent"),
        (bass_voice::port::SLIDE_FROM, "slide-from"),
        (bass_voice::port::SLIDE_TIME, "slide-time"),
        (bass_voice::port::SUB_LEVEL, "sub-level"),
        (bass_voice::port::FM_FEEDBACK, "fm-feedback"),
        (bass_voice::port::FOLD, "fold"),
        (bass_voice::port::BIT_DEPTH, "bit-depth"),
        (bass_voice::port::CLICK_LEVEL, "click-level"),
    ];
    assert_eq!(port_names.len(), bass_voice::PORT_COUNT);
    for (position, (index, name)) in port_names.iter().enumerate() {
        assert_eq!(*index, position, "port constant for {name}");
        assert_eq!(bass_voice::PORTS[*index].0, *name, "port name at {index}");
    }

    let expected = [
        ("freq", 55.0),
        ("mode", 0.0),
        ("cps", 0.5),
        ("onset-time", 0.0),
        ("wave", 0.0),
        ("cutoff", 800.0),
        ("res", 0.3),
        ("drive", 0.2),
        ("detune", 0.15),
        ("ratio", 1.0),
        ("index", 1.0),
        ("amp-attack", 0.002),
        ("amp-decay", 0.3),
        ("sustain", 1.0),
        ("release", 0.05),
        ("lfo-wave", 0.0),
        ("lfo-rate", 4.0),
        ("lfo-depth", 0.0),
        ("lfo-offset", 0.0),
        ("lfo-retrigger", 1.0),
        ("lfo-sync", 1.0),
        ("gate-length", 1.0),
        ("env-mod", 2.0),
        ("env-decay", 0.2),
        ("accent", 0.0),
        ("slide-from", 0.0),
        ("slide-time", 0.06),
        ("sub-level", 0.0),
        ("fm-feedback", 0.0),
        ("fold", 0.0),
        ("bit-depth", 16.0),
        ("click-level", 0.0),
    ];
    for (index, (name, default)) in expected.iter().enumerate() {
        assert_eq!(bass_voice::PORTS[index].0, *name, "port {index}");
        assert_eq!(
            bass_voice::PORTS[index].1,
            *default,
            "default at port {index}"
        );
    }
    for left in 0..bass_voice::PORT_COUNT {
        for right in left + 1..bass_voice::PORT_COUNT {
            assert_ne!(bass_voice::PORTS[left].0, bass_voice::PORTS[right].0);
        }
    }
}

#[test]
fn bass_voice_model_from_port_rounds_and_clamps() {
    use bass_voice::Model;

    assert_eq!(Model::from_port(1.4), Model::Acid);
    assert_eq!(Model::from_port(5.0), Model::Reese);
    assert_eq!(Model::from_port(9.0), Model::Reese);
    assert_eq!(Model::from_port(-3.0), Model::Analog);
    assert_eq!(Model::from_port(f32::NAN), Model::Analog);
}

#[test]
fn bass_voice_sanitize_flushes_denormals_and_non_finite() {
    assert_eq!(bass_voice::sanitize(1.0e-25), 0.0);
    assert_eq!(bass_voice::sanitize(f32::NAN), 0.0);
    assert_eq!(bass_voice::sanitize(f32::INFINITY), 0.0);
    assert_eq!(bass_voice::sanitize(0.5), 0.5);
    assert_eq!(bass_voice::sanitize(-1.0e-10), -1.0e-10);
}

#[test]
fn bass_voice_render_is_finite_and_bounded() {
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
        seed: 8,
    };
    let inputs = std::array::from_fn(|index| {
        bass_voice::PORTS
            .get(index)
            .map_or(Inp::Val(0.0), |(_, value)| Inp::Val(*value))
    });
    let mut state = NodeState::default();
    let mut memory = [0.0; bass_voice::STATE_FLOATS];
    let mut output = [0.0; 256];

    bass_voice::render(&inputs, &mut state, &mut memory, &mut output, &kx);

    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(output.iter().all(|sample| sample.abs() <= 1.0));
}

fn voice_inputs(overrides: &[(usize, f32)]) -> [Inp<'static>; MAX_PORTS] {
    let mut inputs = std::array::from_fn(|index| {
        bass_voice::PORTS
            .get(index)
            .map_or(Inp::Val(0.0), |(_, value)| Inp::Val(*value))
    });
    for &(port, value) in overrides {
        inputs[port] = Inp::Val(value);
    }
    inputs
}

fn render_voice(
    overrides: &[(usize, f32)],
    frames: usize,
    seed: u32,
) -> (Vec<f32>, NodeState, Vec<f32>) {
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
        seed,
    };
    let inputs = voice_inputs(overrides);
    let mut state = NodeState::default();
    let mut memory = vec![0.0; bass_voice::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for block in output.chunks_mut(256) {
        bass_voice::render(&inputs, &mut state, &mut memory, block, &kx);
    }
    (output, state, memory)
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max)
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

fn centroid(samples: &[f32]) -> f32 {
    let n = samples.len().min(2048);
    let mut weighted = 0.0;
    let mut magnitude_sum = 0.0;
    for bin in 1..(n / 2) {
        let mut real = 0.0;
        let mut imag = 0.0;
        for (index, sample) in samples[..n].iter().enumerate() {
            let window = 0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (n - 1) as f32).cos();
            let angle = std::f32::consts::TAU * bin as f32 * index as f32 / n as f32;
            real += sample * window * angle.cos();
            imag -= sample * window * angle.sin();
        }
        let magnitude = real.hypot(imag);
        weighted += bin as f32 * magnitude;
        magnitude_sum += magnitude;
    }
    weighted / magnitude_sum.max(1.0e-12)
}

fn first_divergence(left: &[f32], right: &[f32]) -> Option<usize> {
    left.iter().zip(right.iter()).position(|(a, b)| a != b)
}

#[test]
fn bass_voice_models_render_at_default_levels() {
    for model in 0..6 {
        for frequency in [55.0, 110.0] {
            let (audio, _, _) = render_voice(
                &[
                    (bass_voice::port::MODE, model as f32),
                    (bass_voice::port::FREQ, frequency),
                ],
                48_000,
                41,
            );
            assert!(
                audio
                    .iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 1.0),
                "model {model}"
            );
            assert!(
                rms(&audio) > 1.0e-3,
                "silent model {model} at {frequency} Hz"
            );
            if frequency == 55.0 {
                let level = peak(&audio);
                println!("bass model {model} measured default peak {level:.6}");
                assert!((0.2..=1.0).contains(&level), "model {model} peak {level}");
            }
        }
    }
}

#[test]
fn bass_voice_accent_increases_level_and_filter_brightness() {
    let common = [
        (bass_voice::port::MODE, 1.0),
        (bass_voice::port::FREQ, 55.0),
    ];
    let (plain, _, _) = render_voice(&common, 7200, 7);
    let (accented, _, _) = render_voice(
        &[
            (common[0].0, common[0].1),
            (common[1].0, common[1].1),
            (bass_voice::port::ACCENT, 1.0),
        ],
        7200,
        7,
    );
    let plain_peak = peak(&plain);
    let accent_peak = peak(&accented);
    assert!(
        20.0 * (accent_peak / plain_peak).log10() >= 2.0,
        "plain={plain_peak}, accented={accent_peak}"
    );
    assert!(centroid(&accented[..2400]) > centroid(&plain[..2400]));
}

#[test]
fn bass_voice_sub_slide_starts_low_and_settles_to_target_pitch() {
    let args = [
        (bass_voice::port::MODE, 4.0),
        (bass_voice::port::WAVE, 4.0),
        (bass_voice::port::FREQ, 110.0),
        (bass_voice::port::CUTOFF, 20_000.0),
        (bass_voice::port::RES, 0.0),
        (bass_voice::port::SLIDE_FROM, -12.0),
        (bass_voice::port::SLIDE_TIME, 0.06),
        (bass_voice::port::GATE_LENGTH, 8.0),
    ];
    let (audio, state, memory) = render_voice(&args, 12_288, 3);
    let crossings: Vec<usize> = audio
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, sample)| (audio[i - 1] <= 0.0 && *sample > 0.0).then_some(i))
        .collect();
    assert!(
        crossings.len() > 10,
        "crossings={}, peak={}, first={:?}",
        crossings.len(),
        peak(&audio),
        (&audio[..audio.len().min(24)], state, &memory[..8])
    );
    let first_period = crossings[1] - crossings[0];
    assert!((1.3 * 48_000.0 / 110.0..2.05 * 48_000.0 / 110.0).contains(&(first_period as f32)));
    let later: Vec<f32> = crossings
        .windows(2)
        .filter_map(|pair| {
            (pair[0] >= 3840 && pair[0] < 5760).then_some((pair[1] - pair[0]) as f32)
        })
        .collect();
    let mean_period = later.iter().sum::<f32>() / later.len() as f32;
    assert!(
        (48_000.0 / 110.0 * 0.99..=48_000.0 / 110.0 * 1.01).contains(&mean_period),
        "mean={mean_period}"
    );
    assert!(peak(&audio[..120]) >= 0.9 * peak(&audio[2400..3600]));
}

#[test]
fn bass_voice_slide_does_not_retrigger_filter_envelope() {
    let base = [
        (bass_voice::port::MODE, 1.0),
        (bass_voice::port::FREQ, 55.0),
        (bass_voice::port::ENV_MOD, 4.0),
        (bass_voice::port::CUTOFF, 220.0),
        (bass_voice::port::GATE_LENGTH, 8.0),
    ];
    let (plain, _, _) = render_voice(&base, 2048, 2);
    let (slide, _, _) = render_voice(
        &[
            (base[0].0, base[0].1),
            (base[1].0, base[1].1),
            (base[2].0, base[2].1),
            (base[3].0, base[3].1),
            (base[4].0, base[4].1),
            (bass_voice::port::SLIDE_FROM, -5.0),
        ],
        2048,
        2,
    );
    assert!(centroid(&slide[..1440]) < centroid(&plain[..1440]));

    // Pitch-controlled comparison: with the minimum slide time the pitch settles to the
    // target within a few samples, so any remaining centroid drop comes from the
    // (non-retriggered) filter envelope rather than from the lower starting pitch.
    let (slide_fast, _, _) = render_voice(
        &[
            (base[0].0, base[0].1),
            (base[1].0, base[1].1),
            (base[2].0, base[2].1),
            (base[3].0, base[3].1),
            (base[4].0, base[4].1),
            (bass_voice::port::SLIDE_FROM, -5.0),
            (bass_voice::port::SLIDE_TIME, 0.0001),
        ],
        2048,
        2,
    );
    let plain_centroid = centroid(&plain[..1440]);
    let slide_centroid = centroid(&slide[..1440]);
    let slide_fast_centroid = centroid(&slide_fast[..1440]);
    println!(
        "slide centroids: plain={plain_centroid:.4}, slide(0.06)={slide_centroid:.4}, \
         slide(0.0001)={slide_fast_centroid:.4}, ratio={:.4}",
        slide_fast_centroid / plain_centroid
    );
    assert!(
        slide_fast_centroid < 0.8 * plain_centroid,
        "plain={plain_centroid}, slide_fast={slide_fast_centroid}"
    );
}

#[test]
fn bass_voice_gate_length_owns_release_and_finish() {
    let args = [
        (bass_voice::port::MODE, 4.0),
        (bass_voice::port::WAVE, 4.0),
        (bass_voice::port::CPS, 0.5),
        (bass_voice::port::GATE_LENGTH, 1.0),
        (bass_voice::port::RELEASE, 0.01),
    ];
    let (audio, state, memory) = render_voice(&args, 6912, 5);
    assert_ne!(audio[5900], 0.0);
    assert!(audio[6480..].iter().all(|sample| *sample == 0.0));
    assert!(state.done());
    assert!(memory.iter().all(|value| *value == 0.0));
    let (_, state, _) = render_voice(
        &[
            (args[0].0, args[0].1),
            (args[1].0, args[1].1),
            (bass_voice::port::CPS, 0.5625),
            (args[3].0, args[3].1),
            (args[4].0, args[4].1),
        ],
        6144,
        5,
    );
    assert!(state.done());

    // Note-off timing: the output of a short gate diverges from a never-released long
    // gate exactly where the release starts (within one sample for a zero crossing).
    for (cps, expected) in [(0.5, 6000_usize), (0.5625, 5333_usize)] {
        let voice = |gate_length: f32| {
            render_voice(
                &[
                    (args[0].0, args[0].1),
                    (args[1].0, args[1].1),
                    (bass_voice::port::CPS, cps),
                    (bass_voice::port::GATE_LENGTH, gate_length),
                    (args[4].0, args[4].1),
                ],
                6912,
                5,
            )
            .0
        };
        let short = voice(1.0);
        let long = voice(64.0);
        let first = first_divergence(&short, &long).expect("release must diverge");
        println!("cps {cps}: first gate divergence at sample {first} (expected {expected})");
        assert!(
            first.abs_diff(expected) <= 1,
            "cps={cps}, first divergence={first}, expected={expected}"
        );
    }
}

#[test]
fn bass_voice_finish_keeps_audio_rendered_before_release_end() {
    // The release port clamps to 1e-4 s, so the voice finishes a few samples after the
    // note-off at sample 6000, inside the block that starts at 5888. Samples rendered
    // earlier in that block must survive; only the rest of the block is zeroed.
    let args = [
        (bass_voice::port::MODE, 4.0),
        (bass_voice::port::WAVE, 4.0),
        (bass_voice::port::CPS, 0.5),
        (bass_voice::port::GATE_LENGTH, 1.0),
        (bass_voice::port::RELEASE, 0.0),
    ];
    let (audio, state, memory) = render_voice(&args, 6912, 5);
    assert_ne!(audio[5900], 0.0);
    assert_ne!(audio[5990], 0.0);
    assert!(audio[6010..].iter().all(|sample| *sample == 0.0));
    assert!(state.done());
    assert!(memory.iter().all(|value| *value == 0.0));

    let long_args = [
        (args[0].0, args[0].1),
        (args[1].0, args[1].1),
        (args[2].0, args[2].1),
        (bass_voice::port::GATE_LENGTH, 64.0),
        (args[4].0, args[4].1),
    ];
    let (long, _, _) = render_voice(&long_args, 6912, 5);
    let first = first_divergence(&audio, &long).expect("release must diverge");
    assert!(first >= 5999, "first divergence={first}");
}

#[test]
fn bass_voice_wobble_autocorrelation_tracks_synced_and_free_rate() {
    for (sync, expected_lag) in [(1.0, 50_usize), (0.0, 25_usize)] {
        let args = [
            (bass_voice::port::MODE, 3.0),
            (bass_voice::port::FREQ, 200.0),
            (bass_voice::port::LFO_DEPTH, 1.0),
            (bass_voice::port::LFO_RATE, 4.0),
            (bass_voice::port::LFO_SYNC, sync),
            (bass_voice::port::CPS, 0.5),
            (bass_voice::port::GATE_LENGTH, 64.0),
            (bass_voice::port::WAVE, 4.0),
            (bass_voice::port::DETUNE, 0.0),
            (bass_voice::port::CUTOFF, 20.0),
            (bass_voice::port::RES, 0.0),
            (bass_voice::port::DRIVE, 0.0),
            (bass_voice::port::ENV_MOD, 0.0),
        ];
        let (audio, _, _) = render_voice(&args, 96_000, 9);
        let frames: Vec<f32> = audio.chunks_exact(480).map(rms).collect();
        let settled = &frames[40..];
        let mean = settled.iter().sum::<f32>() / settled.len() as f32;
        let mut correlations = vec![0.0; 70];
        for (lag, correlation) in correlations.iter_mut().enumerate().skip(1) {
            *correlation = settled
                .iter()
                .take(settled.len() - lag)
                .zip(settled.iter().skip(lag))
                .map(|(a, b)| (a - mean) * (b - mean))
                .sum();
        }
        let best = (10..70)
            .max_by(|a, b| correlations[*a].total_cmp(&correlations[*b]))
            .unwrap();
        assert!(
            best.abs_diff(expected_lag) <= 2,
            "sync={sync}, lag={best}, target-correlation={:?}",
            &correlations[expected_lag.saturating_sub(2)..=expected_lag + 2]
        );
    }
}

#[test]
fn bass_voice_extreme_control_grid_stays_finite_and_bounded() {
    for model in 0..6 {
        for frequency in [20.0, 2000.0] {
            for (cutoff, res, drive) in [(20.0, 0.0, 0.0), (20_000.0, 1.0, 1.0)] {
                let args = [
                    (bass_voice::port::MODE, model as f32),
                    (bass_voice::port::FREQ, frequency),
                    (bass_voice::port::CUTOFF, cutoff),
                    (bass_voice::port::RES, res),
                    (bass_voice::port::DRIVE, drive),
                    (bass_voice::port::INDEX, 32.0),
                    (bass_voice::port::FM_FEEDBACK, 1.0),
                    (bass_voice::port::FOLD, 1.0),
                    (bass_voice::port::BIT_DEPTH, 2.0),
                    (
                        bass_voice::port::LFO_DEPTH,
                        if cutoff == 20.0 { -1.0 } else { 1.0 },
                    ),
                    (
                        bass_voice::port::SLIDE_FROM,
                        if frequency == 20.0 { -24.0 } else { 24.0 },
                    ),
                    (bass_voice::port::GATE_LENGTH, 64.0),
                ];
                let (audio, _, _) = render_voice(&args, 96_000, 11);
                assert!(
                    audio
                        .iter()
                        .all(|sample| sample.is_finite() && sample.abs() <= 1.0),
                    "model={model}, freq={frequency}, cutoff={cutoff}"
                );
            }
        }
    }
}

#[test]
fn bass_voice_seed_determinism_is_model_scoped() {
    for model in 0..6 {
        let args = [
            (bass_voice::port::MODE, model as f32),
            (bass_voice::port::GATE_LENGTH, 8.0),
        ];
        let (one, _, _) = render_voice(&args, 4096, 1);
        let (same, _, _) = render_voice(&args, 4096, 1);
        assert_eq!(one, same, "model={model}");
        let (other, _, _) = render_voice(&args, 4096, 2);
        if model == 3 || model == 5 {
            assert_ne!(one, other, "detuned model={model}");
        } else {
            assert_eq!(one, other, "seed-independent model={model}");
        }
    }
}

#[test]
fn bass_voice_render_does_not_allocate() {
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
        seed: 23,
    };
    let inputs = voice_inputs(&[(bass_voice::port::MODE, 3.0)]);
    let mut state = NodeState::default();
    let mut memory = [0.0; bass_voice::STATE_FLOATS];
    let mut output = [0.0; 256];
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        bass_voice::render(&inputs, &mut state, &mut memory, &mut output, &kx);
    });
    assert_eq!(allocations, 0);
}
