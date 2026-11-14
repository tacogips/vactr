//! Stereo-bus, source-style mono excitation into two resonant outputs.
//!
//! This is an original bounded adaptation of the six MIT Rings Part model
//! roles. L/R input is summed to mono before the resonator; output L/R are
//! main/aux. No source tables, chord maps, plucker, or reverb code is used.

use std::f32::consts::TAU;

use super::{FxCtx, FxState, ParamDef};

const LINES: usize = 4;
const MIN_FREQUENCY: f32 = 20.0;
const REVERB_LEN: usize = 512;

/// Bus controls: the final `mix` is blended by `FxUnit`.
pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("model", 0.0, 0.0, 5.0),
    ParamDef::new("structure", 0.5, 0.0, 1.0),
    ParamDef::new("brightness", 0.5, 0.0, 1.0),
    ParamDef::new("damping", 0.5, 0.0, 1.0),
    ParamDef::new("position", 0.5, 0.0, 1.0),
    ParamDef::unit("note", 0.0, -48.0, 48.0, "st"),
    ParamDef::unit("tonic", 0.0, -48.0, 48.0, "st"),
    ParamDef::unit("fm", 0.0, -24.0, 24.0, "st"),
    ParamDef::new("chord", 0.0, 0.0, 10.0),
    ParamDef::new("polyphony", 1.0, 1.0, 4.0),
    ParamDef::new("strum", 0.0, 0.0, 1.0),
    ParamDef::new("internal-exciter", 0.0, 0.0, 1.0),
    ParamDef::new("internal-strum", 1.0, 0.0, 1.0),
    ParamDef::new("internal-note", 1.0, 0.0, 1.0),
    ParamDef::new("external-mix", 1.0, 0.0, 1.0),
    ParamDef::new("gate", 1.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

/// Four comb lines down to 20 Hz plus a short feedback tail.
#[must_use]
pub fn line_len(sr: f32) -> usize {
    (sr.max(1.0) / MIN_FREQUENCY).ceil() as usize + 2
}

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    LINES * line_len(sr) + REVERB_LEN
}

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    st.s[24] = 179.0;
    mem.fill(0.0);
}

fn authored_interval(chord: usize, line: usize) -> f32 {
    ((chord * 5 + line * (2 + chord % 4)) % 13) as f32
}

fn random(seed: &mut f32) -> f32 {
    let next = ((*seed as u32).wrapping_mul(251).wrapping_add(37)) & 0xffff;
    *seed = next as f32;
    next as f32 / 32_767.5 - 1.0
}

