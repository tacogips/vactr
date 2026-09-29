//! Instrument and bus graphs (design 12.1, 12.5, 12.6, 12.8.6).
//!
//! An `inst` body builds a `UGenNode` tree on the evaluator thread (it holds
//! `Rc`, so it never crosses threads); BE-INST lowers it to an `InstDef` or
//! `BusDef`, the POD template the audio side instantiates per voice or bus.

use std::rc::Rc;

use crate::host::wire::Ctl;
use crate::pattern::signal::Sig;
use crate::sched::slots::CtlId;
use crate::value::intern::KwId;

id_newtype!(
    /// An instrument instance.
    InstId(u32)
);

id_newtype!(
    /// A declared bus (`bus :name`).
    BusId(u32)
);

id_newtype!(
    /// An installed sample bank.
    BankRef(u32)
);

id_newtype!(
    /// An installed wavetable.
    TableRef(u32)
);

id_newtype!(
    /// An installed impulse response.
    IrRef(u32)
);

/// The most nodes one instrument or bus graph may have (16.1); more is
/// `graph-too-large`.
pub const NODE_CAP: usize = 256;

/// The number of independent audio channels entering and leaving a graph
/// processing boundary. Bus effect chains use two channels throughout;
/// instrument UGen nodes currently remain mono.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AudioPortShape {
    pub inputs: u8,
    pub outputs: u8,
}

impl EffectKind {
    /// Shape when installed in a bus/master chain. All existing effect
    /// kernels accept two independent slices and write two independent
    /// slices; voice-local effect nodes currently downmix to mono.
    #[must_use]
    pub const fn bus_port_shape(self) -> AudioPortShape {
        let _ = self;
        AudioPortShape::STEREO
    }
}

impl AudioPortShape {
    pub const MONO: Self = Self {
        inputs: 1,
        outputs: 1,
    };
    pub const STEREO: Self = Self {
        inputs: 2,
        outputs: 2,
    };
}

/// What a granular ugen reads (design 12.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GranSrc {
    Sample(BankRef),
    Table(TableRef),
    /// The bus input (the `granulate` effect).
    Bus,
}

/// One node of an instrument template (design 12.1).
#[derive(Clone, PartialEq, Debug)]
pub enum UGenSpec {
    // core
    SinOsc,
    Saw,
    Pulse,
    Tri,
    WhiteNoise,
    /// Left channel of the validated host input for the current render block.
    HostInputL,
    /// Right channel of the validated host input for the current render block.
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
    // synthesis-model ugens (design-music section 4)
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
    /// The original tonal digital drum core (design-music 4.1, DDRUM-003).
    DigitalDrumCore,
    /// The original digital snare core (design-music 4.1, DDRUM-003).
    DigitalSnareCore,
    /// The original digital metal/cymbal core (design-music 4.1, DDRUM-004).
    DigitalMetalCore,
    /// The original digital hat core (design-music 4.1, DDRUM-004).
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
        data: Option<Box<crate::dsp::ugen::stage_linked::StageData>>,
    },
    FrameLfo,
    FrameKeyframe {
        data: Option<Box<crate::dsp::ugen::frame_keyframe::FrameData>>,
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
    /// Sends its input to the voice's auxiliary output and contributes
    /// silence to the ordinary mono graph result.
    AuxOut,
    /// Diverts a graph branch to direct output channel three.
    Out3,
    /// Diverts a graph branch to direct output channel four.
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
    /// Any builtin effect as a ugen (12.5).
    Effect(EffectSpec),
}

/// A connection: node `from`'s output feeds input `port` of node `to`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Edge {
    pub from: u16,
    pub to: u16,
    pub port: u8,
}

/// An instrument template (design 12.1).
#[derive(Clone, PartialEq, Debug)]
pub struct InstDef {
    pub id: InstId,
    /// Header parameters with their defaults (`Ctl::Cell` for a default that
    /// is a tweak site, 11.3).
    pub params: Box<[(CtlId, Ctl)]>,
    pub nodes: Box<[UGenSpec]>,
    pub edges: Box<[Edge]>,
    /// Per-node parameter inputs: `(node, control, value)`.
    pub node_params: Box<[(u16, CtlId, Ctl)]>,
}

