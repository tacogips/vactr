//! Production lo-fi controls, stereo isolation, deterministic clocks and installs.
use super::{bus_def, caps, chain, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{encode_bus, encode_inst, SampleStore, StoreKind};
use crate::dsp::cells::AtomicCells;
use crate::dsp::effects::{self, catalog::spec, FxCtx, FxStats, FxUnit};
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::{EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::Ctl;

fn render(
    p: &[(&str, f32)],
    left: &[f32],
    right: &[f32],
    sr: f32,
    block: usize,
) -> (Vec<f32>, Vec<f32>) {
    let cap = caps();
    let cells = AtomicCells::new(4);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let kind = EffectKind::Lofi;
    let mut memory = vec![0.0; effects::mem_len(kind, sr, &cap)];
    let mut unit = FxUnit::empty();
    let given: Vec<_> = p
        .iter()
        .map(|(name, v)| (effects::param_ctl(kind, name).unwrap(), Ctl::Const(*v)))
        .collect();
    unit.configure(kind, &given, &cells, &mut memory, sr, &cap);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; 1024];
    let mut dry = vec![0.0; 2 * block];
    let mut stats = FxStats::default();
    let mut l = left.to_vec();
    let mut r = right.to_vec();
    for (l, r) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        let (_, allocations) = armed(|| unit.run(&mut memory, l, r, &mut dry, &mut ctx));
        assert_eq!(allocations, 0);
    }
    (l, r)
}
fn signal(sr: f32, n: usize, f: f32) -> Vec<f32> {
    (0..n)
        .map(|i| (std::f32::consts::TAU * f * i as f32 / sr).sin() * 0.4)
        .collect()
}
fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum::<f32>() / a.len() as f32
}
#[test]
fn lofi_composite_mix_zero_is_bit_exact_and_zero_noise_stays_silent() {
    let input = signal(48000.0, 16384, 375.0);
    let opposite: Vec<_> = input.iter().map(|v| -v).collect();
    let (l, r) = render(
        &[("mix", 0.0), ("hiss", 1.0), ("crackle", 1.0)],
        &input,
        &opposite,
        48000.0,
        127,
    );
    assert_eq!(l, input);
    assert_eq!(r, opposite);
    let silence = vec![0.0; 16384];
    let (l, r) = render(&[], &silence, &silence, 48000.0, 127);
    assert!(l.iter().chain(&r).all(|&v| v == 0.0));
    let (l, r) = render(&[], &input, &silence, 48000.0, 127);
    assert!(r.iter().all(|&v| v == 0.0));
    assert!(rms(&l) > 0.1);
}
#[test]
fn lofi_composite_all_controls_change_the_intended_processing() {
    let sr = 48000.0;
    let input = signal(sr, 48000, 1625.0);
    let base = render(&[], &input, &input, sr, 127).0;
    for control in [
        ("tone", 500.0),
        ("drive", 8.0),
        ("wow", 0.9),
        ("flutter", 0.9),
        ("bits", 4.0),
        ("rate", 3000.0),
        ("hiss", 0.7),
        ("crackle", 0.7),
        ("mix", 0.0),
    ] {
        let out = render(&[control], &input, &input, sr, 127).0;
        assert!(difference(&base, &out) > 1e-5, "{control:?} ineffective");
    }
    let clean = render(
        &[
            ("wow", 0.0),
            ("flutter", 0.0),
            ("tone", 18000.0),
            ("drive", 0.1),
            ("bits", 4.0),
            ("rate", 96000.0),
        ],
        &input,
        &input,
        sr,
        127,
    )
    .0;
    assert!(clean
        .iter()
        .all(|v| (v * 8.0 - (v * 8.0).round()).abs() < 1e-5));
    let low = signal(sr, 48000, 200.0);
    let high = signal(sr, 48000, 6000.0);
    let lowout = render(
        &[("tone", 700.0), ("bits", 24.0), ("rate", 96000.0)],
        &low,
        &low,
        sr,
        127,
    )
    .0;
    let highout = render(
        &[("tone", 700.0), ("bits", 24.0), ("rate", 96000.0)],
        &high,
        &high,
        sr,
        127,
    )
    .0;
    assert!(rms(&lowout[1000..]) > rms(&highout[1000..]) * 20.0);
}
#[test]
fn lofi_composite_stereo_timing_partitions_and_valid_extremes_remain_finite() {
    for sr in [44100.0, 48000.0, 96000.0] {
        let input = signal(sr, 32768, 733.0);
        let settings = [
            ("hiss", 0.2),
            ("crackle", 0.4),
            ("wow", 0.9),
            ("flutter", 0.7),
        ];
        let (base_l, base_r) = render(&settings, &input, &input, sr, 32768);
        let (other_l, other_r) = render(&settings, &input, &input, sr, 31);
        assert_eq!(base_l, other_l);
        assert_eq!(base_r, other_r);
        let (l, r) = render(&[], &input, &input, sr, 127);
        assert_eq!(l, r, "shared stereo timing {sr}");
        for upper in [false, true] {
            let p: Vec<_> = effects::params(EffectKind::Lofi)
                .iter()
                .map(|d| (d.name, if upper { d.max } else { d.min }))
                .collect();
            let (l, r) = render(&p, &input, &input, sr, 63);
            assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() < 20.0));
        }
        let p: Vec<_> = effects::params(EffectKind::Lofi)
            .iter()
            .map(|d| (d.name, f32::NAN))
            .collect();
        let (l, r) = render(&p, &input, &input, sr, 63);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()));
    }
}
#[test]
fn lofi_composite_native_browser_bus_and_voice_install_and_old_ids_stay_stable() {
    assert_eq!(EffectKind::ALL.last(), Some(&EffectKind::Lofi));
    let id = EffectKind::ALL
        .iter()
        .position(|&k| k == EffectKind::Lofi)
        .unwrap();
    assert_eq!(EffectKind::ALL[id - 1], EffectKind::DiffusionDelay);
    let effect = spec(EffectKind::Lofi, &[]).unwrap();
    let bus = bus_def(3, vec![effect.clone()]);
    let inst = chain(1, vec![UGenSpec::Saw, UGenSpec::Effect(effect)]);
    let mut native = NativeRig::native();
    native.install(&inst);
    native.install_bus(&bus, false);
    let _ = native.step();
    native.send(event(
        1,
        native.engine.now(),
        &[(ctl::LEGATO, 10.0), (ctl::BUS, 3.0)],
    ));
    let (l, r) = native.run(16);
    assert!(rms(&l) > 1e-4 && rms(&r) > 1e-4);
    let mut browser = BrowserRig::browser(4 << 20);
    for (resource, isbus) in [(1, false), (2, true)] {
        let mut bytes = Vec::new();
        if isbus {
            encode_bus(&bus, false, &mut bytes).unwrap();
            assert_eq!(usize::from(bytes[7]), id);
        } else {
            encode_inst(&inst, &mut bytes).unwrap();
        }
        let mut record = Vec::new();
        encode_graph_record(resource, 1, &bytes, &mut record);
        browser.push(&record);
    }
    let _ = browser.run(4);
    browser.send(event(
        1,
        browser.engine.now(),
        &[(ctl::LEGATO, 10.0), (ctl::BUS, 3.0)],
    ));
    let (l, r) = browser.run(16);
    assert!(rms(&l) > 1e-4 && rms(&r) > 1e-4);
    let declaration = crate::dsp::meta::decl_for("lofi").unwrap();
    assert_eq!(declaration.params.len(), 9);
    assert_eq!(
        declaration
            .params
            .iter()
            .find(|p| p.name == "tone")
            .unwrap()
            .unit,
        crate::dsp::meta::Unit::Hz
    );
    assert_eq!(
        declaration
            .params
            .iter()
            .find(|p| p.name == "hiss")
            .unwrap()
            .unit,
        crate::dsp::meta::Unit::None
    );
}