fn modal(
    excitation: f32,
    frequency: f32,
    structure: f32,
    damping: f32,
    poly: usize,
    sr: f32,
    st: &mut FxState,
) -> (f32, f32) {
    let mut main = 0.0;
    let mut aux = 0.0;
    for mode in 0..poly {
        let hz = (frequency * (1.0 + mode as f32 * (1.15 + structure * 0.8))).min(sr * 0.43);
        let radius = (-(3.0 + damping * 30.0 + mode as f32 * 2.0) / sr).exp();
        let coefficient = 2.0 * radius * (TAU * hz / sr).cos();
        let index = 8 + mode * 2;
        let value = (excitation * 0.09 + coefficient * st.s[index]
            - radius * radius * st.s[index + 1])
            .clamp(-3.0, 3.0);
        st.s[index + 1] = st.s[index];
        st.s[index] = value;
        main += value;
        aux += value * if mode % 2 == 0 { 1.0 } else { -1.0 };
    }
    (main * 0.25, aux * 0.25)
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn strings(
    excitation: f32,
    frequency: f32,
    model: usize,
    structure: f32,
    damping: f32,
    position: f32,
    chord: usize,
    poly: usize,
    sr: f32,
    length: usize,
    st: &mut FxState,
    mem: &mut [f32],
) -> (f32, f32) {
    let mut main = 0.0;
    let mut aux = 0.0;
    for line in 0..poly {
        let ratio = if model == 4 {
            2.0_f32.powf(authored_interval(chord, line) / 12.0)
        } else if model == 1 {
            2.0_f32.powf(authored_interval(chord, line) / 12.0)
                * (1.0 + structure * line as f32 * 0.003)
        } else {
            1.0 + line as f32 * (0.02 + structure * 0.08)
        };
        let hz = (frequency * ratio).clamp(MIN_FREQUENCY, sr * 0.4);
        let period = (sr / hz).round().clamp(2.0, (length - 2) as f32) as usize;
        let write = st.s[4 + line] as usize % length;
        let offset = line * length;
        let read = (write + length - period) % length;
        let previous = mem[offset + read];
        let smooth = st.s[20 + line] + (0.08 + structure * 0.45) * (previous - st.s[20 + line]);
        st.s[20 + line] = smooth;
        let loss = (1.0 - (2.0 + damping * 28.0) / sr).clamp(0.0, 0.999_99);
        mem[offset + write] =
            (smooth * loss + excitation * (0.18 + position * 0.24) / poly as f32).clamp(-2.0, 2.0);
        st.s[4 + line] = ((write + 1) % length) as f32;
        let pickup = previous - smooth * (0.15 + position * 0.7);
        if poly == 1 {
            main += previous;
            aux += pickup;
        } else if line % 2 == 0 {
            main += previous;
        } else {
            aux += pickup;
        }
    }
    let gain = 0.5 / (poly as f32).sqrt();
    (main * gain, aux * gain)
}

/// Process two independent input channels as one external exciter and emit
/// independent resonator main/aux channels. The source Part's `in` is mono.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
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
    let model = p[0].round().clamp(0.0, 5.0) as usize;
    let structure = p[1].clamp(0.0, 1.0);
    let brightness = p[2].clamp(0.0, 1.0);
    let damping = p[3].clamp(0.0, 1.0);
    let position = p[4].clamp(0.0, 1.0);
    let semis = p[6] + p[7] + if p[13] >= 0.5 { p[5] } else { 0.0 };
    let chord = p[8].round().clamp(0.0, 10.0) as usize;
    let poly = p[9].round().clamp(1.0, 4.0) as usize;
    let frequency = (220.0
        * 2.0_f32.powf(semis / 12.0)
        * if model == 0 || model == 3 {
            2.0_f32.powf(authored_interval(chord, poly - 1) / 24.0)
        } else {
            1.0
        })
    .clamp(MIN_FREQUENCY, sr * 0.35);
    let internal_on = p[11] >= 0.5;
    let internal_strum = p[12] >= 0.5;
    let external_mix = p[14].clamp(0.0, 1.0);
    let gate = p[15].clamp(0.0, 1.0);
    let strum = p[10].clamp(0.0, 1.0);
    let triggered = strum >= 0.5 && st.s[0] < 0.5;
    st.s[0] = strum;
    if triggered {
        st.s[2] = 1.0;
    }
    let pulse_decay = (-1.0 / (sr * (0.002 + position * 0.01))).exp();
    let cutoff = 1.0 - (-TAU * (300.0 + brightness * 7_000.0) / sr).exp();
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        let input = (*left + *right) * 0.5;
        st.s[1] += cutoff * (input - st.s[1]);
        let noise = random(&mut st.s[24]);
        let internal = if internal_on {
            if internal_strum {
                noise * st.s[2] * (0.15 + brightness * 0.4)
            } else {
                noise * strum * (0.04 + brightness * 0.12)
            }
        } else {
            0.0
        };
        st.s[2] *= pulse_decay;
        let excitation = (st.s[1] * external_mix + internal * (1.0 - external_mix)) * gate;
        let (mut main, mut aux) = match model {
            0 => modal(excitation, frequency, structure, damping, poly, sr, st),
            3 => {
                st.s[16] = (st.s[16] + frequency * (0.5 + structure * 3.0) / sr).fract();
                st.s[17] = (st.s[17] + frequency / sr).fract();
                st.s[18] += 0.2 * (excitation.abs() - st.s[18]);
                let modulator = (TAU * st.s[16]).sin();
                let carrier =
                    (TAU * st.s[17] + modulator * excitation * (1.0 + brightness * 4.0)).sin();
                (carrier * st.s[18] * 0.45, modulator * st.s[18] * 0.3)
            }
            _ => strings(
                excitation, frequency, model, structure, damping, position, chord, poly, sr,
                length, st, mem,
            ),
        };
        if model == 1 {
            main *= 0.7;
            aux *= 1.2;
        }
        if model == 5 {
            let write = st.s[3] as usize % REVERB_LEN;
            let echo = mem[base + write];
            mem[base + write] = (aux * 0.5 + echo * (0.3 + damping * 0.55)).clamp(-2.0, 2.0);
            st.s[3] = ((write + 1) % REVERB_LEN) as f32;
            main += echo * (1.0 - position) * 0.3;
            aux += echo * position * 0.5;
        }
        *left = main.tanh();
        *right = aux.tanh();
    }
}
