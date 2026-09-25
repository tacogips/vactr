//! Instrument templates and ugen kernels (design 12.1, 12.2, 12.4).
//!
//! An `InstDef` (evaluator side, boxed slices) is first flattened into a
//! fixed-capacity `RawGraph` (natively with `RawGraph::load`, in the
//! browser by decoding install bytes, `arena::decode_graph`), then compiled
//! into a `Template`: nodes in topological order with every input port
//! resolved to a source, the template's control list, per-node delay
//! memory offsets inside a voice's region, and the resources it reads.
//! `Template::build` never allocates, so the worklet can compile into a
//! preallocated template slot.
//!
//! The kernels (`osc`, `filter`, `env`, `fm`, `additive`, `wavetable`,
//! `sample`, and `granular`) render one node for one block into its output
//! buffer; `voice.rs` walks a template in order. Effect nodes run a voice
//! `FxUnit` (`voice.rs`).

pub mod additive;
pub mod catalog;
pub mod env;
pub mod filter;
pub mod fm;
pub mod mixer;
pub mod osc;
pub mod sample;
pub mod wavetable;

use crate::dsp::arena::SampleStore;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellId;
use crate::dsp::controls;
use crate::dsp::effects::prim::{Biquad, DelayLine};
use crate::dsp::effects::{self, FxStats, MAX_FX_PARAMS};
use crate::dsp::graph::{
    BankRef, EffectKind, GranSrc, InstDef, InstId, TableRef, UGenSpec, NODE_CAP,
};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

pub use catalog::MAX_PORTS;

/// The most distinct controls one template reads.
pub const MAX_PARAMS: usize = 48;
/// The most edges one template has.
pub const MAX_EDGES: usize = 1024;
/// The most node parameters one template has (effect parameters included).
pub const MAX_NODE_PARAMS: usize = 1024;
/// The most effect nodes one instrument template has (per-voice units).
pub const MAX_VOICE_FX: usize = 4;
/// The most resources (samples, tables) one template references.
pub const MAX_REFS: usize = 8;
/// The most output (sink) nodes, summed.
pub const MAX_SINKS: usize = 8;

/// The amplitude control (`amp`, and `gain` by Q4).
pub const AMP: CtlId = CtlId::new(1);
/// The pan control.
pub const PAN: CtlId = CtlId::new(4);
/// The bank control: the installed resource id a sample event plays.
pub const BANK: CtlId = CtlId::new(24);

/// A node kind: the POD mirror of `UGenSpec` (effect parameters move to
/// the graph's node parameters).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Node {
    SinOsc,
    Saw,
    Pulse,
    Tri,
    WhiteNoise,
    Lpf,
    Hpf,
    Bpf,
    Delay,
    Comb,
    EnvPerc,
    EnvAdsr,
    Line,
    SamplePlay(BankRef),
    Mul,
    Add,
    Const(f32),
    Param(CtlId),
    Vco {
        unison_max: u8,
    },
    SubOsc,
    Ladder,
    Svf,
    FmOp,
    FmMod,
    PhaseDistortion,
    Additive {
        partials_max: u8,
    },
    Wavetable(TableRef),
    Granular(GranSrc),
    /// An effect as a ugen; `fx` is its per-voice unit index.
    Effect {
        kind: EffectKind,
        fx: u8,
    },
}

impl Node {
    /// The node of a spec (an effect's parameters are not included).
    #[must_use]
    pub fn from_spec(spec: &UGenSpec) -> Node {
        catalog::node_of(spec)
    }

    /// True for the envelope nodes that decide when a voice ends.
    #[must_use]
    pub const fn is_env(&self) -> bool {
        matches!(self, Node::EnvPerc | Node::EnvAdsr)
    }
}

/// Where one input port reads from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Src {
    /// Nothing feeds the port: its constant default.
    Default(f32),
    /// The output buffer of an earlier node (topological index).
    Node(u16),
    Const(f32),
    /// The voice's value of template control `k` (`Template::params`).
    Param(u8),
    /// A template-level cell, re-read every block (control rate).
    Cell(CellId),
}

