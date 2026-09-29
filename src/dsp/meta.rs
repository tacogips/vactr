//! Editor metadata for every builtin (design 13.5, 12.8.8).
//!
//! One `EditorDecl` per catalog name: every effect, every ugen and the
//! seven synthesis templates. A name that is both a ugen and an effect
//! (`lpf`, `delay`, ...) or both a ugen and a template (`additive`,
//! `wavetable`, `granular`) has one entry. Ranges, curves and units come
//! from the effect parameter lists, the ugen ports and the control table;
//! the table is built once, on first use, on the evaluator side (the audio
//! thread never reads it).

use std::sync::OnceLock;

use crate::dsp::controls;
use crate::dsp::effects::{self, ParamDef};
use crate::dsp::graph::{AnalyzerKind, EffectKind};
use crate::dsp::ugen::catalog::{self as ucat, Port, TEMPLATE_NAMES, UGEN_NAMES};
use crate::dsp::ugen::Node;
use crate::sched::slots::CtlId;

/// The meaning-matched editor of a builtin (13.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorKind {
    EqCurve,
    FilterResponse,
    DynamicsTransfer { multiband: bool },
    EnvelopeShape,
    DelayTaps,
    ReverbRoom,
    SamplerWave,
    WavetableFrames,
    GranularRegion,
    LfoShape,
    StereoField,
    XyPad,
    EuclidRing,
    ProbabilityDial,
    LengthHandle,
    Scalar,
}

/// How a slider maps its travel onto the range.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Curve {
    Linear,
    /// Logarithmic (frequencies, long times).
    Log,
    /// Stepped (keyword and count parameters).
    Stepped,
}

/// A display unit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    None,
    Db,
    Seconds,
    Millis,
    Hz,
    Semitones,
}

/// One parameter's editor metadata.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ParamMeta {
    pub name: &'static str,
    pub ctl: CtlId,
    pub range: (f32, f32),
    pub curve: Curve,
    pub unit: Unit,
    /// Parameters drawn together (bands of a multiband unit share one).
    pub group: u8,
}

/// The editor of one builtin.
#[derive(Clone, PartialEq, Debug)]
pub struct EditorDecl {
    pub name: &'static str,
    pub kind: EditorKind,
    pub params: Box<[ParamMeta]>,
}

/// The template header parameters (design-music sections 4 and 6), each a
/// control-table row.
mod templates;
use templates::TEMPLATE_PARAMS;

fn unit_of(name: &str, unit: &str) -> Unit {
    match unit {
        "dB" => Unit::Db,
        "s" => Unit::Seconds,
        "ms" => Unit::Millis,
        "Hz" => Unit::Hz,
        _ => match name {
            "freq" | "cutoff" | "lpf" | "hpf" | "density" | "fb-cutoff" | "noise-cutoff"
            | "fm-tuning" | "transient-freq" | "noise-freq" | "lfo-rate" => Unit::Hz,
            "attack" | "decay" | "release" | "size" | "time" | "dur" | "delaytime" | "fb-decay"
            | "noise-attack" | "noise-hold" | "noise-decay" | "fm-attack" | "fm-hold"
            | "fm-decay" | "amp-attack" | "amp-decay" | "pitch-decay" | "mod-decay"
            | "repeat-time" | "open-decay" | "closed-decay" => Unit::Seconds,
            "fb-level" | "noise-level" | "fm-level" => Unit::Db,
            "pitch" | "semitones" | "coarse" | "pitch-depth" => Unit::Semitones,
            _ => Unit::None,
        },
    }
}

fn curve_of(range: (f32, f32), stepped: bool) -> Curve {
    if stepped {
        Curve::Stepped
    } else if range.0 > 0.0 && range.1 / range.0 >= 100.0 {
        Curve::Log
    } else {
        Curve::Linear
    }
}

fn group_of(name: &str) -> u8 {
    match name.split('-').next() {
        Some("low") => 1,
        Some("mid" | "mid1") => 2,
        Some("mid2") => 3,
        Some("high") => 4,
        _ => 0,
    }
}

fn meta(name: &'static str, ctl: CtlId, range: (f32, f32), unit: &str, stepped: bool) -> ParamMeta {
    ParamMeta {
        name,
        ctl,
        range,
        curve: curve_of(range, stepped),
        unit: unit_of(name, unit),
        group: group_of(name),
    }
}

fn row_meta(name: &'static str) -> Option<ParamMeta> {
    let row = controls::row(name)?;
    let stepped = !matches!(row.domain, controls::CtlDomain::Float);
    Some(meta(name, row.ctl, row.range, "", stepped))
}

