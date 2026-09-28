//! Bounded stereo granular capture inspired by the MIT Clouds granular mode.
//! This is an adaptation: eight grains, host-rate capture, analytic windows,
//! and a compact reverb replace the source scheduling, 32 kHz conversion,
//! lookup tables and diffuser. See THIRD_PARTY_NOTICES.md.

use std::f32::consts::TAU;

use super::{FxCtx, FxState, ParamDef};

const GRAINS: usize = 8;
const STRIDE: usize = 7;
const TAIL: usize = GRAINS * STRIDE;
/// Minimum stereo capture duration required at installation.
pub const CAPTURE_SECONDS: f32 = 0.30;

pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("position", 0.5, 0.0, 1.0),
    ParamDef::new("size", 0.5, 0.0, 1.0),
    ParamDef::unit("pitch", 0.0, -24.0, 24.0, "st"),
    ParamDef::new("density", 0.5, 0.0, 1.0),
    ParamDef::new("texture", 0.5, 0.0, 1.0),
    ParamDef::new("stereo-spread", 0.0, 0.0, 1.0),
    ParamDef::new("feedback", 0.0, 0.0, 0.95),
    ParamDef::new("reverb", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 0.5, 0.0, 1.0),
    ParamDef::new("freeze", 0.0, 0.0, 1.0),
    ParamDef::new("trigger", 0.0, 0.0, 1.0),
    ParamDef::new("gate", 1.0, 0.0, 1.0),
    ParamDef::new("quality", 0.0, 0.0, 3.0),
];

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    let frames = (sr.clamp(8_000.0, 192_000.0) * CAPTURE_SECONDS).ceil() as usize;
    frames * 2 + TAIL
}

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    mem.fill(0.0);
}

#[derive(Clone, Copy)]
struct Grain {
    age: f32,
    duration: f32,
    head: f32,
    step: f32,
    pan: f32,
    active: f32,
    gain: f32,
}

impl Grain {
    fn read(m: &[f32]) -> Self {
        Self {
            age: m[0],
            duration: m[1],
            head: m[2],
            step: m[3],
            pan: m[4],
            active: m[5],
            gain: m[6],
        }
    }

    fn write(self, m: &mut [f32]) {
        m.copy_from_slice(&[
            self.age,
            self.duration,
            self.head,
            self.step,
            self.pan,
            self.active,
            self.gain,
        ]);
    }
}

pub(super) fn read_ring(buf: &[f32], head: f32) -> f32 {
    let p = head.rem_euclid(buf.len() as f32);
    let a = p.floor() as usize;
    let b = (a + 1) % buf.len();
    buf[a] + (buf[b] - buf[a]) * p.fract()
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    grains: &mut [f32],
    st: &mut FxState,
    p: &[f32],
    sr: f32,
    frames: usize,
    write: usize,
    valid: usize,
    ctx: &mut FxCtx<'_>,
) {
    let ratio = 2.0f32.powf(p[2] / 12.0);
    let duration = ((0.012 + p[1] * p[1] * 0.09) * sr).max(8.0);
    let need = (duration * ratio).ceil() as usize + 4;
    if valid <= need + 2 {
        ctx.stats.grains_skipped = ctx.stats.grains_skipped.saturating_add(1);
        return;
    }
    let max_offset = valid.saturating_sub(need + 1);
    let offset = need + 1 + (p[0] * max_offset as f32) as usize;
    for m in grains.chunks_exact_mut(STRIDE) {
        if m[5] <= 0.0 {
            let jitter = st.rng.bipolar();
            let pan = (jitter * p[5] * 0.5).clamp(-0.5, 0.5);
            let head = (write + frames - offset % frames) % frames;
            Grain {
                age: 0.0,
                duration,
                head: head as f32,
                step: ratio,
                pan,
                active: 1.0,
                gain: 0.65,
            }
            .write(m);
            ctx.stats.grains_spawned = ctx.stats.grains_spawned.saturating_add(1);
            return;
        }
    }
    ctx.stats.grains_skipped = ctx.stats.grains_skipped.saturating_add(1);
}

