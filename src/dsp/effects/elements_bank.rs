//! Independent blow/strike bus inputs into original procedural Elements roles.
//!
//! Input L excites blow and input R excites strike. Output L is main and R
//! auxiliary. The published Part also has an alternate voice and different
//! exciter/resonator/reverb equations; this is a bounded adaptation.

use std::f32::consts::TAU;

use super::{FxCtx, FxState, ParamDef};

const LINES: usize = 4;
const EXTRA: usize = 8;
const REVERB: usize = 512;

/// Twenty Patch fields, four performance fields, model, external blend and
/// final dry/wet mix. `FxUnit` applies the final mix after this wet kernel.
pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("el-env-shape", 0.5, 0.0, 1.0),
    ParamDef::new("el-bow-level", 0.0, 0.0, 1.0),
    ParamDef::new("el-bow-timbre", 0.5, 0.0, 1.0),
    ParamDef::new("el-blow-level", 0.5, 0.0, 1.0),
    ParamDef::new("el-blow-meta", 0.5, 0.0, 1.0),
    ParamDef::new("el-blow-timbre", 0.5, 0.0, 1.0),
    ParamDef::new("el-strike-level", 0.5, 0.0, 1.0),
    ParamDef::new("el-strike-meta", 0.5, 0.0, 1.0),
    ParamDef::new("el-strike-timbre", 0.5, 0.0, 1.0),
    ParamDef::new("el-signature", 0.5, 0.0, 1.0),
    ParamDef::new("el-geometry", 0.5, 0.0, 1.0),
    ParamDef::new("el-brightness", 0.5, 0.0, 1.0),
    ParamDef::new("el-damping", 0.5, 0.0, 1.0),
    ParamDef::new("el-position", 0.5, 0.0, 1.0),
    ParamDef::new("el-res-mod-frequency", 0.5, 0.0, 1.0),
    ParamDef::new("el-res-mod-offset", 0.5, 0.0, 1.0),
    ParamDef::new("el-reverb-diffusion", 0.5, 0.0, 1.0),
    ParamDef::new("el-reverb-lp", 0.5, 0.0, 1.0),
    ParamDef::new("el-space", 0.5, 0.0, 2.0),
    ParamDef::new("el-modulation-frequency", 0.5, 0.0, 1.0),
    ParamDef::new("el-gate", 1.0, 0.0, 1.0),
    ParamDef::unit("el-note", 0.0, -48.0, 48.0, "st"),
    ParamDef::new("el-modulation", 0.0, -1.0, 1.0),
    ParamDef::new("el-strength", 1.0, 0.0, 1.0),
    ParamDef::new("el-model", 0.0, 0.0, 2.0),
    ParamDef::new("external-blend", 1.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
    ParamDef::new("el-alternate", 0.0, 0.0, 1.0),
];

#[must_use]
pub fn line_len(sr: f32) -> usize {
    (sr.max(1.0) / 20.0).ceil() as usize + 2
}

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    LINES * line_len(sr) + EXTRA + REVERB
}

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    st.s[1] = 791.0;
    mem.fill(0.0);
}

fn random(seed: &mut f32) -> f32 {
    let next = ((*seed as u32).wrapping_mul(251).wrapping_add(37)) & 0xffff;
    *seed = next as f32;
    next as f32 / 32_767.5 - 1.0
}

