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

mod build_helpers;
use build_helpers::{mem_need, topo_order};

pub mod additive;
pub mod analog_pair;
pub mod analog_percussion;
pub mod braids_cloud;
pub mod braids_digital;
pub mod braids_filter;
pub mod braids_five;
pub mod braids_fm;
pub mod braids_formant;
pub mod braids_noise;
pub mod braids_percussion;
pub mod braids_physical;
pub mod braids_struck;
pub mod braids_subsync;
pub mod braids_triple;
pub mod braids_wave_bank;
pub mod braids_wave_line;
pub mod catalog;
pub mod chip_pair;
pub mod chord_pair;
pub mod clock_noise_pair;
pub mod digital_drum;
pub mod dual_kick;
pub mod elements_internal;
pub mod env;
pub mod feedback_metal;
pub mod filter;
pub mod fm;
pub mod fm_drum;
pub mod fm_pair;
pub mod frame_keyframe;
pub mod frame_lfo;
pub mod fusion_drum;
pub mod grain_pair;
pub mod hat_pair;
pub mod mixer;
pub mod modal_pair;
pub mod number_station;
pub mod osc;
pub mod particle_pair;
pub mod peak_function;
pub mod peak_pulse;
pub mod phase_pair;
pub mod rings_part;
pub mod sample;
pub mod shape_pair;
pub mod six_op_original;
pub mod snare_pair;
pub mod spectrum_pair;
pub mod speech_original;
pub mod stage_chain;
pub mod stage_linked;
pub mod stage_segment;
pub mod string_choir;
pub mod string_machine_pair;
pub mod string_pair;
pub mod swarm_pair;
pub mod table_terrain_pair;
pub mod terrain_pair;
pub mod tidal_function;
pub mod tidal_poly;
pub mod va_filter;
pub mod vactrol_gate;
pub mod voice_layer;
pub mod wavetable;

use crate::dsp::arena::SampleStore;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellId;
use crate::dsp::controls;
use crate::dsp::effects::prim::{Biquad, DelayLine};
use crate::dsp::effects::{self, FxStats, MAX_FX_PARAMS};
use crate::dsp::graph::{
    BankRef, EffectKind, GranSrc, InstDef, InstId, NodeAudioShape, TableRef, UGenSpec, DISCARD,
    MAX_OUTPUTS_PER_NODE, NODE_CAP,
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
    HostInputL,
    HostInputR,
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
    FmDrum,
    FeedbackMetal,
    DigitalDrumCore,
    DigitalSnareCore,
    DigitalMetalCore,
    DigitalHatCore,
    AnalogPercussion,
    VaSource,
    VaFilter,
    PhasePair,
    FmPair,
    SixOpOriginal,
    SpeechOriginal,
    RingsPart,
    StringChoir,
    ElementsInternal,
    TidalFunction,
    TidalPoly,
    PeakFunction,
    StageSegment,
    StageChain,
    StageLinked {
        slot: u8,
    },
    FrameLfo,
    FrameKeyframe {
        slot: u8,
    },
    PeakPulse,
    NumberStation,
    SpectrumPair,
    ClockNoisePair,
    DualKick,
    SnarePair,
    HatPair,
    SwarmPair,
    ParticlePair,
    ModalPair,
    StringPair,
    ChipPair,
    AnalogPair,
    GrainPair,
    ShapePair,
    VactrolGate,
    DecayMod,
    StringMachinePair,
    TerrainPair,
    TableTerrainPair,
    ChordPair,
    BraidsFive,
    BraidsSubSync,
    BraidsTriple,
    BraidsDigital,
    BraidsFilter,
    BraidsFormant,
    BraidsFm,
    BraidsPhysical,
    BraidsStruck,
    BraidsPercussion,
    BraidsWaveBank,
    BraidsWaveLine,
    BraidsNoise,
    BraidsCloud,
    AuxOut,
    Out3,
    Out4,
    FeedbackDrum,
    NoiseDrum,
    SineDrum,
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

pub use mixer::run;

impl Node {
    /// The node of a spec (an effect's parameters are not included).
    #[must_use]
    pub fn from_spec(spec: &UGenSpec) -> Node {
        catalog::node_of(spec)
    }

    /// True for the envelope nodes that decide when a voice ends.
    #[must_use]
    pub const fn is_env(&self) -> bool {
        matches!(
            self,
            Node::EnvPerc
                | Node::EnvAdsr
                | Node::FmDrum
                | Node::FeedbackMetal
                | Node::DigitalDrumCore
                | Node::DigitalSnareCore
                | Node::DigitalMetalCore
                | Node::DigitalHatCore
                | Node::AnalogPercussion
                | Node::DualKick
                | Node::SnarePair
                | Node::HatPair
                | Node::SwarmPair
                | Node::ParticlePair
                | Node::ModalPair
                | Node::StringPair
                | Node::ChipPair
                | Node::AnalogPair
                | Node::GrainPair
                | Node::ShapePair
                | Node::StringMachinePair
                | Node::TerrainPair
                | Node::TableTerrainPair
                | Node::ChordPair
                | Node::BraidsFive
                | Node::BraidsSubSync
                | Node::BraidsTriple
                | Node::BraidsDigital
                | Node::BraidsFilter
                | Node::BraidsFormant
                | Node::BraidsFm
                | Node::BraidsPhysical
                | Node::BraidsStruck
                | Node::BraidsPercussion
                | Node::BraidsWaveBank
                | Node::BraidsWaveLine
                | Node::BraidsNoise
                | Node::BraidsCloud
                | Node::FeedbackDrum
                | Node::NoiseDrum
                | Node::SineDrum
        )
    }
}

/// Where one input port reads from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Src {
    /// Nothing feeds the port: its constant default.
    Default(f32),
    /// A selected output slice of an earlier node.
    Node {
        slice: u16,
        stereo: bool,
    },
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
    /// First channel slice for each output, or the shared discard slice.
    pub outs: [u16; MAX_OUTPUTS_PER_NODE],
    /// Declared output shapes, resolved at template build time.
    pub shape: NodeAudioShape,
    /// Delay memory inside the voice region.
    pub mem_off: u32,
    pub mem_len: u32,
}

impl NodeSpec {
    const EMPTY: NodeSpec = NodeSpec {
        node: Node::Const(0.0),
        inputs: [Src::Default(0.0); MAX_PORTS],
        outs: [DISCARD; MAX_OUTPUTS_PER_NODE],
        shape: NodeAudioShape::MONO,
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
    TooManyBuffers,
    BadEdge,
    Cycle,
    Empty,
    BadFrameData,
    BadStageData,
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
            BuildError::TooManyBuffers => {
                "the instrument needs more than 512 audio channel buffers"
            }
            BuildError::BadEdge => "a connection names a missing node or port",
            BuildError::Cycle => "the graph has a cycle",
            BuildError::Empty => "the graph has no nodes",
            BuildError::BadFrameData => "invalid keyframe data",
            BuildError::BadStageData => "invalid Stages segment data",
            BuildError::MemExceeded => "the instrument state exceeds the voice memory",
        }
    }
}

mod template;
pub use template::{BuildEnv, RawGraph, Template};
