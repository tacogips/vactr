//! Stable codec IDs and strict full-state admission for all expansion effects.
use super::{bus_def, caps, chain, SR};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind};
use crate::dsp::bus::{BusGraph, BusTemplate, SlotState};
use crate::dsp::cells::AtomicCells;
use crate::dsp::effects::{self, catalog::spec};
use crate::dsp::graph::{EffectKind, UGenSpec};
use crate::dsp::ugen::{BuildEnv, BuildError, RawGraph, Template};

// Frozen pre-expansion order: validates every old byte ID, including analyzer/capture entries.
const EXISTING: &[&str] = &[
    "compressor",
    "expander",
    "gate",
    "limiter",
    "multiband-compressor",
    "multiband-expander",
    "transient",
    "multiband-transient",
    "auto-level",
    "sag",
    "peq",
    "geq",
    "dynamic-eq",
    "tilt",
    "tone",
    "loudness-eq",
    "lpf",
    "hpf",
    "bpf",
    "notch",
    "comb",
    "narrow",
    "linear-phase-eq",
    "group-delay-eq",
    "crossover",
    "delay",
    "ping-pong",
    "multitap",
    "time-align",
    "plate",
    "fdn",
    "convolution",
    "scatter",
    "room",
    "saturate",
    "tube",
    "clip",
    "harmonics",
    "exciter",
    "multiband-saturate",
    "sub-synth",
    "bandwidth-extend",
    "dynamic-saturate",
    "chorus",
    "flanger",
    "phaser",
    "tremolo",
    "auto-pan",
    "auto-filter",
    "pitch-shift",
    "pitch-shift-hq",
    "freq-shift",
    "rotary",
    "wow-flutter",
    "doppler",
    "vibrato",
    "bitcrush",
    "decimate",
    "jitter",
    "noise-blend",
    "hum",
    "tape",
    "cassette",
    "vinyl",
    "vinyl-artifacts",
    "codec",
    "radio",
    "tv-audio",
    "digital-error",
    "dsd-imd",
    "modal",
    "horn",
    "width",
    "balance",
    "multiband-balance",
    "ms",
    "crossfeed",
    "crosstalk-cancel",
    "phase-select-eq",
    "spatial-map",
    "pan",
    "matrix",
    "declick",
    "declip",
    "dehum",
    "denoise",
    "gain",
    "mute",
    "polarity",
    "dc-offset",
    "dry-wet",
    "section",
    "channel-divider",
    "dual-mod",
    "fir-crossover",
    "resonant-bank",
    "exciter-bank",
    "stream-envelope",
    "stream-vactr",
    "stream-follower",
    "stream-compressor",
    "stream-filter",
    "stream-lorenz",
    "shift-pair",
    "keyframe-mixer",
    "granulate",
    "level",
    "spectrum",
    "spectrogram",
    "note-spectrogram",
    "oscilloscope",
    "pitch-meter",
    "stereo-meter",
    "texture-grain",
    "texture-stretch",
    "texture-loop",
    "texture-spectral",
];

