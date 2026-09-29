//! Original stereo-derived four-lane gain mixer controlled by the existing
//! analytic Frames poly-LFO role. The four lanes are correlated transforms
//! of the stereo bus (`L`, `R`, `(L+R)/2`, `(L-R)/2`), not four independent
//! audio inputs or a model of the hardware mixer/VCA.

use super::{FxState, ParamDef};
use crate::dsp::ugen::frame_lfo;

pub const PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 2.0, 0.01, 20_000.0, "Hz"),
    ParamDef::new("shape", 0.0, 0.0, 1.0),
    ParamDef::new("spread", 0.75, 0.0, 1.0),
    ParamDef::new("shape-spread", 0.5, 0.0, 1.0),
    ParamDef::new("coupling", 0.5, 0.0, 1.0),
    ParamDef::new("offset", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const STATE_LEN: usize = frame_lfo::STATE_FLOATS;

#[inline]
fn combine(left: f32, right: f32, gains: [f32; 4]) -> (f32, f32) {
    let mid = (left + right) * 0.5;
    let side = (left - right) * 0.5;
    (
        ((gains[0] * left + gains[2] * mid + gains[3] * side) * 0.5).clamp(-4.0, 4.0),
        ((gains[1] * right + gains[2] * mid - gains[3] * side) * 0.5).clamp(-4.0, 4.0),
    )
}

pub fn init(st: &mut FxState, _mem: &mut [f32]) {
    *st = FxState::default();
}

/// Apply the four poly-LFO gain lanes in place. With derived lanes
/// `m=(L+R)/2` and `s=(L-R)/2`, the wet output is
/// `L'=(g0*L + g2*m + g3*s)/2`,
/// `R'=(g1*R + g2*m - g3*s)/2`.
///
/// At all-one gains this preserves the input exactly. For finite inputs in
/// `[-1, 1]`, the output remains within `[-1.5, 1.5]`; a defensive ±4 clamp
/// also bounds hostile or unusually hot inputs. `FxUnit` applies `mix` as a
/// conventional dry/wet blend after this fully-wet kernel.
pub fn process(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    if p.len() < PARAMS.len() {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let hz = p[0].clamp(PARAMS[0].min, PARAMS[0].max);
    let n = l.len().min(r.len());
    for index in 0..n {
        let left = l[index];
        let right = r[index];
        let gains = frame_lfo::next_lanes(
            (&mut st.s[..STATE_LEN])
                .try_into()
                .expect("fixed poly-LFO state"),
            frame_lfo::PolyLfoParams {
                sample_rate: sr,
                frequency_hz: hz,
                shape: p[1],
                spread: p[2],
                shape_spread: p[3],
                coupling: p[4],
                offset: p[5],
            },
        );
        (l[index], r[index]) = combine(left, right, gains);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(changes: &[(usize, f32)]) -> ([f32; 512], [f32; 512]) {
        let mut p: Vec<_> = PARAMS.iter().map(|param| param.default).collect();
        for &(index, value) in changes {
            p[index] = value;
        }
        let mut state = FxState::default();
        let mut left = [0.31; 512];
        let mut right = [-0.17; 512];
        process(&p, &mut state, &mut left, &mut right, 48_000.0);
        (left, right)
    }

    fn delta(a: &([f32; 512], [f32; 512]), b: &([f32; 512], [f32; 512])) -> f32 {
        a.0.iter()
            .chain(&a.1)
            .zip(b.0.iter().chain(&b.1))
            .map(|(x, y)| (x - y).abs())
            .sum::<f32>()
    }

    #[test]
    fn four_lane_recombination_preserves_unity_and_is_bounded() {
        assert_eq!(combine(0.4, -0.2, [1.0; 4]), (0.4, -0.2));

        let (left, right) = combine(100.0, -100.0, [1.0; 4]);
        assert!(left.is_finite() && right.is_finite() && left.abs() <= 4.0 && right.abs() <= 4.0);
    }

    #[test]
    fn every_poly_lfo_control_changes_the_stereo_output() {
        let baseline = render(&[]);
        for (index, value) in [
            (0, 7.0),
            (1, 0.72),
            (2, 0.12),
            (3, 0.94),
            (4, 0.91),
            (5, 0.43),
        ] {
            assert!(
                delta(&baseline, &render(&[(index, value)])) > 1.0e-3,
                "poly-LFO parameter {index}"
            );
        }
    }

    #[test]
    fn supports_rates_and_arbitrary_block_sizes_without_allocations() {
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for size in [1, 17, 64, 255, 256, 511] {
                let mut state = FxState::default();
                let mut left = vec![0.2; size];
                let mut right = vec![-0.3; size];
                let p: Vec<_> = PARAMS.iter().map(|param| param.default).collect();
                let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
                    process(&p, &mut state, &mut left, &mut right, sr);
                });
                assert_eq!(allocations, 0, "{sr} Hz, {size} frames");
                assert!(left
                    .iter()
                    .chain(&right)
                    .all(|sample| sample.is_finite() && sample.abs() <= 4.0));
            }
        }
    }
}
