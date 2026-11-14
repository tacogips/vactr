//! Analytic oscillator and two-path filter for `filter-voice`.
//!
//! This is an independent adaptation of the signal roles and controls of
//! Plaits' MIT-licensed position 0 engine, not a translation of its oscillator,
//! filter, interpolator, or `stmlib` implementations. See THIRD_PARTY_NOTICES.md.

use std::f32::consts::{PI, TAU};

use super::{Inp, Kx, NodeState, MAX_PORTS};

fn finite(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

/// Variable saw/pulse spectrum with an independently phased sub-octave.
/// A finite harmonic sum avoids waveform tables and scales its upper partial
/// with Nyquist. Phase state is allocated within each voice before callbacks.
pub fn source(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let sr = kx.sr.max(1.0);
    let (mut phase, mut sub_phase) = (st.s[0], st.s[1]);
    for (i, y) in out.iter_mut().enumerate() {
        let hz = finite(ins[0].at(i), 20.0, sr * 0.4, 440.0);
        let morph = finite(ins[1].at(i), 0.0, 1.0, 0.5);
        phase = (phase + hz / sr).fract();
        sub_phase = (sub_phase + hz * 0.5 / sr).fract();
        let width = 0.5 + (morph - 0.5) * 0.7;
        let blend = (morph * 1.5).clamp(0.0, 1.0);
        let max_partial = ((sr * 0.45 / hz) as usize).clamp(1, 12);
        let mut saw = 0.0;
        let mut pulse = 0.0;
        for harmonic in 1..=max_partial {
            #[allow(clippy::cast_precision_loss)]
            let n = harmonic as f32;
            let angle = TAU * n * phase;
            saw += angle.sin() / n;
            pulse += (PI * n * width).sin() * (angle - PI * n * width).cos() / n;
        }
        let sub = (TAU * sub_phase).sin();
        let sub_gain = ((morph - 0.5).abs() * 1.2).min(0.5);
        *y = ((1.0 - blend) * saw * 0.48 + blend * pulse * 0.7 + sub * sub_gain).clamp(-1.0, 1.0);
    }
    st.s[0] = phase;
    st.s[1] = sub_phase;
}

/// `mode=0` emits the low-pass main path; `mode=1` emits high-pass auxiliary.
/// Two nodes consume the same oscillator and controls, maintaining separate
/// fixed-size state but producing simultaneous independent graph outputs.
pub fn filter(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let sr = kx.sr.max(1.0);
    let (mut low, mut second) = (st.s[0], st.s[1]);
    for (i, y) in out.iter_mut().enumerate() {
        let input = finite(ins[0].at(i), -4.0, 4.0, 0.0);
        let hz = finite(ins[1].at(i), 20.0, sr * 0.4, 440.0);
        let timbre = finite(ins[2].at(i), 0.0, 1.0, 0.5);
        let harmonics = finite(ins[3].at(i), 0.0, 1.0, 0.5);
        let cutoff = (hz * 2.0_f32.powf((timbre - 0.5) * 9.0)).clamp(20.0, sr * 0.4);
        let alpha = 1.0 - (-TAU * cutoff / sr).exp();
        let drive = 1.0 + harmonics * 2.5;
        let driven = ((input - low * harmonics * 0.6) * drive).tanh();
        low += alpha * (driven - low);
        second += alpha * (low - second);
        let stage = (1.0 - harmonics * 1.3).clamp(0.0, 1.0);
        let lp = (low * (1.0 - stage) + second * stage).tanh();
        let hp = (driven - low).tanh();
        *y = if ins[4].at(i) >= 0.5 { hp } else { lp };
    }
    st.s[0] = low;
    st.s[1] = second;
}
