use super::{
    analyzer, composite_lofi, creative_delay, creative_reverb, cross_mod, delay, dynamic_control,
    dynamics, elements_bank, eq, lofi, modulation, music_filters, musicdsp, quad_mixer,
    resonant_bank, resonator, resonator_extra, restoration, reverb, saturation, shift_pair,
    spatial, stream_cv, stream_dynamics, texture, texture_loop, texture_spectral, texture_stretch,
    utility, EffectKind, FxContext, FxState, GrainLimits, ParamDef,
};

/// The implementation group of a kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Group {
    Dynamics,
    Eq,
    Delay,
    Reverb,
    Saturation,
    Modulation,
    Lofi,
    CompositeLofi,
    Resonator,
    Spatial,
    Restoration,
    Utility,
    Granular,
    Analyzer,
    CrossMod,
    ResonantBank,
    ElementsBank,
    Texture,
    TextureStretch,
    TextureLoop,
    TextureSpectral,
    StreamControl,
    StreamDynamics,
    StreamCv,
    ShiftPair,
    KeyframeMixer,
    MusicDsp,
    MusicFilters,
    ResonatorExtra,
    CreativeReverb,
    CreativeDelay,
}

fn group(kind: EffectKind) -> Group {
    use EffectKind as K;
    match kind {
        K::Compressor
        | K::Expander
        | K::Gate
        | K::Limiter
        | K::MultibandCompressor
        | K::MultibandExpander
        | K::Transient
        | K::MultibandTransient
        | K::AutoLevel
        | K::Sag => Group::Dynamics,
        K::Peq
        | K::Geq
        | K::DynamicEq
        | K::Tilt
        | K::Tone
        | K::LoudnessEq
        | K::Lpf
        | K::Hpf
        | K::Bpf
        | K::Notch
        | K::Comb
        | K::Narrow
        | K::LinearPhaseEq
        | K::GroupDelayEq
        | K::Crossover => Group::Eq,
        K::Delay | K::PingPong | K::Multitap | K::TimeAlign => Group::Delay,
        K::Plate | K::Fdn | K::Convolution | K::Scatter | K::Room => Group::Reverb,
        K::Saturate
        | K::Tube
        | K::Clip
        | K::Harmonics
        | K::Exciter
        | K::MultibandSaturate
        | K::SubSynth
        | K::BandwidthExtend
        | K::DynamicSaturate => Group::Saturation,
        K::Chorus
        | K::Flanger
        | K::Phaser
        | K::Tremolo
        | K::AutoPan
        | K::AutoFilter
        | K::PitchShift
        | K::PitchShiftHq
        | K::FreqShift
        | K::Rotary
        | K::WowFlutter
        | K::Doppler
        | K::Vibrato => Group::Modulation,
        K::Bitcrush
        | K::Decimate
        | K::Jitter
        | K::NoiseBlend
        | K::Hum
        | K::Tape
        | K::Cassette
        | K::Vinyl
        | K::VinylArtifacts
        | K::Codec
        | K::Radio
        | K::TvAudio
        | K::DigitalError
        | K::DsdImd => Group::Lofi,
        K::Modal | K::Horn => Group::Resonator,
        K::Width
        | K::Balance
        | K::MultibandBalance
        | K::Ms
        | K::Crossfeed
        | K::CrosstalkCancel
        | K::PhaseSelectEq
        | K::SpatialMap
        | K::Pan
        | K::Matrix => Group::Spatial,
        K::Declick | K::Declip | K::Dehum | K::Denoise => Group::Restoration,
        K::Gain
        | K::Mute
        | K::Polarity
        | K::DcOffset
        | K::DryWet
        | K::Section
        | K::ChannelDivider
        | K::FirCrossover => Group::Utility,
        K::Granulate => Group::Granular,
        K::CrossMod => Group::CrossMod,
        K::ResonantBank => Group::ResonantBank,
        K::ElementsBank => Group::ElementsBank,
        K::TextureGrain => Group::Texture,
        K::TextureStretch => Group::TextureStretch,
        K::TextureLoop => Group::TextureLoop,
        K::TextureSpectral => Group::TextureSpectral,
        K::StreamEnvelope | K::StreamOpto => Group::StreamControl,
        K::StreamFollower | K::StreamCompressor => Group::StreamDynamics,
        K::StreamFilter | K::StreamLorenz => Group::StreamCv,
        K::ShiftPair => Group::ShiftPair,
        K::KeyframeMixer => Group::KeyframeMixer,
        K::Analyzer(_) => Group::Analyzer,
        K::SvfFilter
        | K::ButterworthFilter
        | K::ChebyshevFilter
        | K::LadderFilter
        | K::AllpassFilter => Group::MusicFilters,
        K::ParametricResonator | K::FeedbackResonator => Group::ResonatorExtra,
        K::Foldback | K::VariableClip | K::AlienWah | K::DynamicConvolution => Group::MusicDsp,
        K::EarlyReflections
        | K::SchroederReverb
        | K::SpringReverb
        | K::SpaceReverb
        | K::ShimmerReverb => Group::CreativeReverb,
        K::TapeDelay | K::DiffusionDelay => Group::CreativeDelay,
        K::Lofi => Group::CompositeLofi,
    }
}

