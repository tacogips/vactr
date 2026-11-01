//! Three square/saw ring-modulation pairs, with analytic oscillator edges.

const LOW_HZ: [f32; 3] = [200.0, 510.0, 730.0];
const HIGH_HZ: [f32; 3] = [7530.0, 8075.0, 10500.0];

pub(super) fn sample(mem: &mut [f32], freq: f32, sr: f32) -> f32 {
    // The original ratio uses f0/(0.01+f0) at 48 kHz and receives 2*f0.
    // Expressing the 0.01 denominator as 480 Hz preserves pitch across hosts.
    let ratio = 2.0 * freq / (480.0 + 2.0 * freq);
    let mut output = 0.0;
    for i in 0..3 {
        let square_phase = &mut mem[i * 2];
        *square_phase = (*square_phase + (LOW_HZ[i] * ratio / sr).min(0.499)).fract();
        let square = if *square_phase < 0.5 { 1.0 } else { -1.0 };
        let saw_phase = &mut mem[i * 2 + 1];
        *saw_phase = (*saw_phase + (HIGH_HZ[i] * ratio / sr).min(0.499)).fract();
        let saw = *saw_phase * 2.0 - 1.0;
        output += square * saw;
    }
    output
}
