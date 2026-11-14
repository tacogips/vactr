//! The ugen catalog: names and input ports (design 12.1, design-music 4, 7).
//!
//! Port `i` of a node is fed by an `Edge { port: i }`, by a node parameter
//! whose control id names the port (`port_ctl`), or else by its default: a
//! port named after a control-table row reads the voice's value of that
//! control (the implicit control names of 12.8.6, B2); any other port reads
//! its constant default. Effect nodes take the audio input on port 0 and
//! the kind's parameters, in catalog order, on ports 1.. .

use crate::dsp::arena::FaultCode;
use crate::dsp::cells::CellId;
use crate::dsp::controls;
use crate::dsp::effects::{self, EFFECT_PARAM_BASE};
use crate::dsp::graph::{BankRef, EffectKind, GranSrc, TableRef, UGenSpec};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

use super::{BuildError, Node, RawGraph};

/// The most input ports a node has: `MAX_PARAMS` (design-music 4.1,
/// DDRUM-002A), so the digital drum family's full control surface fits one
/// core node's port array.
pub const MAX_PORTS: usize = 48;

/// One input port: its name and constant default.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Port {
    pub name: &'static str,
    pub default: f32,
}

const fn p(name: &'static str, default: f32) -> Port {
    Port { name, default }
}

const FREQ: Port = p("freq", 440.0);
const IN: Port = p("in", 0.0);
// The inert call argument lets the generic DSP function syntax construct a
// host-input source node; channel selection is fixed by the node name.
const HOST_INPUT: &[Port] = &[p("source", 0.0)];

const OSC: &[Port] = &[FREQ];
const PULSE: &[Port] = &[FREQ, p("width", 0.5)];
const FILTER: &[Port] = &[IN, p("cutoff", 1200.0), p("res", 0.3)];
const SVF: &[Port] = &[IN, p("cutoff", 1200.0), p("res", 0.3), p("mode", 0.0)];
const DELAY: &[Port] = &[IN, p("time", 0.25), p("feedback", 0.0)];
const COMB: &[Port] = &[
    IN,
    p("time", 0.01),
    p("feedback", 0.5),
    p("mode", 0.0),
    p("damping", 0.0),
];
const DSF: &[Port] = &[
    p("freq", 110.0),
    p("spacing", 1.0),
    p("rolloff", 0.7),
    p("count", 32.0),
];
const CHEBYSHEV: &[Port] = &[
    p("freq", 110.0),
    p("harmonic-1", 1.0),
    p("harmonic-2", 0.0),
    p("harmonic-3", 0.0),
    p("harmonic-4", 0.0),
    p("harmonic-5", 0.0),
    p("harmonic-6", 0.0),
    p("harmonic-7", 0.0),
    p("harmonic-8", 0.0),
];
const GAUSSIAN: &[Port] = &[p("sigma", 0.25), p("mean", 0.0)];
const CHAOS: &[Port] = &[p("rate", 1.0), p("chaos", 0.5), p("output-axis", 0.0)];
const AM_FORMANT: &[Port] = &[
    p("freq", 110.0),
    p("formant-1", 700.0),
    p("formant-2", 1200.0),
    p("bandwidth-1", 100.0),
    p("bandwidth-2", 150.0),
    p("balance", 0.5),
];
const PERC: &[Port] = &[p("attack", 0.01), p("release", 0.3)];
const ADSR: &[Port] = &[
    p("attack", 0.01),
    p("decay", 0.1),
    p("sustain", 1.0),
    p("release", 0.1),
];
const LINE: &[Port] = &[p("from", 1.0), p("to", 0.0), p("dur", 1.0)];
/// `sample-play` reads exactly these controls, plus the event's `bank`
/// (TASK-008 criterion 3: no region machinery beyond begin/end/speed/loop).
pub const SAMPLE: &[Port] = &[
    p("speed", 1.0),
    p("begin", 0.0),
    p("end", 1.0),
    p("loop", 0.0),
    p("region-start-low", 0.0),
    p("region-start-high", -1.0),
    p("region-stop-low", 0.0),
    p("region-stop-high", -1.0),
];
const MUL: &[Port] = &[p("a", 1.0), p("b", 1.0)];
const AUX_OUT: &[Port] = &[IN];
const ADD: &[Port] = &[p("a", 0.0), p("b", 0.0)];
const VCO: &[Port] = &[
    FREQ,
    p("wave", 0.0),
    p("unison", 1.0),
    p("detune", 0.1),
    p("drift", 0.002),
    p("width", 0.5),
];
const FM_OP: &[Port] = &[FREQ, p("ratio", 1.0), p("index", 1.0), p("mod", 0.0)];
const FM_MOD: &[Port] = &[IN, p("mod", 0.0), p("index", 1.0)];
const FM_DRUM: &[Port] = &[
    p("freq", 110.0),
    p("fm-amount", 1.5),
    p("pitch-sweep", 0.5),
    p("decay", 0.4),
    p("drum-noise", 0.0),
    p("drive", 0.0),
];
mod voice_ports;
use voice_ports::*;

