//! Layer-shaped voice completion and transparent default behavior.

use super::{caps, config, ctl, event, BrowserRig, NativeRig};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{decode_graph, encode_inst, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, Node, RawGraph, Template};
use crate::host::wire::{AudioEvent, Ctl};
use crate::sched::slots::SlotId;

fn voice(mode: f32, decay: f32, with_gate: bool, velocity: f32, slot: f32) -> InstDef {
    let mut nodes = vec![UGenSpec::SinOsc];
    let mut edges = Vec::new();
    let mut node_params = Vec::new();
    if with_gate {
        nodes.push(UGenSpec::VactrolGate);
        edges.push(Edge {
            from: 0,
            to: 1,
            port: 0,
            output: 0,
        });
        let params = [220.0, velocity, mode, decay, 0.5, slot, 0.0, 0.0];
        for (port, value) in params.into_iter().enumerate() {
            node_params.push((
                1,
                catalog::port_ctl(&Node::VactrolGate, port + 1).expect("gate control exists"),
                Ctl::Const(value),
            ));
        }
    }
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        node_params: node_params.into_boxed_slice(),
    }
}

fn render(definition: &InstDef, blocks: usize) -> Vec<f32> {
    let mut rig = started(definition);
    rig.run(blocks).0
}

fn started(definition: &InstDef) -> NativeRig {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.max_block = 256;
    let mut rig = NativeRig::native_with(cfg);
    rig.install(definition);
    let _ = rig.step();
    rig.send(event(
        definition.id.get(),
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    rig
}

fn gated_hit(rig: &NativeRig, inst: u32, cut: u32) -> AudioEvent {
    let mut hit = AudioEvent::new(rig.engine.now(), SlotId::new(1), 1, InstId::new(inst));
    for (id, value) in [(ctl::FREQ, 220.0), (ctl::LEGATO, 2.0)] {
        hit.push_ctl(id, Ctl::Const(value))
            .expect("event control fits");
    }
    hit.voice_hint = (cut & 0xff) << 8;
    hit
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn off_gate_is_bit_identical_and_ping_outlives_the_implicit_fade() {
    let plain = render(&voice(0.0, 0.8, false, 1.0, 0.0), 100);
    let off = render(&voice(0.0, 0.8, true, 1.0, 0.0), 100);
    assert_eq!(off.len(), plain.len());
    for (actual, expected) in off.iter().zip(&plain) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }

    let ping = render(&voice(1.0, 0.8, true, 1.0, 0.0), 100);
    let tail = &ping[14_400..16_800];
    let rms = (tail.iter().map(|sample| sample * sample).sum::<f32>() / tail.len() as f32).sqrt();
    assert!(rms > 1.0e-3, "shaped tail RMS was {rms}");
}

#[test]
fn short_ping_ends_and_zero_level_mode_stays_silent() {
    let mut rig = started(&voice(1.0, 0.2, true, 1.0, 0.0));
    let short = rig.run(562).0;
    assert!(short[short.len() - 256..]
        .iter()
        .all(|sample| *sample == 0.0));
    assert!(rig.engine.voices().voices.iter().all(|voice| !voice.active));

    let mut level = started(&voice(2.0, 0.5, true, 0.0, 0.0));
    let silent = level.run(24).0;
    assert!(silent.iter().all(|sample| sample.abs() <= 1.0e-6));
    assert!(level
        .engine
        .voices()
        .voices
        .iter()
        .all(|voice| !voice.active));
}

#[test]
fn bypassed_gate_keeps_fade_end_and_registered_gain() {
    let plain = render(&voice(0.0, 0.8, false, 1.0, 0.0), 100);
    let bypass = render(&voice(1.0, 0.8, true, 1.0, 21.0), 100);
    assert_eq!(bypass.len(), plain.len());
    let plain_end = plain.iter().rposition(|sample| *sample != 0.0);
    let bypass_end = bypass.iter().rposition(|sample| *sample != 0.0);
    assert_eq!(bypass_end, plain_end);
    for (actual, expected) in bypass.iter().zip(plain) {
        assert!((actual - 0.8 * expected).abs() <= 1.0e-6 + 1.0e-6 * expected.abs());
    }
}

#[test]
fn rebuilding_template_clears_gate_count_and_restores_implicit_fade() {
    let rig = NativeRig::native();
    let env = rig.engine.build_env();
    let gated = voice(1.0, 0.8, true, 1.0, 0.0);
    let gate_free = voice(0.0, 0.8, false, 1.0, 0.0);
    let mut raw = RawGraph::boxed();
    raw.load(&gated).expect("gated graph loads");
    let mut template = Template::boxed();
    template.build(&raw, &env).expect("gated template builds");
    assert_eq!(template.gates, 1);
    raw.load(&gate_free).expect("gate-free graph loads");
    template
        .build(&raw, &env)
        .expect("gate-free template rebuilds in place");
    assert_eq!(template.gates, 0);
    let fresh = Template::from_inst(&gate_free, &env).expect("fresh template builds");
    assert_eq!(template.nodes(), fresh.nodes());
    assert_eq!(
        render(&gate_free, 100),
        render(&voice(0.0, 0.8, false, 1.0, 0.0), 100)
    );
}

#[test]
fn cut_group_still_chokes_a_layer_shaped_voice() {
    let mut rig = NativeRig::native_with({
        let mut cfg = config(&caps(), StoreKind::NativeArc);
        cfg.max_block = 256;
        cfg
    });
    let ping = voice(1.0, 0.8, true, 1.0, 0.0);
    rig.install(&ping);
    let _ = rig.step();
    rig.send(gated_hit(&rig, 1, 3));
    let _ = rig.step();
    let first = rig
        .engine
        .voices()
        .voices
        .iter()
        .position(|voice| voice.active)
        .expect("first layer voice starts");
    assert!(rig.engine.voices().voices[first].fade.is_none());
    rig.send(gated_hit(&rig, 1, 3));
    let _ = rig.step();
    assert!(rig.engine.voices().voices[first].fade.is_some());
    let _ = rig.run(3);
    assert!(!rig.engine.voices().voices[first].active);
    assert!(rig.engine.voices().voices.iter().any(|voice| voice.active));
}

#[test]
fn slot_reuse_after_gate_completion_matches_a_fresh_implicit_voice() {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.max_block = 256;
    let mut reused = NativeRig::native_with(cfg);
    let ping = voice(1.0, 0.2, true, 1.0, 0.0);
    let mut plain = voice(0.0, 0.8, false, 1.0, 0.0);
    plain.id = InstId::new(2);
    reused.install(&ping);
    reused.install(&plain);
    let _ = reused.step();
    reused.send(event(
        1,
        reused.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    let _ = reused.run(600);
    assert!(reused
        .engine
        .voices()
        .voices
        .iter()
        .all(|voice| !voice.active));
    reused.send(event(
        2,
        reused.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    let reused_audio = reused.run(100).0;

    let fresh_audio = render(&plain, 100);
    assert_eq!(reused_audio, fresh_audio);
}

fn partition_render(sr: f32, block: usize) -> Vec<f32> {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.sample_rate = sr;
    cfg.max_block = block;
    let mut rig = NativeRig::native_with(cfg);
    rig.install(&voice(1.0, 0.2, true, 1.0, 0.0));
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    let frames = 24_832usize;
    rig.run(frames.div_ceil(block)).0
}

#[test]
fn layer_lifetime_is_bitwise_invariant_across_rates_and_partitions() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let reference = partition_render(sr, 64);
        let other = partition_render(sr, 256);
        assert_eq!(reference, other, "rate {sr}, block 256");
        let other = partition_render(sr, 97);
        assert_eq!(reference, other, "rate {sr}, block 97");
    }
}

#[test]
fn decoded_template_and_browser_voice_match_native() {
    let definition = voice(1.0, 0.2, true, 1.0, 0.0);
    let mut bytes = Vec::new();
    encode_inst(&definition, &mut bytes).expect("instrument encodes");
    let env = NativeRig::native().engine.build_env();
    let native_template = Template::from_inst(&definition, &env).expect("native template builds");
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    decode_graph(&bytes, &mut raw, &mut bus).expect("graph decodes");
    let mut decoded = Template::boxed();
    decoded.build(&raw, &env).expect("decoded template builds");
    assert_eq!(decoded.nodes(), native_template.nodes());
    for i in 0..native_template.n_nodes {
        assert_eq!(decoded.seed_ordinal(i), native_template.seed_ordinal(i));
    }

    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.max_block = 256;
    let mut native = NativeRig::native_with(cfg);
    native.install(&definition);
    let _ = native.step();
    native.send(event(
        1,
        native.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    let native_audio = native.run(188).0;

    let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
    cfg.max_block = 256;
    let mut browser = BrowserRig::browser_with(cfg);
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    browser.push(&record);
    let _ = browser.run(1);
    browser.send(event(
        1,
        browser.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    let browser_audio = browser.run(188).0;
    assert_eq!(native_audio, browser_audio);
}

#[test]
fn rendering_a_layer_voice_keeps_callback_allocation_free() {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.max_block = 256;
    let mut rig = NativeRig::native_with(cfg);
    rig.install(&voice(1.0, 0.2, true, 1.0, 0.0));
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 0.11)],
    ));
    let (_, allocations) = armed(|| {
        let _ = rig.step();
    });
    assert_eq!(allocations, 0);
}
