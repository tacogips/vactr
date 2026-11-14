//! Oscillator kernels: `sin-osc`, `saw`, `pulse`, `tri`, `sub-osc`,
//! `white-noise` and the analog `vco` with unison, detune and drift
//! (design-music section 4). Band-limited edges use PolyBLEP.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// PolyBLEP residual at phase `t` for increment `dt`.
#[inline]
fn blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// The phase increment of `freq` Hz, bounded below Nyquist.
#[inline]
fn inc(freq: f32, sr: f32) -> f32 {
    if freq.is_finite() {
        (freq / sr).clamp(-0.49, 0.49)
    } else {
        0.0
    }
}

#[inline]
fn wrap(ph: f32) -> f32 {
    ph - ph.floor()
}

/// The waveforms of the `wave` control (`controls::WAVES` order).
#[inline]
fn wave_at(wave: usize, ph: f32, dt: f32, width: f32) -> f32 {
    match wave {
        0 => 2.0 * ph - 1.0 - blep(ph, dt),
        1 | 2 => {
            let w = if wave == 2 {
                0.5
            } else {
                width.clamp(0.02, 0.98)
            };
            let mut y = if ph < w { 1.0 } else { -1.0 };
            y += blep(ph, dt);
            y -= blep(wrap(ph - w + 1.0), dt);
            y
        }
        3 => 1.0 - 4.0 * (ph - 0.5).abs(),
        _ => (TAU * ph).sin(),
    }
}

/// `sin-osc freq`.
pub fn sine(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        *y = (TAU * ph).sin();
        ph = wrap(ph + inc(ins[0].at(i), kx.sr));
    }
    st.s[0] = ph;
}

/// `saw freq`.
pub fn saw(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        let dt = inc(ins[0].at(i), kx.sr).abs();
        *y = wave_at(0, ph, dt, 0.5);
        ph = wrap(ph + dt);
    }
    st.s[0] = ph;
}

/// `pulse freq width`.
pub fn pulse(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        let dt = inc(ins[0].at(i), kx.sr).abs();
        *y = wave_at(1, ph, dt, ins[1].at(i));
        ph = wrap(ph + dt);
    }
    st.s[0] = ph;
}

/// `tri freq`.
pub fn tri(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        *y = wave_at(3, ph, 0.0, 0.5);
        ph = wrap(ph + inc(ins[0].at(i), kx.sr).abs());
    }
    st.s[0] = ph;
}

/// `sub-osc freq`: a square one octave below.
pub fn sub(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut ph = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        let dt = inc(0.5 * ins[0].at(i), kx.sr).abs();
        *y = wave_at(2, ph, dt, 0.5);
        ph = wrap(ph + dt);
    }
    st.s[0] = ph;
}

/// `white-noise`, seeded per voice.
pub fn noise(st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0x51ED_2701).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    for y in out.iter_mut() {
        *y = rng.bipolar();
    }
    st.u[0] = rng.s;
}

/// `vco freq wave unison detune drift width`: up to `unison_max` detuned
/// voices (phases and drift walks live in `mem`), normalized by
/// `1/sqrt(n)`.
pub fn vco(
    unison_max: u8,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let max = usize::from(unison_max.max(1)).min(mem.len() / 2);
    if max == 0 {
        out.fill(0.0);
        return;
    }
    let (phases, drifts) = mem.split_at_mut(max);
    let mut rng = Rng::new(if st.u[1] == 0 {
        kx.seed ^ 0x0BAD_5EED
    } else {
        st.u[0]
    });
    if st.u[1] == 0 {
        for ph in phases.iter_mut() {
            *ph = rng.unit();
        }
        drifts[..max].fill(0.0);
        st.u[1] = 1;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let wave = ins[1].first().max(0.0) as usize;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (ins[2].first().round().max(1.0) as usize).min(max);
    let detune = ins[3].first().clamp(0.0, 1.0);
    let drift = ins[4].first().clamp(0.0, 0.1);
    let width = ins[5].first();
    #[allow(clippy::cast_precision_loss)]
    let norm = 1.0 / (n as f32).sqrt();
    // Per-block drift walk: a slow bounded random walk per unison voice.
    for d in drifts[..n].iter_mut() {
        *d = (*d + 0.1 * rng.bipolar()).clamp(-1.0, 1.0);
    }
    for (i, y) in out.iter_mut().enumerate() {
        let f = ins[0].at(i);
        let mut acc = 0.0;
        for k in 0..n {
            #[allow(clippy::cast_precision_loss)]
            let spread = if n > 1 {
                k as f32 / (n - 1) as f32 - 0.5
            } else {
                0.0
            };
            let ratio = 2f32.powf(2.0 * detune * spread / 12.0) * (1.0 + drift * drifts[k]);
            let dt = inc(f * ratio, kx.sr).abs();
            acc += wave_at(wave, phases[k], dt, width);
            phases[k] = wrap(phases[k] + dt);
        }
        *y = acc * norm;
    }
    st.u[0] = rng.s;
}