const FEEDBACK_DRUM: &[Port] = &[
    p("velocity", 1.0),
    p("fb-delay", 100.0),
    p("fb-feedback", 0.99),
    p("fb-decay", 0.5),
    p("fb-cutoff", 5000.0),
    p("fb-q", 1.0),
    p("fb-velocity", 1.0),
    p("fb-level", -6.0),
];
const NOISE_DRUM: &[Port] = &[
    p("velocity", 1.0),
    p("noise-attack", 0.001),
    p("noise-hold", 0.0),
    p("noise-decay", 0.5),
    p("noise-cutoff", 5000.0),
    p("noise-q", 1.0),
    p("noise-pitch-env", 0.0),
    p("noise-velocity", 1.0),
    p("noise-level", -6.0),
];
const SINE_DRUM: &[Port] = &[
    FREQ,
    p("velocity", 1.0),
    p("fm-tuning", 440.0),
    p("fm-keytrack", 0.0),
    p("fm-ratio", 1.0),
    p("fm-index", 0.0),
    p("fm-attack", 0.002),
    p("fm-hold", 0.0),
    p("fm-decay", 1.5),
    p("fm-pitch-env", 0.0),
    p("fm-velocity", 1.0),
    p("fm-level", -6.0),
];
const PD: &[Port] = &[FREQ, p("shape", 0.5)];
const ADDITIVE: &[Port] = &[FREQ, p("count", 8.0), p("tilt", 1.0)];
const WAVETABLE: &[Port] = &[FREQ, p("position", 0.0)];
/// The granular controls (design-music 6), in port order.
pub const GRANULAR: &[Port] = &[
    p("position", 0.0),
    p("density", 24.0),
    p("size", 0.08),
    p("spray", 0.0),
    p("pitch", 0.0),
    p("pitch-spray", 0.0),
    p("envelope", 0.0),
    p("reverse", 0.0),
    p("freeze", 0.0),
    p("stereo-spray", 0.0),
];
const NONE: &[Port] = &[];
const FRAME_KEYFRAME: &[Port] = &[
    p("frame-position", 0.0),
    p("frame-ease1", 1.0),
    p("frame-ease2", 1.0),
    p("frame-ease3", 1.0),
    p("frame-ease4", 1.0),
    p("frame-response1", 0.0),
    p("frame-response2", 0.0),
    p("frame-response3", 0.0),
    p("frame-response4", 0.0),
    p("frame-channel", 0.0),
];
const STAGE_LINKED: &[Port] = &[
    FREQ,
    p("chain-gate", 1.0),
    p("chain-trigger", 0.0),
    p("chain-channel", 0.0),
];

