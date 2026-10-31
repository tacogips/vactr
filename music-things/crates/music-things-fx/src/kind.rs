//! Effect and analyzer kinds.
/// Supported analyzer algorithms.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AnalyzerKind {
    /// Level processing.
    Level,
    /// Spectrum processing.
    Spectrum,
    /// Spectrogram processing.
    Spectrogram,
    /// Note Spectrogram processing.
    NoteSpectrogram,
    /// Oscilloscope processing.
    Oscilloscope,
    /// Pitch Meter processing.
    PitchMeter,
    /// Stereo Meter processing.
    StereoMeter,
}
/// Supported effect algorithms.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectKind {
    /// Compressor processing.
    Compressor,
    /// Expander processing.
    Expander,
    /// Gate processing.
    Gate,
    /// Limiter processing.
    Limiter,
    /// Multiband Compressor processing.
    MultibandCompressor,
    /// Multiband Expander processing.
    MultibandExpander,
    /// Transient processing.
    Transient,
    /// Multiband Transient processing.
    MultibandTransient,
    /// Auto Level processing.
    AutoLevel,
    /// Sag processing.
    Sag,
    /// Peq processing.
    Peq,
    /// Geq processing.
    Geq,
    /// Dynamic Eq processing.
    DynamicEq,
    /// Tilt processing.
    Tilt,
    /// Tone processing.
    Tone,
    /// Loudness Eq processing.
    LoudnessEq,
    /// Lpf processing.
    Lpf,
    /// Hpf processing.
    Hpf,
    /// Bpf processing.
    Bpf,
    /// Notch processing.
    Notch,
    /// Comb processing.
    Comb,
    /// Narrow processing.
    Narrow,
    /// Linear Phase Eq processing.
    LinearPhaseEq,
    /// Group Delay Eq processing.
    GroupDelayEq,
    /// Crossover processing.
    Crossover,
    /// Delay processing.
    Delay,
    /// Ping Pong processing.
    PingPong,
    /// Multitap processing.
    Multitap,
    /// Time Align processing.
    TimeAlign,
    /// Plate processing.
    Plate,
    /// Fdn processing.
    Fdn,
    /// Convolution processing.
    Convolution,
    /// Scatter processing.
    Scatter,
    /// Room processing.
    Room,
    /// Saturate processing.
    Saturate,
    /// Tube processing.
    Tube,
    /// Clip processing.
    Clip,
    /// Harmonics processing.
    Harmonics,
    /// Exciter processing.
    Exciter,
    /// Multiband Saturate processing.
    MultibandSaturate,
    /// Sub Synth processing.
    SubSynth,
    /// Bandwidth Extend processing.
    BandwidthExtend,
    /// Dynamic Saturate processing.
    DynamicSaturate,
    /// Chorus processing.
    Chorus,
    /// Flanger processing.
    Flanger,
    /// Phaser processing.
    Phaser,
    /// Tremolo processing.
    Tremolo,
    /// Auto Pan processing.
    AutoPan,
    /// Auto Filter processing.
    AutoFilter,
    /// Pitch Shift processing.
    PitchShift,
    /// Pitch Shift Hq processing.
    PitchShiftHq,
    /// Freq Shift processing.
    FreqShift,
    /// Rotary processing.
    Rotary,
    /// Wow Flutter processing.
    WowFlutter,
    /// Doppler processing.
    Doppler,
    /// Vibrato processing.
    Vibrato,
    /// Bitcrush processing.
    Bitcrush,
    /// Decimate processing.
    Decimate,
    /// Jitter processing.
    Jitter,
    /// Noise Blend processing.
    NoiseBlend,
    /// Hum processing.
    Hum,
    /// Tape processing.
    Tape,
    /// Cassette processing.
    Cassette,
    /// Vinyl processing.
    Vinyl,
    /// Vinyl Artifacts processing.
    VinylArtifacts,
    /// Codec processing.
    Codec,
    /// Radio processing.
    Radio,
    /// Tv Audio processing.
    TvAudio,
    /// Digital Error processing.
    DigitalError,
    /// Dsd Imd processing.
    DsdImd,
    /// Modal processing.
    Modal,
    /// Horn processing.
    Horn,
    /// Width processing.
    Width,
    /// Balance processing.
    Balance,
    /// Multiband Balance processing.
    MultibandBalance,
    /// Ms processing.
    Ms,
    /// Crossfeed processing.
    Crossfeed,
    /// Crosstalk Cancel processing.
    CrosstalkCancel,
    /// Phase Select Eq processing.
    PhaseSelectEq,
    /// Spatial Map processing.
    SpatialMap,
    /// Pan processing.
    Pan,
    /// Matrix processing.
    Matrix,
    /// Declick processing.
    Declick,
    /// Declip processing.
    Declip,
    /// Dehum processing.
    Dehum,
    /// Denoise processing.
    Denoise,
    /// Gain processing.
    Gain,
    /// Mute processing.
    Mute,
    /// Polarity processing.
    Polarity,
    /// Dc Offset processing.
    DcOffset,
    /// Dry Wet processing.
    DryWet,
    /// Section processing.
    Section,
    /// Channel Divider processing.
    ChannelDivider,
    /// Cross Mod processing.
    CrossMod,
    /// Fir Crossover processing.
    FirCrossover,
    /// Resonant Bank processing.
    ResonantBank,
    /// Elements Bank processing.
    ElementsBank,
    /// Stream Envelope processing.
    StreamEnvelope,
    /// Stream Opto processing.
    StreamOpto,
    /// Stream Follower processing.
    StreamFollower,
    /// Stream Compressor processing.
    StreamCompressor,
    /// Stream Filter processing.
    StreamFilter,
    /// Stream Lorenz processing.
    StreamLorenz,
    /// Shift Pair processing.
    ShiftPair,
    /// Keyframe Mixer processing.
    KeyframeMixer,
    /// Granulate processing.
    Granulate,
    /// Analyzer effect.
    Analyzer(AnalyzerKind),
    /// Texture Grain processing.
    TextureGrain,
    /// Texture Stretch processing.
    TextureStretch,
    /// Texture Loop processing.
    TextureLoop,
    /// Texture Spectral processing.
    TextureSpectral,
    /// Svf Filter processing.
    SvfFilter,
    /// Butterworth Filter processing.
    ButterworthFilter,
    /// Chebyshev Filter processing.
    ChebyshevFilter,
    /// Ladder Filter processing.
    LadderFilter,
    /// Allpass Filter processing.
    AllpassFilter,
    /// Parametric Resonator processing.
    ParametricResonator,
    /// Feedback Resonator processing.
    FeedbackResonator,
    /// Foldback processing.
    Foldback,
    /// Variable Clip processing.
    VariableClip,
    /// Alien Wah processing.
    AlienWah,
    /// Dynamic Convolution processing.
    DynamicConvolution,
    /// Early Reflections processing.
    EarlyReflections,
    /// Schroeder Reverb processing.
    SchroederReverb,
    /// Spring Reverb processing.
    SpringReverb,
    /// Space Reverb processing.
    SpaceReverb,
    /// Shimmer Reverb processing.
    ShimmerReverb,
    /// Tape Delay processing.
    TapeDelay,
    /// Diffusion Delay processing.
    DiffusionDelay,
    /// Lofi processing.
    Lofi,
}
