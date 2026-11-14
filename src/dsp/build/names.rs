//! UGen name, keyword and positional port registry used by graph lowering.

use crate::dsp::graph::{BankRef, GranSrc, TableRef, UGenSpec};
use crate::dsp::ugen::catalog;

/// The first port of a named input that is not in the kind's port list.
pub const NAMED_PORT: u8 = 64;
/// The first synthetic parameter of a signal input.
pub const SIGNAL_CTL_BASE: u16 = 0x4000;
/// The first id of a parameter name outside the control table.
pub const EXTRA_CTL_BASE: u16 = 128;

/// Ugen named parameters outside the control table, `EXTRA_CTL_BASE + i`.
pub const EXTRA_PARAMS: &[&str] = &[
    "in",
    "width",
    "time",
    "feedback",
    "mix",
    "threshold",
    "knee",
    "makeup",
    "ceiling",
    "depth",
    "rate",
    "mode",
    "kind",
    "start",
    "dur",
    "partials",
    "bands",
    "amount",
    "tone",
    "q",
    "low",
    "mid",
    "high",
    "damping",
    "predelay",
    "taps",
    "bits",
    "factor",
    "level",
    "carrier",
    "modulator",
    "section",
    "channels",
    "order",
    "hz",
    "on",
    "offset",
];

/// Every named argument a ugen or effect takes (kept equal to
/// `EXTRA_PARAMS` + the control table by a test).
pub const DSP_KEYWORDS: &[&str] = &[
    "freq",
    "amp",
    "gain",
    "note",
    "n",
    "pan",
    "speed",
    "velocity",
    "begin",
    "end",
    "attack",
    "decay",
    "sustain",
    "release",
    "cutoff",
    "res",
    "wave",
    "unison",
    "detune",
    "drift",
    "ratio",
    "index",
    "algorithm",
    "position",
    "table",
    "bank",
    "loop",
    "density",
    "spray",
    "pitch",
    "pitch-spray",
    "envelope",
    "reverse",
    "freeze",
    "stereo-spray",
    "source",
    "resonance",
    "room",
    "size",
    "shape",
    "fm-amount",
    "pitch-sweep",
    "drum-noise",
    "drive",
    "fb-delay",
    "fb-feedback",
    "fb-decay",
    "fb-cutoff",
    "fb-q",
    "fb-velocity",
    "fb-level",
    "noise-attack",
    "noise-hold",
    "noise-decay",
    "noise-cutoff",
    "noise-q",
    "noise-pitch-env",
    "noise-velocity",
    "noise-level",
    "fm-tuning",
    "fm-keytrack",
    "fm-ratio",
    "fm-index",
    "fm-attack",
    "fm-hold",
    "fm-decay",
    "fm-pitch-env",
    "fm-velocity",
    "fm-level",
    "in",
    "width",
    "time",
    "feedback",
    "mix",
    "threshold",
    "knee",
    "makeup",
    "ceiling",
    "depth",
    "rate",
    "mode",
    "kind",
    "start",
    "dur",
    "partials",
    "bands",
    "amount",
    "tone",
    "q",
    "low",
    "mid",
    "high",
    "damping",
    "predelay",
    "taps",
    "bits",
    "factor",
    "level",
    "carrier",
    "modulator",
    "section",
    "channels",
    "order",
    "hz",
    "on",
    "offset",
];

/// `svf` filter modes (`svf lowpass cutoff res`).
pub const SVF_MODES: &[&str] = &["lowpass", "highpass", "bandpass"];
pub(super) const CODEC_KINDS: &[&str] = &["mp3", "gsm", "sbc", "atrac", "g726"];
pub(super) const RADIO_KINDS: &[&str] = &["am", "fm", "sw"];

