//! Original bounded quadrature frequency shifter informed by Warps' hidden mode.
//! Hilbert FIR coefficients are generated analytically at install; no source
//! quadrature poles, oscillator tables or crossfade lookups are imported.

use std::f32::consts::{PI, TAU};

use super::{FxCtx, FxState, ParamDef};

const TAPS: usize = 127;
const MID: usize = TAPS / 2;
pub const MEM_LEN: usize = TAPS * 3;

pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("carrier-wave", 1.0, 0.0, 3.0),
    ParamDef::new("shift-pot", 0.5, 0.0, 1.0),
    ParamDef::new("shift-cv", 0.0, -1.0, 1.0),
    ParamDef::new("phase-shift", 0.0, 0.0, 1.0),
    ParamDef::new("timbre", 1.0, 0.0, 1.0),
    ParamDef::new("feedback", 0.0, 0.0, 0.95),
    ParamDef::new("dry-wet", 1.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

/// Generate an original windowed ideal-Hilbert FIR and reset fixed rings.
pub fn init(st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    mem.fill(0.0);
    if mem.len() < MEM_LEN {
        return;
    }
    for (index, coefficient) in mem[..TAPS].iter_mut().enumerate() {
        let k = index as i32 - MID as i32;
        if k & 1 != 0 {
            let window = 0.5 - 0.5 * (TAU * index as f32 / (TAPS - 1) as f32).cos();
            *coefficient = 2.0 * window / (PI * k as f32);
        }
    }
}

fn quadrature(coefficients: &[f32], ring: &mut [f32], cursor: usize, input: f32) -> (f32, f32) {
    ring[cursor] = input;
    let delayed = ring[(cursor + TAPS - MID) % TAPS];
    let mut imaginary = 0.0;
    for (tap, coefficient) in coefficients.iter().enumerate() {
        let sample = ring[(cursor + TAPS - tap) % TAPS];
        imaginary += coefficient * sample;
    }
    (delayed, imaginary)
}

fn internal_iq(phase: f32, wave: u8) -> (f32, f32) {
    let angle = TAU * phase;
    let (mut i, mut q) = (angle.cos(), angle.sin());
    match wave {
        2 => {
            i -= (3.0 * angle).cos() / 9.0;
            q -= (3.0 * angle).sin() / 9.0;
            i *= 0.9;
            q *= 0.9;
        }
        3 => {
            for harmonic in 2..=5 {
                let phase = angle * harmonic as f32;
                let gain = 1.0 / harmonic as f32;
                i += phase.cos() * gain;
                q += phase.sin() * gain;
            }
            i *= 0.45;
            q *= 0.45;
        }
        _ => {}
    }
    (i, q)
}

/// Left input is external carrier/phase input; right input is modulator.
/// Left output is the timbre-selected main sideband, right its complementary
/// auxiliary sideband. The FIR adds 63 samples of fixed host-rate latency.
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if p.len() < PARAMS.len() || mem.len() < MEM_LEN {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let (coefficients, rings) = mem.split_at_mut(TAPS);
    let (carrier_ring, modulator_ring) = rings.split_at_mut(TAPS);
    let wave = p[0].round() as u8;
    let frequency = ((p[1] - 0.5) * 2400.0 + p[2] * 600.0).clamp(-ctx.sr * 0.2, ctx.sr * 0.2);
    let rotation = TAU * p[3];
    let timbre = p[4].clamp(0.0, 1.0);
    let feedback = p[5].clamp(0.0, 0.95);
    let wet = p[6].clamp(0.0, 1.0);
    let mut cursor = st.s[0] as usize;
    let mut phase = st.s[1];
    let mut feedback_sample = st.s[2];
    for (main, aux) in l.iter_mut().zip(r.iter_mut()) {
        let (external, input) = (*main, *aux);
        let modulator = (input + feedback_sample * feedback).tanh();
        let (mod_i, mod_q) = quadrature(coefficients, modulator_ring, cursor, modulator);
        let (carrier_i, carrier_q) = if wave == 0 {
            let (i, q) = quadrature(coefficients, carrier_ring, cursor, external);
            let (sin, cos) = rotation.sin_cos();
            (i * cos - q * sin, i * sin + q * cos)
        } else {
            phase = (phase + frequency / ctx.sr).rem_euclid(1.0);
            internal_iq(phase, wave)
        };
        let up = carrier_i * mod_i - carrier_q * mod_q;
        let down = carrier_i * mod_i + carrier_q * mod_q;
        let selected = down + (up - down) * timbre;
        let complementary = up + (down - up) * timbre;
        feedback_sample += 0.15 * (selected.tanh() - feedback_sample);
        *main = (input * (1.0 - wet) + selected * wet).clamp(-4.0, 4.0);
        *aux = (input * (1.0 - wet) + complementary * wet).clamp(-4.0, 4.0);
        cursor = (cursor + 1) % TAPS;
    }
    st.s[0] = cursor as f32;
    st.s[1] = phase;
    st.s[2] = feedback_sample;
}
