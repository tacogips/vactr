//! Digital drum function-pointer compatibility wrappers.
use super::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_synth::digital_drum::*;
/// Forwards the digital drum kernel.
pub fn render_drum(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::digital_drum::render_drum(ins, st, mem, out, kx);
}
/// Forwards the digital drum kernel.
pub fn render_snare(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::digital_drum::render_snare(ins, st, mem, out, kx);
}
/// Metal and hat function-pointer compatibility wrappers.
pub mod metal {
    use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};
    pub use music_things_synth::digital_drum::metal::*;
    /// Forwards the metal kernel.
    pub fn render_metal(
        ins: &[Inp<'_>; MAX_PORTS],
        st: &mut NodeState,
        mem: &mut [f32],
        out: &mut [f32],
        kx: &Kx<'_>,
    ) {
        music_things_synth::digital_drum::metal::render_metal(ins, st, mem, out, kx);
    }
    /// Forwards the metal kernel.
    pub fn render_hat(
        ins: &[Inp<'_>; MAX_PORTS],
        st: &mut NodeState,
        mem: &mut [f32],
        out: &mut [f32],
        kx: &Kx<'_>,
    ) {
        music_things_synth::digital_drum::metal::render_hat(ins, st, mem, out, kx);
    }
}