/// The ports of a node kind (effects: see `effect_port`).
#[must_use]
pub fn ports(node: &Node) -> &'static [Port] {
    match node {
        Node::DsfOsc => DSF,
        Node::ChebyshevOsc => CHEBYSHEV,
        Node::GaussianNoise => GAUSSIAN,
        Node::LorenzOsc | Node::RosslerOsc => CHAOS,
        Node::AmFormantOsc => AM_FORMANT,
        Node::SinOsc | Node::Saw | Node::Tri | Node::SubOsc => OSC,
        Node::Pulse => PULSE,
        Node::WhiteNoise | Node::Const(_) | Node::Param(_) | Node::Effect { .. } => NONE,
        Node::HostInputL | Node::HostInputR => HOST_INPUT,
        Node::Lpf | Node::Hpf | Node::Bpf | Node::Ladder => FILTER,
        Node::Svf => SVF,
        Node::Delay => DELAY,
        Node::Comb => COMB,
        Node::EnvPerc => PERC,
        Node::EnvAdsr => ADSR,
        Node::Line => LINE,
        Node::SamplePlay(_) => SAMPLE,
        Node::Mul => MUL,
        Node::AuxOut | Node::Out3 | Node::Out4 => AUX_OUT,
        Node::Add => ADD,
        Node::Vco { .. } => VCO,
        Node::FmOp => FM_OP,
        Node::FmMod => FM_MOD,
        Node::FmDrum => FM_DRUM,
        Node::FeedbackMetal => FEEDBACK_METAL,
        Node::DigitalDrumCore => DIGITAL_DRUM,
        Node::DigitalSnareCore => DIGITAL_SNARE,
        Node::DigitalMetalCore => DIGITAL_METAL,
        Node::DigitalHatCore => DIGITAL_HAT,
        Node::BassCore => BASS_CORE,
        Node::AnalogPercussion => ANALOG_PERCUSSION,
        Node::VaSource => VA_SOURCE,
        Node::VaFilter => VA_FILTER,
        Node::PhasePair => PHASE_PAIR,
        Node::FmPair => FM_PAIR,
        Node::SixOpOriginal => SIX_OP_ORIGINAL,
        Node::SpeechOriginal => SPEECH_ORIGINAL,
        Node::RingsPart => RINGS_PART,
        Node::StringChoir => STRING_CHOIR,
        Node::ElementsInternal => ELEMENTS_INTERNAL,
        Node::TidalFunction => TIDAL_FUNCTION,
        Node::TidalPoly => TIDAL_POLY,
        Node::PeakFunction => PEAK_FUNCTION,
        Node::StageSegment => STAGE_SEGMENT,
        Node::StageChain => STAGE_CHAIN,
        Node::StageLinked { .. } => STAGE_LINKED,
        Node::FrameLfo => FRAME_LFO,
        Node::FrameKeyframe { .. } => FRAME_KEYFRAME,
        Node::PeakPulse => PEAK_PULSE,
        Node::NumberStation => NUMBER_STATION,
        Node::SpectrumPair => SPECTRUM_PAIR,
        Node::ClockNoisePair => CLOCK_NOISE_PAIR,
        Node::DualKick => DUAL_KICK,
        Node::SnarePair => SNARE_PAIR,
        Node::HatPair => HAT_PAIR,
        Node::SwarmPair => SWARM_PAIR,
        Node::ParticlePair => PARTICLE_PAIR,
        Node::ModalPair => MODAL_PAIR,
        Node::StringPair => STRING_PAIR,
        Node::ChipPair => CHIP_PAIR,
        Node::AnalogPair => ANALOG_PAIR,
        Node::GrainPair => GRAIN_PAIR,
        Node::ShapePair => SHAPE_PAIR,
        Node::VactrolGate => VACTROL_GATE,
        Node::DecayMod => DECAY_MOD,
        Node::BraidsFive => BRAIDS_FIVE,
        Node::BraidsSubSync => BRAIDS_FIVE,
        Node::BraidsTriple => BRAIDS_FIVE,
        Node::BraidsDigital => BRAIDS_FIVE,
        Node::BraidsFilter => BRAIDS_FIVE,
        Node::BraidsFormant => BRAIDS_FIVE,
        Node::BraidsFm => BRAIDS_FIVE,
        Node::BraidsPhysical => BRAIDS_FIVE,
        Node::BraidsStruck => BRAIDS_FIVE,
        Node::BraidsPercussion => BRAIDS_FIVE,
        Node::BraidsWaveBank => BRAIDS_FIVE,
        Node::BraidsWaveLine => BRAIDS_FIVE,
        Node::BraidsNoise => BRAIDS_FIVE,
        Node::BraidsCloud => BRAIDS_FIVE,
        Node::StringMachinePair => STRING_MACHINE_PAIR,
        Node::TerrainPair => TERRAIN_PAIR,
        Node::TableTerrainPair => TABLE_TERRAIN_PAIR,
        Node::ChordPair => CHORD_PAIR,
        Node::FeedbackDrum => FEEDBACK_DRUM,
        Node::NoiseDrum => NOISE_DRUM,
        Node::SineDrum => SINE_DRUM,
        Node::PhaseDistortion => PD,
        Node::Additive { .. } => ADDITIVE,
        Node::Wavetable(_) => WAVETABLE,
        Node::Granular(_) => GRANULAR,
    }
}

