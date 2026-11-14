//! Forwarders preserving stage-chain function-pointer signatures.
use super::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_synth::stage_chain::STATE_FLOATS;
/// Renders the stage-chain output.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::stage_chain::render(ins, st, mem, out, kx);
}
/// Renders value and phase outputs.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::stage_chain::render_pair(ins, st, mem, main, aux, kx);
}
