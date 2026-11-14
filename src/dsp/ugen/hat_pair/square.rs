//! Six-square Schmitt-trigger metallic source (MIT ratio map).

const RATIOS: [f32; 6] = [1.0, 1.304, 1.466, 1.787, 1.932, 2.536];

pub(super) fn sample(mem: &mut [f32], freq: f32, sr: f32) -> f32 {
    let mut count = 0.0;
    for (phase, ratio) in mem.iter_mut().take(6).zip(RATIOS) {
        *phase = (*phase + (2.0 * freq * ratio / sr).min(0.499)).fract();
        if *phase >= 0.5 {
            count += 1.0;
        }
    }
    0.33 * count - 1.0
}
