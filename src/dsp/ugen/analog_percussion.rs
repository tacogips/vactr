//! Analytic resonant percussion inspired by the control roles and exciter
//! families in Emilie Gillet's MIT-licensed Peaks bass, snare and high-hat
//! sources (revision 08460a69). These are new floating-point kernels; no
//! upstream state-variable-filter, oscillator, random or lookup-table code
//! or asset is included. The applicable notice is in THIRD_PARTY_NOTICES.md.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

fn bounded(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        default
    }
}

fn step(phase: &mut f32, hz: f32, sr: f32) -> f32 {
    *phase = (*phase + hz / sr).fract();
    (*phase * TAU).sin()
}

/// Renders one triggered bass, snare or hat voice. `mode` selects the
/// architecture at 0, 1 or 2. All state fits in `NodeState`; the callback
/// allocates no memory and uses the host sample rate for every time/frequency.
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0xAB82_50A7).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    let sr = kx.sr.max(1.0);
    let mut phases = [st.s[0], st.s[1], st.s[2], st.s[3], st.s[4], st.s[5]];
    let mut age = st.s[6];
    let mut lowpass = st.s[7];
    for (i, sample) in out.iter_mut().enumerate() {
        if st.done() {
            *sample = 0.0;
            continue;
        }
        let freq = bounded(ins[0].at(i), 20.0, sr * 0.2, 80.0);
        let punch = bounded(ins[1].at(i), 0.0, 1.0, 0.5);
        let tone = bounded(ins[2].at(i), 0.0, 1.0, 0.5);
        let decay = bounded(ins[3].at(i), 0.01, 4.0, 0.4);
        let snappy = bounded(ins[4].at(i), 0.0, 1.0, 0.5);
        let metal = bounded(ins[5].at(i), 0.0, 1.0, 0.5);
        let mode = bounded(ins[6].at(i), 0.0, 2.0, 0.0).round();
        let amp = (-age / decay).exp();
        let attack = (-age / 0.008).exp();
        let raw = if mode < 0.5 {
            // A struck low resonator with a short falling pitch impulse and
            // a separately adjustable beater transient.
            let sweep = 1.0 + punch * 7.0 * (-age / 0.026).exp();
            let body = step(&mut phases[0], (freq * sweep).min(sr * 0.4), sr);
            let click = (rng.bipolar() * 0.35 + 0.65) * attack * punch;
            let signal = body * amp + click;
            let cutoff = 150.0 + tone * 9_000.0;
            let a = 1.0 - (-TAU * cutoff / sr).exp();
            lowpass += a * (signal - lowpass);
            lowpass
        } else if mode < 1.5 {
            // Two inharmonic body modes and a noise spring, with `tone`
            // shifting body balance and `snappy` setting noise energy.
            let body1 = step(&mut phases[0], freq, sr);
            let body2 = step(&mut phases[1], (freq * 1.89).min(sr * 0.45), sr);
            let body = (body1 * (1.0 - tone * 0.65) + body2 * (0.35 + tone * 0.65)) * amp * 0.42;
            let noise = rng.bipolar();
            let a = 1.0 - (-TAU * (900.0 + tone * 5_000.0) / sr).exp();
            lowpass += a * (noise - lowpass);
            body + (noise - lowpass) * snappy * (-age / (decay * 0.8)).exp() * 0.7
        } else {
            // An original six-oscillator metallic cluster. The ratios are
            // analytic constants, not a copied phase increment/table.
            const RATIOS: [f32; 6] = [1.0, 1.37, 1.72, 2.11, 2.43, 2.79];
            let mut cluster = 0.0;
            for (phase, ratio) in phases.iter_mut().zip(RATIOS) {
                let hz = (freq * ratio).min(sr * 0.45);
                cluster += if step(phase, hz, sr) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
            }
            let excitation = cluster / 6.0 * metal + rng.bipolar() * (1.0 - metal);
            let a = 1.0 - (-TAU * (2_000.0 + tone * 10_000.0).min(sr * 0.45) / sr).exp();
            lowpass += a * (excitation - lowpass);
            (excitation - lowpass).max(0.0) * amp * 1.3
        };
        *sample = raw.clamp(-1.0, 1.0);
        age += 1.0 / sr;
        if age > decay * 12.0 {
            st.finish();
        }
    }
    st.s[..6].copy_from_slice(&phases);
    st.s[6] = age;
    st.s[7] = lowpass;
    st.u[0] = rng.s;
}
