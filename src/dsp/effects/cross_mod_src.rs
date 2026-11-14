//! Warps XMOD sixfold streaming sample-rate conversion.
//!
//! Copyright 2015 Emilie Gillet. Author: Emilie Gillet.
//! Permission is hereby granted, free of charge, to any person obtaining a
//! copy of this software and associated documentation files (the "Software"),
//! to deal in the Software without restriction, including without limitation
//! the rights to use, copy, modify, merge, publish, distribute, sublicense,
//! and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions:
//! The above copyright notice and this permission notice shall be included
//! in all copies or substantial portions of the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//! OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
//! IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
//! USE OR OTHER DEALINGS IN THE SOFTWARE.
//!
//! The 24-coefficient half-kernels below come from the MIT-noticed
//! `warps/dsp/sample_rate_conversion_filters.h` at eurorack revision
//! 08460a69. They are FIR coefficients, not oscillator or audio wavetables.

const RATIO: usize = 6;
const TAPS: usize = 48;
const UP_HISTORY: usize = TAPS / RATIO;
pub(super) const MEM_LEN: usize = 2 * UP_HISTORY + TAPS;

// Preserve the source's printed decimal coefficients; conversion to f32 is
// intentional and matches the source `float` arrays.
#[allow(clippy::excessive_precision)]
const UP_HALF: [f32; TAPS / 2] = [
    4.357_278_6e-4,
    -2.297_029_461e-3,
    -4.703_810_6e-3,
    -8.774_605e-3,
    -1.433_899_145e-2,
    -2.112_793_4e-2,
    -2.853_108_8e-2,
    -3.552_868_3e-2,
    -4.069_863e-2,
    -4.228_981_3e-2,
    -3.836_519_6e-2,
    -2.700_780_7e-2,
    -6.569_014e-3,
    2.407_089_7e-2,
    6.526_452_5e-2,
    1.164_165_7e-1,
    1.758_932_961e-1,
    2.410_483_2e-1,
    3.083_744_5e-1,
    3.737_697_1e-1,
    4.328_923_8e-1,
    4.815_728_4e-1,
    5.162_356e-1,
    5.342_583e-1,
];

#[allow(clippy::excessive_precision)]
const DOWN_HALF: [f32; TAPS / 2] = [
    7.262_131e-5,
    -3.828_382_4e-4,
    -7.839_684_4e-4,
    -1.462_434_1e-3,
    -2.389_832e-3,
    -3.521_322_3e-3,
    -4.755_181_3e-3,
    -5.921_447e-3,
    -6.783_104_9e-3,
    -7.048_302_2e-3,
    -6.394_199_4e-3,
    -4.501_301_159e-3,
    -1.094_835_7e-3,
    4.011_816_173e-3,
    1.087_742_1e-2,
    1.940_276_171e-2,
    2.931_555e-2,
    4.017_472e-2,
    5.139_574_2e-2,
    6.229_495_2e-2,
    7.214_872_8e-2,
    8.026_214e-2,
    8.603_926_5e-2,
    8.904_305e-2,
];

fn mirrored(half: &[f32; TAPS / 2], index: usize) -> f32 {
    half[index.min(TAPS - 1 - index)]
}

fn upsample(history: &[f32], phase: usize) -> f32 {
    history
        .iter()
        .enumerate()
        .map(|(sample, value)| *value * mirrored(&UP_HALF, phase + sample * RATIO))
        .sum()
}

