//! Static checker (design section 7, TASK-004): the diagnostics, the
//! checker types, forcing masks and the native signature table (7.1.3),
//! and the checker itself: unification, the scope chain, inference, the
//! host manifest and the advisory dependency edges.

pub mod check;
pub mod chords;
pub mod deps;
pub mod diag;
pub mod infer;
mod infer_call;
pub mod manifest;
pub mod masks;
pub mod natives;
mod natives_domain;
pub mod scope;
pub mod ty;
pub mod unify;

pub use check::{check, CheckResult};
pub use diag::{DiagCode, Diagnostic, RunOrigin, Severity};
pub use manifest::HostManifest;

#[cfg(test)]
mod tests;
