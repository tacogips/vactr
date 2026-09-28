//! MIT translation of the Plaits analog bass-drum pulse/resonator stages.

use std::f32::consts::PI;

use super::{rate_pole, soft_clip};
use crate::dsp::ugen::NodeState;

const PULSE: usize = 0;
const PULSE_LP: usize = 1;
const FM_LP: usize = 2;
const RETRIG: usize = 3;
const LP_OUT: usize = 4;
const TONE_LP: usize = 5;
const SVF_BP: usize = 6;
const SVF_LP: usize = 7;
const PHASE: usize = 8;
const SUSTAIN_GAIN: usize = 9;
const PRE_GAIN: usize = 10;
const POST_GAIN: usize = 11;

#[inline]
fn diode(mut x: f32) -> f32 {
    if x < 0.0 {
        x *= 2.0;
        0.7 * x / (1.0 + x.abs())
    } else {
        x
    }
}

#[inline]
fn svf_band_low(input: f32, f: f32, q: f32, state: &mut [f32]) -> (f32, f32) {
    let g = (PI * f).tan();
    let r = 1.0 / q;
    let h = 1.0 / (1.0 + r * g + g * g);
    let hp = (input - (r + g) * state[SVF_BP] - state[SVF_LP]) * h;
    let bp = g * hp + state[SVF_BP];
    state[SVF_BP] = g * hp + bp;
    let lp = g * bp + state[SVF_LP];
    state[SVF_LP] = g * bp + lp;
    (bp, lp)
}

/// One sample of the source pulse -> diode -> resonator -> tone -> overdrive.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    m: &mut [f32],
    st: &mut NodeState,
    freq: f32,
    harmonics: f32,
    timbre: f32,
    decay: f32,
    accent: f32,
    sustain: bool,
    gate: bool,
    sr: f32,
    remaining: f32,
) -> f32 {
    let f0 = (freq / sr).min(0.4);
    if st.u[2] == 0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            st.u[0] = (0.001 * sr).round() as u32;
            st.u[1] = (0.006 * sr).round() as u32;
        }
        st.u[2] = 1;
    }
    let pulse_height = 3.0 + 7.0 * accent;
    let pulse_decay = 1.0 - 1.0 / (0.0002 * sr).max(1.0);
    let pulse_filter = (1.0 / (0.0001 * sr)).min(1.0);
    let retrig_decay = 1.0 - 1.0 / (0.05 * sr);
    let mut pulse = if st.u[0] != 0 {
        st.u[0] -= 1;
        m[PULSE] = if st.u[0] == 0 {
            pulse_height - 1.0
        } else {
            pulse_height
        };
        m[PULSE]
    } else {
        m[PULSE] *= pulse_decay;
        m[PULSE]
    };
    if sustain {
        pulse = 0.0;
    }
    m[PULSE_LP] += pulse_filter * (pulse - m[PULSE_LP]);
    let excitation = diode(pulse - m[PULSE_LP] + pulse * 0.044);

    let fm_pulse = if st.u[1] != 0 {
        st.u[1] -= 1;
        m[RETRIG] = if st.u[1] == 0 { -0.8 } else { 0.0 };
        if sustain {
            0.0
        } else {
            1.0
        }
    } else {
        m[RETRIG] *= retrig_decay;
        0.0
    };
    m[FM_LP] += pulse_filter * (fm_pulse - m[FM_LP]);

    let attack_fm = (harmonics * 4.0).min(1.0);
    let self_fm = (harmonics * 4.0 - 1.0).clamp(0.0, 1.0);
    let punch = 0.7 + diode(10.0 * m[LP_OUT] - 1.0);
    let f = (f0 * (1.0 + m[FM_LP] * 1.7 * attack_fm + punch * 0.08 * self_fm)).clamp(0.0, 0.4);
    let q = 1.0 + 1500.0 * 2.0_f32.powf(decay * 80.0 / 12.0) * f0;
    let (resonator, low) = if sustain {
        m[SUSTAIN_GAIN] +=
            rate_pole(0.01, sr) * ((if gate { accent * decay } else { 0.0 }) - m[SUSTAIN_GAIN]);
        m[PHASE] = (m[PHASE] + f).fract();
        let wave = (2.0 * PI * m[PHASE]).sin() * m[SUSTAIN_GAIN];
        (wave, wave)
    } else {
        svf_band_low((excitation - m[RETRIG] * 0.2) * (0.001 / f0), f, q, m)
    };
    m[LP_OUT] = low;
    let tone_f = (4.0 * f0 * 2.0_f32.powf(timbre * 9.0)).min(1.0);
    let leak = 0.08 * (timbre + 0.25);
    m[TONE_LP] += tone_f * (excitation * leak + resonator - m[TONE_LP]);

    // Source Overdrive::Process transfer with block-linear gain interpolation.
    let drive = (harmonics * 2.0 - 1.0).max(0.0) * (1.0 - 16.0 * f0).max(0.0);
    let amount = 0.5 + 0.5 * drive;
    let amount2 = amount * amount;
    let low_gain = amount * 0.5;
    let high_gain = amount2 * amount2 * amount * 24.0;
    let target_pre = low_gain + (high_gain - low_gain) * amount2;
    let squashed = amount * (2.0 - amount);
    let target_post = 1.0 / soft_clip(0.33 + squashed * (target_pre - 0.33));
    m[PRE_GAIN] += (target_pre - m[PRE_GAIN]) / remaining;
    m[POST_GAIN] += (target_post - m[POST_GAIN]) / remaining;
    soft_clip(m[TONE_LP] * m[PRE_GAIN]) * m[POST_GAIN]
}
