//! MOD-004 regression coverage. Every render uses `Rig::run`/`step`, whose
//! callback allocation probe asserts that rendering allocates nothing.

mod cross_path;
mod invariance;

use super::{caps, config, ctl, event, noise, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::catalog::spec as effect_spec;
use crate::dsp::graph::{BankRef, Edge, EffectKind, InstDef, InstId, UGenSpec};
use crate::dsp::ring::{encode_graph_record, encode_sample_begin, encode_slice};
use crate::dsp::ugen::{catalog, Node, RawGraph, Template};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

pub(super) fn graph(nodes: Vec<UGenSpec>, edges: Vec<Edge>) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        node_params: Box::new([]),
    }
}

pub(super) fn graph_with_params(
    nodes: Vec<UGenSpec>,
    edges: Vec<Edge>,
    params: Vec<(CtlId, Ctl)>,
) -> InstDef {
    let mut def = graph(nodes, edges);
    def.params = params.into_boxed_slice();
    def
}

pub(super) fn edge(from: u16, to: u16, port: u8, output: u8) -> Edge {
    Edge {
        from,
        to,
        port,
        output,
    }
}

pub(super) fn mono_graph() -> InstDef {
    graph_with_params(
        vec![
            UGenSpec::SinOsc,
            UGenSpec::Param(CtlId::new(90)),
            UGenSpec::Mul,
        ],
        vec![edge(0, 2, 0, 0), edge(1, 2, 1, 0)],
        vec![(CtlId::new(90), Ctl::Const(0.5))],
    )
}

pub(super) fn va_filter_graph(pair: bool, main_gain: f32) -> InstDef {
    let (nodes, edges, filters) = if pair {
        (
            vec![
                UGenSpec::VaSource,
                UGenSpec::VaFilter,
                UGenSpec::Add,
                UGenSpec::AuxOut,
                UGenSpec::Const(main_gain),
                UGenSpec::Mul,
            ],
            vec![
                edge(0, 1, 0, 0),
                edge(1, 5, 0, 0),
                edge(4, 5, 1, 0),
                edge(5, 2, 0, 0),
                edge(1, 3, 0, 1),
            ],
            vec![1],
        )
    } else {
        (
            vec![
                UGenSpec::VaSource,
                UGenSpec::VaFilter,
                UGenSpec::VaFilter,
                UGenSpec::Add,
                UGenSpec::AuxOut,
                UGenSpec::Const(main_gain),
                UGenSpec::Mul,
            ],
            vec![
                edge(0, 1, 0, 0),
                edge(0, 2, 0, 0),
                edge(1, 6, 0, 0),
                edge(5, 6, 1, 0),
                edge(6, 3, 0, 0),
                edge(2, 4, 0, 0),
            ],
            vec![1, 2],
        )
    };
    let mut def = graph(nodes, edges);
    let mut params = vec![(0, CtlId::new(79), Ctl::Const(0.5))];
    for (index, node) in filters.into_iter().enumerate() {
        for port in [1, 2] {
            params.push((
                node,
                catalog::port_ctl(&Node::VaFilter, port).unwrap(),
                Ctl::Const(0.5),
            ));
        }
        if !pair && index == 1 {
            params.push((
                node,
                catalog::port_ctl(&Node::VaFilter, 4).unwrap(),
                Ctl::Const(1.0),
            ));
        }
    }
    def.node_params = params.into_boxed_slice();
    def
}

