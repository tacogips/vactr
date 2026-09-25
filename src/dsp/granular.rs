//! The granular engine (design 12.6, design-music section 6).
//!
//! One engine, two faces: the `granular` ugen (a static sample or table
//! source, mono, inside a voice) and the `granulate` effect (a live
//! capture of its input, stereo, on a bus). Both keep a preallocated grain
//! pool of `ceil(max_grain_density * max_grain_size)` slots in their delay
//! memory; the effect face also owns a circular capture buffer of
//! `max_capture_seconds` (both clamped to the memory the unit was given).
//!
//! Onsets come from a phase accumulator advanced per sample by
//! `density / sr`; position spray, pitch spray, reverse and stereo spray
//! draw from the unit's seeded stream, so onset count and timing are
//! deterministic given controls, seed and block layout. Envelopes are
//! `:hann :tri :trapezoid :expo` (closed forms of the tables).
//!
//! Live validity. A grain is kept as its read position in the ring and its
//! depth `d` behind the write head (the newest sample is at depth 1). Each
//! block is captured before grains read it, so a grain's depth drops by
//! `step` per sample and rises by the block length at each capture. A spawn
//! is admitted only when its depth stays in `[2, U]` for its whole life,
//! where `U` is the captured extent (`filled`, at most the capacity) and,
//! while live, the one-block guard keeps the in-block motion clear of the
//! head and of the next overwrite. At an unfreeze every active grain is
//! re-checked for its remaining life under the live rule; a failing grain
//! is short-gated (3 ms fade of its last output, no further reads) before
//! capture resumes. Violations and a full pool skip the spawn and count;
//! density, size and depth above the tier caps are clamped and counted.

use std::f32::consts::TAU;

use crate::dsp::arena::SampleView;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::prim::Rng;
use crate::dsp::effects::{FxCtx, FxState, FxStats, ParamDef};
use crate::dsp::graph::GranSrc;
use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

/// The `granulate` parameters (the ugen reads the same names as ports).
pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("position", 0.0, 0.0, 1.0),
    ParamDef::unit("density", 24.0, 0.0, 100_000.0, "Hz"),
    ParamDef::unit("size", 0.08, 0.001, 60.0, "s"),
    ParamDef::new("spray", 0.0, 0.0, 1.0),
    ParamDef::new("pitch", 0.0, -48.0, 48.0),
    ParamDef::new("pitch-spray", 0.0, 0.0, 1.0),
    ParamDef::new("envelope", 0.0, 0.0, 3.0),
    ParamDef::new("reverse", 0.0, 0.0, 1.0),
    ParamDef::new("freeze", 0.0, 0.0, 1.0),
    ParamDef::new("stereo-spray", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

/// Floats per pool slot.
pub const GRAIN_FLOATS: usize = 12;
/// The short gate, seconds (12.8.9).
pub const SHORT_GATE: f32 = 0.003;

// Slot layout.
const ACTIVE: usize = 0;
const POS: usize = 1;
const STEP: usize = 2;
const AGE: usize = 3;
const LEN: usize = 4;
const ENV: usize = 5;
const PAN: usize = 6;
const GATE: usize = 7;
const DEPTH: usize = 8;
const LAST: usize = 9;
const GATE_LEN: usize = 10;

// Effect state (`FxState::s`).
const ACC: usize = 0;
const HEAD: usize = 1;
const FILLED: usize = 2;
const FROZEN: usize = 3;
const CAP: usize = 4;
const POOL: usize = 5;

/// Grain slots for a tier: `ceil(max_grain_density * max_grain_size)`.
#[must_use]
pub fn pool_slots(caps: &CapabilitySet) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (caps.max_grain_density * caps.max_grain_size)
        .ceil()
        .max(1.0) as usize;
    n
}

/// Capture-buffer samples for a tier.
#[must_use]
pub fn capture_len(sr: f32, caps: &CapabilitySet) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (caps.max_capture_seconds * sr).ceil().max(0.0) as usize;
    n
}

/// Preferred memory of a `granular` ugen node: its grain pool.
#[must_use]
pub fn ugen_mem_len(_sr: f32, caps: &CapabilitySet) -> usize {
    pool_slots(caps) * GRAIN_FLOATS
}

/// Preferred memory of a `granulate` unit: pool plus capture buffer.
#[must_use]
pub fn effect_mem_len(sr: f32, caps: &CapabilitySet) -> usize {
    pool_slots(caps) * GRAIN_FLOATS + capture_len(sr, caps)
}

/// Splits a unit's memory into its pool and capture buffer.
fn split(mem: &mut [f32], sr: f32, caps: &CapabilitySet) -> (usize, usize) {
    let slots = pool_slots(caps).min(mem.len() / GRAIN_FLOATS);
    let cap = capture_len(sr, caps).min(mem.len() - slots * GRAIN_FLOATS);
    (slots, cap)
}

