//! Original coupled-FM percussion with feedback and noise excitation.
//!
//! The Larsson EFM thesis and Elektron manual motivated research into FM
//! percussion only. This kernel imports no firmware, source code, equations
//! as expressed, tables, presets, samples, or data from either reference.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::{Rng, Shape};

use super::{Inp, Kx, NodeState, MAX_PORTS};

fn bounded(value: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

/// Render one event-local hit from analytic oscillators and fixed node state.
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0x4D37_A912).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    let sr = kx.sr.max(1.0);
    let mut carrier = st.s[0];
    let mut modulator = st.s[1];
    let mut elapsed = st.s[2];
    let mut prior_mod = st.s[3];
    let mut colored_noise = st.s[4];

    for (i, sample) in out.iter_mut().enumerate() {
        if st.done() {
            *sample = 0.0;
            continue;
        }
        let freq = bounded(ins[0].at(i), 20.0, sr * 0.2, 180.0);
        let velocity = bounded(ins[1].at(i), 0.0, 1.0, 1.0);
        let ratio = bounded(ins[2].at(i), 0.25, 12.0, 2.4);
        let index = bounded(ins[3].at(i), 0.0, 12.0, 3.0);
        let feedback = bounded(ins[4].at(i), 0.0, 0.95, 0.35);
        let mod_decay = bounded(ins[5].at(i), 0.002, 4.0, 0.16);
        let body_decay = bounded(ins[6].at(i), 0.01, 8.0, 0.42);
        let pitch_drop = bounded(ins[7].at(i), 0.0, 4.0, 0.5);
        let noise_level = bounded(ins[8].at(i), 0.0, 1.0, 0.18);
        let noise_decay = bounded(ins[9].at(i), 0.002, 3.0, 0.075);
        let noise_color = bounded(ins[10].at(i), 100.0, sr * 0.45, 5000.0);
        let noise_to_fm = bounded(ins[11].at(i), 0.0, 3.0, 0.45);
        let cutoff = bounded(ins[12].at(i), 40.0, sr * 0.45, 1800.0);
        let resonance = bounded(ins[13].at(i), 0.3, 12.0, 1.5);
        let drive = bounded(ins[14].at(i), 0.0, 1.0, 0.22);

        let body_env = (-elapsed / body_decay).exp();
        let mod_env = (-elapsed / mod_decay).exp();
        let noise_env = (-elapsed / noise_decay).exp();
        let sweep_env = (-elapsed / 0.028).exp();
        let carrier_hz = (freq * (1.0 + pitch_drop * sweep_env)).min(sr * 0.45);
        let mod_hz = (freq * ratio).min(sr * 0.45);

        // An authored one-pole noise colour; noise enters both the modulation
        // network and a separately decaying transient rather than forming a
        // parallel feedback-noise voice.
        let alpha = 1.0 - (-TAU * noise_color / sr).exp();
        colored_noise += alpha * (rng.bipolar() - colored_noise);
        let mod_sample =
            (modulator + feedback * prior_mod + noise_to_fm * colored_noise * noise_env).sin();
        prior_mod = mod_sample;
        let body = (carrier + index * mod_env * mod_sample).sin() * body_env;
        let transient = colored_noise * noise_level * noise_env;
        let raw = body + transient;
        st.bq[0].set(Shape::Bandpass, cutoff, resonance, 0.0, sr);
        let filtered = st.bq[0].run(raw);
        let excitation = raw * 0.7 + filtered * 0.3;
        let gain = 1.0 + drive * 8.0;
        *sample = (excitation * gain).tanh() / gain.tanh() * velocity;

        carrier = (carrier + TAU * carrier_hz / sr).rem_euclid(TAU);
        modulator = (modulator + TAU * mod_hz / sr).rem_euclid(TAU);
        elapsed += 1.0 / sr;
        if elapsed > body_decay.max(mod_decay).max(noise_decay) * 12.0 {
            st.finish();
        }
    }

    st.s[0] = carrier;
    st.s[1] = modulator;
    st.s[2] = elapsed;
    st.s[3] = prior_mod;
    st.s[4] = colored_noise;
    st.u[0] = rng.s;
}
