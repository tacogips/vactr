//! Stereo two-window overlap-add stretch adaptation informed by MIT Clouds.
//! The source uses a bit-sign correlator, 32 kHz conversion and larger
//! windows. This kernel uses bounded float correlation and host-rate capture;
//! it imports no source tables or audio data. See THIRD_PARTY_NOTICES.md.

use super::texture::{self, read_ring};
use super::{FxCtx, FxState};

const WINDOWS: usize = 2;
const WINDOW_STRIDE: usize = 6;
const WINDOW_BASE: usize = 12;

#[derive(Clone, Copy)]
struct Window {
    age: f32,
    duration: f32,
    head: f32,
    step: f32,
    pan: f32,
    active: f32,
}

impl Window {
    fn read(s: &[f32], slot: usize) -> Self {
        let i = WINDOW_BASE + slot * WINDOW_STRIDE;
        Self {
            age: s[i],
            duration: s[i + 1],
            head: s[i + 2],
            step: s[i + 3],
            pan: s[i + 4],
            active: s[i + 5],
        }
    }

    fn write(self, s: &mut [f32], slot: usize) {
        let i = WINDOW_BASE + slot * WINDOW_STRIDE;
        s[i..i + WINDOW_STRIDE].copy_from_slice(&[
            self.age,
            self.duration,
            self.head,
            self.step,
            self.pan,
            self.active,
        ]);
    }
}

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    texture::mem_len(sr)
}

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    texture::init(st, mem);
}