fn template_meta(template: &str, name: &'static str) -> Option<ParamMeta> {
    row_meta(name).or_else(|| {
        let node = match template {
            "feedback-metal-drum" => Node::FeedbackMetal,
            "six-bank-a-voice" | "six-bank-b-voice" | "six-bank-c-voice" => Node::SixOpOriginal,
            "speech-voice" => Node::SpeechOriginal,
            "resonator-voice" => Node::RingsPart,
            "string-choir-voice" => Node::StringChoir,
            "exciter-voice" => Node::ElementsInternal,
            "tidal-voice" => Node::TidalFunction,
            "tidal-poly-voice" => Node::TidalPoly,
            "peak-motion-voice" => Node::PeakFunction,
            "stage-voice" => Node::StageSegment,
            "stage-chain-voice" => Node::StageChain,
            "frame-lfo-voice" => Node::FrameLfo,
            "frame-keyframe-voice" => Node::FrameKeyframe { slot: 0 },
            "peak-pulse-voice" => Node::PeakPulse,
            "number-station-voice" => Node::NumberStation,
            _ => return None,
        };
        ucat::ports(&node)
            .iter()
            .position(|port| {
                port.name == name
                    || (template == "frame-lfo-voice"
                        && matches!(name, "frame-main-channel" | "frame-aux-channel")
                        && port.name == "frame-channel")
                    || (template == "frame-keyframe-voice"
                        && matches!(name, "frame-main-channel" | "frame-aux-channel")
                        && port.name == "frame-channel")
                    || (template == "tidal-poly-voice"
                        && matches!(name, "poly-main-channel" | "poly-aux-channel")
                        && port.name == "poly-channel")
            })
            .and_then(|port| ucat::port_ctl(&node, port))
            .map(|ctl| {
                if template != "resonator-voice"
                    && template != "feedback-metal-drum"
                    && template != "string-choir-voice"
                    && template != "exciter-voice"
                    && template != "tidal-voice"
                    && template != "tidal-poly-voice"
                    && template != "peak-motion-voice"
                    && template != "stage-voice"
                    && template != "stage-chain-voice"
                    && template != "frame-lfo-voice"
                    && template != "frame-keyframe-voice"
                    && template != "peak-pulse-voice"
                    && template != "number-station-voice"
                {
                    return meta(name, ctl, (0.0, 1.0), "", false);
                }
                let (range, stepped) = match name {
                    "metal-ratio" => ((0.25, 12.0), false),
                    "metal-index" => ((0.0, 12.0), false),
                    "metal-feedback" => ((0.0, 0.95), false),
                    "metal-mod-decay" => ((0.002, 4.0), false),
                    "metal-body-decay" => ((0.01, 8.0), false),
                    "metal-pitch-drop" => ((0.0, 4.0), false),
                    "metal-noise-level" => ((0.0, 1.0), false),
                    "metal-noise-decay" => ((0.002, 3.0), false),
                    "metal-noise-color" => ((100.0, 20_000.0), false),
                    "metal-noise-to-fm" => ((0.0, 3.0), false),
                    "metal-cutoff" => ((40.0, 20_000.0), false),
                    "metal-resonance" => ((0.3, 12.0), false),
                    "metal-drive" => ((0.0, 1.0), false),
                    "reso-model" | "choir-fx" => ((0.0, 5.0), true),
                    "ex-model" => ((0.0, 2.0), true),
                    "ex-alternate" => ((0.0, 1.0), true),
                    "ex-space" => ((0.0, 2.0), false),
                    "tide-mode" | "tide-range" => ((0.0, 2.0), true),
                    "tide-output" => ((0.0, 3.0), true),
                    "tide-ratio" => ((0.125, 8.0), false),
                    "tide-pitch" => ((-48.0, 48.0), false),
                    "tide-sync" | "tide-gate" | "tide-clock" | "tide-freeze" => ((0.0, 1.0), true),
                    "poly-mode" => ((0.0, 2.0), true),
                    "poly-output-mode" | "poly-main-channel" | "poly-aux-channel" => {
                        ((0.0, 3.0), true)
                    }
                    "poly-range" | "poly-gate" | "poly-clock" => ((0.0, 1.0), true),
                    "peak-mode" => ((0.0, 2.0), true),
                    "stage-type" => ((0.0, 3.0), true),
                    "stage-function" => ((0.0, 9.0), true),
                    "chain-count" => ((1.0, 6.0), true),
                    "frame-main-channel" | "frame-aux-channel" => ((0.0, 3.0), true),
                    "frame-ease1" | "frame-ease2" | "frame-ease3" | "frame-ease4" => {
                        ((0.0, 5.0), true)
                    }
                    "chain1-type" | "chain2-type" | "chain3-type" | "chain4-type"
                    | "chain5-type" | "chain6-type" => ((0.0, 3.0), true),
                    "chain-gate" | "chain-trigger" | "chain-channel" | "chain1-loop"
                    | "chain2-loop" | "chain3-loop" | "chain4-loop" | "chain5-loop"
                    | "chain6-loop" => ((0.0, 1.0), true),
                    "stage-loop" | "stage-gate" | "stage-trigger" | "stage-clock"
                    | "stage-channel" => ((0.0, 1.0), true),
                    "pulse-mode" => ((0.0, 2.0), true),
                    "pulse-half" | "pulse-gate" | "pulse-trigger" => ((0.0, 1.0), true),
                    "pulse-repeats" => ((0.0, 7.0), true),
                    "pulse-velocity" => ((-1.0, 1.0), false),
                    "station-digit" => ((0.0, 9.0), true),
                    "station-voice" | "station-half" | "station-auto" | "station-gate"
                    | "station-trigger" => ((0.0, 1.0), true),
                    "peak-shape" => ((0.0, 4.0), true),
                    "peak-preset" => ((0.0, 6.0), true),
                    "peak-color" => ((-1.0, 1.0), false),
                    "peak-half" | "peak-gate" | "peak-trigger" | "peak-tap" | "peak-sync" => {
                        ((0.0, 1.0), true)
                    }
                    "ex-note" => ((-48.0, 48.0), false),
                    "ex-modulation" => ((-1.0, 1.0), false),
                    "reso-chord" | "choir-chord" => ((0.0, 10.0), true),
                    "reso-polyphony" | "choir-polyphony" => ((1.0, 4.0), true),
                    "reso-tonic" | "reso-note" | "reso-fm" | "choir-tonic" | "choir-note"
                    | "choir-fm" => ((-48.0, 48.0), false),
                    "reso-internal-exciter"
                    | "reso-internal-strum"
                    | "reso-internal-note"
                    | "choir-internal-exciter"
                    | "choir-internal-strum"
                    | "choir-internal-note" => ((0.0, 1.0), true),
                    _ => ((0.0, 1.0), false),
                };
                let mut result = meta(name, ctl, range, "", stepped);
                if matches!(
                    name,
                    "reso-tonic"
                        | "reso-note"
                        | "reso-fm"
                        | "choir-tonic"
                        | "choir-note"
                        | "choir-fm"
                        | "ex-note"
                ) {
                    result.unit = Unit::Semitones;
                }
                result
            })
    })
}

