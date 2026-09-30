//! Filter and delay kernels: `lpf`/`hpf`/`bpf` (biquads), `ladder`
//! (four-pole with saturating feedback), `svf` (topology-preserving
//! state-variable), `delay` and `comb` over the node's delay memory.

use std::f32::consts::PI;

use crate::dsp::effects::prim::{tanh, DelayLine, Shape};

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// `res` (0..1) to a biquad Q.
fn q_of(res: f32) -> f32 {
    0.5 + 11.5 * res.clamp(0.0, 1.0).powi(2)
}

/// `lpf`/`hpf`/`bpf in cutoff res`; coefficients per block.
pub fn biquad(
    shape: Shape,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    st.bq[0].set(shape, ins[1].first(), q_of(ins[2].first()), 0.0, kx.sr);
    for (i, y) in out.iter_mut().enumerate() {
        *y = st.bq[0].run(ins[0].at(i));
    }
}

/// `ladder in cutoff res`: four one-pole stages with `tanh` feedback.
pub fn ladder(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let fc = ins[1].first().clamp(10.0, 0.45 * kx.sr);
    let g = 1.0 - (-2.0 * PI * fc / kx.sr).exp();
    let k = 3.9 * ins[2].first().clamp(0.0, 1.0);
    let s = &mut st.s;
    for (i, y) in out.iter_mut().enumerate() {
        let x = tanh(ins[0].at(i) - k * s[3]);
        s[0] += g * (x - s[0]);
        s[1] += g * (s[0] - s[1]);
        s[2] += g * (s[1] - s[2]);
        s[3] += g * (s[2] - s[3]);
        if !s[3].is_finite() {
            s[..4].fill(0.0);
        }
        *y = s[3] * (1.0 + 0.5 * k);
    }
}

/// `svf in cutoff res mode` (mode 0 lowpass, 1 highpass, 2 bandpass,
/// 3 notch).
pub fn svf(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let fc = ins[1].first().clamp(10.0, 0.45 * kx.sr);
    let g = (PI * fc / kx.sr).tan();
    let k = 2.0 - 1.96 * ins[2].first().clamp(0.0, 1.0);
    let a1 = 1.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mode = ins[3].first().max(0.0) as u32;
    let (mut ic1, mut ic2) = (st.s[0], st.s[1]);
    for (i, y) in out.iter_mut().enumerate() {
        let x = ins[0].at(i);
        let v3 = x - ic2;
        let v1 = a1 * ic1 + a2 * v3;
        let v2 = ic2 + a2 * ic1 + a3 * v3;
        ic1 = 2.0 * v1 - ic1;
        ic2 = 2.0 * v2 - ic2;
        if !ic1.is_finite() || !ic2.is_finite() {
            ic1 = 0.0;
            ic2 = 0.0;
        }
        let (low, band) = (v2, v1);
        let high = x - k * v1 - v2;
        *y = match mode {
            1 => high,
            2 => band,
            3 => low + high,
            _ => low,
        };
    }
    st.s[0] = ic1;
    st.s[1] = ic2;
}

fn line(st: &mut NodeState, mem: &[f32]) {
    if st.u[2] == 0 {
        let mut cursor = 0;
        st.dl = DelayLine::carve(&mut cursor, mem.len(), mem.len());
        st.u[2] = 1;
    }
}

/// `delay in time feedback`.
pub fn delay(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    line(st, mem);
    let fb = ins[2].first().clamp(0.0, 0.95);
    for (i, y) in out.iter_mut().enumerate() {
        let d = ins[1].at(i).max(0.0) * kx.sr;
        let wet = st.dl.read(mem, d);
        st.dl.write(mem, ins[0].at(i) + fb * wet);
        *y = wet;
    }
}

/// `comb in time feedback`: a feedback comb (input plus the delayed
/// output).
pub fn comb(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    line(st, mem);
    let fb = ins[2].first().clamp(-0.98, 0.98);
    for (i, y) in out.iter_mut().enumerate() {
        let d = ins[1].at(i).max(0.0) * kx.sr;
        let mode = ins[3].at(i).clamp(0.0, 2.0).round() as u32;
        let damping = ins[4].at(i).clamp(0.0, 1.0);
        let input = ins[0].at(i);
        st.s[0] = (1.0 - damping) * st.dl.read(mem, d) + damping * st.s[0];
        let tap = st.s[0];
        let (wet, write) = match mode {
            1 => (input + fb * tap, input),
            2 => {
                let wet = tap - fb * input;
                (wet, input + fb * wet)
            }
            _ => {
                let wet = input + fb * tap;
                (wet, wet)
            }
        };
        st.dl.write(mem, write);
        *y = wet;
    }
}
