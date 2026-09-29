//! Headless renders of every synthesis model (design 12.4, design-music 4
//! and 6) as hand-built `InstDef`s shaped like the prelude
//! templates: `sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`,
//! `granular`. Each renders non-silent, finite audio with zero callback
//! allocation, on both tiers (the browser tier through the install byte
//! codec). The prelude-source path (`.vact` -> `InstDef`) is BE-FINAL's
//! `host/tests/e2e/templates.rs`.

use super::{ctl, event, noise, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{BankRef, Edge, GranSrc, InstDef, InstId, TableRef, UGenSpec};
use crate::dsp::ring::{encode_graph_record, encode_sample_begin, encode_slice};
use crate::dsp::ugen::{RawGraph, Template};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

const BANK: u32 = 30;
const TABLE: u32 = 31;

fn e(from: u16, to: u16, port: u8) -> Edge {
    Edge {
        from,
        to,
        port,
        output: 0,
    }
}

fn def(nodes: Vec<UGenSpec>, edges: Vec<Edge>, node_params: Vec<(u16, CtlId, Ctl)>) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        node_params: node_params.into_boxed_slice(),
    }
}

fn row(name: &str) -> CtlId {
    crate::dsp::controls::row(name).unwrap().ctl
}

/// The models, by template name.
fn models() -> Vec<(&'static str, InstDef)> {
    use UGenSpec as U;
    vec![
        (
            "sampler",
            def(
                vec![U::SamplePlay(BankRef::new(BANK)), U::EnvPerc, U::Mul],
                vec![e(0, 2, 0), e(1, 2, 1)],
                vec![],
            ),
        ),
        (
            "analog",
            def(
                vec![
                    U::Vco { unison_max: 4 },
                    U::SubOsc,
                    U::Const(0.3),
                    U::Mul,
                    U::Add,
                    U::Ladder,
                    U::EnvAdsr,
                    U::Mul,
                    U::Param(ctl::AMP),
                    U::Mul,
                ],
                vec![
                    e(1, 3, 0),
                    e(2, 3, 1),
                    e(0, 4, 0),
                    e(3, 4, 1),
                    e(4, 5, 0),
                    e(5, 7, 0),
                    e(6, 7, 1),
                    e(7, 9, 0),
                    e(8, 9, 1),
                ],
                vec![(0, row("unison"), Ctl::Const(3.0))],
            ),
        ),
        (
            "fm",
            def(
                vec![U::FmOp, U::FmOp, U::EnvAdsr, U::Mul],
                vec![e(0, 1, 3), e(1, 3, 0), e(2, 3, 1)],
                vec![
                    (0, row("ratio"), Ctl::Const(14.0)),
                    (0, row("index"), Ctl::Const(2.0)),
                ],
            ),
        ),
        ("phase-drum", def(vec![U::FmDrum], vec![], vec![])),
        (
            "feedback-metal-drum",
            def(vec![U::FeedbackMetal], vec![], vec![]),
        ),
        (
            "low-drum",
            def(
                vec![U::Const(60.0), U::Const(0.0), U::AnalogPercussion],
                vec![e(0, 2, 0), e(1, 2, 6)],
                vec![],
            ),
        ),
        (
            "wire-drum",
            def(
                vec![U::Const(180.0), U::Const(1.0), U::AnalogPercussion],
                vec![e(0, 2, 0), e(1, 2, 6)],
                vec![],
            ),
        ),
        (
            "metal-hat",
            def(
                vec![U::Const(3900.0), U::Const(2.0), U::AnalogPercussion],
                vec![e(0, 2, 0), e(1, 2, 6)],
                vec![],
            ),
        ),
        (
            "fusion-drum",
            def(
                vec![
                    U::FeedbackDrum,
                    U::NoiseDrum,
                    U::Add,
                    U::SineDrum,
                    U::Add,
                    U::Param(ctl::AMP),
                    U::Mul,
                ],
                vec![
                    e(0, 2, 0),
                    e(1, 2, 1),
                    e(2, 4, 0),
                    e(3, 4, 1),
                    e(4, 6, 0),
                    e(5, 6, 1),
                ],
                vec![],
            ),
        ),
        (
            "pd",
            def(
                vec![U::PhaseDistortion, U::EnvPerc, U::Mul],
                vec![e(0, 2, 0), e(1, 2, 1)],
                vec![(0, row("shape"), Ctl::Const(0.7))],
            ),
        ),
        (
            "additive",
            def(
                vec![U::Additive { partials_max: 8 }, U::Param(ctl::AMP), U::Mul],
                vec![e(0, 2, 0), e(1, 2, 1)],
                vec![],
            ),
        ),
        (
            "wavetable",
            def(
                vec![
                    U::Wavetable(TableRef::new(TABLE)),
                    U::Svf,
                    U::EnvAdsr,
                    U::Mul,
                ],
                vec![e(0, 1, 0), e(1, 3, 0), e(2, 3, 1)],
                vec![],
            ),
        ),
        (
            "granular",
            def(
                vec![
                    U::Granular(GranSrc::Sample(BankRef::new(BANK))),
                    U::EnvAdsr,
                    U::Mul,
                ],
                vec![e(0, 2, 0), e(1, 2, 1)],
                vec![
                    (0, row("density"), Ctl::Const(40.0)),
                    (0, row("size"), Ctl::Const(0.05)),
                ],
            ),
        ),
    ]
}