#[test]
fn lofi_composite_hold_rate_is_in_physical_hz_at_all_sample_rates() {
    for sr in [44100.0, 48000.0, 96000.0] {
        let input = signal(sr, 32768, 997.0);
        let (out, _) = render(
            &[
                ("rate", 500.0),
                ("wow", 0.0),
                ("flutter", 0.0),
                ("bits", 24.0),
            ],
            &input,
            &input,
            sr,
            127,
        );
        let changes = out[1000..].windows(2).filter(|w| w[0] != w[1]).count();
        let expected = 500.0 * (out.len() - 1001) as f32 / sr;
        assert!(
            (changes as f32 - expected).abs() < 2.0,
            "{sr}: {changes} != {expected}"
        );
    }
}

#[test]
fn lofi_composite_automation_and_disabling_existing_clicks_are_bounded() {
    use crate::dsp::effects::FxState;
    let sr = 48000.0;
    let kind = EffectKind::Lofi;
    let cap = caps();
    let mut memory = vec![0.0; effects::mem_len(kind, sr, &cap)];
    let mut state = FxState::default();
    effects::init(kind, &mut state, &mut memory, sr, &cap);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut parameters: Vec<_> = effects::params(kind).iter().map(|p| p.default).collect();
    for block in 0..300 {
        for (value, definition) in parameters.iter_mut().zip(effects::params(kind)) {
            *value = if block % 2 == 0 {
                definition.max
            } else {
                definition.min
            };
        }
        let mut l = [0.0; 64];
        let mut r = [0.0; 64];
        l[0] = 0.5;
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        let (_, allocations) = armed(|| {
            effects::process(
                kind,
                &parameters,
                &mut state,
                &mut memory,
                &mut l,
                &mut r,
                &mut ctx,
            )
        });
        assert_eq!(allocations, 0);
        assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() < 20.0));
    }
    // Isolate stored surface-noise tails: zero is an exact disable, even after a pop.
    state = FxState::default();
    effects::init(kind, &mut state, &mut memory, sr, &cap);
    memory.fill(0.0);
    state.s[3] = 0.25;
    state.s[4] = -0.25;
    parameters = effects::params(kind).iter().map(|p| p.default).collect();
    let mut l = [0.0; 64];
    let mut r = [0.0; 64];
    let mut ctx = FxCtx {
        sr,
        store: &store,
        fft: &fft,
        caps: &cap,
        scratch: &mut scratch,
        analysis: &mut analysis,
        stats: &mut stats,
    };
    effects::process(
        kind,
        &parameters,
        &mut state,
        &mut memory,
        &mut l,
        &mut r,
        &mut ctx,
    );
    assert!(l.iter().chain(&r).all(|&v| v == 0.0));
}
