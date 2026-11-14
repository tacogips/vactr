//! Envelope kernels: `env-perc`, `env-adsr` (gated by the voice) and
//! `line`. An envelope that reaches its end marks its node finished; a voice
//! whose envelopes all finished ends (`voice.rs`).

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// `env-perc attack release`: a linear rise and a squared fall; it ignores
/// the gate.
pub fn perc(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let a = ins[0].first().max(0.0);
    let r = ins[1].first().max(1.0e-4);
    let dt = 1.0 / kx.sr;
    let mut t = st.s[0];
    for y in out.iter_mut() {
        *y = if st.done() {
            0.0
        } else if t < a {
            t / a
        } else {
            let x = (t - a) / r;
            if x >= 1.0 {
                st.finish();
                0.0
            } else {
                (1.0 - x) * (1.0 - x)
            }
        };
        t += dt;
    }
    st.s[0] = t;
}

const ATTACK: u32 = 0;
const DECAY: u32 = 1;
const SUSTAIN: u32 = 2;
const RELEASE: u32 = 3;

/// `env-adsr attack decay sustain release`: linear segments; the gate
/// (`Kx::gate` frames of this block) holds the sustain, its end starts the
/// release from the current level.
pub fn adsr(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let a = ins[0].first().max(1.0e-4) * kx.sr;
    let d = ins[1].first().max(1.0e-4) * kx.sr;
    let s = ins[2].first().clamp(0.0, 1.0);
    let r = ins[3].first().max(1.0e-4) * kx.sr;
    let mut level = st.s[0];
    for (i, y) in out.iter_mut().enumerate() {
        if st.done() {
            *y = 0.0;
            continue;
        }
        if i >= kx.gate && st.u[0] != RELEASE {
            st.u[0] = RELEASE;
            st.s[1] = level.max(0.0);
        }
        match st.u[0] {
            ATTACK => {
                level += 1.0 / a;
                if level >= 1.0 {
                    level = 1.0;
                    st.u[0] = DECAY;
                }
            }
            DECAY => {
                level -= (1.0 - s) / d;
                if level <= s {
                    level = s;
                    st.u[0] = SUSTAIN;
                }
            }
            SUSTAIN => level = s,
            _ => {
                level -= st.s[1].max(1.0e-3) / r;
                if level <= 0.0 {
                    level = 0.0;
                    st.finish();
                }
            }
        }
        *y = level;
    }
    st.s[0] = level;
}

/// `line from to dur`: a linear ramp that then holds `to`.
pub fn line(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let (from, to) = (ins[0].first(), ins[1].first());
    let dur = ins[2].first().max(1.0e-4);
    let dt = 1.0 / kx.sr;
    let mut t = st.s[0];
    for y in out.iter_mut() {
        let x = (t / dur).min(1.0);
        *y = from + (to - from) * x;
        t += dt;
    }
    st.s[0] = t.min(dur + 1.0);
}
