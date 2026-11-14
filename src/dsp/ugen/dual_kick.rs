//! Forwarder preserving the function-pointer signature.
use super::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_synth::dual_kick::*;
/// Forward the render kernel.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::dual_kick::render(ins, st, mem, out, kx);
}