/// One compiled node.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct NodeSpec {
    pub node: Node,
    pub inputs: [Src; MAX_PORTS],
    /// Delay memory inside the voice region.
    pub mem_off: u32,
    pub mem_len: u32,
}

impl NodeSpec {
    const EMPTY: NodeSpec = NodeSpec {
        node: Node::Const(0.0),
        inputs: [Src::Default(0.0); MAX_PORTS],
        mem_off: 0,
        mem_len: 0,
    };
}

/// Per-node scalar state inside a voice.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct NodeState {
    pub s: [f32; 8],
    pub bq: [Biquad; 2],
    pub dl: DelayLine,
    pub u: [u32; 4],
}

impl NodeState {
    /// Marks the node finished (an envelope or a one-shot player).
    pub fn finish(&mut self) {
        self.u[3] = 1;
    }

    /// True once `finish` ran.
    #[must_use]
    pub fn done(&self) -> bool {
        self.u[3] == 1
    }
}

/// One kernel input: an audio-rate buffer or a block-constant value.
#[derive(Clone, Copy, Debug)]
pub enum Inp<'a> {
    Buf(&'a [f32]),
    Val(f32),
}

impl Inp<'_> {
    /// The value at frame `i` (the last frame past the end).
    #[inline]
    #[must_use]
    pub fn at(&self, i: usize) -> f32 {
        match self {
            Inp::Val(v) => *v,
            Inp::Buf(b) => b.get(i).or_else(|| b.last()).copied().unwrap_or(0.0),
        }
    }

    /// The block's first value (control-rate reads).
    #[inline]
    #[must_use]
    pub fn first(&self) -> f32 {
        self.at(0)
    }
}

/// What a kernel may read besides its inputs.
pub struct Kx<'a> {
    pub sr: f32,
    /// Frames of this block during which the gate is open (0 = closed).
    pub gate: usize,
    /// The event's `bank` control (the resource a sample event plays).
    pub bank: Option<u32>,
    pub store: &'a SampleStore,
    pub caps: &'a CapabilitySet,
    pub stats: &'a mut FxStats,
    /// The voice's seed for noise and grain randomness.
    pub seed: u32,
}

/// Why a graph cannot be compiled (reported as `graph-too-large` or an
/// install failure by the caller).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildError {
    TooManyNodes,
    TooManyEdges,
    TooManyParams,
    TooManyEffects,
    BadEdge,
    Cycle,
    Empty,
    /// The per-voice memory cannot hold the template's fixed state.
    MemExceeded,
}

impl BuildError {
    /// A one-line reason.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            BuildError::TooManyNodes => "more than 256 nodes",
            BuildError::TooManyEdges => "too many connections",
            BuildError::TooManyParams => "too many controls or parameters",
            BuildError::TooManyEffects => "more than 4 effects in one instrument",
            BuildError::BadEdge => "a connection names a missing node or port",
            BuildError::Cycle => "the graph has a cycle",
            BuildError::Empty => "the graph has no nodes",
            BuildError::MemExceeded => "the instrument state exceeds the voice memory",
        }
    }
}

/// A flattened instrument graph in fixed-capacity storage.
#[derive(Clone, Debug)]
pub struct RawGraph {
    pub inst: InstId,
    pub n_nodes: usize,
    pub nodes: [Node; NODE_CAP],
    pub n_edges: usize,
    pub edges: [crate::dsp::graph::Edge; MAX_EDGES],
    pub n_node_params: usize,
    pub node_params: [(u16, CtlId, Ctl); MAX_NODE_PARAMS],
    pub n_params: usize,
    pub params: [(CtlId, Ctl); MAX_PARAMS],
}

impl RawGraph {
    /// An empty graph, boxed (it is large).
    #[must_use]
    pub fn boxed() -> Box<Self> {
        Box::new(Self {
            inst: InstId::new(0),
            n_nodes: 0,
            nodes: [Node::Const(0.0); NODE_CAP],
            n_edges: 0,
            edges: [crate::dsp::graph::Edge {
                from: 0,
                to: 0,
                port: 0,
            }; MAX_EDGES],
            n_node_params: 0,
            node_params: [(0, CtlId::new(0), Ctl::Const(0.0)); MAX_NODE_PARAMS],
            n_params: 0,
            params: [(CtlId::new(0), Ctl::Const(0.0)); MAX_PARAMS],
        })
    }

