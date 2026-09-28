//! Shared Clouds quality selector behavior across all four texture modes.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_bus, encode_inst, SampleStore, StoreKind};
use crate::dsp::cells::AtomicCells;
use crate::dsp::effects::{self, texture, FxCtx, FxStats, FxUnit};
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::EffectKind;
use crate::dsp::graph::UGenSpec;
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::Ctl;

const MODES: [EffectKind; 4] = [
    EffectKind::TextureGrain,
    EffectKind::TextureStretch,
    EffectKind::TextureLoop,
    EffectKind::TextureSpectral,
];

fn run(kind: EffectKind, quality: Option<f32>, freeze: bool) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0;
    let cap = caps();
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let cells = AtomicCells::new(1);
    let mut mem = vec![0.0; effects::mem_len(kind, sr, &cap)];
    let mut unit = FxUnit::empty();
    let mut params = vec![
        (effects::param_ctl(kind, "mix").unwrap(), Ctl::Const(1.0)),
        (
            effects::param_ctl(kind, "density").unwrap(),
            Ctl::Const(0.9),
        ),
    ];
    if let Some(value) = quality {
        params.push((
            effects::param_ctl(kind, "quality").unwrap(),
            Ctl::Const(value),
        ));
    }
    unit.configure(kind, &params, &cells, &mut mem, sr, &cap);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; 1024];
    let mut dry = vec![0.0; 512];
    let mut stats = FxStats::default();
    let mut result_l = Vec::new();
    let mut result_r = Vec::new();
    for block in 0..120 {
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        for i in 0..256 {
            let t = (block * 256 + i) as f32 / sr;
            l[i] = (t * 317.0 * std::f32::consts::TAU).sin() * 0.45;
            r[i] = (t * 521.0 * std::f32::consts::TAU).sin() * 0.35;
        }
        if freeze && block == 90 {
            unit.set(9, 1.0);
            unit.set(10, 1.0);
        }
        if freeze && block == 91 {
            unit.set(10, 0.0);
        }
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        unit.run(&mut mem, &mut l, &mut r, &mut dry, &mut ctx);
        if block >= 100 {
            result_l.extend(l);
            result_r.extend(r);
        }
    }
    (result_l, result_r)
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn quality_bits_change_all_modes_and_zero_is_bit_identical_to_default() {
    for kind in MODES {
        assert_eq!(texture::PARAMS.len(), 13);
        let default = run(kind, None, false);
        let high_stereo = run(kind, Some(0.0), false);
        assert_eq!(default, high_stereo, "{kind:?}: quality zero compatibility");
        let high_mono = run(kind, Some(1.0), false);
        let low_stereo = run(kind, Some(2.0), false);
        let low_mono = run(kind, Some(3.0), false);
        for (l, r) in [&default, &high_mono, &low_stereo, &low_mono] {
            assert!(l.iter().chain(r).all(|sample| sample.is_finite()));
            assert!(rms(l) > 1.0e-6, "{kind:?}: audible quality output");
        }
        assert_eq!(high_mono.0, high_mono.1, "{kind:?}: dual mono");
        assert_eq!(low_mono.0, low_mono.1, "{kind:?}: dual mono low fidelity");
        assert!(
            low_mono.0.iter().all(|sample| {
                let step = sample * 127.0;
                (step - step.round()).abs() < 1.0e-4
            }),
            "{kind:?}: final mono output stays on the 8-bit grid"
        );
        assert!(
            difference(&default.0, &low_stereo.0) > 1.0e-5,
            "{kind:?}: low fidelity response"
        );
        assert!(
            difference(&high_mono.0, &low_mono.0) > 1.0e-5,
            "{kind:?}: combined bits response"
        );
    }
}

#[test]
fn low_fidelity_freeze_and_trigger_stay_finite() {
    for kind in MODES {
        let (l, r) = run(kind, Some(3.0), true);
        assert!(l.iter().chain(&r).all(|sample| sample.is_finite()));
        assert_eq!(l, r, "{kind:?}: dual mono after freeze/trigger");
        assert!(rms(&l) > 1.0e-6, "{kind:?}: frozen audio");
    }
}

#[test]
fn editor_marks_quality_as_four_way_stepped_control() {
    for kind in MODES {
        let editor = crate::types::manifest::HostManifest::spec_default()
            .editor_decl(kind.name())
            .unwrap();
        let quality = editor
            .params
            .iter()
            .find(|param| param.name == "quality")
            .unwrap();
        assert_eq!(quality.range, (0.0, 3.0));
        assert_eq!(quality.curve, crate::dsp::meta::Curve::Stepped);
        assert_eq!(quality.ctl, effects::param_ctl(kind, "quality").unwrap());
    }
}

#[test]
fn quality_three_survives_native_browser_rate_and_block_matrix() {
    let sources = [
        chain(1, vec![UGenSpec::SinOsc]),
        chain(2, vec![UGenSpec::Saw]),
    ];
    for kind in MODES {
        let bus = bus_def(
            3,
            vec![effects::catalog::spec(
                kind,
                &[("quality", Ctl::Const(3.0)), ("mix", Ctl::Const(1.0))],
            )
            .unwrap()],
        );
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let blocks = ((sr * 0.35) as usize / block) + 1;
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut native = NativeRig::native_with(cfg);
                for source in &sources {
                    native.install(source);
                }
                native.install_bus(&bus, false);
                let _ = native.step();
                let t = native.engine.now();
                for (inst, pan) in [(1, 0.0), (2, 1.0)] {
                    native.send(event(
                        inst,
                        t,
                        &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 1.0)],
                    ));
                }
                let (l, r) = native.run(blocks);
                assert!(l.iter().chain(&r).all(|sample| sample.is_finite()));
                assert!(rms(&l) > 1.0e-6, "{kind:?} native {sr}/{block}");
                assert_eq!(l, r, "{kind:?} native dual mono {sr}/{block}");

                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut browser = BrowserRig::browser_with(cfg);
                for (resource, source) in [(1, &sources[0]), (2, &sources[1])] {
                    let mut bytes = Vec::new();
                    encode_inst(source, &mut bytes).unwrap();
                    let mut record = Vec::new();
                    encode_graph_record(resource, 1, &bytes, &mut record);
                    browser.push(&record);
                }
                let mut bytes = Vec::new();
                encode_bus(&bus, false, &mut bytes).unwrap();
                let mut record = Vec::new();
                encode_graph_record(3, 1, &bytes, &mut record);
                browser.push(&record);
                let _ = browser.run(6);
                let t = browser.engine.now();
                for (inst, pan) in [(1, 0.0), (2, 1.0)] {
                    browser.send(event(
                        inst,
                        t,
                        &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 1.0)],
                    ));
                }
                let (l, r) = browser.run(blocks);
                assert!(l.iter().chain(&r).all(|sample| sample.is_finite()));
                assert!(rms(&l) > 1.0e-6, "{kind:?} browser {sr}/{block}");
                assert_eq!(l, r, "{kind:?} browser dual mono {sr}/{block}");
            }
        }
    }
}
