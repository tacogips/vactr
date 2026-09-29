//! A public `.vact` master effect processes externally supplied audio.

use super::{E2e, BLOCK};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{encode_inst, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellRead;
use crate::dsp::cells::Mirror;
use crate::dsp::engine::Engine;
use crate::dsp::ring::{
    encode_graph_record, ByteInbox, EngineConfig, EngineIo, EventRing, SpscRing,
};
use crate::host::native::audio::{render_captured, MAX_BLOCK};
use crate::host::native::capture;
use crate::host::native::NativeAudioHost;
use crate::host::wire::AudioEvent;
use crate::sched::slots::SlotId;
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;
use crate::value::value::Value;

#[test]
fn opt_in_external_voice_templates_compile_and_are_discoverable() {
    let mut e = E2e::new();
    e.eval(include_str!(
        "../../../../examples/live-external-voices.vact"
    ));
    let definitions = e.reg.borrow().entries().count();
    assert!(definitions >= 65);
    for caps in [CapabilitySet::native(), CapabilitySet::browser()] {
        let cfg = EngineConfig::new(&caps, 48_000.0, 256, StoreKind::Arena { bytes: 4 << 20 });
        assert!(cfg.template_slots >= definitions);
    }
    let decls = instrument_decls(&e.reg.borrow());
    for name in ["resonator-external-voice", "exciter-external-voice"] {
        assert!(decls.iter().any(|decl| decl.name == name), "{name}");
    }
    let rings = decls
        .iter()
        .find(|decl| decl.name == "resonator-external-voice")
        .unwrap();
    for name in [
        "reso-model",
        "reso-structure",
        "reso-brightness",
        "reso-damping",
        "reso-position",
        "reso-fm",
        "reso-polyphony",
    ] {
        assert!(
            rings.params.iter().any(|param| param.name == name),
            "{name}"
        );
    }
    let elements = decls
        .iter()
        .find(|decl| decl.name == "exciter-external-voice")
        .unwrap();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == "exciter-external-voice")
        .unwrap();
    for (name, expected) in [
        ("ex-gate", 1.0),
        ("ex-blow-level", 1.0),
        ("ex-strike-level", 1.0),
        ("ex-brightness", 0.5),
    ] {
        let declared = entry
            .params
            .iter()
            .find(|param| &*name_of_kw(param.name) == name)
            .unwrap();
        let default = entry
            .def
            .params
            .iter()
            .find(|(ctl, _)| *ctl == declared.ctl)
            .unwrap()
            .1;
        let crate::host::wire::Ctl::Cell(cell) = default else {
            panic!("{name} has no tweak cell")
        };
        assert_eq!(entry.default_cell_value(cell), Some(expected), "{name}");
        assert_eq!(
            e.rt.cells().native().unwrap().get(cell),
            expected,
            "{name} at install"
        );
    }
    for name in [
        "ex-env-shape",
        "ex-bow-level",
        "ex-bow-timbre",
        "ex-blow-level",
        "ex-blow-meta",
        "ex-blow-timbre",
        "ex-strike-level",
        "ex-strike-meta",
        "ex-strike-timbre",
        "ex-signature",
        "ex-geometry",
        "ex-brightness",
        "ex-damping",
        "ex-position",
        "ex-res-mod-frequency",
        "ex-res-mod-offset",
        "ex-reverb-diffusion",
        "ex-reverb-lp",
        "ex-space",
        "ex-modulation-frequency",
        "ex-gate",
        "ex-note",
        "ex-modulation",
        "ex-strength",
        "ex-model",
        "ex-alternate",
    ] {
        assert!(
            elements.params.iter().any(|param| param.name == name),
            "{name}"
        );
    }
}

