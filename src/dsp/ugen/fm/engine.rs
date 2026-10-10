//! Fixed-state FM6 engine shared by patch and macro entry points.

use super::super::{Inp, Kx, NodeState, MAX_PORTS};
use super::algorithms::{algorithm, Algorithm, RENDER_ORDER};
use super::envelope::{log_to_amp, Eg, EG_BLOCK, EG_FLOATS};
use super::patch::{global, op, op_base, Fm6Patch, OP_PARAMS};
use super::scaling::{
    feedback_gain, midi_key, op_freq_hz, op_outlevel, op_rate_scaling, transpose_hz, velocity_midi,
};

pub const FM6_PORTS: [(&str, f32); 6] = [
    ("freq", 440.0),
    ("velocity", 1.0),
    ("algorithm", 0.0),
    ("ratio", 1.0),
    ("index", 1.0),
    ("fm6-feedback", 4.0),
];

pub mod fm6_port {
    pub const FREQ: usize = 0;
    pub const VELOCITY: usize = 1;
    pub const ALGORITHM: usize = 2;
    pub const RATIO: usize = 3;
    pub const INDEX: usize = 4;
    pub const FEEDBACK: usize = 5;
}

pub mod fm_mod_port {
    pub const IN: usize = 0;
    pub const MOD: usize = 1;
    pub const INDEX: usize = 2;
    pub const ALGORITHM: usize = 3;
    pub const FREQ: usize = 4;
    pub const RATIO: usize = 5;
    pub const VELOCITY: usize = 6;
}

pub const MACRO_FEEDBACK: u8 = 4;

const OP_STRIDE: usize = 6 + EG_FLOATS;
const GLOBALS: usize = 6 * OP_STRIDE;
/// Number of per-voice floats used by the engine (six operator records plus globals).
pub const STATE_FLOATS: usize = GLOBALS + 4;

const PHASE: usize = 0;
const HISTORY_1: usize = 1;
const HISTORY_2: usize = 2;
const FREQUENCY: usize = 3;
const AMP_FROM: usize = 4;
const AMP_TO: usize = 5;
const EG: usize = 6;

const ALG: usize = GLOBALS;
const FEEDBACK: usize = GLOBALS + 1;
const MACRO_INDEX: usize = GLOBALS + 2;
const MODE: usize = GLOBALS + 3;
const MODE_PATCH: f32 = 1.0;
const MODE_FM6_MACRO: f32 = 2.0;
const MODE_FM_MOD: f32 = 3.0;
const TAU: f32 = std::f32::consts::TAU;

/// Renders the six-operator core. The patch, when present, selects patch mode;
/// otherwise the six macro controls in `ins` select the algorithm and sound.
pub fn fm6_render(
    ins: &[Inp<'_>; MAX_PORTS],
    patch: Option<&Fm6Patch>,
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        st.finish();
        return;
    }
    render(ins, patch, false, st, mem, out, kx);
}

/// Renders the macro operator set used by `fm-mod`'s algorithm mode.
/// Its carrier level is held at unity; the template owns note lifetime.
pub fn fm_mod_algorithm(
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
    render(ins, None, true, st, mem, out, kx);
}

fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    patch: Option<&Fm6Patch>,
    fm_mod: bool,
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if st.done() {
        out.fill(0.0);
        return;
    }
    if st.u[0] == 0 {
        setup(ins, patch, fm_mod, st, mem, kx);
        st.u[0] = 1;
        st.u[2] = u32::MAX;
    }
    if mem[..STATE_FLOATS].iter().any(|value| !value.is_finite()) {
        mem[..STATE_FLOATS].fill(0.0);
        st.u[0] = 1;
        st.finish();
        out.fill(0.0);
        return;
    }

    let frame_start = st.u[1] as usize;
    let sample_rate = finite_or(kx.sr, 48_000.0).max(1.0);
    if kx.gate < out.len() && st.u[2] == u32::MAX {
        st.u[2] = frame_start.saturating_add(kx.gate).min(u32::MAX as usize) as u32;
    }
    let keyoff_frame = st.u[2] as usize;
    let alg = algorithm(mem[ALG] as u8);
    let feedback = mem[FEEDBACK];
    let mode = mem[MODE];
    let mut released = [false; 6];

    for (frame, sample) in out.iter_mut().enumerate() {
        let absolute = frame_start.saturating_add(frame);
        let chunk_frame = absolute % EG_BLOCK;
        if mode == MODE_PATCH && chunk_frame == 0 {
            for operator in 0..6 {
                let base = operator * OP_STRIDE;
                let mut eg = Eg::load(&mem[base + EG..base + EG + EG_FLOATS]);
                // A key-off takes effect at the next 64-frame chunk boundary.
                if absolute >= keyoff_frame {
                    eg.key_off();
                }
                let level = eg.tick();
                mem[base + AMP_FROM] = mem[base + AMP_TO];
                mem[base + AMP_TO] = log_to_amp(level);
                eg.store(&mut mem[base + EG..base + EG + EG_FLOATS]);
                released[operator] = eg.released_and_silent();
            }
        }

        let mut outputs = [0.0_f32; 6];
        let mut carrier_sum = 0.0;
        let index_env = macro_index_envelope(absolute, sample_rate);
        let carrier_env = if mode == MODE_FM_MOD {
            1.0
        } else if mode == MODE_FM6_MACRO {
            macro_carrier_envelope(absolute, keyoff_frame, sample_rate)
        } else {
            1.0
        };

        for &operator in &RENDER_ORDER {
            let index = usize::from(operator - 1);
            let base = index * OP_STRIDE;
            let muted = mem[base + FREQUENCY] < 0.0;
            let mut phase_mod = 0.0;
            let mut mask = alg.modulators[index];
            while mask != 0 {
                let source = mask.trailing_zeros() as usize;
                phase_mod += outputs[source];
                mask &= mask - 1;
            }
            if alg.feedback.1 == operator {
                let source = usize::from(alg.feedback.0 - 1);
                // Every edge uses delayed source history; alg 4/6 intentionally differ from msfa.
                // msfa skips those multi-operator feedback edges, so no parity tolerance applies.
                phase_mod += feedback
                    * (mem[source * OP_STRIDE + HISTORY_1] + mem[source * OP_STRIDE + HISTORY_2])
                    * 0.5;
            }

            let amplitude = if mode == MODE_PATCH {
                let from = mem[base + AMP_FROM];
                let to = mem[base + AMP_TO];
                let fraction = (chunk_frame + 1) as f32 / EG_BLOCK as f32;
                from + (to - from) * fraction
            } else if alg.carriers & (1 << index) != 0 {
                carrier_env
            } else {
                mem[MACRO_INDEX] / TAU * index_env
            };
            let value = if muted {
                0.0
            } else {
                amplitude * (TAU * (mem[base + PHASE] + phase_mod)).sin()
            };
            outputs[index] = value;
            if alg.carriers & (1 << index) != 0 {
                carrier_sum += value;
            }
            if !muted {
                let phase_step = (mem[base + FREQUENCY] / sample_rate).clamp(-0.49, 0.49);
                mem[base + PHASE] = (mem[base + PHASE] + phase_step).rem_euclid(1.0);
            }
        }

        for operator in 0..6 {
            let base = operator * OP_STRIDE;
            mem[base + HISTORY_2] = mem[base + HISTORY_1];
            mem[base + HISTORY_1] = outputs[operator];
        }
        *sample = (carrier_sum / carrier_count(&alg) as f32).clamp(-1.0, 1.0);

        if mode == MODE_PATCH && absolute >= keyoff_frame && chunk_frame == EG_BLOCK - 1 {
            for operator in 0..6 {
                let base = operator * OP_STRIDE;
                let eg = Eg::load(&mem[base + EG..base + EG + EG_FLOATS]);
                released[operator] = eg.released_and_silent();
            }
        }
        st.u[1] = absolute.saturating_add(1).min(u32::MAX as usize) as u32;
    }

    if mode == MODE_PATCH && (0..6).all(|index| alg.carriers & (1 << index) == 0 || released[index])
    {
        st.finish();
    } else if mode == MODE_FM6_MACRO && keyoff_frame != u32::MAX as usize {
        let elapsed = st.u[1].saturating_sub(keyoff_frame as u32) as f32;
        if elapsed >= sample_rate * 0.3
            && macro_carrier_envelope(st.u[1] as usize, keyoff_frame, sample_rate) < 1.0e-4
        {
            st.finish();
        }
    }
    if mem[..STATE_FLOATS].iter().any(|value| !value.is_finite()) {
        mem[..STATE_FLOATS].fill(0.0);
        st.finish();
        out.fill(0.0);
    }
}

