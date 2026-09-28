//! Original stereo audio placement for two Streams firmware CV-only roles.
//! The published filter controller and Lorenz generator do not process audio.

use std::f32::consts::TAU;

use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

pub const MEM_LEN: usize = 16;

pub const FILTER_PARAMS: &[ParamDef] = &[
    ParamDef::new("offset", 0.5, 0.0, 1.0),
    ParamDef::new("amount", 0.5, 0.0, 1.0),
    ParamDef::new("excite-source", 0.0, 0.0, 1.0),
    ParamDef::new("excite", 0.0, -1.0, 1.0),
    ParamDef::unit("cutoff-min", 80.0, 20.0, 12_000.0, "Hz"),
    ParamDef::unit("cutoff-max", 16_000.0, 100.0, 20_000.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

pub const LORENZ_PARAMS: &[ParamDef] = &[
    ParamDef::new("rate", 0.4, 0.0, 1.0),
    ParamDef::new("balance", 0.5, 0.0, 1.0),
    ParamDef::new("excite-source", 0.0, 0.0, 1.0),
    ParamDef::new("excite", 0.0, -1.0, 1.0),
    ParamDef::unit("cutoff-min", 80.0, 20.0, 12_000.0, "Hz"),
    ParamDef::unit("cutoff-max", 16_000.0, 100.0, 20_000.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

pub fn init(kind: EffectKind, st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    mem.fill(0.0);
    if kind == EffectKind::StreamLorenz && mem.len() >= MEM_LEN {
        mem[0] = 0.1;
        mem[8] = 0.1;
    }
}

fn cutoff_filter(state: &mut f32, input: f32, cv: f32, p: &[f32], sr: f32) -> f32 {
    let hz = (p[4] + (p[5] - p[4]) * cv.clamp(0.0, 1.0)).clamp(20.0, sr * 0.45);
    let coefficient = 1.0 - (-TAU * hz / sr).exp();
    *state += (input - *state) * coefficient;
    (*state).clamp(-4.0, 4.0)
}

fn filter_lane(state: &mut [f32], input: f32, excite: f32, p: &[f32], sr: f32) -> f32 {
    let smoothing = 1.0 - (-1.0 / (0.006 * sr)).exp();
    state[0] += (p[0] - state[0]) * smoothing;
    let centered = (p[1] - 0.5) * 2.0;
    let signed_amount = centered.signum() * centered * centered;
    state[1] += (signed_amount - state[1]) * smoothing;
    let cv = (state[0] + excite.clamp(-1.0, 1.0) * state[1]).clamp(0.0, 1.0);
    cutoff_filter(&mut state[2], input, cv, p, sr)
}

fn lorenz_lane(state: &mut [f32], input: f32, excite: f32, p: &[f32], sr: f32, right: bool) -> f32 {
    let rate = (0.5 + p[0] * 20.0 + excite.clamp(-1.0, 1.0) * 5.0).clamp(0.05, 25.0);
    let dt = (rate / sr).min(0.003);
    let (x, y, z) = (state[0], state[1], state[2]);
    let next_x = x + dt * 10.0 * (y - x);
    let next_y = y + dt * (x * (28.0 - z) - y);
    let next_z = z + dt * (x * y - (8.0 / 3.0) * z);
    if next_x.is_finite()
        && next_y.is_finite()
        && next_z.is_finite()
        && next_x.abs().max(next_y.abs()).max(next_z.abs()) < 100.0
    {
        state[0] = next_x;
        state[1] = next_y;
        state[2] = next_z;
    } else {
        state[..3].copy_from_slice(&[0.1, 0.0, 0.0]);
    }
    let x_cv = ((state[0] + 20.0) / 40.0).clamp(0.0, 1.0);
    let z_cv = (state[2] / 50.0).clamp(0.0, 1.0);
    let (vca_cv, vcf_cv) = if right { (x_cv, z_cv) } else { (z_cv, x_cv) };
    let vca_depth = (p[1] * 2.0).min(1.0);
    let vcf_depth = ((1.0 - p[1]) * 2.0).min(1.0);
    let gain = 1.0 - vca_depth + vca_depth * vca_cv;
    let cutoff_cv = 1.0 - vcf_depth + vcf_depth * vcf_cv;
    cutoff_filter(&mut state[3], input, cutoff_cv, p, sr) * gain
}

/// Apply authored filter or bounded Lorenz controls to independent stereo audio.
pub fn process(
    kind: EffectKind,
    p: &[f32],
    _st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if p.len() < FILTER_PARAMS.len() || mem.len() < MEM_LEN {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let filter = kind == EffectKind::StreamFilter;
    let sidechain = p[2] >= 0.5;
    let sr = ctx.sr.max(1.0);
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        let (audio_l, audio_r) = (*left, *right);
        let detect_l = if sidechain && audio_r.abs() > 1.0e-5 {
            audio_r
        } else {
            audio_l
        } + p[3];
        let detect_r = audio_r + p[3];
        *left = if filter {
            filter_lane(&mut mem[..8], audio_l, detect_l, p, sr)
        } else {
            lorenz_lane(&mut mem[..8], audio_l, detect_l, p, sr, false)
        };
        *right = if filter {
            filter_lane(&mut mem[8..], audio_r, detect_r, p, sr)
        } else {
            lorenz_lane(&mut mem[8..], audio_r, detect_r, p, sr, true)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{lorenz_lane, LORENZ_PARAMS};

    #[test]
    fn lorenz_channel_index_swaps_cv_roles_and_remains_bounded() {
        let p: Vec<_> = LORENZ_PARAMS.iter().map(|def| def.default).collect();
        let mut left = [0.0; 8];
        let mut right = [0.0; 8];
        left[0] = 0.1;
        right[0] = 0.1;
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut channel_difference = 0.0;
        for frame in 0..192_000 {
            let l = lorenz_lane(&mut left, 0.4, 0.0, &p, 48_000.0, false);
            let r = lorenz_lane(&mut right, 0.4, 0.0, &p, 48_000.0, true);
            if frame > 48_000 {
                min = min.min(l);
                max = max.max(l);
                channel_difference += (l - r).abs();
            }
            assert!(l.is_finite() && r.is_finite());
        }
        assert!(max - min > 0.01, "chaotic control must vary over time");
        assert!(channel_difference > 100.0, "index swaps x/z CV roles");
        assert!(left[..3].iter().chain(&right[..3]).all(|v| v.abs() < 100.0));
    }
}
