//! FM1V-40 FM-mod algorithm registration and fm6-core patch codec coverage.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::fm::{
    self, engine,
    patch::{self, Fm6Patch},
};
use crate::dsp::ugen::{
    catalog, BuildError, Inp, Kx, Node, NodeState, RawGraph, Src, Template, MAX_PORTS,
};

fn definition(nodes: Vec<UGenSpec>) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

fn synthetic_patch() -> Fm6Patch {
    let mut patch = Fm6Patch::EMPTY;
    patch.params[patch::global::ALGORITHM] = 0;
    let base = patch::op_base(1);
    patch.params[base + patch::op::OUTPUT_LEVEL] = 99;
    patch.params[base + patch::op::COARSE] = 1;
    patch.params[base + patch::op::R1..base + patch::op::R4 + 1].copy_from_slice(&[99, 99, 99, 99]);
    patch.params[base + patch::op::L1..base + patch::op::L4 + 1].copy_from_slice(&[99, 99, 99, 99]);
    patch
}

fn render_native(def: &InstDef) -> Vec<f32> {
    let mut native = NativeRig::native();
    native.install(def);
    let _ = native.step();
    native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
    native.run(20).0
}

fn render_browser(bytes: &[u8]) -> Vec<f32> {
    let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
    cfg.voice_seconds = 0.5;
    let mut browser = BrowserRig::browser_with(cfg);
    let mut record = Vec::new();
    encode_graph_record(1, 1, bytes, &mut record);
    browser.push(&record);
    let _ = browser.run(6);
    browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
    browser.run(20).0
}

#[test]
fn fm6_registry_fm_mod_ports_append_only() {
    let ports = catalog::ports(&Node::FmMod);
    assert_eq!(
        ports[..3]
            .iter()
            .map(|port| (port.name, port.default))
            .collect::<Vec<_>>(),
        [("in", 0.0), ("mod", 0.0), ("index", 1.0)]
    );
    assert_eq!(
        ports[3..=6]
            .iter()
            .map(|port| port.name)
            .collect::<Vec<_>>(),
        ["algorithm", "freq", "ratio", "velocity"]
    );
    assert_eq!(
        [
            engine::fm_mod_port::ALGORITHM,
            engine::fm_mod_port::FREQ,
            engine::fm_mod_port::RATIO,
            engine::fm_mod_port::VELOCITY,
        ],
        [3, 4, 5, 6]
    );
}

#[test]
fn fm6_registry_negative_algorithm_is_legacy_bit_identical() {
    let input_values = [-0.75, -0.25, 0.25, 0.75];
    let modulation_values = [-0.5, 0.0, 0.25, 0.5];
    let input: [f32; 64] = std::array::from_fn(|index| input_values[index % 4]);
    let modulation: [f32; 64] = std::array::from_fn(|index| modulation_values[index % 4]);
    let mut inputs: [Inp<'_>; MAX_PORTS] = std::array::from_fn(|_| Inp::Val(0.0));
    inputs[engine::fm_mod_port::IN] = Inp::Buf(&input);
    inputs[engine::fm_mod_port::MOD] = Inp::Buf(&modulation);
    inputs[engine::fm_mod_port::INDEX] = Inp::Val(0.7);
    inputs[engine::fm_mod_port::ALGORITHM] = Inp::Val(-3.0);

    let mut expected = [0.0; 64];
    fm::modulate(&inputs, &mut expected);
    let mut actual = [0.0; 64];
    let mut state = NodeState::default();
    state.s[0] = 0.375;
    let original_state = state;
    let mut memory = [3.25; engine::STATE_FLOATS];
    let original_memory = memory;
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: actual.len(),
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 0,
    };
    fm::modulate_with_algorithm(&inputs, &mut state, &mut memory, &mut actual, &kx);
    assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
    assert_eq!(state, original_state);
    assert_eq!(memory, original_memory);
}

