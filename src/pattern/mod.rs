//! Pattern engine (design section 10). TASK-006.

pub mod build;
pub mod combinators;
pub mod eval;
pub mod occ;
pub mod pat;
pub mod query;
pub mod rng;
pub mod signal;
pub mod step;

pub use eval::{AnalyzerId, HostSig, InputCells, QueryCtx, QueryVm};
pub use occ::OccKey;
pub use pat::{PParam, Pat, PatNode, SliceCuts};
pub use query::{query, Controls, Event, QueryResult, TimeSpan};
pub use signal::Sig;
pub use step::Step;

#[cfg(test)]
pub(crate) mod tests;
