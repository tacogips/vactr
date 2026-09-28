//! Original bounded quality conversion shared by four texture adaptations.
//!
//! The published selector uses bit 0 for mono and bit 1 for low fidelity.
//! Vactrol's low-fidelity path uses host-rate two-sample hold and signed
//! 8-bit quantization; it does not copy the source 32/16 kHz SRC filter.

use super::FxState;
use crate::dsp::graph::EffectKind;

const PHASE: usize = 24;
const INPUT_L: usize = 25;
const INPUT_R: usize = 26;
const OUTPUT_L: usize = 27;
const OUTPUT_R: usize = 28;
const QUALITY: usize = 12;

#[derive(Clone, Copy)]
pub(super) struct Applied {
    mono: bool,
    low: bool,
    start_phase: usize,
}

fn code(p: &[f32]) -> u8 {
    let value = p.get(QUALITY).copied().unwrap_or(0.0);
    if value.is_finite() {
        value.round().clamp(0.0, 3.0) as u8
    } else {
        0
    }
}

pub(super) fn mono(kind: EffectKind, p: &[f32]) -> bool {
    matches!(
        kind,
        EffectKind::TextureGrain
            | EffectKind::TextureStretch
            | EffectKind::TextureLoop
            | EffectKind::TextureSpectral
    ) && code(p) & 1 != 0
}

fn quantize(x: f32) -> f32 {
    (x.clamp(-1.0, 1.0) * 127.0).round() / 127.0
}

/// Convert input in place, retaining the half-rate phase across blocks.
pub(super) fn begin(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32]) -> Option<Applied> {
    let quality = code(p);
    if quality == 0 {
        return None;
    }
    let mode = Applied {
        mono: quality & 1 != 0,
        low: quality & 2 != 0,
        start_phase: st.s[PHASE] as usize & 1,
    };
    let mut phase = mode.start_phase;
    let mut held_l = st.s[INPUT_L];
    let mut held_r = st.s[INPUT_R];
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        if mode.mono {
            let mid = (*left + *right) * 0.5;
            *left = mid;
            *right = mid;
        }
        if mode.low {
            if phase == 0 {
                held_l = quantize(*left);
                held_r = quantize(*right);
            }
            *left = held_l;
            *right = held_r;
            phase ^= 1;
        }
    }
    if mode.low {
        st.s[PHASE] = phase as f32;
        st.s[INPUT_L] = held_l;
        st.s[INPUT_R] = held_r;
    }
    Some(mode)
}

/// Convert the wet output using the same phase as its input block.
pub(super) fn end(mode: Option<Applied>, st: &mut FxState, l: &mut [f32], r: &mut [f32]) {
    let Some(mode) = mode else { return };
    let mut phase = mode.start_phase;
    let mut held_l = st.s[OUTPUT_L];
    let mut held_r = st.s[OUTPUT_R];
    for (left, right) in l.iter_mut().zip(r.iter_mut()) {
        if mode.mono {
            let mid = (*left + *right) * 0.5;
            *left = mid;
            *right = mid;
        }
        if mode.low {
            if phase == 0 {
                held_l = quantize(*left);
                held_r = quantize(*right);
            }
            *left = held_l;
            *right = held_r;
            phase ^= 1;
        }
    }
    if mode.low {
        st.s[OUTPUT_L] = held_l;
        st.s[OUTPUT_R] = held_r;
    }
}

#[cfg(test)]
mod tests {
    use super::{begin, end};
    use crate::dsp::effects::FxState;

    #[test]
    fn half_rate_hold_keeps_phase_and_sample_across_odd_blocks() {
        let mut p = [0.0; 13];
        p[12] = 2.0;
        let mut state = FxState::default();
        let (mut a, mut b) = ([0.11, 0.25, 0.38], [0.62, 0.75, 0.88]);
        let mode = begin(&p, &mut state, &mut a, &mut b);
        assert_eq!(a[0], a[1]);
        end(mode, &mut state, &mut a, &mut b);
        let (mut c, mut d) = ([0.51, 0.64], [0.91, 0.97]);
        let mode = begin(&p, &mut state, &mut c, &mut d);
        assert_eq!(c[0], a[2], "odd-block boundary keeps the held value");
        assert_ne!(c[0], c[1], "next half-rate frame updates");
        end(mode, &mut state, &mut c, &mut d);
        assert_eq!(c[0], a[2]);
    }
}