/// Sets up a `granulate` unit (memory already zeroed).
pub fn effect_init(st: &mut FxState, mem: &mut [f32], sr: f32, caps: &CapabilitySet) {
    let (slots, cap) = split(mem, sr, caps);
    #[allow(clippy::cast_precision_loss)]
    {
        st.s[POOL] = slots as f32;
        st.s[CAP] = cap as f32;
    }
    st.rng = Rng::new(0x6A09_E667);
}

/// The controls of one block, clamped to the tier caps.
#[derive(Clone, Copy, Debug)]
struct Ctl {
    position: f32,
    density: f32,
    size: f32,
    spray: f32,
    pitch: f32,
    pitch_spray: f32,
    env: f32,
    reverse: f32,
    freeze: bool,
    stereo: f32,
}

impl Ctl {
    fn read(get: impl Fn(usize) -> f32, caps: &CapabilitySet, stats: &mut FxStats) -> Self {
        let mut density = get(1).max(0.0);
        if density > caps.max_grain_density {
            density = caps.max_grain_density;
            stats.clamped += 1;
        }
        let mut size = get(2).max(0.001);
        if size > caps.max_grain_size {
            size = caps.max_grain_size;
            stats.clamped += 1;
        }
        Self {
            position: get(0).clamp(0.0, 1.0),
            density,
            size,
            spray: get(3).clamp(0.0, 1.0),
            pitch: get(4).clamp(-48.0, 48.0),
            pitch_spray: get(5).clamp(0.0, 1.0),
            env: get(6).clamp(0.0, 3.0),
            reverse: get(7).clamp(0.0, 1.0),
            freeze: get(8) >= 0.5,
            stereo: get(9).clamp(0.0, 1.0),
        }
    }
}

/// The envelope value at `x` in `0..1`.
#[inline]
fn envelope(kind: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    match kind as u32 {
        1 => 1.0 - (2.0 * x - 1.0).abs(),
        2 => (x / 0.1).min(1.0).min((1.0 - x) / 0.1).max(0.0),
        3 => (-5.0 * x).exp() * (x / 0.01).min(1.0),
        _ => 0.5 - 0.5 * (TAU * x).cos(),
    }
}

/// Where grains read.
enum Source<'a> {
    Static(SampleView<'a>),
    Live {
        ring: &'a [f32],
        /// The ring index the next sample is written to.
        head: usize,
        filled: usize,
        frozen: bool,
        guard: usize,
    },
}

impl Source<'_> {
    /// Interpolated read at ring/frame position `p`.
    fn read(&self, p: f32) -> f32 {
        match self {
            Source::Static(v) => {
                let ch = usize::from(v.channels.max(1));
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let i = p.max(0.0) as usize;
                #[allow(clippy::cast_precision_loss)]
                let f = p - i as f32;
                let a = v.data.get(i * ch).copied().unwrap_or(0.0);
                let b = v.data.get((i + 1) * ch).copied().unwrap_or(a);
                a + (b - a) * f
            }
            Source::Live { ring, .. } => {
                let n = ring.len();
                if n == 0 {
                    return 0.0;
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let i = (p.max(0.0) as usize) % n;
                #[allow(clippy::cast_precision_loss)]
                let f = p - p.floor();
                let a = ring[i];
                let b = ring[(i + 1) % n];
                a + (b - a) * f
            }
        }
    }

    /// The admissible depth range `[lo, hi]` of a live grain that reads
    /// `len` more samples at `step` (see the module doc). Live, a block is
    /// captured before grains read it: depth dips by up to `guard * step`
    /// inside a block and a grain spawned mid-block sits up to `guard`
    /// deeper than the continuous line, hence both guards.
    fn window(&self, len: f32, step: f32, live: bool) -> Option<(f32, f32)> {
        let Source::Live {
            ring,
            filled,
            guard,
            ..
        } = self
        else {
            return None;
        };
        #[allow(clippy::cast_precision_loss)]
        let (top, guard) = ((*filled).min(ring.len()) as f32, *guard as f32);
        let (drift, lo_guard, hi_guard) = if live {
            (len * (1.0 - step), guard * step.max(0.0), guard)
        } else {
            (-len * step, 0.0, 0.0)
        };
        // Interpolation reads one sample past the position: keep depth >= 2.
        let lo = 2.0 + lo_guard - drift.min(0.0);
        let hi = top - hi_guard - drift.max(0.0);
        (lo <= hi).then_some((lo, hi))
    }
}