#[test]
fn authored_header_default_tweak_refreshes_the_audio_cell() {
    let mut e = E2e::new();
    e.eval(include_str!(
        "../../../../examples/live-external-voices.vact"
    ));
    let (cell, slot) = {
        let registry = e.reg.borrow();
        let entry = registry
            .entries()
            .find(|entry| &*name_of_kw(entry.name) == "exciter-external-voice")
            .unwrap();
        let declared = entry
            .params
            .iter()
            .find(|param| &*name_of_kw(param.name) == "ex-brightness")
            .unwrap();
        let crate::host::wire::Ctl::Cell(cell) = entry
            .def
            .params
            .iter()
            .find(|(ctl, _)| *ctl == declared.ctl)
            .unwrap()
            .1
        else {
            panic!("brightness default must remain tweakable")
        };
        let slot = entry
            .cells
            .iter()
            .find(|(id, _)| *id == cell)
            .unwrap()
            .1
            .clone();
        (cell, slot)
    };
    assert_eq!(e.rt.cells().native().unwrap().get(cell), 0.5);
    let site =
        e.ev.ns()
            .tweaks()
            .borrow()
            .iter()
            .find(|site| site.slot.id() == slot.id())
            .cloned()
            .unwrap();
    e.ev.set_tweak(site.id, site.form_gen, Value::Float(0.75))
        .unwrap();
    let rep = e.rt.drain(&mut e.ev);
    assert!(rep.faults.is_empty(), "{:?}", rep.faults);
    assert_eq!(e.rt.cells().native().unwrap().get(cell), 0.75);
}

fn voice_with_input(name: &str, lane: usize, connected: bool) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(include_str!(
        "../../../../examples/live-external-voices.vact"
    ));
    let installed_ids: Vec<_> = ["resonator-external-voice", "exciter-external-voice"]
        .iter()
        .map(|target| {
            e.reg
                .borrow()
                .entries()
                .find(|entry| &*name_of_kw(entry.name) == *target)
                .unwrap()
                .def
                .id
        })
        .collect();
    for _ in 0..8 {
        let rep = e.rt.tick(&mut e.ev, e.clock.now());
        assert!(rep.faults.is_empty(), "{:?}", rep.faults);
        let mut out = [0.0; 2 * BLOCK];
        let (_, allocations) = armed(|| e.side.render(&mut out, 2));
        assert_eq!(allocations, 0);
    }
    for (target, id) in ["resonator-external-voice", "exciter-external-voice"]
        .iter()
        .zip(installed_ids)
    {
        assert!(e.side.has_template(id), "{target} did not install");
    }
    let controls = if name == "exciter-external-voice" {
        " > ex-bow-level 0.3 > ex-modulation 0.4 > ex-model 0"
    } else {
        ""
    };
    e.eval(&format!("s :{name} > note [:a3]{controls} > once"));
    let (mut producer, mut consumer, counters) = capture::pair();
    let mut scratch = [0.0; 2 * MAX_BLOCK];
    let mut audio = Vec::new();
    let mut committed = 0;
    for block in 0..48 {
        let rep = e.rt.tick(&mut e.ev, e.clock.now());
        assert!(rep.faults.is_empty(), "{:?}", rep.faults);
        committed += rep.committed;
        let mut input = [0.0; 2 * BLOCK];
        if connected {
            for frame in 0..BLOCK {
                let t = (block * BLOCK + frame) as f32 / 48_000.0;
                input[2 * frame + lane] = (std::f32::consts::TAU * 220.0 * t).sin() * 0.5;
            }
        }
        if connected {
            let (_, allocations) = armed(|| producer.push_interleaved(&input, 2));
            assert_eq!(allocations, 0);
        }
        let mut out = [0.0; 2 * BLOCK];
        let (_, allocations) =
            armed(|| render_captured(&mut e.side, &mut consumer, &mut scratch, &mut out, 2));
        assert_eq!(allocations, 0);
        audio.extend_from_slice(&out);
    }
    assert_eq!(counters.snapshot().overrun, 0);
    assert!(committed > 0, "{name} was never scheduled");
    audio
}

