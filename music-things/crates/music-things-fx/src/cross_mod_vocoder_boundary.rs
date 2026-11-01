//! Host-rate boundary for the fixed 96 kHz vocoder.
//!
//! Phase-interpolated windowed-sinc coefficients are generated at effect
//! installation. Three rings and bounded relative phases persist across
//! arbitrary callback partitions without trigonometry or allocation there.

use std::f64::consts::TAU;

const SOURCE_RATE: f64 = 96_000.0;
const PHASES: usize = 32;
const HEADER: usize = 7;

#[derive(Clone, Copy)]
struct Shape {
    input_taps: usize,
    output_taps: usize,
    input_ring: usize,
    output_ring: usize,
}

fn shape(sr: f32) -> Shape {
    let rate = f64::from(sr.clamp(8_000.0, 192_000.0));
    let taps = |from: f64, to: f64| {
        let count = (48.0 * (from / to).max(1.0)).ceil() as usize;
        (count + 1) & !1
    };
    let input_taps = taps(rate, SOURCE_RATE);
    let output_taps = taps(SOURCE_RATE, rate);
    Shape {
        input_taps,
        output_taps,
        input_ring: input_taps + 32,
        output_ring: output_taps + 32,
    }
}

pub(super) fn mem_len(sr: f32) -> usize {
    if sr == SOURCE_RATE as f32 {
        return 0;
    }
    let s = shape(sr);
    HEADER + (PHASES + 1) * (s.input_taps + s.output_taps) + 2 * s.input_ring + s.output_ring
}

fn table(target: &mut [f32], taps: usize, cutoff: f64) {
    let center = taps as f64 / 2.0 - 1.0;
    for phase in 0..=PHASES {
        let fraction = phase as f64 / PHASES as f64;
        let row = &mut target[phase * taps..(phase + 1) * taps];
        let mut sum = 0.0;
        for (tap, value) in row.iter_mut().enumerate() {
            let x = tap as f64 - center - fraction;
            let angle = TAU * cutoff * x;
            let sinc = if angle.abs() < 1.0e-12 {
                2.0 * cutoff
            } else {
                angle.sin() / (std::f64::consts::PI * x)
            };
            let window_phase = TAU * tap as f64 / (taps - 1) as f64;
            let window = 0.42 - 0.5 * window_phase.cos() + 0.08 * (2.0 * window_phase).cos();
            *value = (sinc * window) as f32;
            sum += sinc * window;
        }
        for value in row {
            *value = (f64::from(*value) / sum) as f32;
        }
    }
}

struct Parts<'a> {
    state: &'a mut [f32],
    input_table: &'a mut [f32],
    output_table: &'a mut [f32],
    carrier: &'a mut [f32],
    modulator: &'a mut [f32],
    output: &'a mut [f32],
}

fn parts(mem: &mut [f32], s: Shape) -> Parts<'_> {
    let (state, rest) = mem.split_at_mut(HEADER);
    let (in_table, rest) = rest.split_at_mut((PHASES + 1) * s.input_taps);
    let (out_table, rest) = rest.split_at_mut((PHASES + 1) * s.output_taps);
    let (carrier, rest) = rest.split_at_mut(s.input_ring);
    let (modulator, output) = rest.split_at_mut(s.input_ring);
    Parts {
        state,
        input_table: in_table,
        output_table: out_table,
        carrier,
        modulator,
        output,
    }
}

fn unpack(hi: f32, lo: f32) -> f64 {
    f64::from(hi) + f64::from(lo)
}

fn pack(state: &mut [f32], offset: usize, value: f64) {
    state[offset] = value as f32;
    state[offset + 1] = (value - f64::from(state[offset])) as f32;
}

pub(super) fn init(mem: &mut [f32], sr: f32) {
    if sr == SOURCE_RATE as f32 {
        return;
    }
    let s = shape(sr);
    debug_assert!(mem.len() >= mem_len(sr));
    mem.fill(0.0);
    let Parts {
        state,
        input_table,
        output_table,
        ..
    } = parts(mem, s);
    let rate = f64::from(sr);
    table(
        input_table,
        s.input_taps,
        0.45 * (SOURCE_RATE / rate).min(1.0),
    );
    table(
        output_table,
        s.output_taps,
        0.45 * (rate / SOURCE_RATE).min(1.0),
    );
    let ratio = SOURCE_RATE / rate;
    let delay = s.input_taps as f64 / 2.0 * ratio + s.output_taps as f64 / 2.0 + 2.0;
    pack(state, 4, 1.0 - delay);
}