/// The named parameters of a kind, in catalog order.
#[must_use]
pub fn params(kind: EffectKind) -> &'static [ParamDef] {
    match group(kind) {
        Group::Dynamics => dynamics::params(kind),
        Group::Eq => eq::params(kind),
        Group::Delay => delay::params(kind),
        Group::Reverb => reverb::params(kind),
        Group::Saturation => saturation::params(kind),
        Group::Modulation => modulation::params(kind),
        Group::Lofi => lofi::params(kind),
        Group::CompositeLofi => composite_lofi::PARAMS,
        Group::Resonator => resonator::params(kind),
        Group::Spatial => spatial::params(kind),
        Group::Restoration => restoration::params(kind),
        Group::Utility => utility::params(kind),
        Group::Granular => super::granular::PARAMS,
        Group::Analyzer => analyzer::params(kind),
        Group::CrossMod => cross_mod::PARAMS,
        Group::ResonantBank => resonant_bank::PARAMS,
        Group::ElementsBank => elements_bank::PARAMS,
        Group::Texture => texture::PARAMS,
        Group::TextureStretch => texture::PARAMS,
        Group::TextureLoop => texture::PARAMS,
        Group::TextureSpectral => texture::PARAMS,
        Group::StreamControl => dynamic_control::PARAMS,
        Group::StreamDynamics => match kind {
            EffectKind::StreamFollower => stream_dynamics::FOLLOWER_PARAMS,
            _ => stream_dynamics::COMPRESSOR_PARAMS,
        },
        Group::StreamCv => match kind {
            EffectKind::StreamFilter => stream_cv::FILTER_PARAMS,
            _ => stream_cv::LORENZ_PARAMS,
        },
        Group::ShiftPair => shift_pair::PARAMS,
        Group::MusicDsp => musicdsp::params(kind),
        Group::MusicFilters => music_filters::params(kind),
        Group::ResonatorExtra => resonator_extra::params(kind),
        Group::CreativeReverb => creative_reverb::params(kind),
        Group::CreativeDelay => creative_delay::params(kind),
        Group::KeyframeMixer => quad_mixer::PARAMS,
    }
}