fn effect_meta(kind: EffectKind) -> Box<[ParamMeta]> {
    effects::params(kind)
        .iter()
        .filter_map(|d: &ParamDef| {
            let ctl = effects::param_ctl(kind, d.name)?;
            let stepped = matches!(d.name, "kind" | "mode" | "count" | "stages" | "bits" | "id")
                || (d.name == "quality"
                    && matches!(
                        kind,
                        EffectKind::TextureGrain
                            | EffectKind::TextureStretch
                            | EffectKind::TextureLoop
                            | EffectKind::TextureSpectral
                    ));
            Some(meta(d.name, ctl, (d.min, d.max), d.unit, stepped))
        })
        .collect()
}

fn effect_editor(kind: EffectKind) -> EditorKind {
    use EffectKind as K;
    match kind {
        K::Compressor
        | K::Expander
        | K::Gate
        | K::Limiter
        | K::Transient
        | K::AutoLevel
        | K::Sag => EditorKind::DynamicsTransfer { multiband: false },
        K::MultibandCompressor | K::MultibandExpander | K::MultibandTransient => {
            EditorKind::DynamicsTransfer { multiband: true }
        }
        K::Peq
        | K::Geq
        | K::DynamicEq
        | K::Tilt
        | K::Tone
        | K::LoudnessEq
        | K::LinearPhaseEq
        | K::GroupDelayEq
        | K::Crossover
        | K::FirCrossover
        | K::PhaseSelectEq => EditorKind::EqCurve,
        K::Lpf | K::Hpf | K::Bpf | K::Notch | K::Comb | K::Narrow | K::AutoFilter => {
            EditorKind::FilterResponse
        }
        K::Delay | K::PingPong | K::Multitap | K::TimeAlign => EditorKind::DelayTaps,
        K::Plate | K::Fdn | K::Convolution | K::Scatter | K::Room => EditorKind::ReverbRoom,
        K::Chorus
        | K::Flanger
        | K::Phaser
        | K::Tremolo
        | K::AutoPan
        | K::Rotary
        | K::WowFlutter
        | K::Vibrato => EditorKind::LfoShape,
        K::Width
        | K::Balance
        | K::MultibandBalance
        | K::Ms
        | K::Crossfeed
        | K::CrosstalkCancel
        | K::SpatialMap
        | K::Matrix => EditorKind::StereoField,
        K::Pan | K::Doppler => EditorKind::XyPad,
        K::Granulate
        | K::TextureGrain
        | K::TextureStretch
        | K::TextureLoop
        | K::TextureSpectral => EditorKind::GranularRegion,
        K::Analyzer(AnalyzerKind::Spectrum | AnalyzerKind::Spectrogram) => EditorKind::EqCurve,
        _ => EditorKind::Scalar,
    }
}

