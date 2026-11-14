//! Voice low-pass gate adapters.
use super::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_synth::lpg::{DECAY_MOD_STATE_FLOATS, GATE_STATE_FLOATS};
/// Forward the render_gate kernel.
pub fn render_gate(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::lpg::render_gate(ins, st, mem, out, kx, crate::dsp::ported::plaits_voice);
}
/// Forward the render_decay_mod kernel.
pub fn render_decay_mod(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::lpg::render_decay_mod(ins, st, mem, out, kx);
}
