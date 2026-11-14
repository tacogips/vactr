//! Mathematical, spectral, statistical and install evidence for synth kernels.
use super::{caps, chain, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_inst, SampleStore, StoreKind};
use crate::dsp::effects::FxStats;
use crate::dsp::graph::UGenSpec;
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, musicdsp_synth as synth, Inp, Kx, Node, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

fn render(
    spec: UGenSpec,
    overrides: &[(&str, f32)],
    sr: f32,
    n: usize,
    partition: usize,
) -> Vec<f32> {
    let node = catalog::node_of(&spec);
    let ports = catalog::ports(&node);
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, p) in ports.iter().enumerate() {
        ins[i] = Inp::Val(p.default);
    }
    for &(name, v) in overrides {
        ins[ports.iter().position(|p| p.name == name).unwrap()] = Inp::Val(v);
    }
    let mut st = NodeState::default();
    let mut mem = vec![0.0; synth::AM_STATE_FLOATS];
    let store = SampleStore::new(StoreKind::NativeArc);
    let cap = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: usize::MAX,
        bank: None,
        store: &store,
        caps: &cap,
        stats: &mut stats,
        seed: 1729,
    };
    let mut out = vec![0.0; n];
    for chunk in out.chunks_mut(partition) {
        let (_, allocations) = crate::dsp::alloc_probe::armed(|| match node {
            Node::DsfOsc => synth::dsf(&ins, &mut st, chunk, &kx),
            Node::ChebyshevOsc => synth::chebyshev(&ins, &mut st, chunk, &kx),
            Node::GaussianNoise => synth::gaussian(&ins, &mut st, chunk, &kx),
            Node::LorenzOsc => synth::lorenz(&ins, &mut st, chunk, &kx),
            Node::RosslerOsc => synth::rossler(&ins, &mut st, chunk, &kx),
            Node::AmFormantOsc => synth::am_formant(&ins, &mut st, &mut mem, chunk, &kx),
            _ => panic!("unexpected synth"),
        });
        assert_eq!(allocations, 0);
    }
    out
}
fn cosine_bin(x: &[f32], bin: usize) -> f32 {
    2.0 * x
        .iter()
        .enumerate()
        .map(|(i, &v)| v * (TAU * bin as f32 * i as f32 / x.len() as f32).cos())
        .sum::<f32>()
        / x.len() as f32
}
fn magnitude(x: &[f32], bin: usize) -> f32 {
    let re = cosine_bin(x, bin);
    let im = 2.0
        * x.iter()
            .enumerate()
            .map(|(i, &v)| v * (TAU * bin as f32 * i as f32 / x.len() as f32).sin())
            .sum::<f32>()
        / x.len() as f32;
    re.hypot(im)
}
fn specs() -> [UGenSpec; 6] {
    [
        UGenSpec::DsfOsc,
        UGenSpec::ChebyshevOsc,
        UGenSpec::GaussianNoise,
        UGenSpec::LorenzOsc,
        UGenSpec::RosslerOsc,
        UGenSpec::AmFormantOsc,
    ]
}