/// The number of ports of a node, effects included.
#[must_use]
pub fn port_count(node: &Node) -> usize {
    match node {
        Node::Effect { kind, .. } => (1 + effects::params(*kind).len()).min(MAX_PORTS),
        _ => ports(node).len(),
    }
}

/// The constant default of port `i` (effects: 0 for the input, the
/// parameter default otherwise).
#[must_use]
pub fn port_default(node: &Node, i: usize) -> f32 {
    match node {
        Node::Effect { kind, .. } => match i {
            0 => 0.0,
            _ => effects::params(*kind).get(i - 1).map_or(0.0, |d| d.default),
        },
        _ => ports(node).get(i).map_or(0.0, |p| p.default),
    }
}

/// The control id that addresses port `i` as a node parameter: the
/// control-table row of the port's name, else `EFFECT_PARAM_BASE + i`.
/// (Effect parameters use their effect-local ids, `effects::param_ctl`.)
#[must_use]
pub fn port_ctl(node: &Node, i: usize) -> Option<CtlId> {
    let port = ports(node).get(i)?;
    Some(match controls::row(port.name) {
        Some(row) => row.ctl,
        None => CtlId::new(EFFECT_PARAM_BASE + u16::try_from(i).ok()?),
    })
}

/// The port a node parameter id addresses.
#[must_use]
pub fn port_of(node: &Node, ctl: CtlId) -> Option<usize> {
    (0..ports(node).len()).find(|&i| port_ctl(node, i) == Some(ctl))
}

/// The voice control a port reads when nothing feeds it (a port named
/// after an instrument-parameter row of the control table), else `None`.
#[must_use]
pub fn implicit_ctl(node: &Node, i: usize) -> Option<CtlId> {
    let port = ports(node).get(i)?;
    controls::row(port.name)
        .filter(|r| r.route == controls::CtlRoute::InstParam)
        .map(|r| r.ctl)
}

