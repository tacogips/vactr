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

pub use music_things_fx::analyzer;
pub mod catalog;
pub use music_things_dsp::prim;
pub use music_things_fx::creative_delay;
pub use music_things_fx::creative_reverb;
pub use music_things_fx::cross_mod;
pub use music_things_fx::delay;
pub use music_things_fx::dynamic_control;
pub use music_things_fx::dynamics;
pub use music_things_fx::elements_bank;
pub use music_things_fx::eq;
pub use music_things_fx::lofi;
pub use music_things_fx::modulation;
pub use music_things_fx::music_filters;
pub use music_things_fx::musicdsp;
mod fx_adapter;
pub use fx_adapter::grain_limits;
pub use music_things_fx::quad_mixer;
pub use music_things_fx::resonant_bank;
pub use music_things_fx::resonator;
pub use music_things_fx::resonator_extra;
pub use music_things_fx::restoration;
pub use music_things_fx::reverb;
pub use music_things_fx::saturation;
pub use music_things_fx::shift_pair;
pub use music_things_fx::spatial;
pub use music_things_fx::stream_cv;
pub use music_things_fx::stream_dynamics;
pub use music_things_fx::texture;
pub use music_things_fx::texture_loop;
use music_things_fx::texture_quality;
pub use music_things_fx::texture_spectral;
pub use music_things_fx::texture_stretch;
pub use music_things_fx::utility;

use crate::dsp::arena::SampleStore;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellRead;
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

pub use music_things_fx::{FxState, FxStats, ParamDef};

/// Reserved codec parameter carrying a constant bus ID, outside the public catalog.
pub const SIDECHAIN_BUS_CTL: CtlId = CtlId::new(0x7ffe);
/// Largest integer exactly representable by the existing f32 control transport.
pub const MAX_SIDECHAIN_BUS: u32 = 1 << 24;

/// The first effect-local parameter id.
pub const EFFECT_PARAM_BASE: u16 = 0x4000;
/// The most parameters one effect kind has.
pub const MAX_FX_PARAMS: usize = 28;

/// Decodes the reserved, exact constant key ID without truncation or clamping.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn sidechain_bus(ctl: Ctl) -> Option<BusId> {
    match ctl {
        Ctl::Const(value)
            if value.is_finite()
                && value >= 1.0
                && value <= MAX_SIDECHAIN_BUS as f32
                && value.fract() == 0.0 =>
        {
            Some(BusId::new(value as u32))
        }
        _ => None,
    }
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

/// The named parameters of a kind, in catalog order.
#[must_use]
pub fn params(kind: EffectKind) -> &'static [ParamDef] {
    music_things_fx::params(kind.into())
}

/// The preferred delay memory of a kind, in floats.
#[must_use]
pub fn mem_len(kind: EffectKind, sr: f32, caps: &CapabilitySet) -> usize {
    music_things_fx::mem_len(kind.into(), sr, &grain_limits(caps))
}

/// Kinds whose declared memory is part of their algorithm.
#[must_use]
pub fn requires_full_memory(kind: EffectKind) -> bool {
    music_things_fx::requires_full_memory(kind.into())
}

/// Sets up a zeroed unit over delay memory.
pub fn init(kind: EffectKind, st: &mut FxState, mem: &mut [f32], sr: f32, caps: &CapabilitySet) {
    music_things_fx::init(kind.into(), st, mem, sr, &grain_limits(caps));
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
    music_things_fx::process(kind.into(), p, st, mem, l, r, ctx);
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
    music_things_fx::mix_index(kind.into())
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
    sidechain: Option<BusId>,
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
            sidechain: None,
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
        self.sidechain = None;
        self.n = u8::try_from(defs.len().min(MAX_FX_PARAMS)).unwrap_or(0);
        for (i, d) in defs.iter().take(MAX_FX_PARAMS).enumerate() {
            self.src[i] = Ctl::Const(d.default);
        }
        for &(id, ctl) in given {
            if kind == EffectKind::Compressor && id == SIDECHAIN_BUS_CTL {
                self.sidechain = sidechain_bus(ctl);
                continue;
            }
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

    /// Clears algorithm state without changing configured parameter targets.
    pub fn clear_state(&mut self, mem: &mut [f32], sr: f32, caps: &CapabilitySet) {
        self.st = FxState::default();
        init(self.kind, &mut self.st, mem, sr, caps);
        self.vals = self.target;
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

    /// Evaluator-resolved key bus; independent of parameter smoothing.
    #[must_use]
    pub const fn sidechain(&self) -> Option<BusId> {
        self.sidechain
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
        self.run_with_key(mem, l, r, dry, ctx, None);
    }

    /// Processes a bus compressor with an optional linked peak key.
    /// A selected but missing bus supplies an all-zero key, never self detection.
    #[allow(clippy::too_many_arguments)]
    pub fn run_with_key(
        &mut self,
        mem: &mut [f32],
        l: &mut [f32],
        r: &mut [f32],
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
        key: Option<&[f32]>,
    ) {
        let n = usize::from(self.n);
        for i in 0..n {
            let (v, t) = (self.vals[i], self.target[i]);
            let next = v + (t - v) * 0.5;
            self.vals[i] = if (next - t).abs() < 1.0e-6 { t } else { next };
        }
        let frames = l.len().min(r.len());
        let mix = mix_index(self.kind).map(|i| self.vals[i].clamp(0.0, 1.0));
        if texture_quality::mono(self.kind, &self.vals[..n]) {
            for (left, right) in l[..frames].iter_mut().zip(r[..frames].iter_mut()) {
                let mid = (*left + *right) * 0.5;
                *left = mid;
                *right = mid;
            }
        }
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
        if self.kind == EffectKind::Compressor {
            dynamics::compress(
                &vals[..n],
                &mut self.st,
                &mut l[..frames],
                &mut r[..frames],
                ctx.sr,
                key.or_else(|| self.sidechain.map(|_| &[][..])),
            );
        } else {
            process(
                self.kind,
                &vals[..n],
                &mut self.st,
                mem,
                &mut l[..frames],
                &mut r[..frames],
                ctx,
            );
        }
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