#[test]
fn fm6_registry_fm_mod_implicit_binding_matches_kernel() {
    let def = definition(vec![UGenSpec::FmMod]);
    let rig = NativeRig::native();
    let mut env = rig.engine.build_env();
    env.voice_mem = 24_000;
    let template = Template::from_inst(&def, &env).expect("FM-mod template");
    let node = template.nodes()[0];
    for (port, name) in ["algorithm", "freq", "ratio", "velocity"]
        .into_iter()
        .enumerate()
    {
        let Src::Param(slot) = node.inputs[3 + port] else {
            panic!("{name} port must bind to an implicit control");
        };
        let row = crate::dsp::controls::row(name).expect("control row");
        assert_eq!(template.params()[usize::from(slot)].0, row.ctl, "{name}");
    }
    assert_eq!(node.node, Node::FmMod);
}

#[test]
fn fm6_registry_fm_template_memory_and_limit() {
    let def = definition(vec![UGenSpec::FmMod]);
    let rig = NativeRig::native();
    let mut env = rig.engine.build_env();
    env.voice_mem = engine::STATE_FLOATS;
    let template = Template::from_inst(&def, &env).expect("FM-mod state fits");
    assert_eq!(template.mem_total, engine::STATE_FLOATS);
    env.voice_mem -= 1;
    assert_eq!(
        Template::from_inst(&def, &env).err(),
        Some(BuildError::MemExceeded)
    );
}

#[test]
fn fm6_registry_codec_parity_with_and_without_patch() {
    for patch in [None, Some(synthetic_patch())] {
        let spec = UGenSpec::Fm6Core {
            patch: patch.map(Box::new),
        };
        let def = definition(vec![spec]);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).expect("encode fm6-core");
        assert_eq!(bytes.get(9), Some(&105));

        let native_rig = NativeRig::native();
        let mut env = native_rig.engine.build_env();
        env.voice_mem = 24_000;
        let native_template = Template::from_inst(&def, &env).expect("native template");
        let mut raw = RawGraph::boxed();
        let mut bus = BusTemplate::new();
        decode_graph(&bytes, &mut raw, &mut bus).expect("decode graph");
        let mut browser_template = Template::boxed();
        browser_template
            .build(&raw, &env)
            .expect("browser template");
        assert_eq!(browser_template.nodes(), native_template.nodes());
        assert_eq!(
            browser_template.n_fm6_payloads,
            native_template.n_fm6_payloads
        );
        assert_eq!(
            &browser_template.fm6_payloads[..browser_template.n_fm6_payloads],
            &native_template.fm6_payloads[..native_template.n_fm6_payloads]
        );

        let native_audio = render_native(&def);
        let browser_audio = render_browser(&bytes);
        for audio in [&native_audio, &browser_audio] {
            assert!(audio.iter().all(|sample| sample.is_finite()));
            assert!(rms(audio) > 1.0e-4);
            assert!(audio.iter().all(|sample| sample.abs() <= 1.0));
        }
    }
}

#[test]
fn fm6_registry_payload_limit_is_four() {
    let patch = synthetic_patch();
    let four = definition(
        (0..4)
            .map(|_| UGenSpec::Fm6Core {
                patch: Some(Box::new(patch)),
            })
            .collect(),
    );
    let base = NativeRig::native();
    let mut env = base.engine.build_env();
    env.voice_mem = 24_000;
    let template = Template::from_inst(&four, &env).expect("four payloads fit");
    assert_eq!(template.n_fm6_payloads, 4);

    let five = definition(
        (0..5)
            .map(|_| UGenSpec::Fm6Core {
                patch: Some(Box::new(patch)),
            })
            .collect(),
    );
    assert_eq!(
        Template::from_inst(&five, &env).err(),
        Some(BuildError::TooManyParams)
    );
}

#[test]
fn fm6_registry_codec_tag_is_105_and_versioned() {
    let def = definition(vec![UGenSpec::Fm6Core { patch: None }]);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).expect("encode macro mode");
    assert_eq!(bytes.get(9), Some(&105));
    assert_eq!(bytes.get(10), Some(&patch::WIRE_VERSION));
    assert_eq!(bytes.get(11), Some(&0));

    let def = definition(vec![UGenSpec::Fm6Core {
        patch: Some(Box::new(synthetic_patch())),
    }]);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).expect("encode patch mode");
    assert_eq!(bytes.get(9), Some(&105));
    assert_eq!(bytes.get(10), Some(&patch::WIRE_VERSION));
    assert_eq!(bytes.get(11), Some(&1));
    assert_eq!(bytes.len(), 17 + patch::VOICE_PARAMS);
}
