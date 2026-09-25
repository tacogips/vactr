//! The builtin effect catalog on the audio side (design 12.5, 12.8.8).
//!
//! Every `EffectKind` is a bounded, allocation-free algorithm over the
//! shared primitives in `prim`. A unit keeps its scalar state inline in an
//! `FxState` (a fixed bank of primitives) and its delay memory in a `mem`
//! region carved at install time; nothing is allocated while processing.
//!
//! Group files implement four functions for their kinds: `params` (the
//! named parameters in catalog order), `mem_len` (preferred delay memory,
//! clamped by the caller), `init` (set up after the state was zeroed) and
//! `process` (stereo, in place, fully wet). `FxUnit::run` blends a kind's
//! `mix` parameter itself, so `mix 0` is bit-identical to the input.
//!
//! Parameters are addressed by effect-local ids: parameter `i` of a kind is
//! `CtlId(EFFECT_PARAM_BASE + i)` (`param_ctl`), because most effect
//! parameter names (`threshold`, `drive`, `mix`, ...) are not rows of the
//! control table.

pub mod analyzer;
pub mod catalog;
pub mod delay;
pub mod dynamics;
pub mod eq;
pub mod lofi;
pub mod modulation;
pub mod prim;
pub mod resonator;
pub mod restoration;
pub mod reverb;
pub mod saturation;
pub mod spatial;
pub mod utility;

use crate::dsp::arena::SampleStore;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellRead;
use crate::dsp::fft::Fft;
use crate::dsp::graph::EffectKind;
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

use self::prim::{Biquad, DelayLine, Follower, Lfo, OnePole, Rng};

/// The first effect-local parameter id.
pub const EFFECT_PARAM_BASE: u16 = 0x4000;
/// The most parameters one effect kind has.
pub const MAX_FX_PARAMS: usize = 16;

/// One named effect parameter.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ParamDef {
    pub name: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    /// Display unit (`"dB"`, `"s"`, `"Hz"`, `""`).
    pub unit: &'static str,
}

impl ParamDef {
    /// A unitless parameter.
    #[must_use]
    pub const fn new(name: &'static str, default: f32, min: f32, max: f32) -> Self {
        Self {
            name,
            default,
            min,
            max,
            unit: "",
        }
    }

    /// A parameter with a display unit.
    #[must_use]
    pub const fn unit(
        name: &'static str,
        default: f32,
        min: f32,
        max: f32,
        unit: &'static str,
    ) -> Self {
        Self {
            name,
            default,
            min,
            max,
            unit,
        }
    }

    /// `v` clamped into the parameter range (NaN -> default).
    #[must_use]
    pub fn clamp(&self, v: f32) -> f32 {
        if v.is_nan() {
            self.default
        } else {
            v.clamp(self.min, self.max)
        }
    }
}

/// The scalar state of one unit: a fixed bank of primitives. Kinds use the
/// entries they need by index.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct FxState {
    pub bq: [Biquad; 16],
    pub op: [OnePole; 8],
    pub env: [Follower; 4],
    pub lfo: [Lfo; 4],
    pub dl: [DelayLine; 8],
    pub s: [f32; 32],
    pub rng: Rng,
}

/// Per-block counters effects report (granular and analyzers, 12.6).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FxStats {
    /// Grain spawns skipped (window violation or pool exhaustion).
    pub grains_skipped: u32,
    /// Controls clamped to a tier cap (density, size, capture depth).
    pub clamped: u32,
    /// Grains spawned.
    pub grains_spawned: u32,
    /// Grains short-gated at an unfreeze.
    pub grains_gated: u32,
    /// Live-buffer reads outside the written, not-yet-overwritten window
    /// (must stay 0; the no-stale-read proof of 12.6).
    pub stale_reads: u32,
}

/// What a unit may read while processing one block.
pub struct FxCtx<'a> {
    pub sr: f32,
    pub store: &'a SampleStore,
    pub fft: &'a Fft,
    pub caps: &'a CapabilitySet,
    /// Free scratch, at least `max(4 * max_block, 4 * FFT_SIZE)` floats.
    pub scratch: &'a mut [f32],
    /// The analysis cell table analyzers write.
    pub analysis: &'a mut [f32],
    pub stats: &'a mut FxStats,
}

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
    Resonator,
    Spatial,
    Restoration,
    Utility,
    Granular,
    Analyzer,
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
        K::Analyzer(_) => Group::Analyzer,
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
        Group::Resonator => resonator::params(kind),
        Group::Spatial => spatial::params(kind),
        Group::Restoration => restoration::params(kind),
        Group::Utility => utility::params(kind),
        Group::Granular => crate::dsp::granular::PARAMS,
        Group::Analyzer => analyzer::params(kind),
    }
}