    /// Empties the graph.
    pub fn clear(&mut self) {
        self.n_nodes = 0;
        self.n_edges = 0;
        self.n_node_params = 0;
        self.n_params = 0;
    }

    /// Appends a node.
    ///
    /// # Errors
    /// `TooManyNodes`.
    pub fn push_node(&mut self, n: Node) -> Result<u16, BuildError> {
        let i = self.n_nodes;
        let slot = self.nodes.get_mut(i).ok_or(BuildError::TooManyNodes)?;
        *slot = n;
        self.n_nodes += 1;
        u16::try_from(i).map_err(|_| BuildError::TooManyNodes)
    }

    /// Appends an edge.
    ///
    /// # Errors
    /// `TooManyEdges`.
    pub fn push_edge(&mut self, e: crate::dsp::graph::Edge) -> Result<(), BuildError> {
        let slot = self
            .edges
            .get_mut(self.n_edges)
            .ok_or(BuildError::TooManyEdges)?;
        *slot = e;
        self.n_edges += 1;
        Ok(())
    }

    /// Appends a node parameter.
    ///
    /// # Errors
    /// `TooManyParams`.
    pub fn push_node_param(&mut self, node: u16, id: CtlId, ctl: Ctl) -> Result<(), BuildError> {
        let slot = self
            .node_params
            .get_mut(self.n_node_params)
            .ok_or(BuildError::TooManyParams)?;
        *slot = (node, id, ctl);
        self.n_node_params += 1;
        Ok(())
    }

    /// Appends a header parameter.
    ///
    /// # Errors
    /// `TooManyParams`.
    pub fn push_param(&mut self, id: CtlId, ctl: Ctl) -> Result<(), BuildError> {
        let slot = self
            .params
            .get_mut(self.n_params)
            .ok_or(BuildError::TooManyParams)?;
        *slot = (id, ctl);
        self.n_params += 1;
        Ok(())
    }

    /// Flattens an `InstDef` (effect parameters become node parameters).
    ///
    /// # Errors
    /// A capacity `BuildError`.
    pub fn load(&mut self, def: &InstDef) -> Result<(), BuildError> {
        self.clear();
        self.inst = def.id;
        if def.nodes.len() > NODE_CAP {
            return Err(BuildError::TooManyNodes);
        }
        for spec in def.nodes.iter() {
            let i = self.push_node(Node::from_spec(spec))?;
            if let UGenSpec::Effect(e) = spec {
                for &(id, ctl) in e.params.iter() {
                    self.push_node_param(i, id, ctl)?;
                }
            }
        }
        for &e in def.edges.iter() {
            self.push_edge(e)?;
        }
        for &(n, id, ctl) in def.node_params.iter() {
            self.push_node_param(n, id, ctl)?;
        }
        for &(id, ctl) in def.params.iter() {
            self.push_param(id, ctl)?;
        }
        Ok(())
    }
}

/// What template compilation depends on.
#[derive(Clone, Copy, Debug)]
pub struct BuildEnv {
    pub sr: f32,
    pub caps: CapabilitySet,
    /// Floats of delay memory per voice.
    pub voice_mem: usize,
}

/// A compiled instrument template (design 12.1).
#[derive(Clone, Debug)]
pub struct Template {
    pub inst: InstId,
    /// The install resource id and generation (16.1).
    pub resource: u32,
    pub gen: u32,
    pub n_nodes: usize,
    pub nodes: [NodeSpec; NODE_CAP],
    pub n_params: usize,
    /// Every control the template reads, with its default.
    pub params: [(CtlId, Ctl); MAX_PARAMS],
    pub n_sinks: usize,
    pub sinks: [u16; MAX_SINKS],
    pub n_fx: usize,
    /// Explicit parameters of each effect node.
    pub fx_params: [[(CtlId, Ctl); MAX_FX_PARAMS]; MAX_VOICE_FX],
    pub fx_n: [u8; MAX_VOICE_FX],
    pub n_refs: usize,
    /// Resources read by the template (samples, tables).
    pub refs: [u32; MAX_REFS],
    pub mem_total: usize,
    pub reads_amp: bool,
    pub reads_pan: bool,
    pub envs: u8,
    pub players: u8,
}

