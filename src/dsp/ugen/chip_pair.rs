//! Chiptune chord/arpeggio and stepped bass adaptation of MIT Plaits sources.
//!
//! The 11 chord interval sets are MIT source-code data. The oscillators here
//! are analytic and the clock is internal; see THIRD_PARTY_NOTICES.md.

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const CHORDS: [[f32; 4]; 11] = [
    [0.0, 0.01, 11.99, 12.0],
    [0.0, 7.0, 7.01, 12.0],
    [0.0, 5.0, 7.0, 12.0],
    [0.0, 3.0, 7.0, 12.0],
    [0.0, 3.0, 7.0, 10.0],
    [0.0, 3.0, 10.0, 14.0],
    [0.0, 3.0, 10.0, 17.0],
    [0.0, 2.0, 9.0, 16.0],
    [0.0, 4.0, 11.0, 14.0],
    [0.0, 4.0, 7.0, 11.0],
    [0.0, 4.0, 7.0, 12.0],
];

/// Five square master/slave phase pairs plus one triangle phase and spare state.
pub const STATE_FLOATS: usize = 16;

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn square(mem: &mut [f32], voice: usize, freq: f32, shape: f32, sr: f32) -> f32 {
    let master = voice * 2;
    let slave = master + 1;
    let ratio = if shape < 0.5 {
        0.51 + 0.98 * shape
    } else {
        1.0 + 16.0 * (shape - 0.5).powi(2)
    };
    let master_rate = (freq / sr).min(0.25);
    let slave_rate = (freq * ratio / sr).min(0.25);
    mem[master] += master_rate;
    if mem[master] >= 1.0 {
        mem[master] -= 1.0;
        mem[slave] = 0.0;
    }
    mem[slave] = (mem[slave] + slave_rate).fract();
    if mem[slave] < 0.5 {
        -1.0
    } else {
        1.0
    }
}

#[inline]
fn triangle(mem: &mut [f32], freq: f32, sr: f32) -> f32 {
    mem[10] = (mem[10] + (freq / sr).min(0.25)).fract();
    let phase = mem[10];
    let step = (phase * 32.0).floor();
    let stepped = if step < 16.0 { step } else { 31.0 - step };
    let nes = stepped / 15.0 * 2.0 - 1.0;
    let smooth = if phase < 0.5 {
        4.0 * phase - 1.0
    } else {
        3.0 - 4.0 * phase
    };
    let fade = ((freq / sr - 1.0 / 64.0) * 64.0).clamp(0.0, 1.0);
    nes * (1.0 - fade) + smooth * fade
}

/// Main is a five-square chord or one internally clocked arpeggio; auxiliary
/// is a stepped 32-step triangle bass. Each event resets both node states.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.24);
    let chord_index = (unit(ins[1].first()) * 10.999) as usize;
    let chord = CHORDS[chord_index];
    let timbre = unit(ins[2].first());
    let shape = unit(ins[3].first()) * 0.995;
    let bass = ins[4].first() >= 0.5;
    let clocked = ins[5].first() >= 0.5;
    let rate = ins[6].first().clamp(1.0, 16.0);
    if st.u[2] == 0 {
        st.u[0] = 0;
        st.u[1] = Rng::new(kx.seed ^ 0xC41F_7007).s;
        st.u[2] = 1;
    }
    let mut rng = Rng::new(st.u[1]);
    let pattern = (timbre * 3.999) as u32;
    let inversion = (timbre * 4.999) as usize;
    for sample in out.iter_mut() {
        if clocked {
            st.s[0] += rate / sr;
            if st.s[0] >= 1.0 {
                st.s[0] -= 1.0;
                st.u[0] = st.u[0].wrapping_add(1);
            }
        }
        let step = st.u[0] as usize;
        let note = match pattern {
            0 => step % 4,
            1 => 3 - step % 4,
            2 => {
                let cycle = step % 6;
                if cycle < 4 {
                    cycle
                } else {
                    6 - cycle
                }
            }
            _ => {
                if st.s[1] != st.u[0] as f32 {
                    st.s[2] = (rng.next_u32() % 4) as f32;
                    st.s[1] = st.u[0] as f32;
                }
                st.s[2] as usize
            }
        };
        let octave = if clocked { (step / 4) % 2 } else { 0 };
        let root_transposition = if octave == 0 { 1.0 } else { 2.0 };
        let value = if bass {
            triangle(mem, freq * 0.5 * root_transposition, sr)
        } else if clocked {
            let note_freq = freq * 2.0_f32.powf(chord[note] / 12.0) * root_transposition;
            square(mem, 0, note_freq, shape, sr)
        } else {
            let mut sum = 0.0;
            for voice in 0..5 {
                let index = (voice + inversion) % 4;
                let octave = (voice + inversion) / 4;
                let ratio = 2.0_f32.powf(chord[index] / 12.0);
                let signed = if voice % 2 == 0 { 1.0 } else { -1.0 };
                sum += 0.2
                    * signed
                    * square(mem, voice, freq * ratio * (1 << octave) as f32, shape, sr);
            }
            sum
        };
        *sample = value;
    }
    st.u[1] = rng.s;
}
