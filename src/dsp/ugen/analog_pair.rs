//! Plaits position 8 VA_VARIANT 2 architecture with analytic oscillators.
//!
//! Main combines variable square and saw; auxiliary is the synchronized
//! variable-shape difference. See THIRD_PARTY_NOTICES.md for MIT provenance.

use super::{Inp, Kx, NodeState, MAX_PORTS};

const INTERVALS: [f32; 5] = [0.0, 7.01, 12.01, 19.01, 24.01];
/// Four oscillator phase pairs and spare fixed state per output node.
pub const STATE_FLOATS: usize = 8;

#[derive(Clone, Copy)]
struct ShapeSpec {
    master_hz: f32,
    slave_hz: f32,
    pw: f32,
    shape: f32,
    sr: f32,
}

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn squash(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

#[inline]
fn detune(harmonics: f32) -> f32 {
    let signed = (2.05 * harmonics - 1.025).clamp(-1.0, 1.0);
    let sign = if signed < 0.0 { -1.0 } else { 1.0 };
    let position = signed.abs() * 3.9999;
    let index = position.floor() as usize;
    let fraction = squash(squash(position.fract()));
    sign * (INTERVALS[index] + fraction * (INTERVALS[index + 1] - INTERVALS[index]))
}

#[inline]
fn shape_wave(phase: f32, pw: f32, shape: f32) -> f32 {
    let square = if phase < pw { 0.0 } else { 1.0 };
    let triangle = if phase < pw {
        phase / pw
    } else {
        1.0 - (phase - pw) / (1.0 - pw)
    };
    let square_amount = (shape - 0.5).max(0.0) * 2.0;
    let triangle_amount = (1.0 - shape * 2.0).max(0.0);
    let saw = phase + square_amount * (square - phase);
    let mixed = saw + triangle_amount * (triangle - saw);
    2.0 * mixed - 1.0
}

#[inline]
fn synced_shape(mem: &mut [f32], master: usize, slave: usize, spec: ShapeSpec) -> f32 {
    let ShapeSpec {
        master_hz,
        slave_hz,
        pw,
        shape,
        sr,
    } = spec;
    mem[master] += (master_hz / sr).min(0.25);
    if mem[master] >= 1.0 {
        mem[master] -= 1.0;
        mem[slave] = 0.0;
    }
    mem[slave] = (mem[slave] + (slave_hz / sr).min(0.25)).fract();
    shape_wave(mem[slave], pw, shape)
}

#[inline]
fn variable_saw(phase: &mut f32, frequency: f32, pw: f32, shape: f32, sr: f32) -> f32 {
    *phase = (*phase + (frequency / sr).min(0.25)).fract();
    let p = *phase;
    let triangle = if p < pw {
        p / pw
    } else {
        1.0 - (p - pw) / (1.0 - pw)
    };
    let notch = if p < pw { p } else { 1.2 };
    2.0 * ((1.0 - shape) * notch + shape * triangle) / 1.2 - 1.0
}

/// Mode 0 is variable square+saw main; mode 1 is sync difference auxiliary.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render_worker(ins, st, mem, out, None, kx);
}

/// Renders the main and sync-difference outputs from one oscillator advance.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    render_worker(ins, st, mem, main, Some(aux), kx);
}

fn render_worker(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
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
    let harmonics = unit(ins[1].first());
    let timbre = unit(ins[2].first());
    let morph = unit(ins[3].first());
    let auxiliary = ins[4].first() >= 0.5;
    let auxiliary_freq = freq * 2.0_f32.powf(detune(harmonics) / 12.0);
    let sync_ratio = 2.0_f32.powf(timbre * timbre * 4.0);
    let shape = (morph * 1.5).min(1.0);
    let pw = (0.5 + (morph - 0.66) * 1.46).clamp(0.005, 0.995);
    let square_pw = (1.3 * timbre - 0.15).clamp(0.005, 0.5);
    let square_sync_ratio = if timbre < 0.5 {
        1.0
    } else {
        2.0_f32.powf((timbre - 0.5).powi(2) * 16.0)
    };
    let saw_pw = (if morph < 0.5 {
        morph + 0.5
    } else {
        2.0 - 2.0 * morph
    } * 1.1)
        .clamp(0.005, 1.0);
    let saw_shape = (10.0 - 21.0 * morph).clamp(0.0, 1.0);
    let square_gain = (timbre * 8.0).min(1.0);
    let saw_gain = (8.0 * (1.0 - morph)).clamp(0.02, 1.0);
    let normalization = square_gain.max(saw_gain).max(0.02).recip();
    if st.u[0] == 0 {
        mem[2] = 0.25;
        st.u[0] = 1;
    }
    for (i, sample) in out.iter_mut().enumerate() {
        let primary = synced_shape(
            mem,
            0,
            1,
            ShapeSpec {
                master_hz: freq,
                slave_hz: freq * sync_ratio,
                pw,
                shape,
                sr,
            },
        );
        let secondary = synced_shape(
            mem,
            2,
            3,
            ShapeSpec {
                master_hz: auxiliary_freq,
                slave_hz: auxiliary_freq * sync_ratio,
                pw,
                shape,
                sr,
            },
        );
        let square = synced_shape(
            mem,
            4,
            5,
            ShapeSpec {
                master_hz: freq,
                slave_hz: freq * square_sync_ratio,
                pw: square_pw,
                shape: 1.0,
                sr,
            },
        );
        let saw = variable_saw(&mut mem[6], auxiliary_freq, saw_pw, saw_shape, sr);
        let main_value = normalization * (0.3 * square_gain * square + 0.5 * saw_gain * saw);
        let aux_value = 0.5 * (secondary - primary);
        *sample = if auxiliary { aux_value } else { main_value };
        if let Some(aux) = aux_out.as_deref_mut() {
            aux[i] = if auxiliary { main_value } else { aux_value };
        }
    }
}
