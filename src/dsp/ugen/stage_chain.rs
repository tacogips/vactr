//! Original bounded six-segment Stages chain adaptation.
//! Source segment transitions informed the roles; timing and curves are
//! analytic, host-rate, and event-local rather than firmware-identical.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 12;

fn unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn duration(control: f32) -> f32 {
    0.002 + control * control * 2.0
}

fn curve(phase: f32, shape: f32) -> f32 {
    phase.powf(2.0_f32.powf((shape - 0.5) * 5.0))
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
    render_worker(ins, st, mem, out, None, kx);
}

/// Renders value and phase outputs from one shared stage-chain update.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
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

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
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
    for (frame, sample) in out.iter_mut().enumerate() {
        let count = ins[1].at(frame).round().clamp(1.0, 6.0) as usize;
        let gate = ins[2].at(frame) >= 0.5;
        let trigger = ins[3].at(frame) >= 0.5;
        let channel = ins[4].at(frame) >= 0.5;
        let gate_rise = gate && mem[6] < 0.5;
        let gate_fall = !gate && mem[6] >= 0.5;
        let trigger_rise = trigger && mem[7] < 0.5;
        mem[6] = f32::from(gate);
        mem[7] = f32::from(trigger);
        if st.u[0] == 0 || gate_rise || trigger_rise {
            let prior = (mem[0] as usize).min(count - 1);
            let prior_step = ins[5 + prior * 4].at(frame).round() as u8 == 1;
            let next = if st.u[0] != 0 && prior_step {
                (prior + 1) % count
            } else {
                0
            };
            mem[0] = next as f32; // active segment
            mem[1] = 0.0; // segment phase
            mem[2] = mem[3]; // starting value
            mem[5] = 1.0; // running
            st.u[0] = 1;
        }
        let mut loop_start = None;
        let mut loop_end = None;
        for index in 0..count {
            if ins[5 + index * 4 + 1].at(frame) >= 0.5 {
                loop_start.get_or_insert(index);
                loop_end = Some(index);
            }
        }
        if gate_fall && loop_end.is_some_and(|end| end + 1 < count) {
            mem[0] = (loop_end.unwrap_or(0) + 1) as f32;
            mem[1] = 0.0;
            mem[2] = mem[3];
        }
        let active = (mem[0] as usize).min(count - 1);
        let port = 5 + active * 4;
        let typ = ins[port].at(frame).round().clamp(0.0, 3.0) as u8;
        let primary = unit(ins[port + 2].at(frame));
        let secondary = unit(ins[port + 3].at(frame));
        let mut complete = false;
        let mut target = mem[3];
        if mem[5] >= 0.5 {
            match typ {
                0 => {
                    mem[1] = (mem[1] + 1.0 / (duration(primary) * sr)).min(1.0);
                    target = if active + 1 == count {
                        0.0
                    } else {
                        unit(ins[5 + (active + 1) * 4 + 2].at(frame))
                    };
                    target = mem[2] + (target - mem[2]) * curve(mem[1], secondary);
                    complete = mem[1] >= 1.0;
                }
                1 => {
                    let alpha = 1.0 - (-1.0 / (duration(secondary) * sr)).exp();
                    target = mem[3] + (primary - mem[3]) * alpha;
                    // A step waits for the next gate/trigger edge or loop exit.
                    complete = false;
                }
                2 => {
                    target = primary;
                    mem[1] = (mem[1] + 1.0 / (duration(secondary) * sr)).min(1.0);
                    complete = mem[1] >= 1.0;
                }
                _ => {
                    let hz = (ins[0].at(frame) * 2.0_f32.powf((primary - 0.5) * 2.0))
                        .clamp(0.01, sr * 0.4);
                    mem[4] = (mem[4] + hz / sr).fract();
                    target = 0.5
                        + 0.5
                            * ((mem[4] * TAU).sin() * (1.0 - secondary)
                                + (mem[4] * 2.0 - 1.0) * secondary);
                    mem[1] = mem[4];
                    complete = mem[4] < hz / sr;
                }
            }
        }
        mem[3] = target;
        if complete {
            let next = if loop_end == Some(active) {
                loop_start.unwrap_or(0)
            } else {
                active + 1
            };
            if next >= count {
                mem[5] = 0.0;
            } else {
                mem[0] = next as f32;
                mem[1] = 0.0;
                mem[2] = target;
            }
        }
        let phase = mem[1] * 2.0 - 1.0;
        *sample = if channel { phase } else { target };
        if let Some(aux) = aux_out.as_deref_mut() {
            aux[frame] = if channel { target } else { phase };
        }
    }
}