#[test]
fn dsf_closed_form_matches_explicit_finite_sum_including_singular_limits() {
    for count in [1, 2, 17, 128] {
        for rolloff in [0.0f32, 0.3, 0.99, 0.999999, 1.0] {
            for spacing in [0.0f32, 0.00001, 0.13, 1.1, 3.1] {
                let mut sum = 0.0;
                let mut norm = 0.0;
                let mut weight = 1.0;
                for j in 0..count {
                    sum += weight * (0.7 + j as f32 * spacing).sin();
                    norm += weight;
                    weight *= rolloff;
                }
                let actual = synth::dsf_value(0.7, spacing, rolloff, count);
                assert!(
                    (actual - sum / norm).abs() < 0.0002,
                    "n {count} a {rolloff} spacing {spacing}: {actual} vs {}",
                    sum / norm
                );
            }
        }
    }
    // All upper requested partials are excluded, leaving a single sine.
    let out = render(
        UGenSpec::DsfOsc,
        &[("freq", 12000.0), ("count", 128.0), ("rolloff", 1.0)],
        48000.0,
        1024,
        127,
    );
    for (i, &y) in out.iter().enumerate() {
        assert!((y - (TAU * (i % 4) as f32 / 4.0).sin()).abs() < 1e-5);
    }
}
#[test]
fn chebyshev_signed_harmonics_match_coefficients_and_prune_nyquist() {
    let n = 8192;
    let freq = 48000.0 * 32.0 / n as f32;
    let out = render(
        UGenSpec::ChebyshevOsc,
        &[
            ("freq", freq),
            ("harmonic-1", 0.5),
            ("harmonic-2", -0.25),
            ("harmonic-5", 0.25),
        ],
        48000.0,
        n,
        31,
    );
    for (h, expected) in [(1, 0.5), (2, -0.25), (3, 0.0), (5, 0.25), (8, 0.0)] {
        assert!((cosine_bin(&out, 32 * h) - expected).abs() < 0.0002);
    }
    let high = render(
        UGenSpec::ChebyshevOsc,
        &[("freq", 13000.0), ("harmonic-1", 0.0), ("harmonic-2", 1.0)],
        48000.0,
        1024,
        64,
    );
    assert!(high.iter().all(|&v| v == 0.0));
}
#[test]
fn gaussian_seed_cache_and_normal_moments_are_measured() {
    let out = render(
        UGenSpec::GaussianNoise,
        &[("sigma", 0.4), ("mean", 0.2)],
        48000.0,
        200_000,
        127,
    );
    let other = render(
        UGenSpec::GaussianNoise,
        &[("sigma", 0.4), ("mean", 0.2)],
        48000.0,
        200_000,
        512,
    );
    assert_eq!(out, other);
    let mean = out.iter().map(|&v| f64::from(v)).sum::<f64>() / out.len() as f64;
    let variance = out
        .iter()
        .map(|&v| (f64::from(v) - mean).powi(2))
        .sum::<f64>()
        / out.len() as f64;
    let fourth = out
        .iter()
        .map(|&v| (f64::from(v) - mean).powi(4))
        .sum::<f64>()
        / out.len() as f64;
    assert!((mean - 0.2).abs() < 0.004 && (variance - 0.16).abs() < 0.003);
    assert!((fourth / (variance * variance) - 3.0).abs() < 0.05);
    let silent = render(
        UGenSpec::GaussianNoise,
        &[("sigma", 0.0), ("mean", -0.3)],
        48000.0,
        512,
        31,
    );
    assert!(silent.iter().all(|&v| v == -0.3));
}
#[test]
fn chaos_is_bounded_partition_deterministic_and_controls_are_functional() {
    for sr in [44100.0, 48000.0, 96000.0] {
        for spec in [UGenSpec::LorenzOsc, UGenSpec::RosslerOsc] {
            let base = render(spec.clone(), &[], sr, 131072, 127);
            assert_eq!(base, render(spec.clone(), &[], sr, 131072, 32768));
            assert!(base.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
            assert!(rms(&base) > 0.01);
            for control in [("rate", 3.0), ("chaos", 0.9), ("output-axis", 2.0)] {
                let changed = render(spec.clone(), &[control], sr, 131072, 61);
                let difference = base
                    .iter()
                    .zip(&changed)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f32>()
                    / base.len() as f32;
                assert!(difference > 0.01, "{spec:?} control {control:?}");
            }
        }
    }
}
#[test]
fn am_kernel_matches_double_carrier_product_and_moves_harmonic_envelope() {
    let freq = 187.5;
    let center = 740.0;
    let bandwidth = 120.0;
    let out = render(
        UGenSpec::AmFormantOsc,
        &[
            ("freq", freq),
            ("formant-1", center),
            ("bandwidth-1", bandwidth),
            ("balance", 0.0),
        ],
        48000.0,
        8192,
        127,
    );
    let target = center / freq;
    let lower = target.floor() as usize;
    let blend = target - lower as f32;
    let weights: Vec<_> = (1..=32)
        .map(|k| (-0.5 * (k as f32 * freq / bandwidth).powi(2)).exp())
        .collect();
    let norm = 1.0 + 2.0 * weights.iter().sum::<f32>();
    let dc = (1.0 - blend) * weights[lower - 1] + blend * weights[lower];
    for (i, &actual) in out.iter().enumerate() {
        let phase = TAU * (i % 256) as f32 / 256.0;
        let kernel = 1.0
            + 2.0
                * weights
                    .iter()
                    .enumerate()
                    .map(|(k, w)| w * ((k + 1) as f32 * phase).cos())
                    .sum::<f32>();
        let carrier = (1.0 - blend) * (lower as f32 * phase).cos()
            + blend * ((lower + 1) as f32 * phase).cos();
        let expected = (kernel * carrier - dc) / norm;
        assert!(
            (actual - expected).abs() < 0.0001,
            "AM product: {actual} vs {expected}"
        );
    }
    let shifted = render(
        UGenSpec::AmFormantOsc,
        &[
            ("freq", freq),
            ("formant-1", 1800.0),
            ("bandwidth-1", bandwidth),
            ("balance", 0.0),
        ],
        48000.0,
        8192,
        256,
    );
    let peak = |x: &[f32]| {
        (1..32)
            .max_by(|&a, &b| magnitude(x, a * 32).total_cmp(&magnitude(x, b * 32)))
            .unwrap()
    };
    assert!(peak(&shifted) > peak(&out) + 4);
    // Exact harmonic pitch: neighbouring Fourier bins carry negligible energy.
    assert!(magnitude(&out, 129) < 0.0001 && magnitude(&out, 130) < 0.0001);
}
#[test]
fn all_synth_defaults_extremes_partitions_and_native_browser_codec_work() {
    for spec in specs() {
        let base = render(spec.clone(), &[], 48000.0, 2048, 127);
        assert_eq!(base, render(spec.clone(), &[], 48000.0, 2048, 2048));
        assert!(base.iter().all(|v| v.is_finite()));
        assert!(rms(&base) > 1e-5);
        let graph = chain(1, vec![spec.clone()]);
        let mut native = NativeRig::native();
        native.install(&graph);
        let _ = native.step();
        native.send(event(1, native.engine.now(), &[(ctl::LEGATO, 10.0)]));
        let (left, _) = native.run(8);
        assert!(rms(&left) > 1e-6, "{spec:?} native silent");
        let mut bytes = Vec::new();
        encode_inst(&graph, &mut bytes).unwrap();
        let mut record = Vec::new();
        encode_graph_record(1, 1, &bytes, &mut record);
        let mut browser = BrowserRig::browser(4 << 20);
        browser.push(&record);
        let _ = browser.run(4);
        browser.send(event(1, browser.engine.now(), &[(ctl::LEGATO, 10.0)]));
        let (left, _) = browser.run(8);
        assert!(rms(&left) > 1e-6, "{spec:?} browser codec silent");
    }
}

#[test]
fn synth_metadata_extremes_audio_automation_and_callbacks_are_bounded() {
    for sr in [44100.0, 48000.0, 96000.0] {
        for spec in specs() {
            let node = catalog::node_of(&spec);
            let name = catalog::ugen_name(&spec);
            let metadata = crate::dsp::meta::decl_for(name).unwrap();
            let ports = catalog::ports(&node);
            assert_eq!(metadata.params.len(), ports.len());
            let mut automation = [[0.0; 64]; MAX_PORTS];
            for (i, p) in ports.iter().enumerate() {
                let parameter = metadata.params.iter().find(|d| d.name == p.name).unwrap();
                assert_eq!(parameter.default, p.default);
                for (j, v) in automation[i].iter_mut().enumerate() {
                    *v = if j % 2 == 0 {
                        parameter.range.0
                    } else {
                        parameter.range.1
                    };
                }
                if p.name.starts_with("harmonic-") {
                    assert_eq!(parameter.range, (-1.0, 1.0));
                }
            }
            let mut ins = [Inp::Val(0.0); MAX_PORTS];
            for i in 0..ports.len() {
                ins[i] = Inp::Buf(&automation[i]);
            }
            let mut st = NodeState::default();
            let mut mem = vec![0.0; synth::AM_STATE_FLOATS];
            let store = SampleStore::new(StoreKind::NativeArc);
            let cap = caps();
            let mut stats = FxStats::default();
            let kx = Kx {
                sr,
                gate: usize::MAX,
                bank: None,
                store: &store,
                caps: &cap,
                stats: &mut stats,
                seed: 1729,
            };
            let mut out = [0.0; 64];
            for _ in 0..100 {
                let (_, allocations) = crate::dsp::alloc_probe::armed(|| match node {
                    Node::DsfOsc => synth::dsf(&ins, &mut st, &mut out, &kx),
                    Node::ChebyshevOsc => synth::chebyshev(&ins, &mut st, &mut out, &kx),
                    Node::GaussianNoise => synth::gaussian(&ins, &mut st, &mut out, &kx),
                    Node::LorenzOsc => synth::lorenz(&ins, &mut st, &mut out, &kx),
                    Node::RosslerOsc => synth::rossler(&ins, &mut st, &mut out, &kx),
                    Node::AmFormantOsc => synth::am_formant(&ins, &mut st, &mut mem, &mut out, &kx),
                    _ => panic!("unexpected synth"),
                });
                assert_eq!(allocations, 0);
                assert!(
                    out.iter().all(|v| v.is_finite() && v.abs() < 10.0),
                    "{name} automatedlimit"
                );
            }
        }
    }
}
