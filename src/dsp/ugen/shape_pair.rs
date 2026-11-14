//! Analytic slope-shaping/folding voice with separate overtone auxiliary.
//!
//! Independently written transfer functions adapt the MIT Plaits position 9
//! signal roles without importing its generated waveshaper/fold tables.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Two oscillator phases and spare fixed state, reserved before rendering.
pub const STATE_FLOATS: usize = 4;

#[inline]
fn slope(phase: f32, width: f32) -> f32 {
    2.0 * if phase < width {
        phase / width
    } else {
        (1.0 - phase) / (1.0 - width)
    } - 1.0
}

#[inline]
fn shape(x: f32, harmonics: f32) -> f32 {
    let bias = (harmonics - 0.5) * 2.0;
    if bias < 0.0 {
        let depth = -bias;
        let curved = x.signum() * x.abs().sqrt();
        x + depth * (curved - x)
    } else {
        let curved = x * x.abs();
        x + bias * (curved - x)
    }
}

#[inline]
fn fold(x: f32, drive: f32) -> f32 {
    let gain = 1.0 + 7.0 * drive;
    let phase = ((x * gain + 1.0) * 0.25).rem_euclid(1.0);
    1.0 - 4.0 * (phase - 0.5).abs()
}

/// Mode 0 is a folded variable-slope main; mode 1 blends a sine-like
/// triangle overtone with a second fold response.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render_worker(ins, mem, out, None, kx);
}

/// Renders folded and overtone outputs from shared oscillator phases.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    let _ = st;
    render_worker(ins, mem, main, Some(aux), kx);
}

fn render_worker(
    ins: &[Inp<'_>; MAX_PORTS],
    mem: &mut [f32],
    out: &mut [f32],
    mut aux_out: Option<&mut [f32]>,
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        if let Some(aux) = aux_out {
            aux.fill(0.0);
        }
        return;
    }
    let sr = kx.sr.max(1.0);
    let freq = ins[0].first().clamp(20.0, sr * 0.24);
    let harmonics = ins[1].first().clamp(0.0, 1.0);
    let timbre = ins[2].first().clamp(0.0, 1.0);
    let morph = ins[3].first().clamp(0.0, 1.0);
    let auxiliary = ins[4].first() >= 0.5;
    let width = (0.5 + 0.45 * morph).clamp(0.05, 0.95);
    let overtone = timbre * (2.0 - timbre);
    let step = (freq / sr).min(0.24);
    for (i, sample) in out.iter_mut().enumerate() {
        mem[0] = (mem[0] + step).fract();
        mem[1] = (mem[1] + step).fract();
        let shaped = shape(slope(mem[0], width), harmonics);
        let folded = fold(shaped, timbre);
        let triangle = slope(mem[1], 0.5);
        let sine = (TAU * (triangle * 0.25 + 0.5)).sin();
        let aux_value = sine + (fold(shaped * 1.7, timbre) - sine) * overtone;
        *sample = if auxiliary { aux_value } else { folded };
        if let Some(aux) = aux_out.as_deref_mut() {
            aux[i] = if auxiliary { folded } else { aux_value };
        }
    }
}