/// Every ugen name and the node it builds (design-music section 2, 4, 6).
pub const UGENS: &[(&str, UGenSpec)] = &[
    ("sin-osc", UGenSpec::SinOsc),
    ("saw", UGenSpec::Saw),
    ("pulse", UGenSpec::Pulse),
    ("tri", UGenSpec::Tri),
    ("white-noise", UGenSpec::WhiteNoise),
    ("host-in-l", UGenSpec::HostInputL),
    ("host-in-r", UGenSpec::HostInputR),
    ("lpf", UGenSpec::Lpf),
    ("hpf", UGenSpec::Hpf),
    ("bpf", UGenSpec::Bpf),
    ("delay", UGenSpec::Delay),
    ("comb", UGenSpec::Comb),
    ("env-perc", UGenSpec::EnvPerc),
    ("env-adsr", UGenSpec::EnvAdsr),
    ("line", UGenSpec::Line),
    ("sample-play", UGenSpec::SamplePlay(BankRef::new(0))),
    ("vco", UGenSpec::Vco { unison_max: 1 }),
    ("sub-osc", UGenSpec::SubOsc),
    ("ladder", UGenSpec::Ladder),
    ("svf", UGenSpec::Svf),
    ("fm-op", UGenSpec::FmOp),
    ("fm-mod", UGenSpec::FmMod),
    ("fm-drum", UGenSpec::FmDrum),
    ("feedback-metal-core", UGenSpec::FeedbackMetal),
    ("digital-drum-core", UGenSpec::DigitalDrumCore),
    ("digital-snare-core", UGenSpec::DigitalSnareCore),
    ("digital-metal-core", UGenSpec::DigitalMetalCore),
    ("digital-hat-core", UGenSpec::DigitalHatCore),
    ("analog-percussion", UGenSpec::AnalogPercussion),
    ("va-source", UGenSpec::VaSource),
    ("va-filter", UGenSpec::VaFilter),
    ("phase-pair", UGenSpec::PhasePair),
    ("fm-pair", UGenSpec::FmPair),
    ("six-op-original", UGenSpec::SixOpOriginal),
    ("speech-original", UGenSpec::SpeechOriginal),
    ("resonator-part-core", UGenSpec::RingsPart),
    ("string-choir-core", UGenSpec::StringChoir),
    ("exciter-core", UGenSpec::ElementsInternal),
    ("tidal-function-core", UGenSpec::TidalFunction),
    ("tidal-poly-core", UGenSpec::TidalPoly),
    ("peak-function-core", UGenSpec::PeakFunction),
    ("stage-segment-core", UGenSpec::StageSegment),
    ("stage-chain-core", UGenSpec::StageChain),
    ("stage-linked-core", UGenSpec::StageLinked { data: None }),
    ("frame-lfo-core", UGenSpec::FrameLfo),
    (
        "frame-keyframe-core",
        UGenSpec::FrameKeyframe { data: None },
    ),
    ("peak-pulse-core", UGenSpec::PeakPulse),
    ("number-station-core", UGenSpec::NumberStation),
    ("spectrum-pair", UGenSpec::SpectrumPair),
    ("clock-noise-pair", UGenSpec::ClockNoisePair),
    ("dual-kick-core", UGenSpec::DualKick),
    ("snare-pair-core", UGenSpec::SnarePair),
    ("hat-pair-core", UGenSpec::HatPair),
    ("swarm-pair-core", UGenSpec::SwarmPair),
    ("particle-pair-core", UGenSpec::ParticlePair),
    ("modal-pair-core", UGenSpec::ModalPair),
    ("string-pair-core", UGenSpec::StringPair),
    ("chip-pair-core", UGenSpec::ChipPair),
    ("analog-pair-core", UGenSpec::AnalogPair),
    ("grain-pair-core", UGenSpec::GrainPair),
    ("shape-pair-core", UGenSpec::ShapePair),
    ("vactrol-gate", UGenSpec::VactrolGate),
    ("decay-mod", UGenSpec::DecayMod),
    ("string-machine-core", UGenSpec::StringMachinePair),
    ("terrain-pair-core", UGenSpec::TerrainPair),
    ("wave-grid-core", UGenSpec::TableTerrainPair),
    ("chord-layer-core", UGenSpec::ChordPair),
    ("macro-five-core", UGenSpec::BraidsFive),
    ("macro-sub-sync-core", UGenSpec::BraidsSubSync),
    ("macro-triple-core", UGenSpec::BraidsTriple),
    ("macro-digital-core", UGenSpec::BraidsDigital),
    ("macro-filter-core", UGenSpec::BraidsFilter),
    ("macro-formant-core", UGenSpec::BraidsFormant),
    ("macro-fm-core", UGenSpec::BraidsFm),
    ("macro-physical-core", UGenSpec::BraidsPhysical),
    ("macro-struck-core", UGenSpec::BraidsStruck),
    ("macro-percussion-core", UGenSpec::BraidsPercussion),
    ("macro-wave-grid-core", UGenSpec::BraidsWaveBank),
    ("macro-wave-line-core", UGenSpec::BraidsWaveLine),
    ("macro-noise-core", UGenSpec::BraidsNoise),
    ("macro-cloud-core", UGenSpec::BraidsCloud),
    ("aux-out", UGenSpec::AuxOut),
    ("out-3", UGenSpec::Out3),
    ("out-4", UGenSpec::Out4),
    ("feedback-drum", UGenSpec::FeedbackDrum),
    ("noise-drum", UGenSpec::NoiseDrum),
    ("sine-drum", UGenSpec::SineDrum),
    ("phase-distortion", UGenSpec::PhaseDistortion),
    ("additive", UGenSpec::Additive { partials_max: 0 }),
    ("wavetable", UGenSpec::Wavetable(TableRef::new(0))),
    (
        "granular",
        UGenSpec::Granular(GranSrc::Sample(BankRef::new(0))),
    ),
];