#[test]
fn external_voices_respond_to_host_lanes() {
    for name in ["resonator-external-voice", "exciter-external-voice"] {
        let quiet = voice_with_input(name, 0, false);
        for lane in 0..2 {
            let wet = voice_with_input(name, lane, true);
            // Remove the master bus's dry input from the comparison. A
            // response then proves the instrument graph consumed the lane.
            let delta: f32 = wet
                .iter()
                .zip(&quiet)
                .enumerate()
                .map(|(i, (a, b))| {
                    let t = (i / 2) as f32 / 48_000.0;
                    let dry = if i % 2 == lane {
                        (std::f32::consts::TAU * 220.0 * t).sin() * 0.5
                    } else {
                        0.0
                    };
                    (a - b - dry).abs()
                })
                .sum();
            assert!(delta > 0.01, "{name} lane {lane}: {delta}");
            assert!(wet.iter().all(|x| x.is_finite()));
        }
    }
    let blow = voice_with_input("exciter-external-voice", 0, true);
    let strike = voice_with_input("exciter-external-voice", 1, true);
    let distinction: f32 = blow.iter().zip(&strike).map(|(a, b)| (a - b).abs()).sum();
    assert!(
        distinction > 0.1,
        "blow and strike lanes must remain distinct: {distinction}"
    );
}

fn browser_voice(name: &str, lane: usize, quad: bool) -> Vec<f32> {
    let mut authored = E2e::new();
    authored.eval(include_str!(
        "../../../../examples/live-external-voices.vact"
    ));
    let def = authored
        .reg
        .borrow()
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == name)
        .unwrap()
        .def
        .clone();
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    let mut cfg = EngineConfig::new(
        &CapabilitySet::browser(),
        48_000.0,
        256,
        StoreKind::Arena { bytes: 4 << 20 },
    );
    cfg.template_slots = 8;
    cfg.bus_slots = 4;
    cfg.voice_seconds = 0.5;
    cfg.bus_seconds = 1.0;
    cfg.orbits = 2;
    cfg.orbit_delay_seconds = 0.5;
    cfg.analysis_cells = 1024;
    cfg.event_capacity = 256;
    cfg.output_channels = if quad { 4 } else { 2 };
    let mut engine = Engine::with_config(cfg);
    let (mut events_tx, mut events_rx) = EventRing::split(256);
    let (mut acks_tx, _acks_rx) = SpscRing::split(256);
    let (mut garbage_tx, _garbage_rx) = SpscRing::split(32);
    let mut cells = Mirror::new(64);
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&record));
    for _ in 0..6 {
        let _ = step_browser(
            &mut engine,
            &mut events_rx,
            &mut inbox,
            &mut acks_tx,
            &mut garbage_tx,
            &mut cells,
            &[0.0; 512],
            quad,
        );
    }
    assert!(engine.pop_fault().is_none());
    assert!(events_tx
        .push(AudioEvent::new(engine.now(), SlotId::new(1), 1, def.id))
        .is_ok());
    let mut input = [0.0; 512];
    input[lane] = 0.8;
    let mut audio = Vec::new();
    for block in 0..8 {
        let active = if block == 0 {
            &input[..]
        } else {
            &[0.0; 512][..]
        };
        let output = step_browser(
            &mut engine,
            &mut events_rx,
            &mut inbox,
            &mut acks_tx,
            &mut garbage_tx,
            &mut cells,
            active,
            quad,
        );
        audio.extend(output);
    }
    assert!(engine.pop_fault().is_none());
    audio
}

#[allow(clippy::too_many_arguments)]
fn step_browser(
    engine: &mut Engine,
    events: &mut crate::dsp::ring::EventConsumer,
    inbox: &mut ByteInbox,
    acks: &mut crate::dsp::ring::AckProducer,
    garbage: &mut crate::dsp::ring::Producer<crate::dsp::ring::Garbage>,
    cells: &mut Mirror,
    input: &[f32],
    quad: bool,
) -> Vec<f32> {
    let mut output = vec![0.0; if quad { 1024 } else { 512 }];
    let mut io = EngineIo {
        events,
        controls: inbox,
        acks,
        cells,
        garbage: Some(garbage),
    };
    let (_, allocations) = armed(|| {
        if quad {
            engine.process_four_with_input(&mut io, input, &mut output, 256);
        } else {
            engine.process_with_input(&mut io, input, &mut output, 256);
        }
    });
    assert_eq!(allocations, 0);
    output
}

