//! The cycle clock (design 11.1, 11.7). TASK-006.
//!
//! Logical time is `Ratio64` cycles end to end; host seconds (`f64`) appear
//! only at the anchors. There is no clock source here: the host passes
//! timestamps in.

// The `clock/clock.rs` layout is fixed by design 7.1.2.
#[allow(clippy::module_inception)]
pub mod clock;
pub mod tempo;

pub use clock::{Clock, ClockSource, MidiClockSync, PULSES_PER_BEAT};
pub use tempo::Tempo;

#[cfg(test)]
mod tests;
