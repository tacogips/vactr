//! Forwarders preserving filter function-pointer signatures.
use super::{Inp, Kx, NodeState, MAX_PORTS};
pub use music_things_synth::va_filter::source;
/// Renders the selected filter path.
pub fn filter(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    music_things_synth::va_filter::filter(ins, st, out, kx);
}
/// Renders both filter paths.
pub fn filter_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    music_things_synth::va_filter::filter_pair(ins, st, main, aux, kx);
}
