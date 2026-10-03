//! Scheduler and slots (design section 11). TASK-007.
//!
//! The module map is fixed by design 12.8.2: BE-SCHED owns the scheduler
//! files, BE-MIDI owns `midi_in` and `midi_clock`.

pub mod cells;
pub mod commit;
pub mod control;
pub mod dryrun;
pub mod ledger;
pub mod midi_clock;
pub mod midi_in;
pub mod offline;
pub mod oneshot;
pub mod render;
pub mod runtime;
pub mod slots;
pub mod staging;
pub mod tap;
pub mod telemetry;

#[cfg(test)]
mod tests;

/// Owned finite song playback and streaming progression.
pub mod song;