/// Runs the grain engine for one block: spawns and renders into `l` (and
/// `r` when stereo).
#[allow(clippy::too_many_arguments)]
fn run(
    c: &Ctl,
    src: &Source<'_>,
    pool: &mut [f32],
    acc: &mut f32,
    rng: &mut Rng,
    sr: f32,
    l: &mut [f32],
    mut r: Option<&mut [f32]>,
    stats: &mut FxStats,
) {
    let n = l.len();
    l.fill(0.0);
    if let Some(r) = r.as_deref_mut() {
        r.fill(0.0);
    }
    // Onsets (sample-accurate within the block).
    for i in 0..n {
        *acc += c.density / sr;
        if *acc >= 1.0 {
            *acc -= 1.0;
            if !spawn(c, src, pool, rng, sr, i, stats) {
                stats.grains_skipped += 1;
            }
        }
    }
    // Render every active grain from its start frame.
    for g in pool.chunks_exact_mut(GRAIN_FLOATS) {
        if g[ACTIVE] == 0.0 {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = (g[ACTIVE] as usize).saturating_sub(1).min(n);
        g[ACTIVE] = 1.0;
        let (gl, gr) = if r.is_some() {
            crate::dsp::effects::prim::pan_gains(0.5 + 0.5 * g[PAN])
        } else {
            (1.0, 0.0)
        };
        for i in start..n {
            let y = if g[GATE] > 0.0 {
                g[GATE] -= 1.0;
                g[LAST] * g[GATE] / g[GATE_LEN].max(1.0)
            } else {
                if let Source::Live { filled, ring, .. } = src {
                    let d = g[DEPTH];
                    #[allow(clippy::cast_precision_loss)]
                    let top = (*filled).min(ring.len()) as f32;
                    if d < 1.0 || d > top {
                        stats.stale_reads += 1;
                    }
                }
                let v = src.read(g[POS]) * envelope(g[ENV], g[AGE] / g[LEN]);
                g[LAST] = v;
                g[POS] += g[STEP];
                if let Source::Live { ring, .. } = src {
                    #[allow(clippy::cast_precision_loss)]
                    let len = ring.len() as f32;
                    g[POS] = g[POS].rem_euclid(len.max(1.0));
                }
                g[DEPTH] -= g[STEP];
                g[AGE] += 1.0;
                v
            };
            l[i] += y * gl;
            if let Some(r) = r.as_deref_mut() {
                r[i] += y * gr;
            }
            let over = g[AGE] >= g[LEN];
            let gated_out = g[GATE_LEN] > 0.0 && g[GATE] <= 0.0;
            if over || gated_out {
                g.fill(0.0);
                break;
            }
        }
    }
}

/// Spawns one grain at frame `at`; false when skipped.
#[allow(clippy::too_many_arguments)]
fn spawn(
    c: &Ctl,
    src: &Source<'_>,
    pool: &mut [f32],
    rng: &mut Rng,
    sr: f32,
    at: usize,
    stats: &mut FxStats,
) -> bool {
    let Some(g) = pool
        .chunks_exact_mut(GRAIN_FLOATS)
        .find(|g| g[ACTIVE] == 0.0)
    else {
        return false;
    };
    let semis = c.pitch + c.pitch_spray * 12.0 * rng.bipolar();
    let mut step = 2f32.powf(semis / 12.0);
    let rev = rng.unit() < c.reverse;
    if rev {
        step = -step;
    }
    let jitter = c.spray * rng.bipolar();
    let pan = c.stereo * rng.bipolar();
    let mut len = (c.size * sr).max(1.0);
    let (pos, depth) = match src {
        Source::Static(v) => {
            #[allow(clippy::cast_precision_loss)]
            let frames = v.frames() as f32;
            if frames < 2.0 {
                return false;
            }
            let p = ((c.position + jitter * 0.5) * frames).clamp(0.0, frames - 1.0);
            (p, 0.0)
        }
        Source::Live {
            ring,
            head,
            frozen,
            guard,
            ..
        } => {
            // A size that cannot fit the capture window is clamped.
            #[allow(clippy::cast_precision_loss)]
            let room = ring.len().saturating_sub(*guard + 2) as f32 / (1.0 + step.abs());
            if len > room {
                if room < 1.0 {
                    return false;
                }
                len = room;
                stats.clamped += 1;
            }
            let Some((lo, hi)) = src.window(len, step, !*frozen) else {
                return false;
            };
            let d = (lo + (c.position + jitter * 0.5).clamp(0.0, 1.0) * (hi - lo)).clamp(lo, hi);
            // The sample at depth `d` sits `d` slots before the head.
            #[allow(clippy::cast_precision_loss)]
            let p = (*head as f32 - d).rem_euclid(ring.len() as f32);
            (p, d)
        }
    };
    g.fill(0.0);
    #[allow(clippy::cast_precision_loss)]
    {
        g[ACTIVE] = (at + 1) as f32;
    }
    g[POS] = pos;
    g[STEP] = step;
    g[LEN] = len;
    g[ENV] = c.env;
    g[PAN] = pan;
    g[DEPTH] = depth;
    stats.grains_spawned += 1;
    true
}

/// Re-checks active grains for their remaining life under the live rule
/// at an unfreeze; failing grains are short-gated.
fn unfreeze(src: &Source<'_>, pool: &mut [f32], sr: f32, stats: &mut FxStats) {
    for g in pool.chunks_exact_mut(GRAIN_FLOATS) {
        if g[ACTIVE] == 0.0 || g[GATE_LEN] > 0.0 {
            continue;
        }
        let remaining = (g[LEN] - g[AGE]).max(0.0);
        let ok = src
            .window(remaining, g[STEP], true)
            .is_some_and(|(lo, hi)| g[DEPTH] >= lo && g[DEPTH] <= hi);
        if !ok {
            let n = (SHORT_GATE * sr).max(1.0);
            g[GATE] = n;
            g[GATE_LEN] = n;
            stats.grains_gated += 1;
        }
    }
}

/// Processes a `granulate` block (stereo in, fully wet stereo out).
pub fn effect_process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let c = Ctl::read(
        |i| p.get(i).copied().unwrap_or(PARAMS[i].default),
        ctx.caps,
        ctx.stats,
    );
    let n = l.len().min(r.len());
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (slots, cap) = (st.s[POOL] as usize, st.s[CAP] as usize);
    let (pool, rest) = mem.split_at_mut((slots * GRAIN_FLOATS).min(mem.len()));
    let cap = cap.min(rest.len());
    let ring = &mut rest[..cap];
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (mut head, mut filled) = (st.s[HEAD] as usize, st.s[FILLED] as usize);
    let was_frozen = st.s[FROZEN] != 0.0;
    if was_frozen && !c.freeze {
        let src = Source::Live {
            ring,
            head,
            filled,
            frozen: false,
            guard: n,
        };
        unfreeze(&src, pool, ctx.sr, ctx.stats);
    }
    // Capture the block first (the one-block guard covers it).
    if !c.freeze && !ring.is_empty() {
        for i in 0..n {
            ring[head] = 0.5 * (l[i] + r[i]);
            head = (head + 1) % ring.len();
        }
        filled = (filled + n).min(ring.len());
        #[allow(clippy::cast_precision_loss)]
        for g in pool.chunks_exact_mut(GRAIN_FLOATS) {
            if g[ACTIVE] != 0.0 {
                g[DEPTH] += n as f32;
            }
        }
    }
    let src = Source::Live {
        ring,
        head,
        filled,
        frozen: c.freeze,
        guard: n,
    };
    let mut acc = st.s[ACC];
    run(
        &c,
        &src,
        pool,
        &mut acc,
        &mut st.rng,
        ctx.sr,
        &mut l[..n],
        Some(&mut r[..n]),
        ctx.stats,
    );
    #[allow(clippy::cast_precision_loss)]
    {
        st.s[ACC] = acc;
        st.s[HEAD] = head as f32;
        st.s[FILLED] = filled as f32;
        st.s[FROZEN] = if c.freeze { 1.0 } else { 0.0 };
    }
}