/// The preferred delay memory of a kind, in floats (the caller clamps it
/// to what the unit's region holds).
#[must_use]
pub fn mem_len(kind: EffectKind, sr: f32, limits: &GrainLimits) -> usize {
    match group(kind) {
        Group::Dynamics => dynamics::mem_len(kind, sr),
        Group::Eq => eq::mem_len(kind, sr),
        Group::Delay => delay::mem_len(kind, sr),
        Group::Reverb => reverb::mem_len(kind, sr),
        Group::Saturation => saturation::mem_len(kind, sr),
        Group::Modulation => modulation::mem_len(kind, sr),
        Group::Lofi => lofi::mem_len(kind, sr),
        Group::CompositeLofi => composite_lofi::mem_len(sr),
        Group::Resonator => resonator::mem_len(kind, sr),
        Group::Spatial => spatial::mem_len(kind, sr),
        Group::Restoration => restoration::mem_len(kind, sr),
        Group::Utility => utility::mem_len(kind, sr),
        Group::Granular => super::granular::effect_mem_len(sr, limits),
        Group::Analyzer => analyzer::mem_len(kind, sr),
        Group::CrossMod => cross_mod::mem_len(sr),
        Group::ResonantBank => resonant_bank::mem_len(sr),
        Group::ElementsBank => elements_bank::mem_len(sr),
        Group::Texture => texture::mem_len(sr),
        Group::TextureStretch => texture_stretch::mem_len(sr),
        Group::TextureLoop => texture_loop::mem_len(sr),
        Group::TextureSpectral => texture_spectral::mem_len(sr),
        Group::StreamControl => dynamic_control::MEM_LEN,
        Group::StreamDynamics => stream_dynamics::MEM_LEN,
        Group::StreamCv => stream_cv::MEM_LEN,
        Group::ShiftPair => shift_pair::MEM_LEN,
        Group::MusicDsp => musicdsp::mem_len(kind, sr),
        Group::MusicFilters => music_filters::mem_len(kind, sr),
        Group::ResonatorExtra => resonator_extra::mem_len(kind, sr),
        Group::CreativeReverb => creative_reverb::mem_len(kind, sr),
        Group::CreativeDelay => creative_delay::mem_len(kind, sr),
        Group::KeyframeMixer => 0,
    }
}

/// Kinds whose declared memory is part of their algorithm, never a clampable hint.
#[must_use]
pub fn requires_full_memory(kind: EffectKind) -> bool {
    use EffectKind as K;
    matches!(
        kind,
        K::CrossMod
            | K::ResonantBank
            | K::ElementsBank
            | K::TextureGrain
            | K::TextureStretch
            | K::TextureLoop
            | K::TextureSpectral
            | K::StreamEnvelope
            | K::StreamOpto
            | K::StreamFollower
            | K::StreamCompressor
            | K::StreamFilter
            | K::StreamLorenz
            | K::ShiftPair
            | K::SvfFilter
            | K::ButterworthFilter
            | K::ChebyshevFilter
            | K::LadderFilter
            | K::AllpassFilter
            | K::ParametricResonator
            | K::FeedbackResonator
            | K::Foldback
            | K::VariableClip
            | K::AlienWah
            | K::DynamicConvolution
            | K::EarlyReflections
            | K::SchroederReverb
            | K::SpringReverb
            | K::SpaceReverb
            | K::ShimmerReverb
            | K::TapeDelay
            | K::DiffusionDelay
            | K::Lofi
    )
}

/// Sets up a zeroed unit over `mem_len` floats of delay memory.
pub fn init(kind: EffectKind, st: &mut FxState, mem: &mut [f32], sr: f32, limits: &GrainLimits) {
    let n = mem.len();
    match group(kind) {
        Group::Dynamics => dynamics::init(kind, st, n, sr),
        Group::Eq => eq::init(kind, st, n, sr),
        Group::Delay => delay::init(kind, st, n, sr),
        Group::Reverb => reverb::init(kind, st, n, sr),
        Group::Saturation => saturation::init(kind, st, n, sr),
        Group::Modulation => modulation::init(kind, st, n, sr),
        Group::Lofi => lofi::init(kind, st, n, sr),
        Group::CompositeLofi => composite_lofi::init(st, n),
        Group::Resonator => resonator::init(kind, st, n, sr),
        Group::Spatial => spatial::init(kind, st, n, sr),
        Group::Restoration => restoration::init(kind, st, n, sr),
        Group::Utility => utility::init(kind, st, n, sr),
        Group::Granular => super::granular::effect_init(st, mem, sr, limits),
        Group::Analyzer => analyzer::init(kind, st, n, sr),
        Group::CrossMod => cross_mod::init(st, mem, sr),
        Group::ResonantBank => resonant_bank::init(st, mem),
        Group::ElementsBank => elements_bank::init(st, mem),
        Group::Texture => texture::init(st, mem),
        Group::TextureStretch => texture_stretch::init(st, mem),
        Group::TextureLoop => texture_loop::init(st, mem),
        Group::TextureSpectral => texture_spectral::init(st, mem),
        Group::StreamControl => dynamic_control::init(st, mem),
        Group::StreamDynamics => stream_dynamics::init(st, mem),
        Group::StreamCv => stream_cv::init(kind, st, mem),
        Group::ShiftPair => shift_pair::init(st, mem),
        Group::MusicDsp => musicdsp::init(kind, st, n, sr),
        Group::MusicFilters => music_filters::init(kind, st, n, sr),
        Group::ResonatorExtra => resonator_extra::init(kind, st, n, sr),
        Group::CreativeReverb => creative_reverb::init(kind, st, n, sr),
        Group::CreativeDelay => creative_delay::init(kind, st, n, sr),
        Group::KeyframeMixer => quad_mixer::init(st, mem),
    }
}

