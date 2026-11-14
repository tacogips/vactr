//! The `sample-play` kernel (design 12.1, TASK-008 criterion 3).
//!
//! A sample event plays the region `[begin, end)` (fractions of the
//! sample) at `speed` (negative plays backwards from `end`), looping inside
//! the region when `loop` is on. These four controls plus the event's
//! `bank` are everything the audio side reads: `chop`, `striate`, `slice`,
//! `splice`, `loop-at` and `fit` are scheduler-side rewrites into them.
//! Every read goes through `get`, so no region, speed or loop setting can
//! index outside the sample.

use crate::dsp::graph::BankRef;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// The frame range `[start, stop)` of a `begin`/`end` region of a sample
/// with `frames` frames; always within `0..=frames` and `start <= stop`.
#[must_use]
pub fn region(frames: usize, begin: f32, end: f32) -> (usize, usize) {
    #[allow(clippy::cast_precision_loss)]
    let n = frames as f32;
    let b = if begin.is_finite() {
        begin.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let e = if end.is_finite() {
        end.clamp(0.0, 1.0)
    } else {
        1.0
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let start = ((b * n).floor() as usize).min(frames);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let stop = ((e * n).ceil() as usize).min(frames);
    (start, stop.max(start))
}

/// `sample-play speed begin end loop`; the resource is the event's `bank`
/// control when present, else the node's bank.
pub fn play(
    bank: BankRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    play_worker(bank, ins, st, out, None, kx);
}

/// Plays the legacy mono mix and both stereo channels from one cursor.
pub fn play_stereo(
    bank: BankRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mono: &mut [f32],
    left: &mut [f32],
    right: &mut [f32],
    kx: &Kx<'_>,
) {
    play_worker(bank, ins, st, mono, Some((left, right)), kx);
}

fn play_worker(
    bank: BankRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    mut stereo_out: Option<(&mut [f32], &mut [f32])>,
    kx: &Kx<'_>,
) {
    let resource = kx.bank.unwrap_or(bank.get());
    let Some(view) = kx.store.get(resource) else {
        out.fill(0.0);
        if let Some((left, right)) = stereo_out.as_mut() {
            left.fill(0.0);
            right.fill(0.0);
        }
        st.finish();
        return;
    };
    let ch = usize::from(view.channels.max(1));
    let frames = view.frames();
    let (start, stop) = region(frames, ins[1].first(), ins[2].first());
    let looping = ins[3].first() >= 0.5;
    #[allow(clippy::cast_precision_loss)]
    let rate = view.rate as f32 / kx.sr;
    let speed = ins[0].first();
    let step = if speed.is_finite() { speed * rate } else { 0.0 };
    if stop <= start || step == 0.0 && st.u[1] != 0 && st.u[0] as usize >= stop {
        out.fill(0.0);
        if let Some((left, right)) = stereo_out.as_mut() {
            left.fill(0.0);
            right.fill(0.0);
        }
        st.finish();
        return;
    }
    // Position = integer frame (u[0]) + fraction (s[0]).
    if st.u[1] == 0 {
        let first = if step < 0.0 { stop - 1 } else { start };
        st.u[0] = u32::try_from(first).unwrap_or(u32::MAX);
        st.s[0] = 0.0;
        st.u[1] = 1;
    }
    let frame = |i: usize| -> f32 {
        let mut acc = 0.0;
        for c in 0..ch {
            acc += view.data.get(i * ch + c).copied().unwrap_or(0.0);
        }
        #[allow(clippy::cast_precision_loss)]
        let s = acc / ch as f32;
        s
    };
    let channel_frame = |i: usize, channel: usize| -> f32 {
        view.data.get(i * ch + channel).copied().unwrap_or(0.0)
    };
    #[allow(clippy::cast_possible_wrap)]
    let (lo, hi) = (start as i64, stop as i64);
    let mut idx = i64::from(st.u[0]);
    let mut frac = st.s[0];
    for (frame_index, y) in out.iter_mut().enumerate() {
        if st.done() {
            *y = 0.0;
            if let Some((left, right)) = stereo_out.as_mut() {
                left[frame_index] = 0.0;
                right[frame_index] = 0.0;
            }
            continue;
        }
        if idx < lo || idx >= hi {
            if looping {
                let span = hi - lo;
                idx = lo + (idx - lo).rem_euclid(span);
            } else {
                st.finish();
                *y = 0.0;
                if let Some((left, right)) = stereo_out.as_mut() {
                    left[frame_index] = 0.0;
                    right[frame_index] = 0.0;
                }
                continue;
            }
        }
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let i = idx as usize;
        let a = frame(i);
        *y = if frac == 0.0 {
            a
        } else {
            let next = if i + 1 < stop { i + 1 } else { i };
            a + (frame(next) - a) * frac
        };
        if let Some((left, right)) = stereo_out.as_mut() {
            let left_a = channel_frame(i, 0);
            let right_channel = usize::from(ch > 1);
            let right_a = channel_frame(i, right_channel);
            if frac == 0.0 {
                left[frame_index] = left_a;
                right[frame_index] = right_a;
            } else {
                let next = if i + 1 < stop { i + 1 } else { i };
                let left_b = channel_frame(next, 0);
                let right_b = channel_frame(next, right_channel);
                left[frame_index] = left_a + (left_b - left_a) * frac;
                right[frame_index] = right_a + (right_b - right_a) * frac;
            }
        }
        let adv = frac + step;
        let whole = adv.floor();
        frac = adv - whole;
        #[allow(clippy::cast_possible_truncation)]
        {
            idx += whole as i64;
        }
    }
    if !looping && (idx < lo || idx >= hi) {
        st.finish();
    }
    st.u[0] = u32::try_from(idx.max(0)).unwrap_or(u32::MAX);
    st.s[0] = frac;
}
