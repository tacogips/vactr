//! Resource adapter for wavetable playback.
use super::{Inp, Kx, NodeState, MAX_PORTS};
use crate::dsp::graph::TableRef;
pub use music_things_synth::wavetable::FRAME;
/// Renders a wavetable resource.
pub fn render(
    table: TableRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let resource = kx.bank.unwrap_or(table.get());
    music_things_synth::wavetable::render_view(kx.store.get(resource), ins, st, out, kx);
}
