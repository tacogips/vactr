//! Three-head nonlinear tape feedback and allpass-diffused rhythmic echo.
use super::creative_reverb::common::{damping, value, Bank};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;
use std::f32::consts::TAU;
const TAPE: &[ParamDef] = &[
    ParamDef::unit("time", 0.22, 0.005, 0.65, "s"),
    ParamDef::new("feedback", 0.45, 0.0, 0.95),
    ParamDef::unit("tone", 4500.0, 100.0, 18000.0, "Hz"),
    ParamDef::new("drive", 1.5, 1.0, 10.0),
    ParamDef::new("wow", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
const DIFFUSION: &[ParamDef] = &[
    ParamDef::unit("time", 0.375, 0.005, 1.9, "s"),
    ParamDef::new("feedback", 0.55, 0.0, 0.95),
    ParamDef::new("diffusion", 0.65, 0.0, 0.85),
    ParamDef::unit("tone", 6000.0, 100.0, 18000.0, "Hz"),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    if kind == EffectKind::TapeDelay {
        TAPE
    } else {
        DIFFUSION
    }
}
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    if kind == EffectKind::TapeDelay {
        Bank::new(1.96, sr).len(2)
    } else {
        Bank::new(1.902, sr).len(2) + Bank::new(0.025, sr).len(6)
    }
}
pub(super) fn init(_kind: EffectKind, _st: &mut FxState, _len: usize, _sr: f32) {}
#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let sr = ctx.sr;
    if mem.len() < mem_len(kind, sr) {
        return;
    }
    let main = Bank::new(
        if kind == EffectKind::TapeDelay {
            1.96
        } else {
            1.902
        },
        sr,
    );
    let small = Bank::new(0.025, sr);
    let (history, diffusers) = mem.split_at_mut(main.len(2));
    let tape = kind == EffectKind::TapeDelay;
    let time = value(p, 0, if tape { 0.22 } else { 0.375 });
    let feedback = value(p, 1, 0.5);
    let tone = value(p, if tape { 2 } else { 3 }, 5000.0).min(sr * 0.45);
    let a = (-TAU * tone / sr).exp();
    let drive = value(p, 3, 1.5);
    let wow = value(p, 4, 0.3);
    let diffusion = value(p, 2, 0.65);
    for (left, right) in l.iter_mut().zip(r) {
        let mut outs = [0.0; 2];
        for (channel, input) in [*left, *right].into_iter().enumerate() {
            let mut wet = if tape {
                let wobble =
                    1.0 + wow * 0.003 * ((TAU * st.s[0]).sin() + 0.2 * (TAU * st.s[1]).sin());
                let d = time * sr * wobble;
                main.read(history, channel, d) * 0.5
                    + main.read(history, channel, d * 2.0) * 0.3
                    + main.read(history, channel, d * 3.0) * 0.2
            } else {
                main.read(history, channel, time * sr)
            };
            if !tape {
                for (k, t) in [0.0037, 0.0113, 0.0211].into_iter().enumerate() {
                    wet = small.allpass(diffusers, channel * 3 + k, wet, t * sr, diffusion);
                }
            }
            let filtered = damping(history, 32 + channel, wet, a);
            let fed = if tape {
                (filtered * drive).tanh() / drive
            } else {
                filtered
            };
            main.write(history, channel, input + fed * feedback);
            outs[channel] = wet;
        }
        st.s[0] = (st.s[0] + 0.53 / sr).fract();
        st.s[1] = (st.s[1] + 6.7 / sr).fract();
        *left = outs[0];
        *right = outs[1];
    }
}