/// The catalog name of a ugen (design-music section 7, kebab-case).
#[must_use]
pub fn ugen_name(spec: &UGenSpec) -> &'static str {
    match spec {
        UGenSpec::DsfOsc => "dsf-osc",
        UGenSpec::ChebyshevOsc => "chebyshev-osc",
        UGenSpec::GaussianNoise => "gaussian-noise",
        UGenSpec::LorenzOsc => "lorenz-osc",
        UGenSpec::RosslerOsc => "rossler-osc",
        UGenSpec::AmFormantOsc => "am-formant-osc",
        UGenSpec::SinOsc => "sin-osc",
        UGenSpec::Saw => "saw",
        UGenSpec::Pulse => "pulse",
        UGenSpec::Tri => "tri",
        UGenSpec::WhiteNoise => "white-noise",
        UGenSpec::HostInputL => "host-in-l",
        UGenSpec::HostInputR => "host-in-r",
        UGenSpec::Lpf => "lpf",
        UGenSpec::Hpf => "hpf",
        UGenSpec::Bpf => "bpf",
        UGenSpec::Delay => "delay",
        UGenSpec::Comb => "comb",
        UGenSpec::EnvPerc => "env-perc",
        UGenSpec::EnvAdsr => "env-adsr",
        UGenSpec::Line => "line",
        UGenSpec::SamplePlay(_) => "sample-play",
        UGenSpec::Mul => "*",
        UGenSpec::Add => "+",
        UGenSpec::Const(_) => "const",
        UGenSpec::Param(_) => "param",
        UGenSpec::Vco { .. } => "vco",
        UGenSpec::SubOsc => "sub-osc",
        UGenSpec::Ladder => "ladder",
        UGenSpec::Svf => "svf",
        UGenSpec::FmOp => "fm-op",
        UGenSpec::FmMod => "fm-mod",
        UGenSpec::FmDrum => "fm-drum",
        UGenSpec::FeedbackMetal => "feedback-metal-core",
        UGenSpec::DigitalDrumCore => "digital-drum-core",
        UGenSpec::DigitalSnareCore => "digital-snare-core",
        UGenSpec::DigitalMetalCore => "digital-metal-core",
        UGenSpec::DigitalHatCore => "digital-hat-core",
        UGenSpec::BassCore => "bass-core",
        UGenSpec::AnalogPercussion => "analog-percussion",
        UGenSpec::VaSource => "va-source",
        UGenSpec::VaFilter => "va-filter",
        UGenSpec::PhasePair => "phase-pair",
        UGenSpec::FmPair => "fm-pair",
        UGenSpec::SixOpOriginal => "six-op-original",
        UGenSpec::SpeechOriginal => "speech-original",
        UGenSpec::RingsPart => "resonator-part-core",
        UGenSpec::StringChoir => "string-choir-core",
        UGenSpec::ElementsInternal => "exciter-core",
        UGenSpec::TidalFunction => "tidal-function-core",
        UGenSpec::TidalPoly => "tidal-poly-core",
        UGenSpec::PeakFunction => "peak-function-core",
        UGenSpec::StageSegment => "stage-segment-core",
        UGenSpec::StageChain => "stage-chain-core",
        UGenSpec::StageLinked { .. } => "stage-linked-core",
        UGenSpec::FrameLfo => "frame-lfo-core",
        UGenSpec::FrameKeyframe { .. } => "frame-keyframe-core",
        UGenSpec::PeakPulse => "peak-pulse-core",
        UGenSpec::NumberStation => "number-station-core",
        UGenSpec::SpectrumPair => "spectrum-pair",
        UGenSpec::ClockNoisePair => "clock-noise-pair",
        UGenSpec::DualKick => "dual-kick-core",
        UGenSpec::SnarePair => "snare-pair-core",
        UGenSpec::HatPair => "hat-pair-core",
        UGenSpec::SwarmPair => "swarm-pair-core",
        UGenSpec::ParticlePair => "particle-pair-core",
        UGenSpec::ModalPair => "modal-pair-core",
        UGenSpec::StringPair => "string-pair-core",
        UGenSpec::ChipPair => "chip-pair-core",
        UGenSpec::AnalogPair => "analog-pair-core",
        UGenSpec::GrainPair => "grain-pair-core",
        UGenSpec::ShapePair => "shape-pair-core",
        UGenSpec::VactrolGate => "vactrol-gate",
        UGenSpec::DecayMod => "decay-mod",
        UGenSpec::StringMachinePair => "string-machine-core",
        UGenSpec::TerrainPair => "terrain-pair-core",
        UGenSpec::TableTerrainPair => "wave-grid-core",
        UGenSpec::ChordPair => "chord-layer-core",
        UGenSpec::BraidsFive => "macro-five-core",
        UGenSpec::BraidsSubSync => "macro-sub-sync-core",
        UGenSpec::BraidsTriple => "macro-triple-core",
        UGenSpec::BraidsDigital => "macro-digital-core",
        UGenSpec::BraidsFilter => "macro-filter-core",
        UGenSpec::BraidsFormant => "macro-formant-core",
        UGenSpec::BraidsFm => "macro-fm-core",
        UGenSpec::BraidsPhysical => "macro-physical-core",
        UGenSpec::BraidsStruck => "macro-struck-core",
        UGenSpec::BraidsPercussion => "macro-percussion-core",
        UGenSpec::BraidsWaveBank => "macro-wave-grid-core",
        UGenSpec::BraidsWaveLine => "macro-wave-line-core",
        UGenSpec::BraidsNoise => "macro-noise-core",
        UGenSpec::BraidsCloud => "macro-cloud-core",
        UGenSpec::AuxOut => "aux-out",
        UGenSpec::Out3 => "out-3",
        UGenSpec::Out4 => "out-4",
        UGenSpec::FeedbackDrum => "feedback-drum",
        UGenSpec::NoiseDrum => "noise-drum",
        UGenSpec::SineDrum => "sine-drum",
        UGenSpec::PhaseDistortion => "phase-distortion",
        UGenSpec::Additive { .. } => "additive",
        UGenSpec::Wavetable(_) => "wavetable",
        UGenSpec::Granular(_) => "granular",
        UGenSpec::Effect(e) => e.kind.name(),
    }
}