fn read(table: &[f32], ring: &[f32], head: usize, phase: f64, taps: usize) -> f32 {
    let center = phase.floor() as isize;
    let fraction = phase - phase.floor();
    let table_phase = fraction * PHASES as f64;
    let row = (table_phase.floor() as usize).min(PHASES - 1);
    let blend = (table_phase - row as f64) as f32;
    let recent = (head + ring.len() - 1) % ring.len();
    let mut sum = 0.0;
    for tap in 0..taps {
        let offset = center + tap as isize - (taps / 2 - 1) as isize;
        debug_assert!(offset <= 0 && offset > -(ring.len() as isize));
        let index = (recent + ring.len() - (-offset as usize)) % ring.len();
        let first = table[row * taps + tap];
        let second = table[(row + 1) * taps + tap];
        sum += ring[index] * (first + (second - first) * blend);
    }
    sum
}

pub(super) fn sample<F: FnMut(f32, f32) -> f32>(
    mem: &mut [f32],
    sr: f32,
    carrier: f32,
    modulator: f32,
    mut source: F,
) -> f32 {
    let s = shape(sr);
    let Parts {
        state,
        input_table,
        output_table,
        carrier: carrier_ring,
        modulator: modulator_ring,
        output: output_ring,
    } = parts(mem, s);
    let mut input_head = state[0] as usize;
    let mut output_head = state[1] as usize;
    carrier_ring[input_head] = carrier;
    modulator_ring[input_head] = modulator;
    input_head = (input_head + 1) % s.input_ring;
    let mut input_phase = unpack(state[2], state[3]);
    let mut output_phase = unpack(state[4], state[5]);
    let input_step = f64::from(sr) / SOURCE_RATE;
    let output_step = SOURCE_RATE / f64::from(sr);
    let mut produced = 0;
    while input_phase <= -(s.input_taps as f64 / 2.0) && produced < 16 {
        let c = read(
            input_table,
            carrier_ring,
            input_head,
            input_phase,
            s.input_taps,
        );
        let m = read(
            input_table,
            modulator_ring,
            input_head,
            input_phase,
            s.input_taps,
        );
        output_ring[output_head] = source(c, m);
        output_head = (output_head + 1) % s.output_ring;
        input_phase += input_step;
        output_phase -= 1.0;
        produced += 1;
    }
    if produced > 0 {
        state[6] = 1.0;
    }
    let out = if output_phase <= -(s.output_taps as f64 / 2.0) && state[6] != 0.0 {
        read(
            output_table,
            output_ring,
            output_head,
            output_phase,
            s.output_taps,
        )
    } else {
        0.0
    };
    input_phase -= 1.0;
    output_phase += output_step;
    state[0] = input_head as f32;
    state[1] = output_head as f32;
    pack(state, 2, input_phase);
    pack(state, 4, output_phase);
    out
}

#[cfg(test)]
mod tests {
    use super::{init, mem_len, sample, shape};
    use std::f32::consts::TAU;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn unity_pitch_and_impulse_latency_cover_supported_rate_extremes() {
        for sr in [8_000.0, 44_100.0, 48_000.0, 192_000.0] {
            let mut mem = vec![0.0; mem_len(sr)];
            init(&mut mem, sr);
            let frames = (sr * 0.15) as usize;
            let mut dc = vec![0.0; frames];
            for out in &mut dc {
                *out = sample(&mut mem, sr, 1.0, 0.0, |carrier, _| carrier);
            }
            assert!(dc.iter().all(|value| value.is_finite()));
            let steady = &dc[frames * 3 / 4..];
            let mean = steady.iter().sum::<f32>() / steady.len() as f32;
            assert!((mean - 1.0).abs() < 0.01, "{sr} Hz DC: {mean}");

            init(&mut mem, sr);
            let mut tone = vec![0.0; frames];
            for (frame, out) in tone.iter_mut().enumerate() {
                let x = (TAU * 220.0 * frame as f32 / sr).sin();
                *out = sample(&mut mem, sr, x, 0.0, |carrier, _| carrier);
            }
            let late = &tone[frames / 2..];
            let crossings = late
                .windows(2)
                .filter(|v| v[0] <= 0.0 && v[1] > 0.0)
                .count();
            let estimate = crossings as f32 / (late.len() as f32 / sr);
            assert!((estimate - 220.0).abs() < 25.0, "{sr} Hz pitch: {estimate}");
            assert!(rms(late) > 0.4, "{sr} Hz tone RMS");

            init(&mut mem, sr);
            let mut impulse = vec![0.0; frames];
            for (frame, out) in impulse.iter_mut().enumerate() {
                *out = sample(&mut mem, sr, f32::from(frame == 0), 0.0, |carrier, _| {
                    carrier
                });
            }
            let peak = impulse
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
                .unwrap();
            assert!(peak.1.abs() > 0.005, "{sr} Hz impulse must survive");
            assert!(
                peak.0 > 0 && peak.0 < (sr * 0.02) as usize,
                "{sr} Hz latency: {peak:?}"
            );
        }
    }

