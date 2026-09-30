//! Original image-source, Schroeder, dispersive spring and modulated/shimmer reverbs.
//! Research concepts are credited in design-docs/references/musicdsp-audit.md.
//! No Eventide implementation or MusicDSP snippet is incorporated.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

pub(crate) mod common;
pub(crate) mod diffuse;
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;
use common::{damping, rt60, value, Bank};

const EARLY: &[ParamDef] = &[
    ParamDef::unit("room-x", 7.0, 1.0, 30.0, "m"),
    ParamDef::unit("room-y", 5.0, 1.0, 30.0, "m"),
    ParamDef::unit("room-z", 3.0, 1.0, 15.0, "m"),
    ParamDef::new("source-x", 0.3, 0.05, 0.95),
    ParamDef::new("source-y", 0.4, 0.05, 0.95),
    ParamDef::new("source-z", 0.5, 0.05, 0.95),
    ParamDef::new("listener-x", 0.7, 0.05, 0.95),
    ParamDef::new("listener-y", 0.6, 0.05, 0.95),
    ParamDef::new("listener-z", 0.5, 0.05, 0.95),
    ParamDef::new("wall-loss", 0.35, 0.0, 1.0),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
const SCHROEDER: &[ParamDef] = &[
    ParamDef::new("size", 0.5, 0.0, 1.0),
    ParamDef::unit("decay", 2.0, 0.1, 15.0, "s"),
    ParamDef::new("damping", 0.45, 0.0, 0.98),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
const SPRING: &[ParamDef] = &[
    ParamDef::unit("length", 0.045, 0.01, 0.09, "s"),
    ParamDef::new("tension", 0.5, 0.0, 1.0),
    ParamDef::new("dispersion", 0.65, 0.0, 1.0),
    ParamDef::unit("decay", 2.5, 0.1, 12.0, "s"),
    ParamDef::new("loss", 0.4, 0.0, 0.95),
    ParamDef::new("drive", 1.4, 1.0, 8.0),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    match kind {
        EffectKind::EarlyReflections => EARLY,
        EffectKind::SchroederReverb => SCHROEDER,
        EffectKind::SpringReverb => SPRING,
        _ => diffuse::params(kind),
    }
}
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    match kind {
        EffectKind::EarlyReflections => Bank::new(0.21, sr).len(2),
        EffectKind::SchroederReverb => Bank::new(0.11, sr).len(12),
        EffectKind::SpringReverb => Bank::new(0.12, sr).len(2),
        _ => diffuse::mem_len(kind, sr),
    }
}
pub(super) fn init(_kind: EffectKind, _st: &mut FxState, _len: usize, _sr: f32) {}
#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if mem.len() < mem_len(kind, ctx.sr) {
        return;
    }
    match kind {
        EffectKind::EarlyReflections => early(p, mem, l, r, ctx.sr),
        EffectKind::SchroederReverb => schroeder(p, mem, l, r, ctx.sr),
        EffectKind::SpringReverb => spring(p, mem, l, r, ctx.sr),
        _ => diffuse::process(kind, p, st, mem, l, r, ctx.sr),
    }
}
fn early(p: &[f32], mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let bank = Bank::new(0.21, sr);
    let dims = [value(p, 0, 7.0), value(p, 1, 5.0), value(p, 2, 3.0)];
    let source = [
        value(p, 3, 0.3) * dims[0],
        value(p, 4, 0.4) * dims[1],
        value(p, 5, 0.5) * dims[2],
    ];
    let listener = [
        value(p, 6, 0.7) * dims[0],
        value(p, 7, 0.6) * dims[1],
        value(p, 8, 0.5) * dims[2],
    ];
    let reflect = 1.0 - value(p, 9, 0.35);
    let mut taps = [(0.0, 0.0); 6];
    for (wall, tap) in taps.iter_mut().enumerate() {
        let axis = wall / 2;
        let mut image = source;
        image[axis] = if wall % 2 == 0 {
            -source[axis]
        } else {
            2.0 * dims[axis] - source[axis]
        };
        let distance = image
            .iter()
            .zip(listener)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();
        *tap = (distance / 343.0 * sr, reflect / (1.0 + distance));
    }
    for (left, right) in l.iter_mut().zip(r) {
        let input = [*left, *right];
        let mut out = [0.0; 2];
        for channel in 0..2 {
            for (delay, gain) in taps {
                out[channel] += bank.read(mem, channel, delay) * gain;
            }
            bank.write(mem, channel, input[channel]);
        }
        *left = out[0];
        *right = out[1];
    }
}
fn schroeder(p: &[f32], mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let bank = Bank::new(0.11, sr);
    let scale = 0.5 + value(p, 0, 0.5) * 1.5;
    let decay = value(p, 1, 2.0);
    let loss = value(p, 2, 0.45);
    let bases = [0.0297, 0.0371, 0.0413, 0.0479];
    for (left, right) in l.iter_mut().zip(r) {
        let mut outs = [0.0; 2];
        for (channel, input) in [*left, *right].into_iter().enumerate() {
            let mut out = 0.0;
            for (k, base) in bases.iter().enumerate() {
                let id = channel * 6 + k;
                let delay = base * scale + (channel as f32) * 0.00073;
                let y = bank.read(mem, id, (delay * sr).round());
                let filtered = damping(mem, 32 + id, y, loss);
                bank.write(mem, id, input * 0.5 + filtered * rt60(delay, decay));
                out += y * 0.25;
            }
            out = bank.allpass(mem, channel * 6 + 4, out, 0.0053 * scale * sr, 0.65);
            outs[channel] = bank.allpass(mem, channel * 6 + 5, out, 0.0017 * scale * sr, 0.55);
        }
        *left = outs[0];
        *right = outs[1];
    }
}
fn spring(p: &[f32], mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let bank = Bank::new(0.12, sr);
    let delay = value(p, 0, 0.045) / (0.8 + 0.4 * value(p, 1, 0.5));
    let dispersion = value(p, 2, 0.65);
    let gain = rt60(delay + 24.0 / sr, value(p, 3, 2.5));
    let loss = value(p, 4, 0.4);
    let drive = value(p, 5, 1.4);
    for (left, right) in l.iter_mut().zip(r) {
        let mut outs = [0.0; 2];
        for (channel, input) in [*left, *right].into_iter().enumerate() {
            let mut wave = bank.read(
                mem,
                channel,
                (delay * sr * (1.0 + 0.017 * channel as f32)).round(),
            );
            // Cascaded first-order allpasses model frequency-dependent wave velocity.
            // Each stage is lossless; damping and RT60 gain account for propagation loss.
            for stage in 0..24 {
                let slot = 48 + channel * 24 + stage;
                let a = dispersion * (0.2 + 0.65 * stage as f32 / 23.0);
                let y = -a * wave + mem[slot];
                mem[slot] = wave + a * y;
                wave = y;
            }
            outs[channel] = wave;
            let filtered = damping(mem, 32 + channel, wave, loss);
            let excitation = (input * drive).tanh() / drive;
            bank.write(mem, channel, excitation * 0.5 + gain * filtered);
        }
        *left = outs[0];
        *right = outs[1];
    }
}