fn ugen_node(name: &str) -> Option<Node> {
    Some(match name {
        "sin-osc" => Node::SinOsc,
        "saw" => Node::Saw,
        "pulse" => Node::Pulse,
        "tri" => Node::Tri,
        "white-noise" => Node::WhiteNoise,
        "host-in-l" => Node::HostInputL,
        "host-in-r" => Node::HostInputR,
        "lpf" => Node::Lpf,
        "hpf" => Node::Hpf,
        "bpf" => Node::Bpf,
        "delay" => Node::Delay,
        "comb" => Node::Comb,
        "env-perc" => Node::EnvPerc,
        "env-adsr" => Node::EnvAdsr,
        "line" => Node::Line,
        "sample-play" => Node::SamplePlay(crate::dsp::graph::BankRef::new(0)),
        "vco" => Node::Vco { unison_max: 16 },
        "sub-osc" => Node::SubOsc,
        "ladder" => Node::Ladder,
        "svf" => Node::Svf,
        "fm-op" => Node::FmOp,
        "fm-mod" => Node::FmMod,
        "fm-drum" => Node::FmDrum,
        "feedback-metal-core" => Node::FeedbackMetal,
        "digital-drum-core" => Node::DigitalDrumCore,
        "digital-snare-core" => Node::DigitalSnareCore,
        "digital-metal-core" => Node::DigitalMetalCore,
        "digital-hat-core" => Node::DigitalHatCore,
        "analog-percussion" => Node::AnalogPercussion,
        "va-source" => Node::VaSource,
        "va-filter" => Node::VaFilter,
        "phase-pair" => Node::PhasePair,
        "fm-pair" => Node::FmPair,
        "six-op-original" => Node::SixOpOriginal,
        "speech-original" => Node::SpeechOriginal,
        "resonator-part-core" => Node::RingsPart,
        "string-choir-core" => Node::StringChoir,
        "exciter-core" => Node::ElementsInternal,
        "tidal-function-core" => Node::TidalFunction,
        "tidal-poly-core" => Node::TidalPoly,
        "peak-function-core" => Node::PeakFunction,
        "stage-segment-core" => Node::StageSegment,
        "stage-chain-core" => Node::StageChain,
        "stage-linked-core" => Node::StageLinked { slot: 0 },
        "frame-lfo-core" => Node::FrameLfo,
        "frame-keyframe-core" => Node::FrameKeyframe { slot: 0 },
        "peak-pulse-core" => Node::PeakPulse,
        "number-station-core" => Node::NumberStation,
        "spectrum-pair" => Node::SpectrumPair,
        "clock-noise-pair" => Node::ClockNoisePair,
        "dual-kick-core" => Node::DualKick,
        "snare-pair-core" => Node::SnarePair,
        "hat-pair-core" => Node::HatPair,
        "swarm-pair-core" => Node::SwarmPair,
        "particle-pair-core" => Node::ParticlePair,
        "modal-pair-core" => Node::ModalPair,
        "string-pair-core" => Node::StringPair,
        "chip-pair-core" => Node::ChipPair,
        "analog-pair-core" => Node::AnalogPair,
        "grain-pair-core" => Node::GrainPair,
        "shape-pair-core" => Node::ShapePair,
        "string-machine-core" => Node::StringMachinePair,
        "terrain-pair-core" => Node::TerrainPair,
        "wave-grid-core" => Node::TableTerrainPair,
        "chord-layer-core" => Node::ChordPair,
        "macro-five-core" => Node::BraidsFive,
        "macro-sub-sync-core" => Node::BraidsSubSync,
        "macro-triple-core" => Node::BraidsTriple,
        "macro-digital-core" => Node::BraidsDigital,
        "macro-filter-core" => Node::BraidsFilter,
        "macro-formant-core" => Node::BraidsFormant,
        "macro-fm-core" => Node::BraidsFm,
        "macro-physical-core" => Node::BraidsPhysical,
        "macro-struck-core" => Node::BraidsStruck,
        "macro-percussion-core" => Node::BraidsPercussion,
        "macro-wave-grid-core" => Node::BraidsWaveBank,
        "macro-wave-line-core" => Node::BraidsWaveLine,
        "macro-noise-core" => Node::BraidsNoise,
        "macro-cloud-core" => Node::BraidsCloud,
        "aux-out" => Node::AuxOut,
        "out-3" => Node::Out3,
        "out-4" => Node::Out4,
        "feedback-drum" => Node::FeedbackDrum,
        "noise-drum" => Node::NoiseDrum,
        "sine-drum" => Node::SineDrum,
        "phase-distortion" => Node::PhaseDistortion,
        "additive" => Node::Additive { partials_max: 64 },
        "wavetable" => Node::Wavetable(crate::dsp::graph::TableRef::new(0)),
        "granular" => Node::Granular(crate::dsp::graph::GranSrc::Bus),
        _ => return None,
    })
}

