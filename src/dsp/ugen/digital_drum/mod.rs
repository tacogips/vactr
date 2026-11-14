//! Original programmable digital drum kernels (design-music 4.1,
//! DDRUM-001/003/004): the tonal drum/snare family (`tonal`) and the
//! metal/cymbal and hat family (`metal`), sharing one signal-chain
//! implementation (`chain`) per the shared core contract (DDRUM-003/004):
//! sources, then transient, then filter (`filter-type`, `cutoff`, `res`,
//! `filter-drive`), then `drive`, then sample-and-hold decimation, then the
//! amplitude envelope, then the velocity-scaled output stage. A typed
//! velocity-depth route and a typed per-voice LFO route modulate their
//! target at audio rate; `repeat-count`/`repeat-time` (tonal and metal)
//! retrigger the family's own envelopes inside one voice; the voice ends
//! once the last repeat's tail has decayed (`NodeState::finish`,
//! `Node::is_env`).
//!
//! All oscillator, envelope, filter-routing and modulation-routing DSP is
//! original: naive (non-band-limited) waveforms, an attack/decay envelope
//! blended linearly and exponentially by a `*-slope` control, a
//! hand-authored velocity/LFO target router, and (metal/hat) an inharmonic
//! oscillator bank built from an original ratio set (the square roots of
//! the first few primes) rather than any third-party table. No third-party
//! wave table, sample, preset or numeric lookup data is used.
//!
//! No stereo path is available at these mono cores (`pan` stays with the
//! voice, per the shared core contract), so `lfo-target`'s domain has no
//! `pan` entry.

mod chain;
pub mod metal;
pub mod tonal;

pub use tonal::{render_drum, render_snare, STATE_FLOATS};
