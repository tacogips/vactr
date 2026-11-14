//! Stereo variable-delay and frozen-loop adaptation informed by MIT Clouds.
//! Host-rate glide, original four-point cubic reads and pitch-aware wrap
//! crossfade translate the looping player's playback roles. Vactr retains
//! live pitch, texture and quality extensions; source SRC, processor mix and
//! generated tables remain excluded. See THIRD_PARTY_NOTICES.md.

use super::texture;
use super::{FxCtx, FxState};

const FADE_SECONDS: f32 = 64.0 / 32_000.0;
const DELAY_ALPHA_AT_32K: f32 = 0.00005;

fn delay_alpha(sr: f32) -> f32 {
    1.0 - (1.0 - DELAY_ALPHA_AT_32K).powf(32_000.0 / sr)
}

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    texture::mem_len(sr)
}

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    texture::init(st, mem);
}

/// Four-point Lagrange interpolation at circular sample positions -1..2.
/// This is independently derived polynomial interpolation, not the source
/// AudioBuffer's Hermite code or coefficients.
fn read_cubic(buf: &[f32], head: f32) -> f32 {
    let n = buf.len();
    let p = head.rem_euclid(n as f32);
    // f32 rounding can produce exactly n after rem_euclid near the edge.
    let base = (p.floor() as usize) % n;
    let t = p.fract();
    let a = buf[(base + n - 1) % n];
    let b = buf[base];
    let c = buf[(base + 1) % n];
    let d = buf[(base + 2) % n];
    let wa = -t * (t - 1.0) * (t - 2.0) / 6.0;
    let wb = (t + 1.0) * (t - 1.0) * (t - 2.0) / 2.0;
    let wc = -(t + 1.0) * t * (t - 2.0) / 2.0;
    let wd = (t + 1.0) * t * (t - 1.0) / 6.0;
    a * wa + b * wb + c * wc + d * wd
}

fn live_read(
    buf: &[f32],
    write: usize,
    delay: f32,
    phase: f32,
    window: f32,
    ratio: f32,
    valid: usize,
) -> f32 {
    let half = (phase + window * 0.5).rem_euclid(window);
    let a = (delay + (1.0 - ratio) * phase).clamp(3.0, valid.saturating_sub(2) as f32);
    let b = (delay + (1.0 - ratio) * half).clamp(3.0, valid.saturating_sub(2) as f32);
    let x = phase / window;
    let blend = 1.0 - (2.0 * x - 1.0).abs();
    let base = write as f32;
    read_cubic(buf, base - a) * blend + read_cubic(buf, base - b) * (1.0 - blend)
}

