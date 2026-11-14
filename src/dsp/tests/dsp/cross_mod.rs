//! The two-input modulation bus effect from `.vact`-facing effect metadata
//! through native and browser graph installation to distinct main/aux audio.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::SampleStore;
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::{cross_mod, FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn def(
    algorithm: f32,
    timbre: f32,
    drive: f32,
    wave: f32,
    frequency: f32,
) -> crate::dsp::graph::BusDef {
    def_with_drives(algorithm, timbre, drive, wave, frequency, 1.0, 1.0)
}

fn def_with_drives(
    algorithm: f32,
    timbre: f32,
    drive: f32,
    wave: f32,
    frequency: f32,
    carrier_drive: f32,
    modulator_drive: f32,
) -> crate::dsp::graph::BusDef {
    let effect = spec(
        EffectKind::CrossMod,
        &[
            ("algorithm", Ctl::Const(algorithm)),
            ("timbre", Ctl::Const(timbre)),
            ("drive", Ctl::Const(drive)),
            ("carrier-wave", Ctl::Const(wave)),
            ("carrier-frequency", Ctl::Const(frequency)),
            ("carrier-drive", Ctl::Const(carrier_drive)),
            ("modulator-drive", Ctl::Const(modulator_drive)),
        ],
    )
    .unwrap();
    bus_def(3, vec![effect])
}

fn sources() -> [crate::dsp::graph::InstDef; 2] {
    [
        chain(1, vec![UGenSpec::SinOsc]),
        chain(2, vec![UGenSpec::Saw]),
    ]
}

fn send<C: crate::dsp::engine::CellStore, S: crate::dsp::ring::ControlSource>(
    rig: &mut super::Rig<C, S>,
) {
    let t = rig.engine.now();
    for (id, pan) in [(1, 0.0), (2, 1.0)] {
        rig.send(event(
            id,
            t,
            &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
        ));
    }
}

fn native(
    algorithm: f32,
    timbre: f32,
    drive: f32,
    wave: f32,
    frequency: f32,
) -> (Vec<f32>, Vec<f32>) {
    native_with_drives(algorithm, timbre, drive, wave, frequency, 1.0, 1.0)
}

fn native_with_drives(
    algorithm: f32,
    timbre: f32,
    drive: f32,
    wave: f32,
    frequency: f32,
    carrier_drive: f32,
    modulator_drive: f32,
) -> (Vec<f32>, Vec<f32>) {
    let mut rig = NativeRig::native();
    for source in sources() {
        rig.install(&source);
    }
    rig.install_bus(
        &def_with_drives(
            algorithm,
            timbre,
            drive,
            wave,
            frequency,
            carrier_drive,
            modulator_drive,
        ),
        false,
    );
    rig.install_bus(&bus_def(0, vec![]), true);
    let _ = rig.step();
    send(&mut rig);
    rig.run(6)
}

#[test]
fn separate_channel_drives_change_main_and_raw_aux_independently() {
    let base = native_with_drives(0.0, 0.5, 1.0, 0.0, 220.0, 1.0, 1.0);
    let carrier_off = native_with_drives(0.0, 0.5, 1.0, 0.0, 220.0, 0.0, 1.0);
    let modulator_off = native_with_drives(0.0, 0.5, 1.0, 0.0, 220.0, 1.0, 0.0);
    assert!(difference(&base.0, &carrier_off.0) > 1.0e-3);
    assert!(difference(&base.0, &modulator_off.0) > 1.0e-3);
    assert!(difference(&base.1, &carrier_off.1) > 1.0e-3);
    assert!(difference(&base.1, &modulator_off.1) > 1.0e-3);
    let internal_a = native_with_drives(0.0, 0.5, 1.0, 2.0, 220.0, 0.0, 1.0);
    let internal_b = native_with_drives(0.0, 0.5, 1.0, 2.0, 220.0, 1.0, 1.0);
    assert!(difference(&internal_a.1, &internal_b.1) < 1.0e-6);
    assert!(difference(&internal_a.0, &internal_b.0) < 1.0e-6);
}

#[test]
fn amplifier_and_vocoder_render_without_callback_allocation() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let caps = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    for sr in [8_000.0, 48_000.0, 96_000.0, 192_000.0] {
        let mut mem = vec![0.0; cross_mod::mem_len(sr)];
        let mut state = FxState::default();
        cross_mod::init(&mut state, &mut mem, sr);
        let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
            for (algorithm, wave) in [(7.0, 0.0), (0.0, 1.0), (5.5, 2.0), (7.0, 3.0)] {
                let params = [algorithm, 0.6, 1.0, wave, 220.0, 0.8, 0.6];
                let mut l = [0.4; 256];
                let mut r = [0.2; 256];
                let mut ctx = FxCtx {
                    sr,
                    store: &store,
                    fft: &fft,
                    caps: &caps,
                    scratch: &mut scratch,
                    analysis: &mut analysis,
                    stats: &mut stats,
                };
                cross_mod::process(&params, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
                assert!(l.iter().chain(r.iter()).all(|sample| sample.is_finite()));
            }
        });
        assert_eq!(allocations, 0, "{sr} Hz callback");
    }
}

