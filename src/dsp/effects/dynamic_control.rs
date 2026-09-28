//! Original stereo digital gain/filter effects using Streams control roles.
//!
//! Published Streams firmware emits CV for an analog VCA/VCF. These effects
//! generate their own audio gain and low-pass stages; they are not source
//! audio ports or hardware models. No generated control lookup is imported.

use std::f32::consts::TAU;

use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

pub const MEM_LEN: usize = 16;

pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("shape", 0.5, 0.0, 1.0),
    ParamDef::new("response", 0.5, 0.0, 1.0),
    ParamDef::new("global-attack", 0.3, 0.0, 1.0),
    ParamDef::new("global-decay", 0.5, 0.0, 1.0),
    ParamDef::new("alternate", 0.0, 0.0, 1.0),
    ParamDef::new("linked", 0.0, 0.0, 1.0),
    ParamDef::new("excite-source", 0.0, 0.0, 1.0),
    ParamDef::new("excite", 0.0, 0.0, 1.0),
    ParamDef::new("trigger", 0.0, 0.0, 1.0),
    ParamDef::new("gate", 0.0, 0.0, 1.0),
    ParamDef::new("threshold", 0.08, 0.0, 1.0),
    ParamDef::unit("cutoff-min", 100.0, 20.0, 12_000.0, "Hz"),
    ParamDef::unit("cutoff-max", 12_000.0, 100.0, 20_000.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    mem.fill(0.0);
}

fn times(p: &[f32]) -> (f32, f32) {
    if p[5] >= 0.5 {
        (0.005 + p[2].powi(2) * 0.995, 0.05 + p[3].powi(2) * 4.95)
    } else {
        let shape = p[0].clamp(0.0, 1.0);
        if shape < 0.5 {
            (0.005, 0.05 + shape * 3.9)
        } else {
            (0.005 + (shape - 0.5) * 0.99, 2.0 - (shape - 0.5) * 3.0)
        }
    }
}

fn envelope(
    value: &mut [f32],
    high: bool,
    trigger: bool,
    alternate: bool,
    attack: f32,
    decay: f32,
    sr: f32,
) -> f32 {
    let mut stage = value[1] as u8;
    if trigger {
        stage = 1;
        value[2] = 0.0;
    }
    if alternate && !high && stage == 3 {
        stage = 4;
        value[2] = 0.0;
    }
    let dt = 1.0 / sr;
    match stage {
        1 => {
            value[0] = (value[0] + dt / attack).min(1.0);
            if value[0] >= 1.0 {
                stage = if alternate && high { 3 } else { 2 };
            }
        }
        2 | 4 => {
            value[0] *= (-5.0 * dt / decay).exp();
            if value[0] < 1.0e-4 {
                value[0] = 0.0;
                stage = 0;
            }
        }
        3 => value[0] = 1.0,
        _ => value[0] = 0.0,
    }
    value[1] = f32::from(stage);
    value[0]
}

fn vactrol(
    value: &mut [f32],
    excitation: f32,
    trigger: bool,
    plucked: bool,
    attack: f32,
    decay: f32,
    sr: f32,
) -> f32 {
    let target = if plucked {
        if trigger {
            value[0] = 1.0;
        }
        value[0] *= (-1.0 / (decay * sr)).exp();
        value[0]
    } else {
        excitation
    };
    let time = if target > value[2] { attack } else { decay };
    let coefficient = 1.0 - (-1.0 / (time * sr)).exp();
    value[2] += (target - value[2]) * coefficient;
    value[3] += (value[2] - value[3]) * coefficient;
    // A smooth authored photoresponse, with retained second-order lag.
    let x = value[3].clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn channel(
    kind: EffectKind,
    p: &[f32],
    state: &mut [f32],
    excitation: f32,
    forced: (bool, bool),
    input: f32,
    sr: f32,
) -> f32 {
    let detect = if excitation > state[5] { 0.001 } else { 0.02 };
    state[5] += (excitation - state[5]) * (1.0 - (-1.0 / (detect * sr)).exp());
    let threshold = p[10].clamp(0.0, 1.0);
    let high = forced.0 || state[5] > threshold;
    let rising = (high && state[6] < 0.5) || (forced.1 && state[7] < 0.5);
    state[6] = if high { 1.0 } else { 0.0 };
    state[7] = if forced.1 { 1.0 } else { 0.0 };
    let (attack, decay) = times(p);
    let alternate = p[4] >= 0.5;
    let gain = if kind == EffectKind::StreamEnvelope {
        envelope(state, high, rising, alternate, attack, decay, sr)
    } else {
        vactrol(
            state,
            if high { state[5] } else { 0.0 },
            rising,
            alternate,
            attack,
            decay,
            sr,
        )
    };
    let response = p[1].clamp(0.0, 1.0);
    let amount = if response < 0.5 {
        response * 2.0
    } else {
        (1.0 - response) * 2.0
    };
    let offset = if response < 0.5 {
        0.0
    } else {
        (response - 0.5) * 2.0
    };
    let cutoff = p[11] + (p[12] - p[11]) * (offset + amount * gain).clamp(0.0, 1.0);
    let alpha = 1.0 - (-TAU * cutoff.clamp(20.0, sr * 0.45) / sr).exp();
    state[4] += (input - state[4]) * alpha;
    (state[4] * gain).clamp(-4.0, 4.0)
}

/// Process independent stereo audio with self-derived or right-sidechain
/// excitation. The right input remains a separate audio lane in both modes.
pub fn process(
    kind: EffectKind,
    p: &[f32],
    _st: &mut FxState,
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
    let sr = ctx.sr.max(1.0);
    let linked = p[5] >= 0.5;
    let sidechain = p[6] >= 0.5;
    let gate = p[9] >= 0.5;
    let trigger = p[8] >= 0.5;
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        let source_l = *left;
        let source_r = *right;
        let shared = if sidechain {
            source_r.abs()
        } else {
            (source_l.abs() + source_r.abs()) * 0.5
        };
        let extra = p[7].clamp(0.0, 1.0);
        let excite_l = if linked || sidechain {
            shared
        } else {
            source_l.abs()
        };
        let excite_r = if linked || sidechain {
            shared
        } else {
            source_r.abs()
        };
        *left = channel(
            kind,
            p,
            &mut mem[..8],
            (excite_l + extra).min(1.0),
            (gate, trigger),
            source_l,
            sr,
        );
        *right = channel(
            kind,
            p,
            &mut mem[8..16],
            (excite_r + extra).min(1.0),
            (gate, trigger),
            source_r,
            sr,
        );
    }
}