fn setup(
    ins: &[Inp<'_>; MAX_PORTS],
    patch: Option<&Fm6Patch>,
    fm_mod: bool,
    st: &mut NodeState,
    mem: &mut [f32],
    kx: &Kx<'_>,
) {
    mem[..STATE_FLOATS].fill(0.0);
    let patch = patch.filter(|candidate| candidate.validate().is_ok());
    let freq = finite_or(
        ins[if fm_mod {
            fm_mod_port::FREQ
        } else {
            fm6_port::FREQ
        }]
        .first(),
        440.0,
    )
    .clamp(0.0, 20_000.0);
    let velocity = finite_or(
        ins[if fm_mod {
            fm_mod_port::VELOCITY
        } else {
            fm6_port::VELOCITY
        }]
        .first(),
        1.0,
    )
    .clamp(0.0, 1.0);
    let (alg, feedback, mode, index_radians) = if let Some(patch) = patch {
        (
            patch.algorithm().saturating_add(1),
            feedback_gain(patch.params[global::FEEDBACK]),
            MODE_PATCH,
            0.0,
        )
    } else {
        let (algorithm_value, ratio_index, index_index, feedback_index) = if fm_mod {
            (
                ins[fm_mod_port::ALGORITHM].first(),
                fm_mod_port::RATIO,
                fm_mod_port::INDEX,
                MACRO_FEEDBACK as f32,
            )
        } else {
            (
                ins[fm6_port::ALGORITHM].first(),
                fm6_port::RATIO,
                fm6_port::INDEX,
                ins[fm6_port::FEEDBACK].first(),
            )
        };
        let algorithm_value = finite_or(algorithm_value, 1.0).round().clamp(1.0, 32.0) as u8;
        let ratio = finite_or(ins[ratio_index].first(), 1.0).clamp(0.0625, 32.0);
        let index = finite_or(ins[index_index].first(), 1.0).clamp(0.0, 32.0) * velocity;
        let fb = if fm_mod {
            feedback_index as u8
        } else {
            finite_or(feedback_index, 4.0).round().clamp(0.0, 7.0) as u8
        };
        let mode = if fm_mod { MODE_FM_MOD } else { MODE_FM6_MACRO };
        for operator in 1_usize..=6 {
            let base = (operator - 1) * OP_STRIDE;
            let is_carrier = algorithm(algorithm_value).carriers & (1 << (operator - 1)) != 0;
            let ratio = if is_carrier { 1.0 } else { ratio };
            mem[base + FREQUENCY] = freq * ratio;
            mem[base + AMP_FROM] = 1.0;
            mem[base + AMP_TO] = 1.0;
        }
        (algorithm_value, feedback_gain(fb), mode, index)
    };
    mem[ALG] = alg as f32;
    mem[FEEDBACK] = feedback;
    mem[MACRO_INDEX] = index_radians;
    mem[MODE] = mode;

    if let Some(patch) = patch {
        let key = midi_key(freq);
        let vel = velocity_midi(velocity);
        let transposed = transpose_hz(freq, patch.params[global::TRANSPOSE]);
        for operator in 1_usize..=6 {
            let base = (operator - 1) * OP_STRIDE;
            let param_base = op_base(usize::from(operator));
            let params = &patch.params[param_base..param_base + OP_PARAMS];
            mem[base + FREQUENCY] = op_freq_hz(
                transposed,
                params[op::OSC_MODE],
                params[op::COARSE],
                params[op::FINE],
                params[op::DETUNE],
            );
            let rates = std::array::from_fn(|index| params[op::R1 + index]);
            let levels = std::array::from_fn(|index| params[op::L1 + index]);
            let eg = Eg::new(
                rates,
                levels,
                op_outlevel(patch, usize::from(operator), key, vel),
                op_rate_scaling(patch, usize::from(operator), key),
                kx.sr,
            );
            eg.store(&mut mem[base + EG..base + EG + EG_FLOATS]);
            mem[base + AMP_FROM] = log_to_amp(0);
            mem[base + AMP_TO] = log_to_amp(0);
        }
    }
    st.u[0] = 0;
    st.u[1] = 0;
    st.u[2] = u32::MAX;
}

fn carrier_count(algorithm: &Algorithm) -> u32 {
    algorithm.carriers.count_ones().max(1)
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn macro_index_envelope(frame: usize, sample_rate: f32) -> f32 {
    let sr = finite_or(sample_rate, 48_000.0).max(1.0);
    let attack = (sr * 0.01).max(1.0);
    let t = frame as f32;
    if t < attack {
        t / attack
    } else {
        (-(t - attack) / (sr * 0.8)).exp()
    }
}

fn macro_carrier_envelope(frame: usize, keyoff_frame: usize, sample_rate: f32) -> f32 {
    let sr = finite_or(sample_rate, 48_000.0).max(1.0);
    let attack = (sr * 0.01).max(1.0);
    if keyoff_frame == u32::MAX as usize || frame < keyoff_frame {
        (frame as f32 / attack).min(1.0)
    } else {
        let release = (frame - keyoff_frame) as f32;
        (-release * (std::f32::consts::LN_10 * 3.0) / (sr * 0.3)).exp()
    }
}

/// Test hook that mutes an operator, including its contribution to feedback history.
#[cfg(test)]
pub(crate) fn silence_op(mem: &mut [f32], operator: usize) {
    if (1..=6).contains(&operator) && mem.len() >= STATE_FLOATS {
        let base = (operator - 1) * OP_STRIDE;
        mem[base + PHASE] = 0.0;
        mem[base + HISTORY_1] = 0.0;
        mem[base + HISTORY_2] = 0.0;
        mem[base + FREQUENCY] = -1.0;
    }
}
