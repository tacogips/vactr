//! Fixed streaming source-rate converters for the 96 kHz Warps vocoder bank.
//! The individually MIT-noticed filter half-kernels live in the sibling file.

#[path = "cross_mod_vocoder_fir_coeffs.rs"]
mod coeffs;

pub(super) const FIR_STATE_LEN: usize = 37 + 49 + 12 + 12;
pub(super) const MID_DOWN: std::ops::Range<usize> = 0..37;
pub(super) const LOW_DOWN: std::ops::Range<usize> = 37..86;
pub(super) const LOW_UP: std::ops::Range<usize> = 86..98;
pub(super) const MID_UP: std::ops::Range<usize> = 98..110;

#[inline]
fn coefficient(half: &[f32], tap: usize, taps: usize) -> f32 {
    half[tap.min(taps - 1 - tap)]
}

fn down<const R: usize, const T: usize>(
    state: &mut [f32],
    input: &[f32],
    output: &mut [f32],
    half: &[f32],
) {
    debug_assert_eq!(state.len(), T + 1);
    debug_assert_eq!(input.len(), output.len() * R);
    let (history, cursor_word) = state.split_at_mut(T);
    let mut cursor = cursor_word[0] as usize;
    for (frame, &sample) in input.iter().enumerate() {
        cursor = (cursor + 1) % T;
        history[cursor] = sample;
        if (frame + 1) % R == 0 {
            output[frame / R] = (0..T)
                .map(|tap| {
                    let index = (cursor + T - tap) % T;
                    history[index] * coefficient(half, tap, T)
                })
                .sum();
        }
    }
    cursor_word[0] = cursor as f32;
}

fn up<const R: usize, const T: usize>(
    state: &mut [f32],
    input: &[f32],
    output: &mut [f32],
    half: &[f32],
) {
    debug_assert_eq!(state.len(), T / R);
    debug_assert_eq!(output.len(), input.len() * R);
    for (frame, &sample) in input.iter().enumerate() {
        state.copy_within(0..T / R - 1, 1);
        state[0] = sample;
        for phase in 0..R {
            output[frame * R + phase] = state
                .iter()
                .enumerate()
                .map(|(tap, &history)| history * coefficient(half, phase + tap * R, T))
                .sum();
        }
    }
}

pub(super) fn down3(state: &mut [f32], input: &[f32], output: &mut [f32]) {
    down::<3, 36>(state, input, output, &coeffs::DOWN3_HALF);
}

pub(super) fn down4(state: &mut [f32], input: &[f32], output: &mut [f32]) {
    down::<4, 48>(state, input, output, &coeffs::DOWN4_HALF);
}

pub(super) fn up4(state: &mut [f32], input: &[f32], output: &mut [f32]) {
    up::<4, 48>(state, input, output, &coeffs::UP4_HALF);
}

pub(super) fn up3(state: &mut [f32], input: &[f32], output: &mut [f32]) {
    up::<3, 36>(state, input, output, &coeffs::UP3_HALF);
}
