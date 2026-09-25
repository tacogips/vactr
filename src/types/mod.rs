//! Static checker (design section 7). TASK-004; only diagnostics exist so far.

pub mod diag;

pub use diag::{DiagCode, Diagnostic, RunOrigin, Severity};
