//! Original four-lane Frames PolyLfo role adaptation.
//! Analytic waves replace generated tables; stereo selects any two lanes,
//! while an opt-in four-output template emits four synchronized core lanes.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 8;

/// Host-rate parameters for one four-lane poly-LFO step. Spread, shape,
/// shape-spread, coupling and offset use the same normalized 0..1 controls
/// as the `frame-lfo-core` node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolyLfoParams {
    pub sample_rate: f32,
    pub frequency_hz: f32,
    pub shape: f32,
    pub spread: f32,
    pub shape_spread: f32,
    pub coupling: f32,
    pub offset: f32,
}

/// Advances the original four-lane poly-LFO adaptation and returns each
/// unipolar lane. `phase` stores four lane phases followed by four previous
/// neighbor sine values; callers own this fixed-size state.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn next_lanes(phase: &mut [f32; STATE_FLOATS], params: PolyLfoParams) -> [f32; 4] {
    let sr = params.sample_rate.max(1.0);
    let hz = params.frequency_hz.clamp(0.01, sr * 0.4);
    let shape = unit(params.shape);
    let spread = unit(params.spread) * 2.0 - 1.0;
    let shape_spread = unit(params.shape_spread) * 2.0 - 1.0;
    let coupling = unit(params.coupling) * 2.0 - 1.0;
    let offset = unit(params.offset);
    let old_sines = [phase[4], phase[5], phase[6], phase[7]];
    if spread >= 0.0 {
        phase[0] = (phase[0] + hz / sr).fract();
        for lane in 1..4 {
            phase[lane] = (phase[0] + lane as f32 * spread * 0.25).fract();
        }
    } else {
        for (lane, p) in phase[..4].iter_mut().enumerate() {
            let ratio = 2.0_f32.powf(lane as f32 * -spread * 0.35);
            *p = (*p + (hz * ratio).min(sr * 0.4) / sr).fract();
        }
    }
    let mut lanes = [0.0; 4];
    for lane in 0..4 {
        let neighbor = if coupling >= 0.0 {
            (lane + 1) % 4
        } else {
            (lane + 3) % 4
        };
        let p = phase[lane] + offset + old_sines[neighbor] * coupling.abs() * 0.16;
        let local_shape = (shape + lane as f32 * shape_spread * 0.16).clamp(0.0, 1.0);
        lanes[lane] = (wave(p, local_shape) + 1.0) * 0.5;
        phase[4 + lane] = (p * TAU).sin();
    }
    lanes
}

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn wave(phase: f32, shape: f32) -> f32 {
    let p = phase.rem_euclid(1.0);
    let sine = (p * TAU).sin();
    let triangle = 1.0 - 4.0 * (p - 0.5).abs();
    let saw = p * 2.0 - 1.0;
    let pulse = if p < 0.5 { 1.0 } else { -1.0 };
    let x = unit(shape) * 3.0;
    let i = x.floor().min(2.0) as u8;
    let blend = if x >= 3.0 { 1.0 } else { x.fract() };
    match i {
        0 => sine + (triangle - sine) * blend,
        1 => triangle + (saw - triangle) * blend,
        _ => saw + (pulse - saw) * blend,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    for (frame, sample) in out.iter_mut().enumerate() {
        let selected = ins[6].at(frame).round().clamp(0.0, 3.0) as usize;
        if st.u[0] == 0 {
            mem[..4].fill(0.0);
            mem[4..].fill(0.0);
            st.u[0] = 1;
        }
        let mut phase = [0.0; STATE_FLOATS];
        phase.copy_from_slice(&mem[..STATE_FLOATS]);
        let lanes = next_lanes(
            &mut phase,
            PolyLfoParams {
                sample_rate: sr,
                frequency_hz: ins[0].at(frame),
                shape: ins[1].at(frame),
                spread: ins[2].at(frame),
                shape_spread: ins[3].at(frame),
                coupling: ins[4].at(frame),
                offset: ins[5].at(frame),
            },
        );
        mem[..STATE_FLOATS].copy_from_slice(&phase);
        *sample = lanes[selected];
    }
}