/// An effect with its parameters (design 12.5).
#[derive(Clone, PartialEq, Debug)]
pub struct EffectSpec {
    pub kind: EffectKind,
    pub params: Box<[(CtlId, Ctl)]>,
}

/// A bus (or `master`) effect chain (design 12.5).
#[derive(Clone, PartialEq, Debug)]
pub struct BusDef {
    pub id: BusId,
    pub chain: Box<[EffectSpec]>,
}

macro_rules! analyzer_kinds {
    ($($variant:ident => $name:literal,)*) => {
        /// An analyzer: a transparent tap writing cells (design-music 5).
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum AnalyzerKind {
            $($variant,)*
        }

        impl AnalyzerKind {
            /// Every analyzer, in catalog order.
            pub const ALL: &'static [AnalyzerKind] = &[$(AnalyzerKind::$variant,)*];

            /// The catalog name.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(AnalyzerKind::$variant => $name,)*
                }
            }
        }
    };
}

analyzer_kinds! {
    Level => "level",
    Spectrum => "spectrum",
    Spectrogram => "spectrogram",
    NoteSpectrogram => "note-spectrogram",
    Oscilloscope => "oscilloscope",
    PitchMeter => "pitch-meter",
    StereoMeter => "stereo-meter",
}

macro_rules! effect_kinds {
    ($($variant:ident => $name:literal,)*) => {
        /// A builtin effect: one variant per design-music section 5 name.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum EffectKind {
            $($variant,)*
            /// The granular effect (design-music 6).
            Granulate,
            Analyzer(AnalyzerKind),
            /// Stereo capture and granular playback adaptation.
            TextureGrain,
            /// Stereo overlap-add stretch adaptation.
            TextureStretch,
            /// Stereo variable-delay and frozen-loop adaptation.
            TextureLoop,
            /// Bounded stereo spectral texture adaptation.
            TextureSpectral,
        }

        impl EffectKind {
            /// Every effect, in catalog order, `granulate` and the analyzers
            /// last.
            pub const ALL: &'static [EffectKind] = &[
                $(EffectKind::$variant,)*
                EffectKind::Granulate,
                EffectKind::Analyzer(AnalyzerKind::Level),
                EffectKind::Analyzer(AnalyzerKind::Spectrum),
                EffectKind::Analyzer(AnalyzerKind::Spectrogram),
                EffectKind::Analyzer(AnalyzerKind::NoteSpectrogram),
                EffectKind::Analyzer(AnalyzerKind::Oscilloscope),
                EffectKind::Analyzer(AnalyzerKind::PitchMeter),
                EffectKind::Analyzer(AnalyzerKind::StereoMeter),
                EffectKind::TextureGrain,
                EffectKind::TextureStretch,
                EffectKind::TextureLoop,
                EffectKind::TextureSpectral,
            ];

            /// The catalog name (kebab-case).
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(EffectKind::$variant => $name,)*
                    EffectKind::Granulate => "granulate",
                    EffectKind::Analyzer(a) => a.name(),
                    EffectKind::TextureGrain => "texture-grain",
                    EffectKind::TextureStretch => "texture-stretch",
                    EffectKind::TextureLoop => "texture-loop",
                    EffectKind::TextureSpectral => "texture-spectral",
                }
            }
        }
    };
}