#[test]
fn authored_external_voices_survive_browser_codec_in_stereo_and_quad() {
    for name in ["resonator-external-voice", "exciter-external-voice"] {
        for quad in [false, true] {
            let left = browser_voice(name, 0, quad);
            let right = browser_voice(name, 1, quad);
            assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
            let difference: f32 = left.iter().zip(&right).map(|(a, b)| (a - b).abs()).sum();
            assert!(difference > 0.01, "{name}/{quad}: {difference}");
            if quad {
                assert!(left
                    .chunks_exact(4)
                    .all(|channels| channels[2] == 0.0 && channels[3] == 0.0));
            }
        }
    }
}

fn external(wet: bool) -> (Vec<f32>, Vec<f32>) {
    let mut e = E2e::new();
    e.eval("master:\n\tresonant-bank model: 2 structure: 0.7 brightness: 0.8 damping: 0.4 position: 0.6 note: 2 tonic: 1 fm: 0 chord: 3 polyphony: 3 strum: 0 internal-exciter: 0 internal-strum: 1 internal-note: 1 external-mix: 1 gate: 1 mix: 1");
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut input = vec![0.0; 2 * BLOCK];
    for block in 0..20 {
        for (frame, lr) in input.chunks_exact_mut(2).enumerate() {
            let t = (block * BLOCK + frame) as f32 / 48_000.0;
            lr[0] = if wet {
                (std::f32::consts::TAU * 220.0 * t).sin() * 0.25
            } else {
                0.0
            };
            lr[1] = 0.0;
        }
        let rep = e.rt.tick(&mut e.ev, e.clock.now());
        assert!(rep.faults.is_empty(), "{:?}", rep.faults);
        let mut out = [0.0; 2 * BLOCK];
        let (_, allocations) = armed(|| e.side.render_with_input(&input, &mut out, 2));
        assert_eq!(allocations, 0);
        left.extend(out.chunks_exact(2).map(|lr| lr[0]));
        right.extend(out.chunks_exact(2).map(|lr| lr[1]));
    }
    (left, right)
}

#[test]
fn vact_master_resonator_responds_to_connected_input_without_a_voice() {
    let connected = external(true);
    let disconnected = external(false);
    assert!(connected
        .0
        .iter()
        .chain(&connected.1)
        .all(|x| x.is_finite()));
    assert!(connected
        .0
        .iter()
        .chain(&connected.1)
        .any(|x| x.abs() > 1.0e-5));
    assert!(disconnected
        .0
        .iter()
        .chain(&disconnected.1)
        .all(|x| *x == 0.0));
    assert_ne!(
        connected.0, connected.1,
        "resonator main/aux remain separate"
    );
}

#[test]
fn native_audio_side_maps_mono_and_quad_without_input_in_direct_stems() {
    let (_, mut stereo) = NativeAudioHost::headless(48_000, CapabilitySet::native(), 64);
    let mono = [0.25; BLOCK];
    let mut out = [0.0; 2 * BLOCK];
    let (_, allocations) = armed(|| stereo.render_with_input(&mono, &mut out, 2));
    assert_eq!(allocations, 0);
    assert!(out.chunks_exact(2).all(|lr| lr == [0.25, 0.25]));

    let (_, mut quad) = NativeAudioHost::headless_quad(48_000, CapabilitySet::native(), 64);
    let input = [0.2, 0.7].repeat(BLOCK);
    let mut four = [0.0; 4 * BLOCK];
    let (_, allocations) = armed(|| quad.render_with_input(&input, &mut four, 4));
    assert_eq!(allocations, 0);
    assert!(four
        .chunks_exact(4)
        .all(|frame| frame == [0.2, 0.7, 0.0, 0.0]));
}