/// Processes one stereo block in place, fully wet.
#[allow(clippy::too_many_arguments)]
pub fn process<C: FxContext + ?Sized>(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut C,
) {
    let ctx = &mut ctx.fx_ctx();
    match group(kind) {
        Group::Dynamics => dynamics::process(kind, p, st, mem, l, r, ctx),
        Group::Eq => eq::process(kind, p, st, mem, l, r, ctx),
        Group::Delay => delay::process(kind, p, st, mem, l, r, ctx),
        Group::Reverb => reverb::process(kind, p, st, mem, l, r, ctx),
        Group::Saturation => saturation::process(kind, p, st, mem, l, r, ctx),
        Group::Modulation => modulation::process(kind, p, st, mem, l, r, ctx),
        Group::Lofi => lofi::process(kind, p, st, mem, l, r, ctx),
        Group::CompositeLofi => composite_lofi::process(p, st, mem, l, r, ctx),
        Group::Resonator => resonator::process(kind, p, st, mem, l, r, ctx),
        Group::Spatial => spatial::process(kind, p, st, mem, l, r, ctx),
        Group::Restoration => restoration::process(kind, p, st, mem, l, r, ctx),
        Group::Utility => utility::process(kind, p, st, mem, l, r, ctx),
        Group::Granular => super::granular::effect_process(p, st, mem, l, r, ctx),
        Group::Analyzer => analyzer::process(kind, p, st, mem, l, r, ctx),
        Group::CrossMod => cross_mod::process(p, st, mem, l, r, ctx),
        Group::ResonantBank => resonant_bank::process(p, st, mem, l, r, ctx),
        Group::ElementsBank => elements_bank::process(p, st, mem, l, r, ctx),
        Group::Texture => texture::process(p, st, mem, l, r, ctx),
        Group::TextureStretch => texture_stretch::process(p, st, mem, l, r, ctx),
        Group::TextureLoop => texture_loop::process(p, st, mem, l, r, ctx),
        Group::TextureSpectral => texture_spectral::process(p, st, mem, l, r, ctx),
        Group::StreamControl => dynamic_control::process(kind, p, st, mem, l, r, ctx),
        Group::StreamDynamics => stream_dynamics::process(kind, p, st, mem, l, r, ctx),
        Group::StreamCv => stream_cv::process(kind, p, st, mem, l, r, ctx),
        Group::ShiftPair => shift_pair::process(p, st, mem, l, r, ctx),
        Group::MusicDsp => musicdsp::process(kind, p, st, mem, l, r, ctx),
        Group::MusicFilters => music_filters::process(kind, p, st, mem, l, r, ctx),
        Group::ResonatorExtra => resonator_extra::process(kind, p, st, mem, l, r, ctx),
        Group::CreativeReverb => creative_reverb::process(kind, p, st, mem, l, r, ctx),
        Group::CreativeDelay => creative_delay::process(kind, p, st, mem, l, r, ctx),
        Group::KeyframeMixer => quad_mixer::process(p, st, l, r, ctx.sr),
    }
}

/// The index of a kind's `mix` parameter.
#[must_use]
pub fn mix_index(kind: EffectKind) -> Option<usize> {
    params(kind).iter().position(|p| p.name == "mix")
}