/// The user-facing ugen names (design-music section 7 sound row), each once.
pub const UGEN_NAMES: &[&str] = &[
    "dsf-osc",
    "chebyshev-osc",
    "gaussian-noise",
    "lorenz-osc",
    "rossler-osc",
    "am-formant-osc",
    "sin-osc",
    "saw",
    "pulse",
    "tri",
    "white-noise",
    "host-in-l",
    "host-in-r",
    "lpf",
    "hpf",
    "bpf",
    "delay",
    "comb",
    "env-perc",
    "env-adsr",
    "line",
    "sample-play",
    "vco",
    "sub-osc",
    "ladder",
    "svf",
    "fm-op",
    "fm-mod",
    "fm-drum",
    "feedback-metal-core",
    "digital-drum-core",
    "digital-snare-core",
    "digital-metal-core",
    "digital-hat-core",
    "analog-percussion",
    "va-source",
    "va-filter",
    "phase-pair",
    "fm-pair",
    "six-op-original",
    "speech-original",
    "resonator-part-core",
    "string-choir-core",
    "exciter-core",
    "tidal-function-core",
    "tidal-poly-core",
    "peak-function-core",
    "stage-segment-core",
    "stage-chain-core",
    "stage-linked-core",
    "frame-lfo-core",
    "frame-keyframe-core",
    "peak-pulse-core",
    "number-station-core",
    "spectrum-pair",
    "clock-noise-pair",
    "dual-kick-core",
    "snare-pair-core",
    "hat-pair-core",
    "swarm-pair-core",
    "particle-pair-core",
    "modal-pair-core",
    "string-pair-core",
    "chip-pair-core",
    "analog-pair-core",
    "grain-pair-core",
    "shape-pair-core",
    "vactrol-gate",
    "decay-mod",
    "string-machine-core",
    "terrain-pair-core",
    "wave-grid-core",
    "chord-layer-core",
    "macro-five-core",
    "macro-sub-sync-core",
    "macro-triple-core",
    "macro-digital-core",
    "macro-filter-core",
    "macro-formant-core",
    "macro-fm-core",
    "macro-physical-core",
    "macro-struck-core",
    "macro-percussion-core",
    "macro-wave-grid-core",
    "macro-wave-line-core",
    "macro-noise-core",
    "macro-cloud-core",
    "aux-out",
    "out-3",
    "out-4",
    "feedback-drum",
    "noise-drum",
    "sine-drum",
    "phase-distortion",
    "additive",
    "wavetable",
    "granular",
    "bass-core",
];

