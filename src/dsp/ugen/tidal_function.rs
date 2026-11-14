//! Original analytic Tides-generation-one function adaptation.
//!
//! The default source uses fixed-point ramp/filter/wavefolder stages and a
//! delayed 16-sample block. This kernel uses direct host-rate curves and
//! imports no generated Tides resources or optional wavetable-hack data.

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 16;

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn curve(phase: f32, slope: f32, shape: f32) -> f32 {
    let split = (0.08 + slope * 0.84).clamp(0.08, 0.92);
    let exponent = 2.0_f32.powf((shape - 0.5) * 4.0);
    if phase < split {
        (phase / split).powf(exponent)
    } else {
        (1.0 - (phase - split) / (1.0 - split)).powf(1.0 / exponent)
    }
}

/// Bounded host-sample hold corresponding to the source's 48/1 generator
/// samples; low range divides its generator clock by four.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn eor_hold_samples(sr: f32, range: u8, hz: f32) -> f32 {
    let clock_divider = if range == 2 { 4.0 } else { 1.0 };
    let threshold_hz = (44_739_242.0 / 4_294_967_296.0) * 48_000.0 / clock_divider;
    let generator_samples = if hz < threshold_hz { 48.0 } else { 1.0 };
    (generator_samples * sr / 48_000.0 * clock_divider)
        .round()
        .max(1.0)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    for (frame, output) in out.iter_mut().enumerate() {
        let shape = unit(ins[1].at(frame));
        let slope = unit(ins[2].at(frame));
        let smoothness = unit(ins[3].at(frame));
        let ratio = ins[4].at(frame).clamp(0.125, 8.0);
        let sync = ins[5].at(frame) >= 0.5;
        let gate = ins[6].at(frame) >= 0.5;
        let clock = ins[7].at(frame) >= 0.5;
        let freeze = ins[8].at(frame) >= 0.5;
        let mode = ins[9].at(frame).round().clamp(0.0, 2.0) as u8;
        let range = ins[10].at(frame).round().clamp(0.0, 2.0) as u8;
        let pitch = ins[11].at(frame).clamp(-48.0, 48.0);
        let output_kind = ins[12].at(frame).round().clamp(0.0, 3.0) as u8;
        let gate_rise = gate && mem[2] < 0.5;
        let clock_rise = clock && mem[3] < 0.5;
        mem[2] = if gate { 1.0 } else { 0.0 };
        mem[3] = if clock { 1.0 } else { 0.0 };
        if mode == 1 && st.u[0] == 0 {
            st.u[0] = 1;
        }
        if !freeze {
            if gate_rise {
                mem[0] = 0.0;
                st.u[0] = 1;
                mem[9] = 0.0;
            }
            if clock_rise {
                mem[4] = mem[5];
                mem[5] = 0.0;
                if sync {
                    mem[0] = 0.0;
                    st.u[0] = 1;
                    mem[9] = 0.0;
                }
            }
            let range_scale = match range {
                0 => 1.0,
                1 => 0.02,
                _ => 0.0004,
            };
            let base_hz = ins[0].at(frame).clamp(0.01, sr * 0.4);
            let mut hz = base_hz * 2.0_f32.powf(pitch / 12.0) * range_scale * ratio;
            if sync && mem[4] > 1.0 {
                hz = (sr / mem[4] * ratio).clamp(0.0001, sr * 0.4);
            }
            hz = hz.clamp(0.0001, sr * 0.4);
            mem[5] = (mem[5] + 1.0).min(sr * 60.0);
            let split = 0.08 + slope * 0.84;
            let sustain_at = if range == 0 { 0.5 } else { split };
            let advance = match mode {
                0 => st.u[0] != 0,
                1 => true,
                _ => st.u[0] != 0 && (!gate || mem[0] < sustain_at),
            };
            let mut looped = false;
            if advance {
                mem[0] += hz / sr;
                if mode == 2 && gate && mem[0] >= sustain_at {
                    mem[0] = sustain_at;
                }
                if mode == 2 && !gate && mem[0] < sustain_at {
                    mem[0] = sustain_at + hz / sr;
                }
                if mem[0] >= 1.0 {
                    if mode == 1 {
                        mem[0] = mem[0].fract();
                        looped = true;
                    } else {
                        mem[0] = 1.0;
                        st.u[0] = 0;
                    }
                }
            }
            // Translate the source's 48 or one generator-sample EOR hold
            // into host samples. Low range divides the generator clock by 4.
            if looped {
                mem[9] = eor_hold_samples(sr, range, hz);
            }
            let sustained = mode == 2 && gate && mem[0] >= sustain_at;
            mem[7] = if mem[0] >= split || st.u[0] == 0 || sustained {
                1.0
            } else {
                0.0
            };
            mem[8] = if st.u[0] == 0 || mem[9] > 0.0 {
                1.0
            } else {
                0.0
            };
            mem[9] = (mem[9] - 1.0).max(0.0);
            let raw = if st.u[0] == 0 && mode != 1 && mem[0] >= 1.0 {
                0.0
            } else {
                curve(mem[0], slope, shape)
            };
            let alpha = 1.0 - (-1.0 / (sr * (0.00002 + smoothness * 0.02))).exp();
            mem[1] += alpha * (raw - mem[1]);
            // A small original soft fold stands in for the generated source
            // wavefolder table; neutral at smoothness <= 0.5.
            let drive = (smoothness - 0.5).max(0.0) * 2.0;
            mem[6] = ((1.0 - drive) * mem[1] + drive * (mem[1] * 2.4).tanh() / 2.4_f32.tanh())
                .clamp(0.0, 1.0);
        }
        *output = match output_kind {
            0 => mem[6],
            1 => mem[6] * 2.0 - 1.0,
            2 => mem[7],
            _ => mem[8],
        };
    }
}
