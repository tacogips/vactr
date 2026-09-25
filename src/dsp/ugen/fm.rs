//! FM and phase-distortion kernels: `fm-op`, `fm-mod`, `phase-distortion`
//! (design-music section 4).

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// `fm-op freq ratio index mod`: a sine operator at `freq * ratio`, output
/// amplitude `index`, phase-modulated by `mod` (radians).
pub fn op(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        let f = ins[0].at(i) * ins[1].at(i);
        *y = ins[2].at(i) * (TAU * ph + ins[3].at(i)).sin();
        let dt = if f.is_finite() {
            (f / kx.sr).clamp(-0.49, 0.49)
        } else {
            0.0
        };
        ph += dt;
        ph -= ph.floor();
    }
    st.s[0] = ph;
}

/// `fm-mod in mod index`: modulates an already rendered carrier.
///
/// Approximation: the carrier's phase is recovered as `asin(in)` (exact
/// for a unit sine carrier within a half period, folded elsewhere) and
/// shifted by `index * mod`; the result keeps the carrier's amplitude
/// envelope `|in|` bounded to 1.
pub fn modulate(ins: &[Inp<'_>; MAX_PORTS], out: &mut [f32]) {
    for (i, y) in out.iter_mut().enumerate() {
        let x = ins[0].at(i);
        let x = if x.is_finite() {
            x.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        *y = (x.asin() + ins[2].at(i) * ins[1].at(i)).sin();
    }
}

/// `phase-distortion freq shape`: a cosine read through a two-segment
/// phase warp (Casio CZ style); `shape` 0 is a sine, 1 a near-saw.
pub fn phase_distortion(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        let d = 0.5 - 0.48 * ins[1].at(i).clamp(0.0, 1.0);
        let warped = if ph < d {
            0.5 * ph / d
        } else {
            0.5 + 0.5 * (ph - d) / (1.0 - d)
        };
        *y = -(TAU * warped).cos();
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