fn ugen_editor(name: &str) -> EditorKind {
    match name {
        "lpf" | "hpf" | "bpf" | "ladder" | "svf" | "comb" => EditorKind::FilterResponse,
        "env-perc"
        | "env-adsr"
        | "line"
        | "fm-drum"
        | "phase-drum"
        | "feedback-drum"
        | "noise-drum"
        | "sine-drum"
        | "fusion-drum"
        | "feedback-metal-core"
        | "feedback-metal-drum"
        | "digital-drum-core"
        | "digital-snare-core"
        | "digital-metal-core"
        | "digital-hat-core"
        | "digital-drum"
        | "digital-snare"
        | "digital-metal"
        | "digital-hat" => EditorKind::EnvelopeShape,
        "delay" => EditorKind::DelayTaps,
        "sample-play" | "sampler" => EditorKind::SamplerWave,
        "wavetable" => EditorKind::WavetableFrames,
        "granular" => EditorKind::GranularRegion,
        _ => EditorKind::Scalar,
    }
}

fn ugen_meta(node: &Node) -> Box<[ParamMeta]> {
    ucat::ports(node)
        .iter()
        .enumerate()
        .filter_map(|(i, p): (usize, &Port)| {
            let ctl = ucat::port_ctl(node, i)?;
            Some(row_meta(p.name).unwrap_or_else(|| {
                let hi = if p.default.abs() > 1.0 {
                    p.default.abs() * 4.0
                } else {
                    1.0
                };
                meta(
                    p.name,
                    ctl,
                    (0.0, hi),
                    "",
                    matches!(p.name, "mode" | "count"),
                )
            }))
        })
        .collect()
}

fn build() -> Vec<EditorDecl> {
    let mut out: Vec<EditorDecl> = Vec::new();
    let mut push = |d: EditorDecl| {
        if !out.iter().any(|e| e.name == d.name) {
            out.push(d);
        }
    };
    for &kind in EffectKind::ALL {
        push(EditorDecl {
            name: kind.name(),
            kind: effect_editor(kind),
            params: effect_meta(kind),
        });
    }
    for &name in UGEN_NAMES {
        if let Some(node) = ugen_node(name) {
            push(EditorDecl {
                name,
                kind: ugen_editor(name),
                params: ugen_meta(&node),
            });
        }
    }
    for &name in TEMPLATE_NAMES {
        let params = TEMPLATE_PARAMS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[][..], |(_, p)| *p)
            .iter()
            .filter_map(|p| template_meta(name, p))
            .collect();
        push(EditorDecl {
            name,
            kind: ugen_editor(name),
            params,
        });
    }
    out
}

/// Every declaration, in catalog order.
pub fn all() -> &'static [EditorDecl] {
    static TABLE: OnceLock<Vec<EditorDecl>> = OnceLock::new();
    TABLE.get_or_init(build)
}

/// The editor declaration of a builtin name.
#[must_use]
pub fn decl_for(name: &str) -> Option<&'static EditorDecl> {
    all().iter().find(|d| d.name == name)
}