#[test]
fn vocoder_limiter_is_partition_stable_and_does_not_touch_xmod_or_aux() {
    let render = |algorithm: f32, peak: f32, sr: f32, chunks: &[(usize, usize)]| {
        let store = SampleStore::new(StoreKind::NativeArc);
        let fft = Fft::new(1024);
        let caps = caps();
        let mut scratch = vec![0.0; 4096];
        let mut analysis = vec![0.0; 1024];
        let mut mem = vec![0.0; cross_mod::mem_len(sr)];
        let mut state = FxState::default();
        let mut stats = FxStats::default();
        let params = [algorithm, 0.7, 1.0, 1.0, 330.0, 1.0, 1.0];
        cross_mod::init(&mut state, &mut mem, sr);
        state.s[25] = peak;
        let mut l: Vec<_> = (0..513)
            .map(|frame| (frame as f32 * 317.0 * std::f32::consts::TAU / sr).sin() * 0.9)
            .collect();
        let mut r: Vec<_> = (0..513)
            .map(|frame| (frame as f32 * 521.0 * std::f32::consts::TAU / sr).sin() * 0.8)
            .collect();
        for &(start, end) in chunks {
            let mut ctx = FxCtx {
                sr,
                store: &store,
                fft: &fft,
                caps: &caps,
                scratch: &mut scratch,
                analysis: &mut analysis,
                stats: &mut stats,
            };
            cross_mod::process(
                &params,
                &mut state,
                &mut mem,
                &mut l[start..end],
                &mut r[start..end],
                &mut ctx,
            );
        }
        (l, r, state.s[25])
    };
    let whole = render(7.0, 0.5, 48_000.0, &[(0, 513)]);
    let split = render(
        7.0,
        0.5,
        48_000.0,
        &[(0, 17), (17, 81), (81, 257), (257, 513)],
    );
    assert_eq!(whole, split);
    assert!(whole.2.is_finite() && whole.2 > 0.0);
    let whole_source = render(7.0, 0.5, 96_000.0, &[(0, 513)]);
    let split_source = render(
        7.0,
        0.5,
        96_000.0,
        &[(0, 17), (17, 64), (64, 256), (256, 513)],
    );
    assert_eq!(
        whole_source, split_source,
        "60-frame vocoder cadence must ignore callbacks"
    );
    assert!(rms(&whole_source.0[60..]) > 1.0e-5);

    let xmod_a = render(3.0, 0.5, 48_000.0, &[(0, 513)]);
    let xmod_b = render(3.0, 10.0, 48_000.0, &[(0, 513)]);
    assert_eq!(xmod_a.0, xmod_b.0);
    assert_eq!(xmod_a.1, xmod_b.1);
    assert_eq!(xmod_a.2, 0.5);
    assert_eq!(xmod_b.2, 10.0);
    let vocoder_with_high_peak = render(7.0, 10.0, 48_000.0, &[(0, 513)]);
    assert!(difference(&whole.0, &vocoder_with_high_peak.0) > 1.0e-4);
    assert_eq!(whole.1, vocoder_with_high_peak.1, "aux bypasses limiter");
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn one_float_short_install_preserves_live_bus_and_exact_fit_runs() {
    let sr = 48_000.0;
    let caps = caps();
    let cells = Mirror::new(64);
    let room = crate::dsp::effects::mem_len(EffectKind::Room, sr, &caps);
    let slot_mem_for = |available: usize| {
        (available..=available * 2)
            .find(|&total| total - room.min(total / 4) == available)
            .unwrap()
    };
    let short = slot_mem_for(cross_mod::mem_len(sr) - 1);
    let mut buses = BusGraph::new(3, 64, short, &cells, sr, &caps);
    let mut old = BusTemplate::new();
    old.bus = BusId::new(3);
    old.push(EffectKind::Gain);
    assert!(buses.install(&old, false, 10, 1, &cells, sr, &caps));
    let live_slot = buses.find(BusId::new(3)).unwrap();
    let candidate = BusTemplate::from_def(&def(3.0, 0.5, 0.5, 0.0, 220.0)).unwrap();
    assert!(!buses.install(&candidate, false, 11, 1, &cells, sr, &caps));
    assert_eq!(buses.find(BusId::new(3)), Some(live_slot));
    assert_eq!(buses.slots[live_slot].resource, 10);

    // A rejected replacement leaves the original gain bus rendering audio.
    buses.slots[live_slot].l[..64].fill(0.25);
    buses.slots[live_slot].r[..64].fill(-0.125);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut dry = [0.0; 64];
    let mut l = [0.0; 64];
    let mut r = [0.0; 64];
    let mut ctx = FxCtx {
        sr,
        store: &store,
        fft: &fft,
        caps: &caps,
        scratch: &mut scratch,
        analysis: &mut analysis,
        stats: &mut stats,
    };
    buses.render(64, &cells, &mut dry, &mut ctx, &mut l, &mut r);
    assert!(rms(&l) > 0.1 && rms(&r) > 0.05);

    let exact = slot_mem_for(cross_mod::mem_len(sr));
    let mut exact_buses = BusGraph::new(3, 64, exact, &cells, sr, &caps);
    assert!(exact_buses.install(&candidate, false, 12, 1, &cells, sr, &caps));
    let slot = exact_buses.find(BusId::new(3)).unwrap();
    assert_eq!(exact_buses.slots[slot].resource, 12);
    exact_buses.slots[slot].l[..64].fill(0.25);
    exact_buses.slots[slot].r[..64].fill(-0.125);
    l.fill(0.0);
    r.fill(0.0);
    exact_buses.render(64, &cells, &mut dry, &mut ctx, &mut l, &mut r);
    assert!(l.iter().chain(&r).all(|sample| sample.is_finite()));
    assert!(rms(&l) > 1.0e-4 && rms(&r) > 1.0e-4);

    // The browser wire decoder reaches the same strict install preflight.
    let mut bytes = Vec::new();
    encode_bus(&def(3.0, 0.5, 0.5, 0.0, 220.0), false, &mut bytes).unwrap();
    let mut decoded = BusTemplate::new();
    let mut raw = RawGraph::boxed();
    assert_eq!(
        decode_graph(&bytes, &mut raw, &mut decoded).unwrap(),
        GraphKind::Bus(BusId::new(3))
    );
    let mut browser_short = BusGraph::new(3, 64, short, &cells, sr, &caps);
    assert!(browser_short.install(&old, false, 20, 1, &cells, sr, &caps));
    let browser_slot = browser_short.find(BusId::new(3)).unwrap();
    assert!(!browser_short.install(&decoded, false, 21, 1, &cells, sr, &caps));
    assert_eq!(browser_short.find(BusId::new(3)), Some(browser_slot));
    assert_eq!(browser_short.slots[browser_slot].resource, 20);
    let mut browser_exact = BusGraph::new(3, 64, exact, &cells, sr, &caps);
    assert!(browser_exact.install(&decoded, false, 22, 1, &cells, sr, &caps));
}

#[test]
fn rate_dependent_vocoder_memory_rejects_short_installs_without_retiring_audio() {
    let caps = caps();
    let cells = Mirror::new(64);
    let candidate = BusTemplate::from_def(&def(7.0, 0.5, 0.5, 0.0, 220.0)).unwrap();
    for sr in [8_000.0, 44_100.0, 96_000.0, 192_000.0] {
        let room = crate::dsp::effects::mem_len(EffectKind::Room, sr, &caps);
        let want = cross_mod::mem_len(sr);
        let slot_mem_for = |available: usize| {
            (available..=available * 2)
                .find(|&total| total - room.min(total / 4) == available)
                .unwrap()
        };
        let mut short = BusGraph::new(3, 64, slot_mem_for(want - 1), &cells, sr, &caps);
        let mut old = BusTemplate::new();
        old.bus = BusId::new(3);
        old.push(EffectKind::Gain);
        assert!(short.install(&old, false, 1, 1, &cells, sr, &caps));
        let live = short.find(BusId::new(3)).unwrap();
        assert!(!short.install(&candidate, false, 2, 1, &cells, sr, &caps));
        assert_eq!(short.find(BusId::new(3)), Some(live));
        assert_eq!(short.slots[live].resource, 1);
        let mut exact = BusGraph::new(3, 64, slot_mem_for(want), &cells, sr, &caps);
        assert!(exact.install(&candidate, false, 3, 1, &cells, sr, &caps));
        assert_eq!(exact.slots[exact.find(BusId::new(3)).unwrap()].resource, 3);
    }
    assert!(cross_mod::mem_len(96_000.0) < cross_mod::mem_len(48_000.0));
}

#[test]
fn all_seven_modes_and_controls_have_finite_audio_response() {
    let mut modes = Vec::new();
    for n in 0..=6 {
        let (main, aux) = native(n as f32, 0.5, 0.2, 0.0, 220.0);
        assert!(main.iter().chain(&aux).all(|x| x.is_finite()), "mode {n}");
        assert!(rms(&aux) > 1.0e-3, "mode {n}: aux carries input sum");
        assert!(rms(&main) > 1.0e-5, "mode {n}: main audible");
        modes.push(main);
    }
    for n in 1..7 {
        assert!(
            difference(&modes[n - 1], &modes[n]) > 1.0e-4,
            "mode {n} distinct"
        );
    }
    let (base, _) = native(3.0, 0.5, 0.2, 0.0, 220.0);
    for (name, variant) in [
        ("algorithm morph", native(3.5, 0.5, 0.2, 0.0, 220.0)),
        ("timbre", native(3.0, 0.9, 0.2, 0.0, 220.0)),
        ("drive", native(3.0, 0.5, 0.9, 0.0, 220.0)),
        ("internal wave", native(3.0, 0.5, 0.2, 2.0, 220.0)),
        ("filtered noise carrier", native(3.0, 0.5, 0.2, 5.0, 220.0)),
        ("phase-modulated carrier", native(3.0, 0.5, 0.2, 6.0, 220.0)),
    ] {
        assert!(difference(&base, &variant.0) > 1.0e-4, "{name}");
    }
    let (internal_220, _) = native(3.0, 0.5, 0.2, 2.0, 220.0);
    let (internal_660, _) = native(3.0, 0.5, 0.2, 2.0, 660.0);
    assert!(
        difference(&internal_220, &internal_660) > 1.0e-4,
        "carrier frequency changes audio with the same internal wave"
    );
    let (_, aux_external) = native(3.0, 0.5, 0.2, 0.0, 220.0);
    let (_, aux_internal) = native(3.0, 0.5, 0.2, 1.0, 220.0);
    assert!(difference(&aux_external, &aux_internal) > 1.0e-3);
}

#[test]
fn three_internal_carrier_selectors_switch_between_xmod_and_vocoder_roles() {
    for wave in [1.0, 2.0, 3.0] {
        let xmod = native(0.0, 0.5, 0.6, wave, 330.0);
        let vocoder = native(7.0, 0.5, 0.6, wave, 330.0);
        assert!(rms(&xmod.1) > 1.0e-3, "wave {wave} XMOD aux");
        assert!(rms(&vocoder.1) > 1.0e-3, "wave {wave} vocoder aux");
        assert!(
            difference(&xmod.1, &vocoder.1) > 1.0e-2,
            "wave {wave} changes source role"
        );
    }
}

#[test]
fn comparator_to_vocoder_bridge_is_audible_and_continuous() {
    let xmod = native(5.4, 0.6, 0.2, 0.0, 220.0).0;
    let center = native(5.6, 0.6, 0.2, 0.0, 220.0).0;
    let vocoder = native(5.8, 0.6, 0.2, 0.0, 220.0).0;
    assert!(rms(&xmod) > 1.0e-5 && rms(&center) > 1.0e-5 && rms(&vocoder) > 1.0e-5);
    assert!(difference(&xmod, &center) > 1.0e-4);
    assert!(difference(&center, &vocoder) > 1.0e-4);
    let below = native(5.599, 0.6, 0.2, 0.0, 220.0).0;
    let above = native(5.601, 0.6, 0.2, 0.0, 220.0).0;
    assert!(difference(&below, &above) < 0.01);
}

fn vocoder_silence_tail(algorithm: f32) -> (f32, f32) {
    let sr = 48_000.0;
    let mut params: Vec<_> = cross_mod::PARAMS
        .iter()
        .map(|param| param.default)
        .collect();
    params[0] = 6.0;
    params[3] = 0.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; cross_mod::mem_len(sr)];
    cross_mod::init(&mut state, &mut mem, sr);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let caps = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut tail = Vec::new();
    for block in 0..160 {
        if block == 32 {
            params[0] = algorithm;
        }
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        for frame in 0..256 {
            let sample = (block * 256 + frame) as f32;
            l[frame] = (sample * 220.0 * std::f32::consts::TAU / sr).sin() * 0.6;
            if block < 32 {
                r[frame] = (sample * 113.0 * std::f32::consts::TAU / sr).sin() * 0.6;
            }
        }
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &caps,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        cross_mod::process(&params, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
        if block >= 144 {
            tail.extend(l);
        }
    }
    let envelope = cross_mod::vocoder_envelope_energy(&mem);
    (rms(&tail), envelope)
}

#[test]
fn vocoder_release_and_freeze_keep_distinct_audio_after_modulator_silence() {
    let fast = vocoder_silence_tail(6.0);
    let slow = vocoder_silence_tail(7.0);
    let frozen = vocoder_silence_tail(8.0);
    assert!(fast.0.is_finite() && slow.0.is_finite() && frozen.0.is_finite());
    assert!(slow.0 > fast.0 * 2.0, "release 7 vs 6: {slow:?} {fast:?}");
    assert!(
        (frozen.0 - slow.0).abs() > 1.0e-6,
        "freeze changes source-bank audio: {frozen:?} {slow:?}"
    );
    assert!(slow.1 > fast.1 * 2.0);
    assert!(frozen.1 > slow.1 * 1.2);
}

#[test]
fn host_rate_vocoder_formant_shift_changes_filtered_main_without_raw_aux_change() {
    let low = native(7.0, 0.15, 0.8, 0.0, 220.0);
    let high = native(7.0, 0.85, 0.8, 0.0, 220.0);
    assert!(difference(&low.0, &high.0) > 1.0e-5);
    assert_eq!(low.1, high.1, "raw auxiliary does not use formant shift");
}

#[test]
fn full_rate_extremes_install_and_render_vocoder_on_native_host() {
    for sr in [8_000.0, 192_000.0] {
        let mut cfg = config(&caps(), StoreKind::NativeArc);
        cfg.sample_rate = sr;
        cfg.max_block = 256;
        let mut rig = NativeRig::native_with(cfg);
        for source in sources() {
            rig.install(&source);
        }
        rig.install_bus(&def(7.0, 0.5, 0.8, 0.0, 220.0), false);
        rig.install_bus(&bus_def(0, vec![]), true);
        let _ = rig.step();
        send(&mut rig);
        let (main, aux) = rig.run(20);
        assert!(main.iter().chain(&aux).all(|sample| sample.is_finite()));
        assert!(rms(&main) > 1.0e-5, "{sr} Hz main");
        assert!(rms(&aux) > 1.0e-3, "{sr} Hz aux");
    }
}

#[test]
fn browser_codec_preserves_two_inputs_main_and_aux() {
    let mut rig = BrowserRig::browser(4 << 20);
    for (resource, source) in sources().into_iter().enumerate() {
        let mut bytes = Vec::new();
        encode_inst(&source, &mut bytes).unwrap();
        let mut rec = Vec::new();
        encode_graph_record(10 + resource as u32, 1, &bytes, &mut rec);
        rig.push(&rec);
    }
    for (resource, graph, master) in [
        (20, def(3.0, 0.7, 0.6, 0.0, 220.0), false),
        (21, bus_def(0, vec![]), true),
    ] {
        let mut bytes = Vec::new();
        encode_bus(&graph, master, &mut bytes).unwrap();
        let mut rec = Vec::new();
        encode_graph_record(resource, 1, &bytes, &mut rec);
        rig.push(&rec);
    }
    let _ = rig.run(6);
    send(&mut rig);
    let (main, aux) = rig.run(6);
    assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
    assert!(rms(&main) > 1.0e-4 && rms(&aux) > 1.0e-4);
    assert!(difference(&main, &aux) > 1.0e-3);
}

#[test]
fn modulation_effect_has_editor_controls() {
    let decl = crate::types::manifest::HostManifest::spec_default()
        .editor_decl("dual-mod")
        .unwrap();
    for name in [
        "algorithm",
        "timbre",
        "drive",
        "carrier-wave",
        "carrier-frequency",
        "carrier-drive",
        "modulator-drive",
    ] {
        assert!(decl.params.iter().any(|p| p.name == name), "{name}");
    }
    let algorithm = decl.params.iter().find(|p| p.name == "algorithm").unwrap();
    assert_eq!(algorithm.range, (0.0, 8.0));
}

#[test]
fn translated_modes_keep_stereo_and_late_start_across_host_rates() {
    let sources = sources();
    for (algorithm, wave) in [(2.5, 1.0), (5.5, 2.0), (7.0, 3.0)] {
        let effect = def_with_drives(algorithm, 0.7, 0.3, wave, 220.0, 0.8, 0.6);
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut native = NativeRig::native_with(cfg);
                for source in &sources {
                    native.install(source);
                }
                native.install_bus(&effect, false);
                native.install_bus(&bus_def(0, vec![]), true);
                let _ = native.step();
                let start = native.engine.now() + 17.0 / f64::from(sr);
                for (id, pan) in [(1, 0.0), (2, 1.0)] {
                    native.send(event(
                        id,
                        start,
                        &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 1.0)],
                    ));
                }
                let (main, aux) = native.run(12);
                // An internal carrier is a free-running bus source, so its
                // main and aux may sound before the late event begins.
                assert!(
                    rms(&aux[..17]) > 1.0e-5,
                    "internal carrier runs before the late voice"
                );
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(rms(&main) > 1.0e-5 && rms(&aux) > 1.0e-5);
                assert!(difference(&main, &aux) > 1.0e-4);

                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut browser = BrowserRig::browser_with(cfg);
                for (resource, source) in [(10, &sources[0]), (11, &sources[1])] {
                    let mut bytes = Vec::new();
                    encode_inst(source, &mut bytes).unwrap();
                    let mut record = Vec::new();
                    encode_graph_record(resource, 1, &bytes, &mut record);
                    browser.push(&record);
                }
                for (resource, graph, master) in
                    [(20, &effect, false), (21, &bus_def(0, vec![]), true)]
                {
                    let mut bytes = Vec::new();
                    encode_bus(graph, master, &mut bytes).unwrap();
                    let mut record = Vec::new();
                    encode_graph_record(resource, 1, &bytes, &mut record);
                    browser.push(&record);
                }
                let _ = browser.run(6);
                let start = browser.engine.now() + 17.0 / f64::from(sr);
                for (id, pan) in [(1, 0.0), (2, 1.0)] {
                    browser.send(event(
                        id,
                        start,
                        &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 1.0)],
                    ));
                }
                let (main, aux) = browser.run(12);
                // Browser has the same free-running source contract.
                assert!(
                    rms(&aux[..17]) > 1.0e-5,
                    "internal carrier runs before the late voice"
                );
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(rms(&main) > 1.0e-5 && rms(&aux) > 1.0e-5);
                assert!(difference(&main, &aux) > 1.0e-4);
            }
        }
    }
}