/// The preferred delay memory of a kind, in floats (the caller clamps it
/// to what the unit's region holds).
#[must_use]
pub fn mem_len(kind: EffectKind, sr: f32, caps: &CapabilitySet) -> usize {
    match group(kind) {
        Group::Dynamics => dynamics::mem_len(kind, sr),
        Group::Eq => eq::mem_len(kind, sr),
        Group::Delay => delay::mem_len(kind, sr),
        Group::Reverb => reverb::mem_len(kind, sr),
        Group::Saturation => saturation::mem_len(kind, sr),
        Group::Modulation => modulation::mem_len(kind, sr),
        Group::Lofi => lofi::mem_len(kind, sr),
        Group::Resonator => resonator::mem_len(kind, sr),
        Group::Spatial => spatial::mem_len(kind, sr),
        Group::Restoration => restoration::mem_len(kind, sr),
        Group::Utility => utility::mem_len(kind, sr),
        Group::Granular => crate::dsp::granular::effect_mem_len(sr, caps),
        Group::Analyzer => analyzer::mem_len(kind, sr),
    }
}

/// Sets up a zeroed unit over `mem_len` floats of delay memory.
pub fn init(kind: EffectKind, st: &mut FxState, mem: &mut [f32], sr: f32, caps: &CapabilitySet) {
    let n = mem.len();
    match group(kind) {
        Group::Dynamics => dynamics::init(kind, st, n, sr),
        Group::Eq => eq::init(kind, st, n, sr),
        Group::Delay => delay::init(kind, st, n, sr),
        Group::Reverb => reverb::init(kind, st, n, sr),
        Group::Saturation => saturation::init(kind, st, n, sr),
        Group::Modulation => modulation::init(kind, st, n, sr),
        Group::Lofi => lofi::init(kind, st, n, sr),
        Group::Resonator => resonator::init(kind, st, n, sr),
        Group::Spatial => spatial::init(kind, st, n, sr),
        Group::Restoration => restoration::init(kind, st, n, sr),
        Group::Utility => utility::init(kind, st, n, sr),
        Group::Granular => crate::dsp::granular::effect_init(st, mem, sr, caps),
        Group::Analyzer => analyzer::init(kind, st, n, sr),
    }
}

/// Processes one stereo block in place, fully wet.
#[allow(clippy::too_many_arguments)]
pub fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    match group(kind) {
        Group::Dynamics => dynamics::process(kind, p, st, mem, l, r, ctx),
        Group::Eq => eq::process(kind, p, st, mem, l, r, ctx),
        Group::Delay => delay::process(kind, p, st, mem, l, r, ctx),
        Group::Reverb => reverb::process(kind, p, st, mem, l, r, ctx),
        Group::Saturation => saturation::process(kind, p, st, mem, l, r, ctx),
        Group::Modulation => modulation::process(kind, p, st, mem, l, r, ctx),
        Group::Lofi => lofi::process(kind, p, st, mem, l, r, ctx),
        Group::Resonator => resonator::process(kind, p, st, mem, l, r, ctx),
        Group::Spatial => spatial::process(kind, p, st, mem, l, r, ctx),
        Group::Restoration => restoration::process(kind, p, st, mem, l, r, ctx),
        Group::Utility => utility::process(kind, p, st, mem, l, r, ctx),
        Group::Granular => crate::dsp::granular::effect_process(p, st, mem, l, r, ctx),
        Group::Analyzer => analyzer::process(kind, p, st, mem, l, r, ctx),
    }
}

/// The effect-local id of a kind's parameter.
#[must_use]
pub fn param_ctl(kind: EffectKind, name: &str) -> Option<CtlId> {
    let i = params(kind).iter().position(|p| p.name == name)?;
    Some(CtlId::new(EFFECT_PARAM_BASE + u16::try_from(i).ok()?))
}

/// The parameter index an effect-local id names.
#[must_use]
pub fn param_index(kind: EffectKind, ctl: CtlId) -> Option<usize> {
    let i = usize::from(ctl.get().checked_sub(EFFECT_PARAM_BASE)?);
    (i < params(kind).len()).then_some(i)
}

/// The index of a kind's `mix` parameter.
#[must_use]
pub fn mix_index(kind: EffectKind) -> Option<usize> {
    params(kind).iter().position(|p| p.name == "mix")
}

/// One instantiated effect: parameters with their sources and the scalar
/// state. Its delay memory lives outside (a voice or bus region).
#[derive(Clone, Copy, Debug)]
pub struct FxUnit {
    pub kind: EffectKind,
    n: u8,
    /// A cell-backed parameter is re-read every block.
    src: [Ctl; MAX_FX_PARAMS],
    /// Targets and the smoothed values the algorithm sees.
    target: [f32; MAX_FX_PARAMS],
    vals: [f32; MAX_FX_PARAMS],
    pub st: FxState,
}

/// True when a convolution's `ir` parameter (a resource id) is `id`.
#[must_use]
pub fn ir_is(kind: EffectKind, ir: Ctl, id: u32) -> bool {
    #[allow(clippy::cast_precision_loss)]
    let want = id as f32;
    kind == EffectKind::Convolution && matches!(ir, Ctl::Const(v) if v >= 0.0 && v == want)
}

impl FxUnit {
    /// True when a parameter reads `cell` (the retire in-use check, 11.3).
    #[must_use]
    pub fn reads_cell(&self, cell: crate::dsp::cells::CellId) -> bool {
        self.src[..usize::from(self.n)].contains(&Ctl::Cell(cell))
    }

