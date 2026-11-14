use crate::dsp::effects::quad_mixer::{process, PARAMS};
use crate::dsp::effects::FxState;

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