/// Processes one host frame with two upsampled inputs and one downsampled
/// XMOD output. `cursor` is the newest downsample-history slot, persisted in
/// fixed effect state. Histories are preallocated in the effect memory tail.
pub(super) fn process(
    mem: &mut [f32],
    cursor: &mut usize,
    carrier: f32,
    modulator: f32,
    mut xmod: impl FnMut(f32, f32) -> f32,
) -> f32 {
    let (carrier_history, rest) = mem.split_at_mut(UP_HISTORY);
    let (modulator_history, down_history) = rest.split_at_mut(UP_HISTORY);
    carrier_history.copy_within(0..UP_HISTORY - 1, 1);
    modulator_history.copy_within(0..UP_HISTORY - 1, 1);
    carrier_history[0] = carrier;
    modulator_history[0] = modulator;
    for phase in 0..RATIO {
        let up_carrier = upsample(carrier_history, phase);
        let up_modulator = upsample(modulator_history, phase);
        *cursor = (*cursor + 1) % TAPS;
        down_history[*cursor] = xmod(up_carrier, up_modulator);
    }
    (0..TAPS)
        .map(|tap| {
            let index = (*cursor + TAPS - tap) % TAPS;
            down_history[index] * mirrored(&DOWN_HALF, tap)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{mirrored, process, upsample, DOWN_HALF, MEM_LEN, UP_HALF};
    use std::f32::consts::TAU;

    #[test]
    fn sixfold_fir_preserves_dc_and_has_bounded_impulse_latency() {
        let mut mem = [0.0; MEM_LEN];
        let mut cursor = 0;
        let mut tail = 0.0;
        for frame in 0..256 {
            let output = process(&mut mem, &mut cursor, 0.37, -0.11, |carrier, _| carrier);
            if frame >= 192 {
                tail += output;
            }
        }
        assert!(
            (tail / 64.0 - 0.37).abs() < 0.01,
            "DC gain: {}",
            tail / 64.0
        );

        mem.fill(0.0);
        cursor = 0;
        let mut impulse = [0.0; 32];
        for (frame, output) in impulse.iter_mut().enumerate() {
            *output = process(
                &mut mem,
                &mut cursor,
                f32::from(frame == 0),
                0.0,
                |carrier, _| carrier,
            );
        }
        let peak = impulse
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap();
        assert!((7..=9).contains(&peak.0), "impulse peak frame: {peak:?}");
        assert!((impulse.iter().sum::<f32>() - 1.0).abs() < 0.02);
    }

    #[test]
    fn source_half_kernel_symmetry_and_polyphase_orientation() {
        for tap in 0..48 {
            assert_eq!(mirrored(&UP_HALF, tap), mirrored(&UP_HALF, 47 - tap));
            assert_eq!(mirrored(&DOWN_HALF, tap), mirrored(&DOWN_HALF, 47 - tap));
        }
        let mut history = [0.0; 8];
        history[0] = 1.0;
        for (phase, coefficient) in UP_HALF.iter().enumerate().take(6) {
            assert_eq!(upsample(&history, phase), *coefficient);
        }
        let expected_first: f32 = (0..6)
            .map(|phase| UP_HALF[phase] * DOWN_HALF[5 - phase])
            .sum();
        let mut mem = [0.0; MEM_LEN];
        let mut cursor = 0;
        let first = process(&mut mem, &mut cursor, 1.0, 0.0, |carrier, _| carrier);
        assert!((first - expected_first).abs() < 1.0e-9);
    }

    #[test]
    fn streaming_history_is_independent_of_host_chunk_partition() {
        let mut whole_mem = [0.0; MEM_LEN];
        let mut split_mem = [0.0; MEM_LEN];
        let mut whole_cursor = 0;
        let mut split_cursor = 0;
        let mut whole = [0.0; 193];
        let mut split = [0.0; 193];
        for (frame, output) in whole.iter_mut().enumerate() {
            let carrier = (TAU * 0.137 * frame as f32).sin();
            let modulator = (TAU * 0.089 * frame as f32).sin();
            *output = process(
                &mut whole_mem,
                &mut whole_cursor,
                carrier,
                modulator,
                |a, b| a * b,
            );
        }
        for chunk in [0..17, 17..64, 64..193] {
            for (offset, output) in split[chunk.clone()].iter_mut().enumerate() {
                let frame = chunk.start + offset;
                let carrier = (TAU * 0.137 * frame as f32).sin();
                let modulator = (TAU * 0.089 * frame as f32).sin();
                *output = process(
                    &mut split_mem,
                    &mut split_cursor,
                    carrier,
                    modulator,
                    |a, b| a * b,
                );
            }
        }
        assert_eq!(whole, split);
        assert_eq!(whole_mem, split_mem);
        assert_eq!(whole_cursor, split_cursor);
    }

    #[test]
    fn sixfold_xmod_rejects_sum_frequency_alias_vs_direct_host_rate() {
        let mut mem = [0.0; MEM_LEN];
        let mut cursor = 0;
        let mut direct_re = 0.0;
        let mut direct_im = 0.0;
        let mut filtered_re = 0.0;
        let mut filtered_im = 0.0;
        let frames = 8192;
        let carrier_hz = 2880.0 / frames as f32;
        let modulator_hz = 2560.0 / frames as f32;
        let alias_hz = 1.0 - carrier_hz - modulator_hz;
        for frame in 0..(frames + 256) {
            let carrier = (TAU * carrier_hz * frame as f32).sin();
            let modulator = (TAU * modulator_hz * frame as f32).sin();
            let direct = super::super::xmod_output(3.0, carrier, modulator, 0.0);
            let filtered = process(&mut mem, &mut cursor, carrier, modulator, |a, b| {
                super::super::xmod_output(3.0, a, b, 0.0)
            });
            if frame >= 256 {
                let phase = TAU * alias_hz * (frame - 256) as f32;
                direct_re += direct * phase.cos();
                direct_im += direct * phase.sin();
                filtered_re += filtered * phase.cos();
                filtered_im += filtered * phase.sin();
            }
        }
        let direct_alias = direct_re.hypot(direct_im);
        let filtered_alias = filtered_re.hypot(filtered_im);
        assert!(direct_alias > 100.0, "direct alias: {direct_alias}");
        assert!(
            filtered_alias < direct_alias * 0.5,
            "filtered {filtered_alias}, direct {direct_alias}"
        );
    }
}
