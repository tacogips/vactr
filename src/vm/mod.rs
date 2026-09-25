//! Bytecode VM (design section 8): frames, fuel, call-boundary forcing,
//! Query effect mode, the Query VM handle and the natives.

pub mod call;
pub mod fail;
pub mod frame;
pub mod natives;
pub mod ops;
pub mod query_vm;
// The `vm/vm.rs` layout is fixed by design 7.1.2.
#[allow(clippy::module_inception)]
pub mod vm;

pub use call::NativeCx;
pub use fail::{FailCode, Failure, Origin};
pub use query_vm::VmQuery;
pub use vm::{EffectMode, ReadObserver, Vm};

#[cfg(test)]
pub(crate) mod tests;
