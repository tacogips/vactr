//! Bytecode VM (design section 8). TASK-005; only failures exist so far.

pub mod fail;

pub use fail::{FailCode, Failure, Origin};
