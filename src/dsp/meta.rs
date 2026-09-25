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
const TEMPLATE_PARAMS: &[(&str, &[&str])] = &[
    (
        "sampler",
        &[
            "bank", "n", "speed", "begin", "end", "loop", "attack", "release",
        ],
    ),
    (
        "analog",
        &[
            "wave", "cutoff", "res", "unison", "detune", "drift", "attack", "decay", "sustain",
            "release",
        ],
    ),
    (
        "fm",
        &[
            "algorithm",
            "ratio",
            "index",
            "attack",
            "decay",
            "sustain",
            "release",
        ],
    ),
    ("pd", &["shape", "attack", "release"]),
    ("additive", &["amp", "attack", "release"]),
    (
        "wavetable",
        &[
            "table", "position", "cutoff", "res", "attack", "decay", "sustain", "release",
        ],
    ),
    (
        "granular",
        &[
            "source",
            "size",
            "density",
            "position",
            "spray",
            "pitch",
            "pitch-spray",
            "envelope",
            "reverse",
            "freeze",
        ],
    ),
];

fn unit_of(name: &str, unit: &str) -> Unit {
    match unit {
        "dB" => Unit::Db,
        "s" => Unit::Seconds,
        "ms" => Unit::Millis,
        "Hz" => Unit::Hz,
        _ => match name {
            "freq" | "cutoff" | "lpf" | "hpf" | "density" => Unit::Hz,
            "attack" | "decay" | "release" | "size" | "time" | "dur" | "delaytime" => Unit::Seconds,
            "pitch" | "semitones" => Unit::Semitones,
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

fn effect_meta(kind: EffectKind) -> Box<[ParamMeta]> {
    effects::params(kind)
        .iter()
        .filter_map(|d: &ParamDef| {
            let ctl = effects::param_ctl(kind, d.name)?;
            let stepped = matches!(d.name, "kind" | "mode" | "count" | "stages" | "bits" | "id");
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
        K::Granulate => EditorKind::GranularRegion,
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
        "env-perc" | "env-adsr" | "line" => EditorKind::EnvelopeShape,
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
            .filter_map(|p| row_meta(p))
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