impl Template {
    /// An empty template, boxed (it is large).
    #[must_use]
    pub fn boxed() -> Box<Self> {
        Box::new(Self {
            inst: InstId::new(0),
            resource: 0,
            gen: 0,
            n_nodes: 0,
            nodes: [NodeSpec::EMPTY; NODE_CAP],
            n_params: 0,
            params: [(CtlId::new(0), Ctl::Const(0.0)); MAX_PARAMS],
            n_sinks: 0,
            sinks: [0; MAX_SINKS],
            n_fx: 0,
            fx_params: [[(CtlId::new(0), Ctl::Const(0.0)); MAX_FX_PARAMS]; MAX_VOICE_FX],
            fx_n: [0; MAX_VOICE_FX],
            n_refs: 0,
            refs: [0; MAX_REFS],
            mem_total: 0,
            reads_amp: false,
            reads_pan: false,
            envs: 0,
            players: 0,
        })
    }

    /// Compiles an `InstDef` into a new boxed template (evaluator side).
    ///
    /// # Errors
    /// A `BuildError`.
    pub fn from_inst(def: &InstDef, env: &BuildEnv) -> Result<Box<Self>, BuildError> {
        let mut raw = RawGraph::boxed();
        raw.load(def)?;
        let mut t = Template::boxed();
        t.build(&raw, env)?;
        Ok(t)
    }

    /// The template's nodes in order.
    #[must_use]
    pub fn nodes(&self) -> &[NodeSpec] {
        &self.nodes[..self.n_nodes]
    }

    /// The controls the template reads.
    #[must_use]
    pub fn params(&self) -> &[(CtlId, Ctl)] {
        &self.params[..self.n_params]
    }

    /// The resources the template reads.
    #[must_use]
    pub fn refs(&self) -> &[u32] {
        &self.refs[..self.n_refs]
    }

    /// True when the template reads resource `id`: a sample, table, or a
    /// convolution effect node's impulse response.
    #[must_use]
    pub fn reads_resource(&self, id: u32) -> bool {
        self.refs().contains(&id)
            || self.nodes().iter().any(|n| match n.node {
                Node::Effect { kind, fx } => {
                    let fx = usize::from(fx);
                    self.fx_params[fx][..usize::from(self.fx_n[fx])]
                        .iter()
                        .any(|&(c, v)| {
                            effects::param_index(kind, c)
                                .is_some_and(|i| effects::params(kind)[i].name == "ir")
                                && effects::ir_is(kind, v, id)
                        })
                }
                _ => false,
            })
    }

    /// True when the template reads `cell`: as a control default, a
    /// node-port input or an effect-node parameter (11.3 retire check).
    #[must_use]
    pub fn reads_cell(&self, cell: CellId) -> bool {
        let c = Ctl::Cell(cell);
        self.params().iter().any(|(_, v)| *v == c)
            || self
                .nodes()
                .iter()
                .any(|n| n.inputs.contains(&Src::Cell(cell)))
            || (0..self.n_fx).any(|k| {
                self.fx_params[k][..usize::from(self.fx_n[k])]
                    .iter()
                    .any(|(_, v)| *v == c)
            })
    }

    fn param_slot(&mut self, id: CtlId) -> Result<u8, BuildError> {
        if let Some(i) = self.params().iter().position(|(c, _)| *c == id) {
            return u8::try_from(i).map_err(|_| BuildError::TooManyParams);
        }
        let i = self.n_params;
        let default = controls::row_by_id(id).map_or(0.0, |r| r.default);
        let slot = self.params.get_mut(i).ok_or(BuildError::TooManyParams)?;
        *slot = (id, Ctl::Const(default));
        self.n_params += 1;
        u8::try_from(i).map_err(|_| BuildError::TooManyParams)
    }