    #[test]
    fn output_lowpass_rejects_source_audio_above_host_nyquist() {
        for sr in [8_000.0, 44_100.0, 48_000.0] {
            let render = |frequency: f32| {
                let mut mem = vec![0.0; mem_len(sr)];
                init(&mut mem, sr);
                let mut source_frame = 0usize;
                let frames = (sr * 0.12) as usize;
                let mut out = vec![0.0; frames];
                for value in &mut out {
                    *value = sample(&mut mem, sr, 0.0, 0.0, |_, _| {
                        let x = (TAU * frequency * source_frame as f32 / 96_000.0).sin();
                        source_frame += 1;
                        x
                    });
                }
                rms(&out[frames / 2..])
            };
            let pass = render(1_000.0);
            let rejected = render(if sr == 8_000.0 { 30_000.0 } else { 38_000.0 });
            assert!(pass > 0.4, "{sr} Hz passband: {pass}");
            assert!(rejected < pass * 0.1, "{sr} Hz alias: {rejected} vs {pass}");
        }
    }

    #[test]
    fn input_lowpass_rejects_host_audio_above_source_nyquist() {
        let sr = 192_000.0;
        let render = |frequency: f32| {
            let mut mem = vec![0.0; mem_len(sr)];
            init(&mut mem, sr);
            let frames = (sr * 0.1) as usize;
            let mut output = vec![0.0; frames];
            for (frame, value) in output.iter_mut().enumerate() {
                let input = (TAU * frequency * frame as f32 / sr).sin();
                *value = sample(&mut mem, sr, input, 0.0, |carrier, _| carrier);
            }
            rms(&output[frames / 2..])
        };
        let pass = render(1_000.0);
        let rejected = render(70_000.0);
        assert!(pass > 0.4);
        assert!(rejected < pass * 0.1, "input alias: {rejected} vs {pass}");
    }

    #[test]
    fn relative_phases_remain_bounded_after_many_callback_sized_chunks() {
        for sr in [8_000.0, 44_100.0, 192_000.0] {
            let mut mem = vec![0.0; mem_len(sr)];
            init(&mut mem, sr);
            for _ in 0..(sr as usize / 10) {
                let out = sample(&mut mem, sr, 0.0, 0.0, |_, _| 0.0);
                assert!(out.is_finite());
            }
            let s = shape(sr);
            let in_phase = super::unpack(mem[2], mem[3]);
            let out_phase = super::unpack(mem[4], mem[5]);
            assert!(in_phase.abs() < s.input_taps as f64);
            assert!(out_phase.abs() < (s.output_taps + s.input_taps * 12) as f64);
        }
    }

    #[test]
    fn packed_relative_phase_stays_accurate_over_two_million_frames() {
        let sr = 44_100.0f64;
        let step = sr / 96_000.0;
        let mut words = [0.0f32; 2];
        let s = shape(sr as f32);
        let output_step = 96_000.0 / sr;
        let delay = s.input_taps as f64 / 2.0 * output_step + s.output_taps as f64 / 2.0 + 2.0;
        let mut output_words = [0.0f32; 2];
        super::pack(&mut output_words, 0, 1.0 - delay);
        let mut generated = 0usize;
        for frame in 0..2_000_000 {
            let mut phase = super::unpack(words[0], words[1]);
            let mut output_phase = super::unpack(output_words[0], output_words[1]);
            while phase <= -24.0 {
                phase += step;
                generated += 1;
                output_phase -= 1.0;
            }
            phase -= 1.0;
            output_phase += output_step;
            super::pack(&mut words, 0, phase);
            super::pack(&mut output_words, 0, output_phase);
            if frame >= 100 {
                assert!(phase >= -25.0 && phase < -23.0 + step);
                assert!(output_phase.abs() < 2.0 * s.output_taps as f64);
            }
        }
        let expected = ((2_000_000.0 - 24.0) * 96_000.0 / sr).floor() as usize;
        assert!(
            generated.abs_diff(expected) <= 2,
            "phase drift: {generated} vs {expected}"
        );
    }
}