/// Processes a `granular` ugen block from a static source (mono). A `:bus`
/// source has no input inside a voice and renders silence.
pub fn ugen_process(
    src: GranSrc,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &mut Kx<'_>,
) {
    let resource = match src {
        GranSrc::Sample(b) => kx.bank.unwrap_or(b.get()),
        GranSrc::Table(t) => t.get(),
        GranSrc::Bus => {
            out.fill(0.0);
            return;
        }
    };
    let Some(view) = kx.store.get(resource) else {
        out.fill(0.0);
        return;
    };
    let mut c = Ctl::read(|i| ins[i].first(), kx.caps, kx.stats);
    // Freeze on a static source pins the position (s[1]).
    if c.freeze {
        if st.s[2] == 0.0 {
            st.s[1] = c.position;
        }
        c.position = st.s[1];
    }
    st.s[2] = if c.freeze { 1.0 } else { 0.0 };
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0x3C6E_F372).s;
        st.u[1] = 1;
    }
    let slots = pool_slots(kx.caps).min(mem.len() / GRAIN_FLOATS);
    let pool = &mut mem[..slots * GRAIN_FLOATS];
    let mut rng = Rng::new(st.u[0]);
    let mut acc = st.s[0];
    let source = Source::Static(view);
    run(
        &c, &source, pool, &mut acc, &mut rng, kx.sr, out, None, kx.stats,
    );
    st.s[0] = acc;
    st.u[0] = rng.s;
}