pub(super) fn fm_graph(pair: bool) -> InstDef {
    if pair {
        graph(
            vec![UGenSpec::FmPair, UGenSpec::Add, UGenSpec::AuxOut],
            vec![edge(0, 1, 0, 0), edge(0, 2, 0, 1)],
        )
    } else {
        let mut def = graph(
            vec![
                UGenSpec::FmPair,
                UGenSpec::FmPair,
                UGenSpec::Add,
                UGenSpec::AuxOut,
            ],
            vec![edge(0, 2, 0, 0), edge(1, 3, 0, 0)],
        );
        def.node_params = vec![(
            1,
            catalog::port_ctl(&Node::FmPair, 4).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice();
        def
    }
}

pub(super) fn stereo_graph() -> InstDef {
    let gain = effect_spec(EffectKind::Gain, &[("gain", Ctl::Const(-6.0206))]).unwrap();
    graph_with_params(
        vec![
            UGenSpec::SamplePlay(BankRef::new(41)),
            UGenSpec::Effect(gain),
            UGenSpec::Param(CtlId::new(90)),
            UGenSpec::Mul,
        ],
        vec![edge(0, 1, 0, 1), edge(1, 3, 0, 0), edge(2, 3, 1, 0)],
        vec![(CtlId::new(90), Ctl::Const(0.5))],
    )
}

pub(super) fn mono_sample_graph() -> InstDef {
    graph_with_params(
        vec![
            UGenSpec::SamplePlay(BankRef::new(41)),
            UGenSpec::Param(CtlId::new(90)),
            UGenSpec::Mul,
        ],
        vec![edge(0, 2, 0, 0), edge(1, 2, 1, 0)],
        vec![(CtlId::new(90), Ctl::Const(0.5))],
    )
}

pub(super) fn stereo_frames(left_zero: bool, right_zero: bool) -> Vec<f32> {
    let left = if left_zero {
        vec![0.0; 16_384]
    } else {
        noise(16_384, 0.5, 3)
    };
    let right = if right_zero {
        vec![0.0; 16_384]
    } else {
        noise(16_384, 0.5, 7)
    };
    let mut interleaved = Vec::with_capacity(32_768);
    for (&l, &r) in left.iter().zip(&right) {
        interleaved.extend([l, r]);
    }
    interleaved
}

pub(super) fn native_rig(rate: f32, block: usize) -> NativeRig {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.sample_rate = rate;
    cfg.max_block = block;
    NativeRig::native_with(cfg)
}

pub(super) fn browser_rig(rate: f32, block: usize) -> BrowserRig {
    let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
    cfg.sample_rate = rate;
    cfg.max_block = block;
    BrowserRig::browser_with(cfg)
}

pub(super) fn load_native(rig: &mut NativeRig, def: &InstDef, frames: Option<&[f32]>) {
    if let Some(frames) = frames {
        rig.sample(41, frames.to_vec(), 2);
    }
    rig.install(def);
}

pub(super) fn load_browser(rig: &mut BrowserRig, def: &InstDef, frames: Option<&[f32]>) {
    if let Some(frames) = frames {
        let frames_per_channel = u32::try_from(frames.len() / 2).unwrap();
        let begin = encode_sample_begin(41, 1, frames_per_channel, 2, 48_000);
        rig.push(&begin);
        let mut slice = Vec::new();
        for (index, chunk) in frames.chunks(16_384).enumerate() {
            let offset = u32::try_from(index * 16_384).unwrap();
            encode_slice(41, offset, chunk, &mut slice);
            rig.push(&slice);
        }
    }
    let mut bytes = Vec::new();
    encode_inst(def, &mut bytes).unwrap();
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    rig.push(&record);
}

pub(super) trait WindowRig {
    fn window_event(&mut self, rate: f32, ctls: &[(CtlId, f32)]);
    fn window_run(&mut self, blocks: usize) -> (Vec<f32>, Vec<f32>);
}

impl WindowRig for NativeRig {
    fn window_event(&mut self, rate: f32, ctls: &[(CtlId, f32)]) {
        // The quarter-frame offset keeps the onset at frame 2048 in every block
        // partition; f64 block-end accumulation can otherwise place an exact
        // block-boundary event at offset n-1 of the previous block.
        self.send(event(1, 2048.25 / f64::from(rate), ctls));
    }
    fn window_run(&mut self, blocks: usize) -> (Vec<f32>, Vec<f32>) {
        self.run(blocks)
    }
}

impl WindowRig for BrowserRig {
    fn window_event(&mut self, rate: f32, ctls: &[(CtlId, f32)]) {
        self.send(event(1, 2048.25 / f64::from(rate), ctls));
    }
    fn window_run(&mut self, blocks: usize) -> (Vec<f32>, Vec<f32>) {
        self.run(blocks)
    }
}

pub(super) fn render_windowed<R: WindowRig>(
    rig: &mut R,
    ctls: &[(CtlId, f32)],
    rate: f32,
    block: usize,
) -> (Vec<f32>, Vec<f32>) {
    rig.window_event(rate, ctls);
    let (left, right) = rig.window_run((10_240_usize).div_ceil(block));
    (left[2048..10_240].to_vec(), right[2048..10_240].to_vec())
}

pub(super) fn default_ctls() -> [(CtlId, f32); 2] {
    [(ctl::FREQ, 220.0), (ctl::LEGATO, 10.0)]
}

pub(super) fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|sample| sample.to_bits()).collect()
}

pub(super) fn decoded_template(def: &InstDef, env: &crate::dsp::ugen::BuildEnv) -> Box<Template> {
    let mut bytes = Vec::new();
    encode_inst(def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    decode_graph(&bytes, &mut raw, &mut bus).unwrap();
    let mut template = Template::boxed();
    template.build(&raw, env).unwrap();
    template
}