#[test]
fn musicdsp_catalog_append_preserves_every_existing_byte_id() {
    assert_eq!(EffectKind::ALL.len(), EXISTING.len() + 19);
    for (index, name) in EXISTING.iter().enumerate() {
        let kind = EffectKind::from_name(name).unwrap();
        assert_eq!(EffectKind::ALL[index], kind, "{name} index changed");
        let mut bytes = Vec::new();
        encode_bus(
            &bus_def(3, vec![spec(kind, &[]).unwrap()]),
            false,
            &mut bytes,
        )
        .unwrap();
        assert_eq!(usize::from(bytes[7]), index);
    }
}
#[test]
fn musicdsp_catalog_new_effects_roundtrip_bus_and_inst_on_browser_transport() {
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    for kind in EffectKind::ALL.iter().skip(EXISTING.len()).copied() {
        let effect = spec(kind, &[]).unwrap();
        let mut bytes = Vec::new();
        encode_bus(&bus_def(3, vec![effect.clone()]), false, &mut bytes).unwrap();
        assert_eq!(
            decode_graph(&bytes, &mut raw, &mut bus),
            Ok(GraphKind::Bus(crate::dsp::graph::BusId::new(3)))
        );
        assert_eq!(bus.kinds[0], kind);
        let definition = chain(1, vec![UGenSpec::Const(0.5), UGenSpec::Effect(effect)]);
        encode_inst(&definition, &mut bytes).unwrap();
        assert_eq!(
            decode_graph(&bytes, &mut raw, &mut bus),
            Ok(GraphKind::Inst)
        );
        let env = BuildEnv {
            sr: SR,
            caps: caps(),
            voice_mem: effects::mem_len(kind, SR, &caps()),
        };
        let mut template = Template::boxed();
        assert!(
            template.build(&raw, &env).is_ok(),
            "{} full memory",
            kind.name()
        );
    }
}
#[test]
fn musicdsp_full_memory_rejects_short_bus_and_voice_before_retiring_live_graph() {
    for kind in EffectKind::ALL.iter().skip(EXISTING.len()).copied() {
        let need = effects::mem_len(kind, SR, &caps());
        if need == 0 {
            continue;
        }
        assert!(effects::requires_full_memory(kind));
        let mut raw = RawGraph::boxed();
        raw.load(&chain(
            1,
            vec![
                UGenSpec::Const(0.5),
                UGenSpec::Effect(spec(kind, &[]).unwrap()),
            ],
        ))
        .unwrap();
        let mut template = Template::boxed();
        let env = BuildEnv {
            sr: SR,
            caps: caps(),
            voice_mem: need - 1,
        };
        assert_eq!(
            template.build(&raw, &env),
            Err(BuildError::MemExceeded),
            "{} voice memory",
            kind.name()
        );
        let cells = AtomicCells::new(4);
        let mut graph = BusGraph::new(3, 128, need - 1, &cells, SR, &caps());
        let initial = BusTemplate::from_def(&bus_def(3, vec![])).unwrap();
        assert!(graph.install(&initial, false, 1, 1, &cells, SR, &caps()));
        let slot = graph.find(initial.bus).unwrap();
        let proposed = BusTemplate::from_def(&bus_def(3, vec![spec(kind, &[]).unwrap()])).unwrap();
        assert!(
            !graph.install(&proposed, false, 2, 2, &cells, SR, &caps()),
            "{} bus memory",
            kind.name()
        );
        assert_eq!(graph.find(initial.bus), Some(slot));
        assert_eq!(graph.slots[slot].state, SlotState::Live);
    }
}

#[test]
fn musicdsp_default_budgets_admit_all_new_instrument_effects_on_native_and_browser() {
    use super::{ctl, event, rms, BrowserRig, NativeRig};
    use crate::dsp::arena::StoreKind;
    use crate::dsp::engine::EngineConfig;
    use crate::dsp::ring::encode_graph_record;
    for kind in EffectKind::ALL.iter().skip(EXISTING.len()).copied() {
        let def = chain(
            1,
            vec![
                UGenSpec::Const(440.0),
                UGenSpec::SinOsc,
                UGenSpec::Effect(spec(kind, &[]).unwrap()),
            ],
        );
        let mut native =
            NativeRig::native_with(EngineConfig::new(&caps(), SR, 256, StoreKind::NativeArc));
        native.install(&def);
        let _ = native.step();
        native.send(event(1, native.engine.now(), &[(ctl::LEGATO, 1.0)]));
        let (left, right) = native.run(96);
        assert!(
            left.iter().chain(&right).all(|v| v.is_finite()),
            "native {}",
            kind.name()
        );
        assert!(rms(&left) > 1e-5, "native {} audible", kind.name());
        let mut browser = BrowserRig::browser_with(EngineConfig::new(
            &caps(),
            SR,
            256,
            StoreKind::Arena { bytes: 4 << 20 },
        ));
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut record = Vec::new();
        encode_graph_record(1, 1, &bytes, &mut record);
        browser.push(&record);
        let _ = browser.run(6);
        browser.send(event(1, browser.engine.now(), &[(ctl::LEGATO, 1.0)]));
        let (left, right) = browser.run(96);
        assert!(
            left.iter().chain(&right).all(|v| v.is_finite()),
            "browser {}",
            kind.name()
        );
        assert!(rms(&left) > 1e-5, "browser {} audible", kind.name());
    }
}

