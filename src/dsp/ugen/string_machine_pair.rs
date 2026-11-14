//! Forwarders preserving function-pointer signatures.
use super::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_synth::string_machine_pair::*;
/// Forward the render kernel.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::string_machine_pair::render(ins, _st, mem, out, kx);
}
/// Forward the render_pair kernel.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::string_machine_pair::render_pair(ins, st, mem, main, aux, kx);
}