/// The prelude synthesis templates (design 12.4).
pub const TEMPLATE_NAMES: &[&str] = &[
    "sampler",
    "analog",
    "fm",
    "phase-drum",
    "feedback-metal-drum",
    "digital-drum",
    "digital-snare",
    "digital-metal",
    "digital-hat",
    "low-drum",
    "wire-drum",
    "metal-hat",
    "filter-voice",
    "phase-pair-voice",
    "fm-pair-voice",
    "six-bank-a-voice",
    "six-bank-b-voice",
    "six-bank-c-voice",
    "speech-voice",
    "resonator-voice",
    "string-choir-voice",
    "exciter-voice",
    "spectrum-voice",
    "clock-noise-voice",
    "dual-kick-voice",
    "dual-snare-voice",
    "dual-hat-voice",
    "swarm-voice",
    "particle-voice",
    "modal-voice",
    "string-voice",
    "chip-voice",
    "analog-pair-voice",
    "grain-pair-voice",
    "shape-voice",
    "string-machine-voice",
    "terrain-voice",
    "wave-grid-voice",
    "chord-layer-voice",
    "macro-five-voice",
    "macro-sub-sync-voice",
    "macro-triple-voice",
    "macro-digital-voice",
    "macro-filter-voice",
    "macro-formant-voice",
    "macro-fm-voice",
    "macro-physical-voice",
    "macro-struck-voice",
    "macro-percussion-voice",
    "macro-wave-grid-voice",
    "macro-wave-line-voice",
    "macro-noise-voice",
    "macro-cloud-voice",
    "fusion-drum",
    "pd",
    "additive",
    "wavetable",
    "granular",
    "tidal-voice",
    "tidal-poly-voice",
    "peak-motion-voice",
    "stage-voice",
    "stage-chain-voice",
    "frame-lfo-voice",
    "frame-keyframe-voice",
    "peak-pulse-voice",
    "number-station-voice",
    "analog-bass",
    "acid-bass",
    "fm-bass",
    "wobble-bass",
    "sub-bass",
    "reese-bass",
];