/// Process independent stereo capture/playback in the preallocated bus region.
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if mem.len() < mem_len(ctx.sr) || p.len() < PARAMS.len() {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let frames = (mem.len() - TAIL) / 2;
    let (capture, grains) = mem.split_at_mut(frames * 2);
    let (left, right) = capture.split_at_mut(frames);
    let mut write = st.s[0] as usize % frames;
    let mut valid = (st.s[1] as usize).min(frames);
    let mut phase = st.s[2];
    let mut prev_trigger = st.s[3] >= 0.5;
    let mut previous_l = st.s[4];
    let mut previous_r = st.s[5];
    let mut tail_l = st.s[6];
    let mut tail_r = st.s[7];
    let quality_phase = super::texture_quality::begin(p, st, l, r);
    let grain_frames = (0.012 + p[1] * p[1] * 0.09) * ctx.sr;
    let interval = (grain_frames / (0.5 + p[3] * 7.5)).max(1.0);
    let reverb_decay = (-1.0 / (ctx.sr * (0.025 + p[7] * 0.24))).exp();
    for (out_l, out_r) in l.iter_mut().zip(r.iter_mut()) {
        let input_l = *out_l;
        let input_r = *out_r;
        if p[9] < 0.5 {
            left[write] = (input_l + p[6] * previous_l).clamp(-4.0, 4.0);
            right[write] = (input_r + p[6] * previous_r).clamp(-4.0, 4.0);
            write = (write + 1) % frames;
            valid = (valid + 1).min(frames);
        }
        let triggered = p[10] >= 0.5 && !prev_trigger;
        prev_trigger = p[10] >= 0.5;
        if p[11] >= 0.5 {
            phase += 1.0;
            if triggered || (p[3] > 0.0 && phase >= interval) {
                spawn(grains, st, p, ctx.sr, frames, write, valid, ctx);
                phase = 0.0;
            }
        }
        let mut wet_l = 0.0;
        let mut wet_r = 0.0;
        let mut active: f32 = 0.0;
        for m in grains.chunks_exact_mut(STRIDE) {
            if m[5] <= 0.0 {
                continue;
            }
            let mut g = Grain::read(m);
            let x = g.age / g.duration;
            let triangle = (1.0 - (2.0 * x - 1.0).abs()).max(0.0);
            let hann = 0.5 - 0.5 * (TAU * x).cos();
            let rectangular = if x < 0.08 {
                x / 0.08
            } else if x > 0.92 {
                (1.0 - x) / 0.08
            } else {
                1.0
            };
            let window = if p[4] < 0.5 {
                rectangular + (triangle - rectangular) * p[4] * 2.0
            } else {
                triangle + (hann - triangle) * (p[4] - 0.5) * 2.0
            };
            let a = read_ring(left, g.head) * window * g.gain;
            let b = read_ring(right, g.head) * window * g.gain;
            wet_l += a * (1.0 - g.pan) + b * (-g.pan).max(0.0);
            wet_r += b * (1.0 + g.pan) + a * g.pan.max(0.0);
            active += 1.0;
            g.head = (g.head + g.step).rem_euclid(frames as f32);
            g.age += 1.0;
            if g.age >= g.duration {
                g.active = 0.0;
            }
            g.write(m);
        }
        if active > 0.0 {
            let normalize = active.sqrt().recip();
            wet_l *= normalize;
            wet_r *= normalize;
        }
        // A bounded, cross-coupled reverb tail; not the Clouds diffuser.
        tail_l = tail_l * reverb_decay + wet_r * (1.0 - reverb_decay);
        tail_r = tail_r * reverb_decay + wet_l * (1.0 - reverb_decay);
        let echo_l = wet_l + p[7] * (tail_l * 2.0 + tail_r) * 0.5;
        let echo_r = wet_r + p[7] * (tail_r * 2.0 + tail_l) * 0.5;
        previous_l = echo_l;
        previous_r = echo_r;
        *out_l = echo_l.clamp(-4.0, 4.0);
        *out_r = echo_r.clamp(-4.0, 4.0);
    }
    st.s[0] = write as f32;
    st.s[1] = valid as f32;
    st.s[2] = phase.rem_euclid((ctx.sr * 2.0).max(1.0));
    st.s[3] = if prev_trigger { 1.0 } else { 0.0 };
    st.s[4] = previous_l;
    st.s[5] = previous_r;
    st.s[6] = tail_l;
    st.s[7] = tail_r;
    super::texture_quality::end(quality_phase, st, l, r);
}