fn candidate_score(left: &[f32], right: &[f32], other: Window, candidate: f32, ratio: f32) -> f32 {
    if other.active <= 0.0 {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut a2 = 1.0e-6;
    let mut b2 = 1.0e-6;
    for j in 0..16 {
        // `head` advances each rendered sample; `age` is only the window
        // envelope clock. Adding it here would skip the elapsed audio twice.
        let reference = other.head + other.step * j as f32;
        let target = candidate + ratio * j as f32;
        let a = read_ring(left, reference) + read_ring(right, reference);
        let b = read_ring(left, target) + read_ring(right, target);
        dot += a * b;
        a2 += a * a;
        b2 += b * b;
    }
    dot / (a2 * b2).sqrt()
}

#[allow(clippy::too_many_arguments)]
fn schedule(
    st: &mut FxState,
    left: &[f32],
    right: &[f32],
    p: &[f32],
    write: usize,
    valid: usize,
    sr: f32,
    ctx: &mut FxCtx<'_>,
) {
    let frames = left.len();
    let ratio = 2.0f32.powf(p[2] / 12.0);
    let duration = ((0.018 + p[1] * p[1] * 0.06) * sr)
        .min(frames as f32 / (ratio * 2.5))
        .max(16.0);
    let need = (duration * ratio).ceil() as usize + 20;
    if valid <= need + 4 {
        ctx.stats.grains_skipped = ctx.stats.grains_skipped.saturating_add(1);
        return;
    }
    let a = Window::read(&st.s, 0);
    let b = Window::read(&st.s, 1);
    let slot = if a.active <= 0.0 {
        0
    } else if b.active <= 0.0 {
        1
    } else if a.age >= b.age {
        0
    } else {
        1
    };
    let other = if slot == 0 { b } else { a };
    let range = valid - need - 2;
    let base_offset = need + 2 + (p[0] * range as f32) as usize;
    let search = ((p[3] * 0.018 * sr) as usize).min(range / 2);
    let mut best = (write + frames - base_offset % frames) % frames;
    let mut best_score = f32::NEG_INFINITY;
    // Small bounded correlation search. Density controls the search width
    // and therefore the amount of splice diffusion.
    for k in 0..9 {
        let shift = (k as isize - 4) * (search as isize / 4);
        let candidate_offset = (base_offset as isize + shift)
            .clamp((need + 2) as isize, valid.saturating_sub(1) as isize)
            as usize;
        let candidate = (write + frames - candidate_offset % frames) % frames;
        let score = candidate_score(left, right, other, candidate as f32, ratio)
            - (shift.unsigned_abs() as f32 / (search.max(1) as f32)) * 0.03;
        if score > best_score {
            best_score = score;
            best = candidate;
        }
    }
    // Diffusion also affects an initial splice with no alignment reference.
    if other.active <= 0.0 {
        let jitter = (st.rng.bipolar() * search as f32) as isize;
        let offset = (base_offset as isize + jitter)
            .clamp((need + 2) as isize, valid.saturating_sub(1) as isize)
            as usize;
        best = (write + frames - offset % frames) % frames;
    }
    Window {
        age: 0.0,
        duration,
        head: best as f32,
        step: ratio,
        pan: st.rng.bipolar() * p[5] * 0.5,
        active: 1.0,
    }
    .write(&mut st.s, slot);
    ctx.stats.grains_spawned = ctx.stats.grains_spawned.saturating_add(1);
}

/// Host-rate stereo capture with two independently aligned overlap windows.
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if mem.len() < mem_len(ctx.sr) || p.len() < texture::PARAMS.len() {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let frames = (mem.len() - 8 * 7) / 2;
    let (capture, _) = mem.split_at_mut(frames * 2);
    let (left, right) = capture.split_at_mut(frames);
    let mut write = st.s[0] as usize % frames;
    let mut valid = (st.s[1] as usize).min(frames);
    let mut previous_l = st.s[3];
    let mut previous_r = st.s[4];
    let mut tail_l = st.s[5];
    let mut tail_r = st.s[6];
    let mut filter_l = st.s[7];
    let mut filter_r = st.s[8];
    let mut prev_trigger = st.s[2] >= 0.5;
    let quality_phase = super::texture_quality::begin(p, st, l, r);
    let filter_rate = 0.025 + p[4] * p[4] * 0.94;
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
            let a = Window::read(&st.s, 0);
            let b = Window::read(&st.s, 1);
            if triggered
                || (a.active <= 0.0 && b.active <= 0.0)
                || (a.active > 0.0 && a.age >= a.duration * 0.5 && b.active <= 0.0)
                || (b.active > 0.0 && b.age >= b.duration * 0.5 && a.active <= 0.0)
            {
                schedule(st, left, right, p, write, valid, ctx.sr, ctx);
            }
        }
        let mut wet_l = 0.0;
        let mut wet_r = 0.0;
        for slot in 0..WINDOWS {
            let mut w = Window::read(&st.s, slot);
            if w.active <= 0.0 {
                continue;
            }
            let x = w.age / w.duration;
            let gain = (1.0 - (2.0 * x - 1.0).abs()).max(0.0);
            let a = read_ring(left, w.head) * gain;
            let b = read_ring(right, w.head) * gain;
            wet_l += a * (1.0 - w.pan) + b * (-w.pan).max(0.0);
            wet_r += b * (1.0 + w.pan) + a * w.pan.max(0.0);
            w.age += 1.0;
            w.head = (w.head + w.step).rem_euclid(frames as f32);
            if w.age >= w.duration {
                w.active = 0.0;
            }
            w.write(&mut st.s, slot);
        }
        filter_l += filter_rate * (wet_l - filter_l);
        filter_r += filter_rate * (wet_r - filter_r);
        tail_l = tail_l * reverb_decay + filter_r * (1.0 - reverb_decay);
        tail_r = tail_r * reverb_decay + filter_l * (1.0 - reverb_decay);
        previous_l = filter_l + p[7] * (tail_l * 2.0 + tail_r) * 0.5;
        previous_r = filter_r + p[7] * (tail_r * 2.0 + tail_l) * 0.5;
        *out_l = previous_l.clamp(-4.0, 4.0);
        *out_r = previous_r.clamp(-4.0, 4.0);
    }
    st.s[0] = write as f32;
    st.s[1] = valid as f32;
    st.s[2] = if prev_trigger { 1.0 } else { 0.0 };
    st.s[3] = previous_l;
    st.s[4] = previous_r;
    st.s[5] = tail_l;
    st.s[6] = tail_r;
    st.s[7] = filter_l;
    st.s[8] = filter_r;
    super::texture_quality::end(quality_phase, st, l, r);
}

#[cfg(test)]
mod tests {
    use super::{candidate_score, Window};

    #[test]
    fn splice_alignment_uses_the_current_playback_head() {
        let mut left = [0.0; 128];
        let mut right = [0.0; 128];
        left[53] = 1.0;
        left[57] = -0.7;
        left[63] = 0.5;
        right[59] = 0.8;
        right[65] = -0.4;
        let advanced = Window {
            age: 20.0,
            duration: 80.0,
            head: 53.0,
            step: 1.0,
            pan: 0.0,
            active: 1.0,
        };
        let at_head = candidate_score(&left, &right, advanced, 53.0, 1.0);
        let double_advanced = candidate_score(&left, &right, advanced, 73.0, 1.0);
        assert!(
            at_head > 0.99,
            "the live transient segment aligns: {at_head}"
        );
        assert!(
            double_advanced < 0.1,
            "elapsed window age must not shift the source twice: {double_advanced}"
        );
    }
}
