//! Original stereo digital effects informed by Streams follower/compressor CV roles.
//! Streams firmware emits CV for analog hardware; this is independent audio DSP.

use std::f32::consts::TAU;

use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

pub const MEM_LEN: usize = 32;

pub const FOLLOWER_PARAMS: &[ParamDef] = &[
    ParamDef::new("shape", 0.5, 0.0, 1.0),
    ParamDef::new("response", 0.5, 0.0, 1.0),
    ParamDef::new("global-attack", 0.3, 0.0, 1.0),
    ParamDef::new("global-decay", 0.5, 0.0, 1.0),
    ParamDef::new("alternate", 0.0, 0.0, 1.0),
    ParamDef::new("linked", 0.0, 0.0, 1.0),
    ParamDef::new("excite-source", 0.0, 0.0, 1.0),
    ParamDef::new("excite", 0.0, 0.0, 1.0),
    ParamDef::unit("cutoff-min", 100.0, 20.0, 12_000.0, "Hz"),
    ParamDef::unit("cutoff-max", 12_000.0, 100.0, 20_000.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

pub const COMPRESSOR_PARAMS: &[ParamDef] = &[
    ParamDef::new("threshold", 0.55, 0.0, 1.0),
    ParamDef::new("amount", 0.35, 0.0, 1.0),
    ParamDef::new("global-attack", 0.3, 0.0, 1.0),
    ParamDef::new("global-decay", 0.5, 0.0, 1.0),
    ParamDef::new("global-threshold", 0.55, 0.0, 1.0),
    ParamDef::new("global-amount", 0.35, 0.0, 1.0),
    ParamDef::new("alternate", 0.0, 0.0, 1.0),
    ParamDef::new("linked", 0.0, 0.0, 1.0),
    ParamDef::new("excite-source", 0.0, 0.0, 1.0),
    ParamDef::new("excite", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    *st = FxState::default();
    mem.fill(0.0);
}

fn coefficient(seconds: f32, sr: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sr)).exp()
}

fn follower_times(p: &[f32]) -> (f32, f32) {
    if p[5] >= 0.5 {
        (0.001 + p[2] * 0.099, 0.01 + p[3] * 0.99)
    } else {
        let shape = p[0].clamp(0.0, 1.0);
        if shape < 0.5 {
            (0.001 + shape * 0.002, 0.01 + shape * 0.18)
        } else {
            (0.002 + (shape - 0.5) * 0.036, 0.1 + (shape - 0.5) * 0.2)
        }
    }
}

fn follower_lane(
    state: &mut [f32],
    input: f32,
    excite: f32,
    p: &[f32],
    attack: f32,
    decay: f32,
    sr: f32,
) -> f32 {
    let low_a = 1.0 - (-TAU * 220.0 / sr).exp();
    let mid_a = 1.0 - (-TAU * 2200.0 / sr).exp();
    state[0] += (excite - state[0]) * low_a;
    state[1] += (excite - state[1]) * mid_a;
    let bands = [state[0], state[1] - state[0], excite - state[1]];
    let mut sum = 0.0;
    let mut centroid = 0.0;
    for (index, band) in bands.into_iter().enumerate() {
        let energy = band * band;
        let env = &mut state[2 + index];
        let time = if energy > *env { attack } else { decay };
        *env += (energy - *env) * coefficient(time * (1.0 + index as f32 * 0.25), sr);
        let level = env.sqrt();
        sum += level;
        centroid += level * index as f32 * 0.5;
    }
    let level = (sum * 0.6).clamp(0.0, 1.0);
    let spectral = if sum > 1.0e-8 { centroid / sum } else { 0.0 };
    state[5] += (spectral - state[5]) * coefficient(0.03, sr);
    let response = p[1].clamp(0.0, 1.0);
    let amount = (1.0 - (2.0 * response - 1.0).abs()).max(0.0);
    let offset = (response - 0.5).max(0.0) * 2.0;
    let freq_cv = (offset + amount * state[5]).clamp(0.0, 1.0);
    let alternate = p[4] >= 0.5;
    // The alternate keeps the amplitude path neutral and lets spectral CV
    // drive only the digital filter; normal mode retains envelope gain.
    let gain = if alternate { 1.0 } else { level.sqrt() };
    let cutoff = p[8] + (p[9] - p[8]) * freq_cv;
    let alpha = 1.0 - (-TAU * cutoff.clamp(20.0, sr * 0.45) / sr).exp();
    state[6] += (input - state[6]) * alpha;
    (state[6] * gain).clamp(-4.0, 4.0)
}

fn compressor_times(p: &[f32]) -> (f32, f32) {
    if p[7] >= 0.5 {
        (0.001 + p[2] * 0.499, 0.05 + p[3] * 4.95)
    } else if p[6] >= 0.5 {
        (0.002, 0.07)
    } else {
        (0.0002, 0.15)
    }
}

fn compressor_lane(
    state: &mut [f32],
    input: f32,
    excite: f32,
    p: &[f32],
    attack: f32,
    decay: f32,
    sr: f32,
) -> f32 {
    let linked = p[7] >= 0.5;
    let threshold = if linked { p[4] } else { p[0] };
    let amount = if linked { p[5] } else { p[1] };
    let energy = excite * excite;
    let time = if energy > state[0] { attack } else { decay };
    state[0] += (energy - state[0]) * coefficient(time, sr);
    let level_db = 10.0 * state[0].max(1.0e-12).log10();
    let threshold_db = -60.0 + threshold * 60.0;
    let ratio = 1.0 + amount.min(0.5) * 38.0;
    let over = level_db - threshold_db;
    let knee = if p[6] >= 0.5 {
        let width = 10.0;
        if over <= -width * 0.5 {
            0.0
        } else if over >= width * 0.5 {
            over
        } else {
            (over + width * 0.5).powi(2) / (2.0 * width)
        }
    } else {
        over.max(0.0)
    };
    let reduction = knee * (1.0 - 1.0 / ratio);
    let makeup = (amount - 0.5).max(0.0) * 24.0;
    let gain = 10.0_f32.powf((makeup - reduction) / 20.0).min(4.0);
    (input * gain).clamp(-4.0, 4.0)
}

/// Process a stereo block using authored three-band follower or compressor controls.
pub fn process(
    kind: EffectKind,
    p: &[f32],
    _st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let follower = kind == EffectKind::StreamFollower;
    let required = if follower {
        FOLLOWER_PARAMS.len()
    } else {
        COMPRESSOR_PARAMS.len()
    };
    if p.len() < required || mem.len() < MEM_LEN {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let sr = ctx.sr.max(1.0);
    let (attack, decay) = if follower {
        follower_times(p)
    } else {
        compressor_times(p)
    };
    let source = if follower { 6 } else { 8 };
    let extra = if follower { 7 } else { 9 };
    let sidechain = p[source] >= 0.5;
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        let (audio_l, audio_r) = (*left, *right);
        let excite_l = if sidechain && audio_r.abs() > 1.0e-5 {
            audio_r
        } else {
            audio_l
        } + p[extra];
        let excite_r = audio_r + p[extra];
        *left = if follower {
            follower_lane(&mut mem[..16], audio_l, excite_l, p, attack, decay, sr)
        } else {
            compressor_lane(&mut mem[..16], audio_l, excite_l, p, attack, decay, sr)
        };
        *right = if follower {
            follower_lane(&mut mem[16..], audio_r, excite_r, p, attack, decay, sr)
        } else {
            compressor_lane(&mut mem[16..], audio_r, excite_r, p, attack, decay, sr)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{follower_lane, FOLLOWER_PARAMS};

    #[test]
    fn three_band_centroid_tracks_high_frequency_above_low_frequency() {
        let p: Vec<_> = FOLLOWER_PARAMS.iter().map(|def| def.default).collect();
        let measure = |hz: f32| {
            let mut state = [0.0; 16];
            for frame in 0..96_000 {
                let phase = hz * frame as f32 * std::f32::consts::TAU / 48_000.0;
                let sample = phase.sin() * 0.4;
                let _ = follower_lane(&mut state, sample, sample, &p, 0.01, 0.1, 48_000.0);
            }
            state[5]
        };
        assert!(measure(4_000.0) > measure(150.0) + 0.2);
    }
}