#[test]
fn musicdsp_default_budgets_cover_192khz_and_keep_state_ceiling() {
    use crate::dsp::arena::StoreKind;
    use crate::dsp::engine::EngineConfig;
    use crate::dsp::ring::ConfigError;
    for sr in [8000.0, 44100.0, 48000.0, 96000.0, 192000.0] {
        let cfg = EngineConfig::new(&caps(), sr, 256, StoreKind::NativeArc);
        assert_eq!(cfg.validate(), Ok(()));
        for kind in EffectKind::ALL.iter().skip(EXISTING.len()).copied() {
            assert!(
                effects::mem_len(kind, sr, &caps()) <= (cfg.voice_seconds * sr) as usize,
                "{} @ {sr} fits voice",
                kind.name()
            );
        }
        let mut oversize = cfg;
        oversize.voice_seconds = (EngineConfig::MAX_STATE_SAMPLES + 16.0) / sr;
        assert_eq!(oversize.validate(), Err(ConfigError::StateBudget));
        oversize = cfg;
        oversize.bus_seconds = (EngineConfig::MAX_STATE_SAMPLES + 16.0) / sr;
        assert_eq!(oversize.validate(), Err(ConfigError::StateBudget));
    }
}

#[test]
fn musicdsp_all_effects_run_in_native_and_browser_buses_with_default_budgets() {
    use super::{ctl, event, rms, BrowserRig, NativeRig};
    use crate::dsp::arena::StoreKind;
    use crate::dsp::engine::EngineConfig;
    use crate::dsp::ring::encode_graph_record;
    for kind in EffectKind::ALL.iter().skip(EXISTING.len()).copied() {
        let instrument = chain(1, vec![UGenSpec::Const(440.0), UGenSpec::SinOsc]);
        let bus = bus_def(3, vec![spec(kind, &[]).unwrap()]);
        let mut native =
            NativeRig::native_with(EngineConfig::new(&caps(), SR, 256, StoreKind::NativeArc));
        native.install(&instrument);
        native.install_bus(&bus, false);
        let _ = native.step();
        native.send(event(
            1,
            native.engine.now(),
            &[(ctl::LEGATO, 1.0), (ctl::BUS, 3.0)],
        ));
        let (l, r) = native.run(96);
        assert!(
            l.iter().chain(&r).all(|x| x.is_finite()),
            "native bus {}",
            kind.name()
        );
        assert!(rms(&l) > 1e-5, "native bus {} audible", kind.name());
        assert!(native.engine.buses().find(bus.id).is_some());
        let mut browser = BrowserRig::browser_with(EngineConfig::new(
            &caps(),
            SR,
            256,
            StoreKind::Arena { bytes: 4 << 20 },
        ));
        let mut bytes = Vec::new();
        let mut record = Vec::new();
        encode_inst(&instrument, &mut bytes).unwrap();
        encode_graph_record(1, 1, &bytes, &mut record);
        browser.push(&record);
        encode_bus(&bus, false, &mut bytes).unwrap();
        encode_graph_record(2, 1, &bytes, &mut record);
        browser.push(&record);
        let _ = browser.run(6);
        browser.send(event(
            1,
            browser.engine.now(),
            &[(ctl::LEGATO, 1.0), (ctl::BUS, 3.0)],
        ));
        let (l, r) = browser.run(96);
        assert!(
            l.iter().chain(&r).all(|x| x.is_finite()),
            "browser bus {}",
            kind.name()
        );
        assert!(rms(&l) > 1e-5, "browser bus {} audible", kind.name());
        assert!(browser.engine.buses().find(bus.id).is_some());
    }
}