    fn add_ref(&mut self, resource: u32) {
        if self.refs().contains(&resource) || self.n_refs >= MAX_REFS {
            return;
        }
        self.refs[self.n_refs] = resource;
        self.n_refs += 1;
    }

    /// Compiles `raw` in place (no allocation).
    ///
    /// # Errors
    /// A `BuildError`; the template is then unusable until rebuilt.
    pub fn build(&mut self, raw: &RawGraph, env: &BuildEnv) -> Result<(), BuildError> {
        let n = raw.n_nodes;
        if n == 0 {
            return Err(BuildError::Empty);
        }
        if n > NODE_CAP {
            return Err(BuildError::TooManyNodes);
        }
        let edges = &raw.edges[..raw.n_edges.min(MAX_EDGES)];
        let order = topo_order(n, edges)?;
        let mut pos = [0u16; NODE_CAP];
        for (new, &old) in order[..n].iter().enumerate() {
            pos[usize::from(old)] = u16::try_from(new).map_err(|_| BuildError::TooManyNodes)?;
        }
        self.inst = raw.inst;
        self.n_nodes = n;
        self.n_params = 0;
        self.n_fx = 0;
        self.n_refs = 0;
        self.n_sinks = 0;
        self.envs = 0;
        self.players = 0;
        for &(id, ctl) in &raw.params[..raw.n_params.min(MAX_PARAMS)] {
            let k = usize::from(self.param_slot(id)?);
            self.params[k] = (id, ctl);
        }
        for (new, &old) in order[..n].iter().enumerate() {
            self.compile_node(new, usize::from(old), raw, &pos)?;
        }
        for (new, &old) in order[..n].iter().enumerate() {
            if !edges.iter().any(|e| e.from == old) && self.n_sinks < MAX_SINKS {
                self.sinks[self.n_sinks] =
                    u16::try_from(new).map_err(|_| BuildError::TooManyNodes)?;
                self.n_sinks += 1;
            }
        }
        self.assign_mem(env)?;
        self.reads_amp = self.params().iter().any(|(c, _)| *c == AMP);
        self.reads_pan = self.params().iter().any(|(c, _)| *c == PAN);
        Ok(())
    }

    fn compile_node(
        &mut self,
        new: usize,
        old: usize,
        raw: &RawGraph,
        pos: &[u16; NODE_CAP],
    ) -> Result<(), BuildError> {
        let mut node = raw.nodes[old];
        let mut inputs = [Src::Default(0.0); MAX_PORTS];
        if let Node::Effect { kind, .. } = node {
            if self.n_fx >= MAX_VOICE_FX {
                return Err(BuildError::TooManyEffects);
            }
            let fx = self.n_fx;
            self.n_fx += 1;
            self.fx_n[fx] = 0;
            node = Node::Effect {
                kind,
                fx: u8::try_from(fx).map_err(|_| BuildError::TooManyEffects)?,
            };
        }
        for (i, input) in inputs
            .iter_mut()
            .enumerate()
            .take(catalog::port_count(&node))
        {
            *input = match catalog::implicit_ctl(&node, i) {
                Some(ctl) => Src::Param(self.param_slot(ctl)?),
                None => Src::Default(catalog::port_default(&node, i)),
            };
        }
        match node {
            Node::Param(ctl) => inputs[0] = Src::Param(self.param_slot(ctl)?),
            Node::EnvPerc | Node::EnvAdsr => self.envs += 1,
            Node::SamplePlay(b) => {
                self.players += 1;
                self.add_ref(b.get());
            }
            Node::Wavetable(t) => self.add_ref(t.get()),
            Node::Granular(GranSrc::Sample(b)) => self.add_ref(b.get()),
            Node::Granular(GranSrc::Table(t)) => self.add_ref(t.get()),
            _ => {}
        }
        let old16 = u16::try_from(old).map_err(|_| BuildError::TooManyNodes)?;
        for &(nd, id, ctl) in &raw.node_params[..raw.n_node_params.min(MAX_NODE_PARAMS)] {
            if nd != old16 {
                continue;
            }
            match node {
                Node::Effect { kind, fx } => {
                    if effects::param_index(kind, id).is_some() {
                        let fx = usize::from(fx);
                        let k = usize::from(self.fx_n[fx]);
                        if k >= MAX_FX_PARAMS {
                            return Err(BuildError::TooManyParams);
                        }
                        self.fx_params[fx][k] = (id, ctl);
                        self.fx_n[fx] += 1;
                    }
                }
                _ => {
                    if let Some(p) = catalog::port_of(&node, id) {
                        inputs[p] = match ctl {
                            Ctl::Const(v) => Src::Const(v),
                            Ctl::Cell(c) => Src::Cell(c),
                        };
                    }
                }
            }
        }
        for e in &raw.edges[..raw.n_edges.min(MAX_EDGES)] {
            if e.to != old16 {
                continue;
            }
            let port = usize::from(e.port);
            if port >= catalog::port_count(&node) {
                return Err(BuildError::BadEdge);
            }
            inputs[port] = Src::Node(pos[usize::from(e.from)]);
        }
        self.nodes[new] = NodeSpec {
            node,
            inputs,
            mem_off: 0,
            mem_len: 0,
        };
        Ok(())
    }

