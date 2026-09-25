//! The `wavetable` kernel (design 12.4): an installed table resource is a
//! sequence of single-cycle frames of `FRAME` samples (channel 0); a table
//! shorter than one frame is one frame. `position` (0..1) morphs linearly
//! between neighbouring frames.

use crate::dsp::graph::TableRef;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Samples per wavetable frame.
pub const FRAME: usize = 2048;

/// `wavetable freq position`; the resource is the event's `bank` control
/// when present, else the node's table (R2d, matching `sample.rs` and
/// `granular.rs`); a missing table renders silence.
pub fn render(
    table: TableRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let resource = kx.bank.unwrap_or(table.get());
    let Some(view) = kx.store.get(resource) else {
        out.fill(0.0);
        return;
    };
    let ch = usize::from(view.channels.max(1));
    let total = view.frames();
    if total == 0 {
        out.fill(0.0);
        return;
    }
    let frame_len = FRAME.min(total);
    let frames = (total / frame_len).max(1);
    let at = |i: usize| view.data.get(i * ch).copied().unwrap_or(0.0);
    #[allow(clippy::cast_precision_loss)]
    let pos = ins[1].first().clamp(0.0, 1.0) * (frames - 1) as f32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let f0 = (pos as usize).min(frames - 1);
    let f1 = (f0 + 1).min(frames - 1);
    #[allow(clippy::cast_precision_loss)]
    let morph = pos - f0 as f32;
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let x = ph * frame_len as f32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let j = (x as usize).min(frame_len - 1);
        let j1 = (j + 1) % frame_len;
        #[allow(clippy::cast_precision_loss)]
        let frac = x - j as f32;
        let read = |f: usize| {
            let a = at(f * frame_len + j);
            let b = at(f * frame_len + j1);
            a + (b - a) * frac
        };
        *y = read(f0) + (read(f1) - read(f0)) * morph;
        let f = ins[0].at(i);
        let dt = if f.is_finite() {
            (f / kx.sr).clamp(0.0, 0.49)
        } else {
            0.0
        };
        ph += dt;
        ph -= ph.floor();
    }
    st.s[0] = ph;
}
