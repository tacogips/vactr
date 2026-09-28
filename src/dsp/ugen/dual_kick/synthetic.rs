//! MIT translation of Plaits' synthetic bass-drum envelope and transient path.

use std::f32::consts::{PI, TAU};

use super::rate_pole;
use crate::dsp::effects::prim::Rng;
use crate::dsp::ugen::NodeState;

const PHASE: usize = 0;
const PHASE_NOISE: usize = 1;
const FM: usize = 2;
const FM_LP: usize = 3;
const BODY: usize = 4;
const BODY_LP: usize = 5;
const TRANSIENT: usize = 6;
const TRANSIENT_LP: usize = 7;
const SUSTAIN_GAIN: usize = 8;
const TONE_LP: usize = 9;
const CLICK_LP: usize = 10;
const CLICK_HP: usize = 11;
const CLICK_SVF_BP: usize = 12;
const CLICK_SVF_LP: usize = 13;
const NOISE_LP: usize = 14;
const NOISE_HP: usize = 15;

#[inline]
fn distorted_sine(phase: f32, phase_noise: f32, dirt: f32) -> f32 {
    let phase = (phase + phase_noise * dirt).rem_euclid(1.0);
    let triangle = (if phase < 0.5 { phase } else { 1.0 - phase }) * 4.0 - 1.0;
    let bent = 2.0 * triangle / (1.0 + triangle.abs());
    let clean = (TAU * (phase + 0.75)).sin();
    bent + (1.0 - dirt) * (clean - bent)
}

#[inline]
fn transistor_vca(sample: f32, gain: f32) -> f32 {
    let driven = (sample - 0.6) * gain;
    3.0 * driven / (2.0 + driven.abs()) + gain * 0.3
}

#[inline]
fn click(input: f32, m: &mut [f32], sr: f32) -> f32 {
    let rise = rate_pole(0.5, sr);
    let fall = rate_pole(0.1, sr);
    let slope = if input > m[CLICK_LP] { rise } else { fall };
    m[CLICK_LP] += slope * (input - m[CLICK_LP]);
    m[CLICK_HP] += rate_pole(0.04, sr) * (m[CLICK_LP] - m[CLICK_HP]);
    let x = m[CLICK_LP] - m[CLICK_HP];
    // Source 5 kHz, Q=2 SVF lowpass, with host-rate normalized cutoff.
    let g = (PI * (5_000.0 / sr).min(0.4)).tan();
    let r = 0.5;
    let h = 1.0 / (1.0 + r * g + g * g);
    let hp = (x - (r + g) * m[CLICK_SVF_BP] - m[CLICK_SVF_LP]) * h;
    let bp = g * hp + m[CLICK_SVF_BP];
    m[CLICK_SVF_BP] = g * hp + bp;
    let lp = g * bp + m[CLICK_SVF_LP];
    m[CLICK_SVF_LP] = g * bp + lp;
    lp
}

#[inline]
fn attack_noise(sample: f32, m: &mut [f32], sr: f32) -> f32 {
    m[NOISE_LP] += rate_pole(0.05, sr) * (sample - m[NOISE_LP]);
    m[NOISE_HP] += rate_pole(0.005, sr) * (m[NOISE_LP] - m[NOISE_HP]);
    m[NOISE_LP] - m[NOISE_HP]
}

/// One sample of the source FM/body/transient envelopes, oscillator, click,
/// filtered noise, VCA and tone path. RNG is per-node, deterministic and
/// held entirely in `NodeState`; no global stmlib random state is imported.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    m: &mut [f32],
    st: &mut NodeState,
    freq: f32,
    harmonics: f32,
    tone: f32,
    decay: f32,
    accent: f32,
    sustain: bool,
    gate: bool,
    sr: f32,
    seed: u32,
) -> f32 {
    if st.s[2] == 0.0 {
        m[FM] = 1.0;
        m[BODY] = 0.3 + 0.7 * accent;
        m[TRANSIENT] = m[BODY];
        st.u[0] = Rng::new(seed ^ 0xC2D4_97A1).s;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            st.u[1] = (sr * 0.001).round() as u32;
            st.u[2] = (sr * 0.0013).round() as u32;
        }
        st.s[2] = 1.0;
    }
    let mut rng = Rng::new(st.u[0]);
    let f0 = (freq / sr).min(0.4);
    let decay2 = decay * decay;
    let fm_decay_amt = (harmonics * 2.0 - 1.0).max(0.0).powi(2);
    let fm_decay = (1.0 - 1.0 / (0.008 * (1.0 + fm_decay_amt * 4.0) * sr)).clamp(0.0, 1.0);
    let body_decay = (1.0 - 2.0_f32.powf(-decay2 * 5.0) / (0.02 * sr)).clamp(0.0, 1.0);
    let transient_decay = (1.0 - 1.0 / (0.005 * sr)).clamp(0.0, 1.0);
    let env_filter = rate_pole(0.1, sr);
    let random = rng.bipolar();
    m[PHASE_NOISE] += rate_pole(0.002, sr) * (random * 0.5 - m[PHASE_NOISE]);
    let dirt = (if sustain {
        harmonics
    } else {
        0.4 - 0.25 * decay2
    }) * (1.0 - 8.0 * f0).max(0.0);
    let mix = if sustain {
        m[SUSTAIN_GAIN] +=
            env_filter * ((if gate { accent * decay2 } else { 0.0 }) - m[SUSTAIN_GAIN]);
        m[PHASE] = (m[PHASE] + f0).fract();
        -transistor_vca(
            distorted_sine(m[PHASE], m[PHASE_NOISE], dirt),
            m[SUSTAIN_GAIN],
        )
    } else {
        if st.u[2] != 0 {
            st.u[2] -= 1;
            m[PHASE] = 0.25;
        } else {
            m[FM] *= fm_decay;
            let fm = 1.0 + (harmonics * 2.0).min(1.0) * 3.5 * m[FM_LP];
            m[PHASE] = (m[PHASE] + (f0 * fm).min(0.5)).fract();
        }
        if st.u[1] != 0 {
            st.u[1] -= 1;
        } else {
            m[BODY] *= body_decay;
            m[TRANSIENT] *= transient_decay;
        }
        m[BODY_LP] += env_filter * (m[BODY] - m[BODY_LP]);
        m[TRANSIENT_LP] += env_filter * (m[TRANSIENT] - m[TRANSIENT_LP]);
        m[FM_LP] += env_filter * (m[FM] - m[FM_LP]);
        let body = distorted_sine(m[PHASE], m[PHASE_NOISE], dirt);
        let click = click(if st.u[1] != 0 { 0.0 } else { 1.0 }, m, sr);
        let noise = attack_noise((rng.bipolar() + 1.0) * 0.5, m, sr);
        -transistor_vca(body, m[BODY_LP]) - (click + noise) * m[TRANSIENT_LP] * tone
    };
    let tone_f = (4.0 * f0 * 2.0_f32.powf(tone * 9.0)).min(1.0);
    m[TONE_LP] += tone_f * (mix - m[TONE_LP]);
    st.u[0] = rng.s;
    m[TONE_LP]
}
