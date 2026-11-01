//! Forwarders preserving function-pointer signatures.
use super::{Inp, Kx, NodeState, MAX_PORTS};
/// Forward the render kernel.
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    music_things_synth::fm_pair::render(ins, st, out, kx);
}
/// Forward the render_pair kernel.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::fm_pair::render_pair(ins, st, main, aux, kx);
}
