//! Resource adapters for sample playback.
use super::{Inp, Kx, NodeState, MAX_PORTS};
use crate::dsp::graph::BankRef;
pub use music_things_synth::sample::{frame_region, region};
/// Plays a sample resource as mono.
pub fn play(
    bank: BankRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let resource = kx.bank.unwrap_or(bank.get());
    music_things_synth::sample::play_view(kx.store.get(resource), ins, st, out, kx);
}
/// Plays a sample resource as mono and stereo channels.
pub fn play_stereo(
    bank: BankRef,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mono: &mut [f32],
    left: &mut [f32],
    right: &mut [f32],
    kx: &Kx<'_>,
) {
    let resource = kx.bank.unwrap_or(bank.get());
    music_things_synth::sample::play_stereo_view(
        kx.store.get(resource),
        ins,
        st,
        mono,
        left,
        right,
        kx,
    );
}