/// Two frames of a saw-then-sine wavetable.
fn table() -> Vec<f32> {
    let n = crate::dsp::ugen::wavetable::FRAME;
    #[allow(clippy::cast_precision_loss)]
    (0..2 * n)
        .map(|i| {
            let x = (i % n) as f32 / n as f32;
            if i < n {
                2.0 * x - 1.0
            } else {
                (std::f32::consts::TAU * x).sin()
            }
        })
        .collect()
}

fn note(rig_now: f64) -> crate::host::wire::AudioEvent {
    event(
        1,
        rig_now,
        &[
            (ctl::FREQ, 220.0),
            (ctl::AMP, 0.8),
            (ctl::LEGATO, 0.2),
            (ctl::PAN, 0.5),
        ],
    )
}

#[test]
fn every_model_renders_on_the_native_tier() {
    for (name, d) in models() {
        let mut rig = NativeRig::native();
        rig.sample(BANK, noise(24_000, 0.5, 9), 1);
        rig.sample(TABLE, table(), 1);
        rig.install(&d);
        let _ = rig.step();
        let t = rig.engine.now();
        rig.send(note(t));
        let (l, r) = rig.run(120);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{name} finite");
        assert!(rms(&l) > 1.0e-3, "{name} sounds ({})", rms(&l));
        assert!(l.iter().all(|v| v.abs() < 4.0), "{name} bounded");
    }
}

#[test]
fn every_model_renders_on_the_browser_tier_through_the_codec() {
    for (name, d) in models() {
        let mut rig = BrowserRig::browser(4 << 20);
        for (res, data) in [(BANK, noise(16_384, 0.5, 9)), (TABLE, table())] {
            let frames = data.len();
            rig.push(&encode_sample_begin(
                res,
                1,
                u32::try_from(frames).unwrap(),
                1,
                48_000,
            ));
            for (k, chunk) in data.chunks(16_384).enumerate() {
                let mut rec = Vec::new();
                encode_slice(res, u32::try_from(k * 16_384).unwrap(), chunk, &mut rec);
                rig.push(&rec);
            }
        }
        let mut bytes = Vec::new();
        encode_inst(&d, &mut bytes).unwrap();
        let mut rec = Vec::new();
        encode_graph_record(200, 1, &bytes, &mut rec);
        rig.push(&rec);
        let _ = rig.run(6);
        assert!(
            rig.engine.template(InstId::new(1)).is_some(),
            "{name} installed"
        );
        let t = rig.engine.now();
        rig.send(note(t));
        let (l, _) = rig.run(120);
        assert!(rms(&l) > 1.0e-3, "{name} sounds in the browser tier");
    }
}

#[test]
fn the_codec_round_trips_to_the_same_template() {
    let env = NativeRig::native().engine.build_env();
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    for (name, d) in models() {
        let native = Template::from_inst(&d, &env).unwrap();
        let mut bytes = Vec::new();
        encode_inst(&d, &mut bytes).unwrap();
        decode_graph(&bytes, &mut raw, &mut bus).unwrap();
        let mut decoded = Template::boxed();
        decoded.build(&raw, &env).unwrap();
        assert_eq!(native.nodes(), decoded.nodes(), "{name} nodes");
        assert_eq!(native.params(), decoded.params(), "{name} params");
    }
}