#[allow(clippy::too_many_arguments)]
fn modal(
    input: f32,
    hz: f32,
    geometry: f32,
    brightness: f32,
    damping: f32,
    position: f32,
    sr: f32,
    st: &mut FxState,
) -> (f32, f32) {
    let mut center = 0.0;
    let mut side = 0.0;
    for mode in 0..8 {
        let ratio = 1.0 + mode as f32 * (1.1 + geometry * 0.7);
        let frequency = (hz * ratio).min(sr * 0.43);
        let radius = (-(2.0 + damping * 33.0 + mode as f32 * (1.0 - brightness * 0.7)) / sr).exp();
        let coefficient = 2.0 * radius * (TAU * frequency / sr).cos();
        let index = 8 + mode * 2;
        let value = (input * (0.025 + position * 0.05) / (1.0 + mode as f32)
            + coefficient * st.s[index]
            - radius * radius * st.s[index + 1])
            .clamp(-4.0, 4.0);
        st.s[index + 1] = st.s[index];
        st.s[index] = value;
        center += value * (0.65 + brightness * mode as f32 * 0.06);
        side += value * (TAU * (position + mode as f32 * 0.13)).cos();
    }
    (center * 0.18, side * 0.18)
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn strings(
    input: f32,
    hz: f32,
    model: usize,
    geometry: f32,
    brightness: f32,
    damping: f32,
    position: f32,
    sr: f32,
    length: usize,
    st: &mut FxState,
    mem: &mut [f32],
) -> (f32, f32) {
    let count = if model == 1 { 1 } else { LINES };
    let mut center = 0.0;
    let mut side = 0.0;
    for line in 0..count {
        let degree = if model == 1 {
            0.0
        } else {
            ((line * 4 + (geometry * 10.0).round() as usize * (line + 1)) % 13) as f32
        };
        let frequency = (hz * 2.0_f32.powf(degree / 12.0)).clamp(20.0, sr * 0.4);
        let period = (sr / frequency).round().clamp(2.0, (length - 2) as f32) as usize;
        let write = st.s[24 + line] as usize % length;
        let offset = line * length;
        let read = (write + length - period) % length;
        let sample = mem[offset + read];
        let smooth = st.s[28 + line] + (0.07 + brightness * 0.5) * (sample - st.s[28 + line]);
        st.s[28 + line] = smooth;
        let loss = (1.0 - (2.0 + damping * 30.0) / sr).clamp(0.0, 0.999_99);
        mem[offset + write] =
            (smooth * loss + input * (0.13 + position * 0.23) / count as f32).clamp(-3.0, 3.0);
        st.s[24 + line] = ((write + 1) % length) as f32;
        center += sample;
        side += (sample - smooth * (0.2 + position * 0.7)) * if line % 2 == 0 { 1.0 } else { -1.0 };
    }
    let scale = 0.5 / (count as f32).sqrt();
    (center * scale, side * scale)
}

/// Render wet stereo main/aux. Left and right inputs remain separate until
/// their respective blow and strike excitation stages.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let sr = ctx.sr.max(1.0);
    let length = line_len(sr);
    let base = LINES * length;
    if mem.len() < mem_len(sr) || p.len() < PARAMS.len() {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let model = p[24].round().clamp(0.0, 2.0) as usize;
    let frequency = (220.0 * 2.0_f32.powf(p[21].clamp(-48.0, 48.0) / 12.0)).clamp(20.0, sr * 0.35);
    let gate = p[20].clamp(0.0, 1.0);
    let strength = p[23].clamp(0.0, 1.0);
    let external = p[25].clamp(0.0, 1.0);
    if p[27] >= 0.5 {
        let state =
            &mut mem[base + EXTRA..base + EXTRA + crate::dsp::elements_ominous::STATE_FLOATS];
        for (left, right) in l.iter_mut().zip(r.iter_mut()) {
            let noise = random(&mut st.s[1]);
            let blow = *left * external + noise * p[3] * gate * (1.0 - external);
            let strike = *right * external + noise * p[6] * (1.0 - external);
            let (main, aux) = crate::dsp::elements_ominous::sample(p, state, blow, strike, sr);
            *left = main;
            *right = aux;
        }
        return;
    }
    let blow_cutoff = 1.0 - (-TAU * (180.0 + p[5] * 10_000.0) / sr).exp();
    let strike_cutoff = 1.0 - (-TAU * (300.0 + p[8] * 12_000.0) / sr).exp();
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        let blow_in = *left;
        let strike_in = *right;
        st.s[2] += blow_cutoff * (blow_in - st.s[2]);
        st.s[3] += strike_cutoff * (strike_in - st.s[3]);
        let noise = random(&mut st.s[1]);
        let age = st.s[0];
        let strike_env = (-age / (0.004 + p[0] * 0.3)).exp();
        st.s[4] = (st.s[4] + frequency / sr).fract();
        st.s[5] = (st.s[5] + (0.005 + p[19] * 8.0) / sr).fract();
        let bow = (TAU * st.s[4]).sin().tanh() * p[1] * gate * (0.1 + p[2] * 0.9);
        let internal_blow = noise * p[3] * gate * (0.2 + p[4] * 0.8);
        let internal_strike = strike_env
            * p[6]
            * (noise * (1.0 - p[7]) + (TAU * st.s[4] * (1.0 + p[7] * 7.0)).sin() * p[7]);
        let blow = (st.s[2] * (1.0 - p[4]) + (blow_in - st.s[2]) * p[4])
            * (0.2 + p[3] * 0.8)
            * (0.2 + p[5] * 0.8);
        let strike = (st.s[3] * (1.0 - p[7]) + (strike_in - st.s[3]) * p[7])
            * (0.2 + p[6] * 0.8)
            * (0.2 + p[8] * 0.8);
        let tremolo = 1.0 + p[22].clamp(-1.0, 1.0) * (TAU * st.s[5]).sin() * 0.4;
        let excitation = ((bow
            + external * (blow + strike)
            + (1.0 - external) * (internal_blow + internal_strike))
            * (0.1 + strength * 0.9)
            * tremolo
            * (0.6 + p[9] * 1.4))
            .tanh();
        mem[base] = (mem[base] + (0.1 + p[14] * 9.0) / sr).fract();
        let vibrato = (TAU * mem[base]).sin() * (0.01 + p[15] * 0.06);
        let hz = (frequency * (1.0 + vibrato)).clamp(20.0, sr * 0.4);
        let (center, side) = if model == 0 {
            modal(excitation, hz, p[10], p[11], p[12], p[13], sr, st)
        } else {
            strings(
                excitation, hz, model, p[10], p[11], p[12], p[13], sr, length, st, mem,
            )
        };
        let write = mem[base + 1] as usize % REVERB;
        let echo_index = base + EXTRA + write;
        let echo = mem[echo_index];
        mem[base + 2] += (0.02 + p[17] * 0.8) * (echo - mem[base + 2]);
        if p[18] < 1.75 {
            mem[echo_index] =
                (center * 0.3 + mem[base + 2] * (0.1 + p[16] * 0.82)).clamp(-3.0, 3.0);
        }
        mem[base + 1] = ((write + 1) % REVERB) as f32;
        let wet = (p[18] * 0.5).min(1.0);
        *left = (center + mem[base + 2] * wet * 0.9).tanh();
        *right = (side + excitation * (0.15 + p[13] * 0.25) + mem[base + 2] * wet * 0.4).tanh();
        st.s[0] = (age + 1.0 / sr).min(10.0);
    }
}