/// The node of a spec (an effect's parameters are not included).
#[must_use]
pub fn node_of(spec: &UGenSpec) -> Node {
    match spec {
        UGenSpec::DsfOsc => Node::DsfOsc,
        UGenSpec::ChebyshevOsc => Node::ChebyshevOsc,
        UGenSpec::GaussianNoise => Node::GaussianNoise,
        UGenSpec::LorenzOsc => Node::LorenzOsc,
        UGenSpec::RosslerOsc => Node::RosslerOsc,
        UGenSpec::AmFormantOsc => Node::AmFormantOsc,
        UGenSpec::SinOsc => Node::SinOsc,
        UGenSpec::Saw => Node::Saw,
        UGenSpec::Pulse => Node::Pulse,
        UGenSpec::Tri => Node::Tri,
        UGenSpec::WhiteNoise => Node::WhiteNoise,
        UGenSpec::HostInputL => Node::HostInputL,
        UGenSpec::HostInputR => Node::HostInputR,
        UGenSpec::Lpf => Node::Lpf,
        UGenSpec::Hpf => Node::Hpf,
        UGenSpec::Bpf => Node::Bpf,
        UGenSpec::Delay => Node::Delay,
        UGenSpec::Comb => Node::Comb,
        UGenSpec::EnvPerc => Node::EnvPerc,
        UGenSpec::EnvAdsr => Node::EnvAdsr,
        UGenSpec::Line => Node::Line,
        UGenSpec::SamplePlay(b) => Node::SamplePlay(*b),
        UGenSpec::Mul => Node::Mul,
        UGenSpec::Add => Node::Add,
        UGenSpec::Const(v) => Node::Const(*v),
        UGenSpec::Param(c) => Node::Param(*c),
        UGenSpec::Vco { unison_max } => Node::Vco {
            unison_max: *unison_max,
        },
        UGenSpec::SubOsc => Node::SubOsc,
        UGenSpec::Ladder => Node::Ladder,
        UGenSpec::Svf => Node::Svf,
        UGenSpec::FmOp => Node::FmOp,
        UGenSpec::FmMod => Node::FmMod,
        UGenSpec::FmDrum => Node::FmDrum,
        UGenSpec::FeedbackMetal => Node::FeedbackMetal,
        UGenSpec::DigitalDrumCore => Node::DigitalDrumCore,
        UGenSpec::DigitalSnareCore => Node::DigitalSnareCore,
        UGenSpec::DigitalMetalCore => Node::DigitalMetalCore,
        UGenSpec::DigitalHatCore => Node::DigitalHatCore,
        UGenSpec::BassCore => Node::BassCore,
        UGenSpec::AnalogPercussion => Node::AnalogPercussion,
        UGenSpec::VaSource => Node::VaSource,
        UGenSpec::VaFilter => Node::VaFilter,
        UGenSpec::PhasePair => Node::PhasePair,
        UGenSpec::FmPair => Node::FmPair,
        UGenSpec::SixOpOriginal => Node::SixOpOriginal,
        UGenSpec::SpeechOriginal => Node::SpeechOriginal,
        UGenSpec::RingsPart => Node::RingsPart,
        UGenSpec::StringChoir => Node::StringChoir,
        UGenSpec::ElementsInternal => Node::ElementsInternal,
        UGenSpec::TidalFunction => Node::TidalFunction,
        UGenSpec::TidalPoly => Node::TidalPoly,
        UGenSpec::PeakFunction => Node::PeakFunction,
        UGenSpec::StageSegment => Node::StageSegment,
        UGenSpec::StageChain => Node::StageChain,
        UGenSpec::StageLinked { .. } => Node::StageLinked { slot: 0 },
        UGenSpec::FrameLfo => Node::FrameLfo,
        UGenSpec::FrameKeyframe { .. } => Node::FrameKeyframe { slot: 0 },
        UGenSpec::PeakPulse => Node::PeakPulse,
        UGenSpec::NumberStation => Node::NumberStation,
        UGenSpec::SpectrumPair => Node::SpectrumPair,
        UGenSpec::ClockNoisePair => Node::ClockNoisePair,
        UGenSpec::DualKick => Node::DualKick,
        UGenSpec::SnarePair => Node::SnarePair,
        UGenSpec::HatPair => Node::HatPair,
        UGenSpec::SwarmPair => Node::SwarmPair,
        UGenSpec::ParticlePair => Node::ParticlePair,
        UGenSpec::ModalPair => Node::ModalPair,
        UGenSpec::StringPair => Node::StringPair,
        UGenSpec::ChipPair => Node::ChipPair,
        UGenSpec::AnalogPair => Node::AnalogPair,
        UGenSpec::GrainPair => Node::GrainPair,
        UGenSpec::ShapePair => Node::ShapePair,
        UGenSpec::VactrolGate => Node::VactrolGate,
        UGenSpec::DecayMod => Node::DecayMod,
        UGenSpec::StringMachinePair => Node::StringMachinePair,
        UGenSpec::TerrainPair => Node::TerrainPair,
        UGenSpec::TableTerrainPair => Node::TableTerrainPair,
        UGenSpec::ChordPair => Node::ChordPair,
        UGenSpec::BraidsFive => Node::BraidsFive,
        UGenSpec::BraidsSubSync => Node::BraidsSubSync,
        UGenSpec::BraidsTriple => Node::BraidsTriple,
        UGenSpec::BraidsDigital => Node::BraidsDigital,
        UGenSpec::BraidsFilter => Node::BraidsFilter,
        UGenSpec::BraidsFormant => Node::BraidsFormant,
        UGenSpec::BraidsFm => Node::BraidsFm,
        UGenSpec::BraidsPhysical => Node::BraidsPhysical,
        UGenSpec::BraidsStruck => Node::BraidsStruck,
        UGenSpec::BraidsPercussion => Node::BraidsPercussion,
        UGenSpec::BraidsWaveBank => Node::BraidsWaveBank,
        UGenSpec::BraidsWaveLine => Node::BraidsWaveLine,
        UGenSpec::BraidsNoise => Node::BraidsNoise,
        UGenSpec::BraidsCloud => Node::BraidsCloud,
        UGenSpec::AuxOut => Node::AuxOut,
        UGenSpec::Out3 => Node::Out3,
        UGenSpec::Out4 => Node::Out4,
        UGenSpec::FeedbackDrum => Node::FeedbackDrum,
        UGenSpec::NoiseDrum => Node::NoiseDrum,
        UGenSpec::SineDrum => Node::SineDrum,
        UGenSpec::PhaseDistortion => Node::PhaseDistortion,
        UGenSpec::Additive { partials_max } => Node::Additive {
            partials_max: *partials_max,
        },
        UGenSpec::Wavetable(t) => Node::Wavetable(*t),
        UGenSpec::Granular(g) => Node::Granular(*g),
        UGenSpec::Effect(e) => Node::Effect {
            kind: e.kind,
            fx: 0,
        },
    }
}

mod codec;
pub(crate) use codec::{effect_index, get_node, put_spec, In, Out};