    /// True when the unit reads resource `id` (a convolution impulse
    /// response).
    #[must_use]
    pub fn reads_resource(&self, id: u32) -> bool {
        params(self.kind)
            .iter()
            .position(|p| p.name == "ir")
            .is_some_and(|i| ir_is(self.kind, self.src[i], id))
    }

    /// A `gain` unit at its defaults (a placeholder slot).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            kind: EffectKind::Gain,
            n: 0,
            src: [Ctl::Const(0.0); MAX_FX_PARAMS],
            target: [0.0; MAX_FX_PARAMS],
            vals: [0.0; MAX_FX_PARAMS],
            st: FxState::default(),
        }
    }

    /// Configures the unit for `kind` with explicit parameters (effect-local
    /// ids; unknown ids are ignored), zeroes `mem` and initializes the state.
    /// Cell parameters read their current value from `cells`.
    pub fn configure<C: CellRead + ?Sized>(
        &mut self,
        kind: EffectKind,
        given: &[(CtlId, Ctl)],
        cells: &C,
        mem: &mut [f32],
        sr: f32,
        caps: &CapabilitySet,
    ) {
        let defs = params(kind);
        self.kind = kind;
        self.n = u8::try_from(defs.len().min(MAX_FX_PARAMS)).unwrap_or(0);
        for (i, d) in defs.iter().take(MAX_FX_PARAMS).enumerate() {
            self.src[i] = Ctl::Const(d.default);
        }
        for &(id, ctl) in given {
            if let Some(i) = param_index(kind, id) {
                if i < MAX_FX_PARAMS {
                    self.src[i] = ctl;
                }
            }
        }
        for i in 0..usize::from(self.n) {
            let v = self.resolve(i, cells);
            self.target[i] = v;
            self.vals[i] = v;
        }
        mem.fill(0.0);
        self.st = FxState::default();
        init(kind, &mut self.st, mem, sr, caps);
    }

    fn resolve<C: CellRead + ?Sized>(&self, i: usize, cells: &C) -> f32 {
        let raw = match self.src[i] {
            Ctl::Const(v) => v,
            Ctl::Cell(c) => cells.get(c),
        };
        params(self.kind)[i].clamp(raw)
    }

    /// Re-reads cell-backed parameters (once per block).
    pub fn update<C: CellRead + ?Sized>(&mut self, cells: &C) {
        for i in 0..usize::from(self.n) {
            if matches!(self.src[i], Ctl::Cell(_)) {
                self.target[i] = self.resolve(i, cells);
            }
        }
    }

    /// Sets parameter `i` from outside (a pattern control routed to the
    /// unit, or an audio-rate input sampled at the block start).
    pub fn set(&mut self, i: usize, v: f32) {
        if i < usize::from(self.n) {
            self.src[i] = Ctl::Const(v);
            self.target[i] = params(self.kind)[i].clamp(v);
        }
    }

    /// The target of parameter `i` (what the smoothed value moves to).
    #[must_use]
    pub fn target(&self, i: usize) -> f32 {
        self.target.get(i).copied().unwrap_or(0.0)
    }

    /// The current (smoothed) value of parameter `i`.
    #[must_use]
    pub fn value(&self, i: usize) -> f32 {
        self.vals.get(i).copied().unwrap_or(0.0)
    }

    /// Processes one stereo block in place; `dry` is scratch of at least
    /// `2 * l.len()` floats for the mix blend.
    pub fn run(
        &mut self,
        mem: &mut [f32],
        l: &mut [f32],
        r: &mut [f32],
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
    ) {
        let n = usize::from(self.n);
        for i in 0..n {
            let (v, t) = (self.vals[i], self.target[i]);
            let next = v + (t - v) * 0.5;
            self.vals[i] = if (next - t).abs() < 1.0e-6 { t } else { next };
        }
        let frames = l.len().min(r.len());
        let mix = mix_index(self.kind).map(|i| self.vals[i].clamp(0.0, 1.0));
        if mix == Some(0.0) || frames == 0 {
            return;
        }
        let blend = mix.filter(|m| *m < 1.0);
        let (dl, dr) = dry.split_at_mut(dry.len() / 2);
        let blend = blend.filter(|_| dl.len() >= frames && dr.len() >= frames);
        if blend.is_some() {
            dl[..frames].copy_from_slice(&l[..frames]);
            dr[..frames].copy_from_slice(&r[..frames]);
        }
        let vals = self.vals;
        process(
            self.kind,
            &vals[..n],
            &mut self.st,
            mem,
            &mut l[..frames],
            &mut r[..frames],
            ctx,
        );
        if let Some(m) = blend {
            for i in 0..frames {
                l[i] = dl[i] + (l[i] - dl[i]) * m;
                r[i] = dr[i] + (r[i] - dr[i]) * m;
            }
        }
        for x in l[..frames].iter_mut().chain(r[..frames].iter_mut()) {
            if !x.is_finite() {
                *x = 0.0;
            }
        }
    }
}