/// Process independent stereo live delay or frozen loop in preallocated memory.
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if mem.len() < mem_len(ctx.sr) || p.len() < texture::PARAMS.len() {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let frames = (mem.len() - 8 * 7) / 2;
    let (capture, _) = mem.split_at_mut(frames * 2);
    let (left, right) = capture.split_at_mut(frames);
    let sr = ctx.sr.max(8_000.0);
    let mut write = st.s[0] as usize % frames;
    let mut valid = (st.s[1] as usize).min(frames);
    let mut prev_freeze = st.s[2] >= 0.5;
    let mut prev_trigger = st.s[3] >= 0.5;
    let mut tap_counter = st.s[4];
    let mut tap_period = st.s[5];
    let mut synced = st.s[6] >= 0.5;
    let mut live_phase = st.s[7];
    let mut loop_phase = st.s[8];
    let mut current_delay = st.s[9];
    let mut loop_point = st.s[10];
    let mut loop_duration = st.s[18];
    let mut tail_start = st.s[20];
    let mut tail_duration = st.s[21];
    let mut loop_reset = st.s[22];
    let mut prev_l = st.s[11];
    let mut prev_r = st.s[12];
    let mut tail_l = st.s[13];
    let mut tail_r = st.s[14];
    let mut filter_l = st.s[15];
    let mut filter_r = st.s[16];
    let mut wobble = st.s[17];
    let mut gate = st.s[19];
    let quality_phase = super::texture_quality::begin(p, st, l, r);
    let ratio = 2.0f32.powf(p[2] / 12.0);
    let window = (0.003 + p[1] * p[1] * 0.037) * sr;
    let filter_rate = 0.03 + p[4] * p[4] * 0.94;
    let reverb_decay = (-1.0 / (sr * (0.025 + p[7] * 0.24))).exp();
    let fade = (FADE_SECONDS * sr).max(8.0);
    let delay_alpha = delay_alpha(sr);
    for (out_l, out_r) in l.iter_mut().zip(r.iter_mut()) {
        let frozen = p[9] >= 0.5;
        if !frozen {
            left[write] = (*out_l + p[6] * prev_l).clamp(-4.0, 4.0);
            right[write] = (*out_r + p[6] * prev_r).clamp(-4.0, 4.0);
            write = (write + 1) % frames;
            valid = (valid + 1).min(frames);
        }
        tap_counter += 1.0;
        if tap_counter >= frames as f32 {
            tap_counter = frames as f32;
            synced = false;
        }
        let triggered = p[10] >= 0.5 && !prev_trigger;
        prev_trigger = p[10] >= 0.5;
        if triggered {
            if tap_counter >= 128.0 && tap_counter < frames as f32 {
                tap_period = tap_counter;
                synced = true;
            }
            tap_counter = 0.0;
            loop_reset = loop_phase;
            loop_phase = 0.0;
        }
        if frozen && !prev_freeze {
            loop_phase = 0.0;
            loop_duration = 0.0;
            loop_reset = 0.0;
        }
        prev_freeze = frozen;
        // `density` is a bounded diffusion/jitter of the read location.
        wobble += 0.002 * (st.rng.bipolar() - wobble);
        let spread = p[5] * 0.3;
        let (mut wet_l, mut wet_r) = if frozen {
            let max_delay = (valid.saturating_sub(fade as usize + 8) as f32).max(16.0);
            let length = if synced {
                tap_period.clamp(16.0, max_delay)
            } else {
                ((0.01 + 0.99 * p[1].powi(3)) * max_delay).clamp(16.0, max_delay)
            };
            let mut point = p[0] * max_delay * 15.0 / 16.0 + fade;
            if point + length >= max_delay {
                point = (max_delay - length).max(0.0);
            }
            let increment = if synced { 1.0 } else { ratio };
            if loop_phase >= loop_duration || loop_phase == 0.0 {
                if loop_phase >= loop_duration {
                    loop_reset = loop_duration;
                }
                loop_reset = loop_reset.min(loop_duration);
                tail_start = loop_duration - loop_reset + loop_point;
                loop_phase = 0.0;
                tail_duration = fade.min(fade * increment);
                loop_point = point;
                loop_duration = length;
            }
            loop_phase += increment;
            let gain = if tail_duration > 0.0 {
                (loop_phase / tail_duration).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let jitter = wobble * p[3] * (sr * 0.002);
            let front = write as f32 - 4.0 - (loop_duration - loop_phase + loop_point);
            let a = read_cubic(left, front + jitter) * gain
                + read_cubic(
                    left,
                    write as f32 - 4.0 - (tail_start - loop_phase) + jitter,
                ) * (1.0 - gain);
            let b = read_cubic(right, front - jitter) * gain
                + read_cubic(
                    right,
                    write as f32 - 4.0 - (tail_start - loop_phase) - jitter,
                ) * (1.0 - gain);
            (a, b)
        } else if valid < 8 {
            (0.0, 0.0)
        } else {
            let max_delay = (valid.saturating_sub(4) as f32).max(4.0);
            let wanted = if synced {
                tap_period
            } else {
                (0.005 + p[0] * (0.02 + p[1] * 0.2)) * sr
            };
            let target = (wanted + wobble * p[3] * sr * 0.002).clamp(4.0, max_delay);
            current_delay += delay_alpha * (target - current_delay);
            let delay = current_delay.clamp(4.0, max_delay);
            live_phase = (live_phase + 1.0).rem_euclid(window);
            (
                live_read(left, write, delay, live_phase, window, ratio, valid),
                live_read(right, write, delay, live_phase, window, ratio, valid),
            )
        };
        let mid = (wet_l + wet_r) * 0.5;
        wet_l += spread * (wet_l - mid);
        wet_r += spread * (wet_r - mid);
        filter_l += filter_rate * (wet_l - filter_l);
        filter_r += filter_rate * (wet_r - filter_r);
        tail_l = tail_l * reverb_decay + filter_r * (1.0 - reverb_decay);
        tail_r = tail_r * reverb_decay + filter_l * (1.0 - reverb_decay);
        prev_l = filter_l + p[7] * (tail_l * 2.0 + tail_r) * 0.5;
        prev_r = filter_r + p[7] * (tail_r * 2.0 + tail_l) * 0.5;
        let gate_target = if p[11] >= 0.5 { 1.0 } else { 0.0 };
        gate += 0.005 * (gate_target - gate);
        *out_l = (prev_l * gate).clamp(-4.0, 4.0);
        *out_r = (prev_r * gate).clamp(-4.0, 4.0);
    }
    st.s[0] = write as f32;
    st.s[1] = valid as f32;
    st.s[2] = if prev_freeze { 1.0 } else { 0.0 };
    st.s[3] = if prev_trigger { 1.0 } else { 0.0 };
    st.s[4] = tap_counter;
    st.s[5] = tap_period;
    st.s[6] = if synced { 1.0 } else { 0.0 };
    st.s[7] = live_phase;
    st.s[8] = loop_phase;
    st.s[9] = current_delay;
    st.s[10] = loop_point;
    st.s[11] = prev_l;
    st.s[12] = prev_r;
    st.s[13] = tail_l;
    st.s[14] = tail_r;
    st.s[15] = filter_l;
    st.s[16] = filter_r;
    st.s[17] = wobble;
    st.s[18] = loop_duration;
    st.s[19] = gate;
    st.s[20] = tail_start;
    st.s[21] = tail_duration;
    st.s[22] = loop_reset;
    super::texture_quality::end(quality_phase, st, l, r);
}

#[cfg(test)]
mod tests {
    use super::{delay_alpha, read_cubic};

    #[test]
    fn cubic_capture_read_preserves_constant_linear_and_wrap_boundaries() {
        let constant = [0.7; 8];
        for head in [0.0, 0.25, 2.5, 7.75, -0.25, 8.25] {
            assert!((read_cubic(&constant, head) - 0.7).abs() < 1.0e-6);
        }
        let ramp = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        for head in [1.25, 2.5, 3.75, 5.5] {
            assert!((read_cubic(&ramp, head) - head).abs() < 1.0e-5);
        }
        assert!((read_cubic(&ramp, 7.0) - 7.0).abs() < 1.0e-6);
        assert!((read_cubic(&ramp, 8.0) - 0.0).abs() < 1.0e-6);
        assert!((read_cubic(&ramp, -1.0) - 7.0).abs() < 1.0e-6);
        assert!(read_cubic(&ramp, 7.5).is_finite());
    }

    #[test]
    fn glide_has_same_wall_time_at_supported_host_rates() {
        let reference = (1.0 - delay_alpha(32_000.0)).powf(32_000.0);
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            let remaining = (1.0 - delay_alpha(sr)).powf(sr);
            assert!((remaining - reference).abs() < 0.002, "{sr}");
        }
    }
}
