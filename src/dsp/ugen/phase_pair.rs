//! Table-free phase-distortion voice with synchronized/free-running paths.
//!
//! This is an independent analytic adaptation of the four control roles and
//! dual-path architecture of Plaits' MIT-licensed position 1 engine. It uses
//! an original equal-step ratio mapping, not `lut_fm_frequency_quantizer` or
//! any generated Plaits resource. See THIRD_PARTY_NOTICES.md.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

fn bounded(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

fn asymmetric_triangle(phase: f32, width: f32) -> f32 {
    if phase < width {
        phase / width * 2.0 - 1.0
    } else {
        (1.0 - phase) / (1.0 - width) * 2.0 - 1.0
    }
}

/// Original table-free, 25-position ratio map shared by the analytic
/// phase-distortion and FM adaptations.
pub(super) fn equal_step_ratio(harmonics: f32) -> f32 {
    let step = (harmonics.clamp(0.0, 1.0) * 24.0).round();
    2.0_f32.powf((step - 8.0) / 12.0)
}

/// Renders one phase-distortion path. Mode zero resets the modulator at each
/// carrier wrap; mode one keeps its independent phase. The template places
/// these nodes in parallel, sending the free-running path to `aux-out`.
/// Two substeps per host sample are averaged without temporary buffers.
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let sr = kx.sr.max(1.0);
    let (mut carrier_phase, mut mod_phase) = (st.s[0], st.s[1]);
    for (i, y) in out.iter_mut().enumerate() {
        let pitch = bounded(ins[0].at(i), 20.0, sr * 0.4, 220.0);
        let harmonics = bounded(ins[1].at(i), 0.0, 1.0, 0.5);
        let timbre = bounded(ins[2].at(i), 0.0, 1.0, 0.5);
        let morph = bounded(ins[3].at(i), 0.0, 1.0, 0.5);
        let free_running = ins[4].at(i) >= 0.5;

        // An original 25-position equal-ratio map: intentionally distinct
        // from the Plaits resource's curated inharmonic ratio sequence.
        let ratio = equal_step_ratio(harmonics);
        let mod_hz = (pitch * ratio).min(sr * 0.4);
        let width = 0.15 + 0.7 * morph;
        let depth = 0.48 * timbre * timbre;
        let mut sum = 0.0;
        for _ in 0..2 {
            carrier_phase += pitch / (sr * 2.0);
            if carrier_phase >= 1.0 {
                carrier_phase -= 1.0;
                if !free_running {
                    mod_phase = 0.0;
                }
            }
            mod_phase = (mod_phase + mod_hz / (sr * 2.0)).fract();
            let warp = asymmetric_triangle(mod_phase, width);
            sum += (TAU * (carrier_phase + depth * warp)).sin();
        }
        *y = sum * 0.5;
    }
    st.s[0] = carrier_phase;
    st.s[1] = mod_phase;
}
