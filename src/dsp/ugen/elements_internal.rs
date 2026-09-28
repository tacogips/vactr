//! Original procedural Elements-role internal voice adaptation.
//!
//! The three exciter and resonator roles use no source sample recordings,
//! generated lookup arrays, GPL sample generator, or aggregate resources.
//! Optional host-input blow and strike lanes are distinct event-local exciters.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

// Four authored event-local strings keep both output nodes within the
// default host's fixed half-second state budget; source Strings has five.
const STRINGS: usize = 4;
const EXTRA: usize = 128;
const REVERB: usize = 512;
const LOW_HZ: f32 = 20.0;

#[must_use]
pub fn line_len(sr: f32) -> usize {
    (sr.max(1.0) / LOW_HZ).ceil() as usize + 2
}

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    STRINGS * line_len(sr) + EXTRA + REVERB
}

fn random(seed: &mut f32) -> f32 {
    let next = ((*seed as u32).wrapping_mul(251).wrapping_add(37)) & 0xffff;
    *seed = next as f32;
    next as f32 / 32_767.5 - 1.0
}

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[allow(clippy::too_many_arguments)]
fn modal(
    mem: &mut [f32],
    base: usize,
    input: f32,
    hz: f32,
    geometry: f32,
    brightness: f32,
    damping: f32,
    position: f32,
    sr: f32,
) -> (f32, f32) {
    let mut center = 0.0;
    let mut side = 0.0;
    for mode in 0..8 {
        let ratio = 1.0 + mode as f32 * (1.12 + geometry * 0.67);
        let frequency = (hz * ratio).min(sr * 0.43);
        let radius = (-(2.0 + damping * 34.0 + mode as f32 * (1.0 - brightness * 0.7)) / sr).exp();
        let coefficient = 2.0 * radius * (TAU * frequency / sr).cos();
        let offset = base + 16 + mode * 2;
        let gain = (1.0 + brightness * mode as f32 * 0.1) / (1.0 + mode as f32);
        let value = (input * gain * (0.035 + position * 0.04) + coefficient * mem[offset]
            - radius * radius * mem[offset + 1])
            .clamp(-4.0, 4.0);
        mem[offset + 1] = mem[offset];
        mem[offset] = value;
        center += value;
        side += value * (TAU * (position + mode as f32 * 0.13)).cos();
    }
    (center * 0.17, side * 0.17)
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn strings(
    mem: &mut [f32],
    base: usize,
    length: usize,
    input: f32,
    hz: f32,
    model: usize,
    geometry: f32,
    brightness: f32,
    damping: f32,
    position: f32,
    sr: f32,
) -> (f32, f32) {
    let count = if model == 1 { 1 } else { STRINGS };
    let mut center = 0.0;
    let mut side = 0.0;
    for index in 0..count {
        // An authored arithmetic chord map replaces the source interval array.
        let interval = if model == 1 {
            0.0
        } else {
            ((index * 3 + (geometry * 10.0).round() as usize * (index + 1)) % 13) as f32
        };
        let frequency = (hz * 2.0_f32.powf(interval / 12.0)).clamp(LOW_HZ, sr * 0.4);
        let period = (sr / frequency).round().clamp(2.0, (length - 2) as f32) as usize;
        let write = mem[base + index] as usize % length;
        let offset = index * length;
        let read = (write + length - period) % length;
        let sample = mem[offset + read];
        let smooth_index = base + 8 + index;
        let smooth = mem[smooth_index] + (0.07 + brightness * 0.5) * (sample - mem[smooth_index]);
        mem[smooth_index] = smooth;
        let loss = (1.0 - (2.0 + damping * 31.0) / sr).clamp(0.0, 0.999_99);
        mem[offset + write] =
            (smooth * loss + input * (0.14 + position * 0.24) / count as f32).clamp(-3.0, 3.0);
        mem[base + index] = ((write + 1) % length) as f32;
        center += sample;
        side +=
            (sample - smooth * (0.2 + position * 0.7)) * if index % 2 == 0 { 1.0 } else { -1.0 };
    }
    let scale = 0.5 / (count as f32).sqrt();
    (center * scale, side * scale)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let sr = kx.sr.max(1.0);
    let length = line_len(sr);
    let base = STRINGS * length;
    if mem.len() < mem_len(sr) {
        out.fill(0.0);
        return;
    }
    if st.u[0] == 0 {
        mem[base + 48] = ((kx.seed ^ 0x4371) & 0xffff).max(1) as f32;
        st.u[0] = 1;
    }
    let aux = ins[26].first() >= 0.5;
    for (frame, output) in out.iter_mut().enumerate() {
        if ins[27].at(frame) >= 0.5 {
            let mut patch = [0.0; 24];
            for (index, value) in patch.iter_mut().enumerate() {
                *value = ins[index + 1].at(frame);
            }
            let freq = ins[0].at(frame).clamp(20.0, sr * 0.35);
            patch[21] += 12.0 * (freq / 220.0).log2();
            let state = &mut mem[base + 64..base + 64 + crate::dsp::elements_ominous::STATE_FLOATS];
            let (main, side) = crate::dsp::elements_ominous::sample(
                &patch,
                state,
                ins[28].at(frame),
                ins[29].at(frame),
                sr,
            );
            *output = if aux { side } else { main };
            continue;
        }
        let get = |index: usize| unit(ins[index].at(frame));
        let env_shape = get(1);
        let bow_level = get(2);
        let bow_timbre = get(3);
        let blow_level = get(4);
        let blow_meta = get(5);
        let blow_timbre = get(6);
        let strike_level = get(7);
        let strike_meta = get(8);
        let strike_timbre = get(9);
        let signature = get(10);
        let geometry = get(11);
        let brightness = get(12);
        let damping = get(13);
        let position = get(14);
        let res_mod_frequency = get(15);
        let res_mod_offset = get(16);
        let diffusion = get(17);
        let reverb_lp = get(18);
        let space = ins[19].at(frame).clamp(0.0, 2.0);
        let modulation_frequency = get(20);
        let gate = get(21);
        let note = ins[22].at(frame).clamp(-48.0, 48.0);
        let modulation = ins[23].at(frame).clamp(-1.0, 1.0);
        let strength = get(24);
        let model = ins[25].at(frame).round().clamp(0.0, 2.0) as usize;
        let age = mem[base + 49];
        let wave_phase = mem[base + 50];
        let random = random(&mut mem[base + 48]);
        let lowpass = 1.0 - (-TAU * (150.0 + blow_timbre * 10_000.0) / sr).exp();
        mem[base + 51] += lowpass * (random - mem[base + 51]);
        let blow_color = mem[base + 51] * (1.0 - blow_meta) + (random - mem[base + 51]) * blow_meta;
        let strike_decay = (0.004 + env_shape * 0.3) * (0.4 + strength * 0.6);
        let strike_env = (-age / strike_decay).exp();
        let click = (TAU * wave_phase * (1.0 + strike_meta * 7.0)).sin();
        let external_blow = ins[28].at(frame);
        let external_strike = ins[29].at(frame);
        let external_blow = if external_blow.is_finite() {
            external_blow.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        let external_strike = if external_strike.is_finite() {
            external_strike.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        let strike = strike_level
            * strike_env
            * (click * strike_timbre + random * (1.0 - strike_timbre))
            * (0.1 + strength * 0.9)
            + external_strike * strike_level;
        let bow = bow_level * gate * (TAU * wave_phase).sin().tanh() * (0.1 + bow_timbre * 0.9);
        let blow = blow_level * gate * (blow_color * (0.1 + strength * 0.9) + external_blow);
        mem[base + 52] = (mem[base + 52] + (0.005 + modulation_frequency * 8.0) / sr).fract();
        let tremolo = 1.0 + modulation * (TAU * mem[base + 52]).sin() * 0.4;
        let excitation = ((strike + bow + blow) * tremolo * (0.6 + signature * 1.4)).tanh();
        mem[base + 53] = (mem[base + 53] + (0.1 + res_mod_frequency * 9.0) / sr).fract();
        let vibrato = (TAU * mem[base + 53]).sin() * (0.01 + res_mod_offset * 0.06);
        let hz = (ins[0].at(frame).clamp(LOW_HZ, sr * 0.3)
            * 2.0_f32.powf(note / 12.0)
            * (1.0 + vibrato))
            .clamp(LOW_HZ, sr * 0.35);
        let (center, side) = if model == 0 {
            modal(
                mem, base, excitation, hz, geometry, brightness, damping, position, sr,
            )
        } else {
            strings(
                mem, base, length, excitation, hz, model, geometry, brightness, damping, position,
                sr,
            )
        };
        let write = mem[base + 54] as usize % REVERB;
        let echo_index = base + EXTRA + write;
        let echo = mem[echo_index];
        mem[base + 55] += (0.02 + reverb_lp * 0.8) * (echo - mem[base + 55]);
        if space < 1.75 {
            mem[echo_index] =
                (center * 0.3 + mem[base + 55] * (0.1 + diffusion * 0.82)).clamp(-3.0, 3.0);
        }
        mem[base + 54] = ((write + 1) % REVERB) as f32;
        let wet = (space * 0.5).min(1.0);
        let main = center + mem[base + 55] * wet * 0.9;
        let other = side + excitation * (0.15 + position * 0.25) + mem[base + 55] * wet * 0.4;
        *output = if aux { other.tanh() } else { main.tanh() };
        mem[base + 49] = age + 1.0 / sr;
        mem[base + 50] = (wave_phase + hz / sr).fract();
    }
}