/// The ugen named `name`.
#[must_use]
pub fn ugen_named(name: &str) -> Option<UGenSpec> {
    UGENS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| s.clone())
}

/// True for a ugen whose first positional input is an audio input.
#[must_use]
pub fn is_processor(spec: &UGenSpec) -> bool {
    matches!(
        spec,
        UGenSpec::Lpf
            | UGenSpec::Hpf
            | UGenSpec::Bpf
            | UGenSpec::Delay
            | UGenSpec::Comb
            | UGenSpec::Ladder
            | UGenSpec::Svf
            | UGenSpec::FmMod
            | UGenSpec::VactrolGate
            | UGenSpec::Mul
            | UGenSpec::Add
    )
}

mod table;
pub use table::ports;

/// True for a voice-level resource argument (R2a), skipped before `input()`.
pub(super) fn is_resource_arg(spec: &UGenSpec, name: &str) -> bool {
    match spec {
        UGenSpec::SamplePlay(_) => matches!(name, "bank" | "n"),
        UGenSpec::Wavetable(_) => name == "table",
        UGenSpec::Granular(_) => name == "source",
        _ => false,
    }
}

/// The runtime catalog's name for `name`, when it differs (R2a).
fn catalog_alias<'a>(spec: &UGenSpec, name: &'a str) -> &'a str {
    match (spec, name) {
        (UGenSpec::SamplePlay(_), "rate") => "speed",
        (UGenSpec::Line, "start") => "from",
        (UGenSpec::Line, "end") => "to",
        (UGenSpec::FmMod, "carrier") => "in",
        (UGenSpec::FmMod, "modulator") => "mod",
        (UGenSpec::Mul | UGenSpec::Add, "in") => "a",
        (UGenSpec::Mul | UGenSpec::Add, "amount") => "b",
        _ => name,
    }
}

/// `name`'s index in the RUNTIME catalog port list, after the alias; `None`
/// when it names no runtime port of this ugen.
pub(super) fn catalog_port(spec: &UGenSpec, name: &str) -> Option<u8> {
    let alias = catalog_alias(spec, name);
    let node = catalog::node_of(spec);
    catalog::ports(&node)
        .iter()
        .position(|p| p.name == alias)
        .and_then(|i| u8::try_from(i).ok())
}