    /// Carves each node's delay memory: fixed needs first (oscillator
    /// phases), then the delay lines, clamped to what is left.
    fn assign_mem(&mut self, env: &BuildEnv) -> Result<(), BuildError> {
        let mut cursor = 0usize;
        for pass in 0..2 {
            for i in 0..self.n_nodes {
                let (fixed, want) = mem_need(&self.nodes[i].node, env);
                let len = if pass == 0 {
                    if fixed == 0 {
                        continue;
                    }
                    if cursor + fixed > env.voice_mem {
                        return Err(BuildError::MemExceeded);
                    }
                    fixed
                } else {
                    if want == 0 {
                        continue;
                    }
                    want.min(env.voice_mem - cursor)
                };
                let spec = &mut self.nodes[i];
                spec.mem_off = u32::try_from(cursor).map_err(|_| BuildError::MemExceeded)?;
                spec.mem_len = u32::try_from(len).map_err(|_| BuildError::MemExceeded)?;
                cursor += len;
            }
        }
        self.mem_total = cursor;
        Ok(())
    }
}

/// `(fixed, clampable)` memory a node wants, in floats.
fn mem_need(node: &Node, env: &BuildEnv) -> (usize, usize) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let secs = |s: f32| (s * env.sr) as usize;
    match node {
        Node::Vco { unison_max } => (usize::from((*unison_max).max(1)) * 2, 0),
        Node::Additive { partials_max } => (usize::from((*partials_max).max(1)), 0),
        Node::Delay => (0, secs(0.5)),
        Node::Comb => (0, secs(0.1)),
        Node::Granular(_) => (0, crate::dsp::granular::ugen_mem_len(env.sr, &env.caps)),
        Node::Effect { kind, .. } => (0, effects::mem_len(*kind, env.sr, &env.caps)),
        _ => (0, 0),
    }
}

/// Kahn's algorithm over fixed arrays.
fn topo_order(n: usize, edges: &[crate::dsp::graph::Edge]) -> Result<[u16; NODE_CAP], BuildError> {
    let mut indeg = [0u16; NODE_CAP];
    for e in edges {
        let (from, to) = (usize::from(e.from), usize::from(e.to));
        if from >= n || to >= n {
            return Err(BuildError::BadEdge);
        }
        indeg[to] += 1;
    }
    let mut order = [0u16; NODE_CAP];
    let mut len = 0;
    for (i, d) in indeg.iter().enumerate().take(n) {
        if *d == 0 {
            order[len] = u16::try_from(i).map_err(|_| BuildError::TooManyNodes)?;
            len += 1;
        }
    }
    let mut head = 0;
    while head < len {
        let v = order[head];
        head += 1;
        for e in edges.iter().filter(|e| e.from == v) {
            let to = usize::from(e.to);
            indeg[to] -= 1;
            if indeg[to] == 0 {
                order[len] = e.to;
                len += 1;
            }
        }
    }
    if len < n {
        return Err(BuildError::Cycle);
    }
    Ok(order)
}

pub use mixer::run;