effect_kinds! {
    // dynamics
    Compressor => "compressor",
    Expander => "expander",
    Gate => "gate",
    Limiter => "limiter",
    MultibandCompressor => "multiband-compressor",
    MultibandExpander => "multiband-expander",
    Transient => "transient",
    MultibandTransient => "multiband-transient",
    AutoLevel => "auto-level",
    Sag => "sag",
    // eq and filters
    Peq => "peq",
    Geq => "geq",
    DynamicEq => "dynamic-eq",
    Tilt => "tilt",
    Tone => "tone",
    LoudnessEq => "loudness-eq",
    Lpf => "lpf",
    Hpf => "hpf",
    Bpf => "bpf",
    Notch => "notch",
    Comb => "comb",
    Narrow => "narrow",
    LinearPhaseEq => "linear-phase-eq",
    GroupDelayEq => "group-delay-eq",
    Crossover => "crossover",
    // delay
    Delay => "delay",
    PingPong => "ping-pong",
    Multitap => "multitap",
    TimeAlign => "time-align",
    // reverb
    Plate => "plate",
    Fdn => "fdn",
    Convolution => "convolution",
    Scatter => "scatter",
    Room => "room",
    // saturation
    Saturate => "saturate",
    Tube => "tube",
    Clip => "clip",
    Harmonics => "harmonics",
    Exciter => "exciter",
    MultibandSaturate => "multiband-saturate",
    SubSynth => "sub-synth",
    BandwidthExtend => "bandwidth-extend",
    DynamicSaturate => "dynamic-saturate",
    // modulation
    Chorus => "chorus",
    Flanger => "flanger",
    Phaser => "phaser",
    Tremolo => "tremolo",
    AutoPan => "auto-pan",
    AutoFilter => "auto-filter",
    PitchShift => "pitch-shift",
    PitchShiftHq => "pitch-shift-hq",
    FreqShift => "freq-shift",
    Rotary => "rotary",
    WowFlutter => "wow-flutter",
    Doppler => "doppler",
    Vibrato => "vibrato",
    // lo-fi
    Bitcrush => "bitcrush",
    Decimate => "decimate",
    Jitter => "jitter",
    NoiseBlend => "noise-blend",
    Hum => "hum",
    Tape => "tape",
    Cassette => "cassette",
    Vinyl => "vinyl",
    VinylArtifacts => "vinyl-artifacts",
    Codec => "codec",
    Radio => "radio",
    TvAudio => "tv-audio",
    DigitalError => "digital-error",
    DsdImd => "dsd-imd",
    // resonator
    Modal => "modal",
    Horn => "horn",
    // spatial
    Width => "width",
    Balance => "balance",
    MultibandBalance => "multiband-balance",
    Ms => "ms",
    Crossfeed => "crossfeed",
    CrosstalkCancel => "crosstalk-cancel",
    PhaseSelectEq => "phase-select-eq",
    SpatialMap => "spatial-map",
    Pan => "pan",
    Matrix => "matrix",
    // restoration
    Declick => "declick",
    Declip => "declip",
    Dehum => "dehum",
    Denoise => "denoise",
    // utility
    Gain => "gain",
    Mute => "mute",
    Polarity => "polarity",
    DcOffset => "dc-offset",
    DryWet => "dry-wet",
    Section => "section",
    ChannelDivider => "channel-divider",
    CrossMod => "dual-mod",
    FirCrossover => "fir-crossover",
    ResonantBank => "resonant-bank",
    ElementsBank => "exciter-bank",
    StreamEnvelope => "stream-envelope",
    StreamVactr => "stream-vactr",
    StreamFollower => "stream-follower",
    StreamCompressor => "stream-compressor",
    StreamFilter => "stream-filter",
    StreamLorenz => "stream-lorenz",
    ShiftPair => "shift-pair",
    KeyframeMixer => "keyframe-mixer",
}

impl EffectKind {
    /// The effect with this catalog name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<EffectKind> {
        EffectKind::ALL.iter().copied().find(|k| k.name() == name)
    }
}

/// An evaluator-side ugen tree node: what an `inst` or `bus` body returns
/// (`Value::UGen`). BE-INST gives it meaning and lowers it (`dsp::build`).
#[derive(Clone, Debug)]
pub struct UGenNode {
    pub kind: UGenKind,
    /// Positional (`None`) and named arguments, in call order.
    pub args: Box<[(Option<KwId>, UGenInput)]>,
}

/// What a `UGenNode` computes.
#[derive(Clone, Debug)]
pub enum UGenKind {
    Ugen(UGenSpec),
    Effect(EffectKind),
    /// The implicit subject of a `bus` body: the bus input.
    BusInput,
}

/// One ugen input.
#[derive(Clone, Debug)]
pub enum UGenInput {
    Node(Rc<UGenNode>),
    Const(f32),
    /// A control name (a header parameter or an implicit control, B2).
    Param(CtlId),
    /// A signal sampled once per tick into a control cell (12.8.6).
    Signal(Rc<Sig>),
    Keyword(KwId),
    /// A realization-time constant list (`partials`).
    List(Rc<[f32]>),
}
