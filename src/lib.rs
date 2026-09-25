//! vactrol - Vactrol scripting language
//!
//! The core crate, laid out as in `design-docs/specs/design-implementation.md`
//! section 4. Every top-level form goes read -> expand -> check -> compile ->
//! run (section 7.1.1):
//!
//! - `value`: the runtime value model (numbers, lists, dicts, paths, sounds).
//! - `reader`: the lexer, layout rules and the node tree; `expand`: the
//!   expander that lowers surface sugar to the kernel forms.
//! - `types`: the static checker with inference, the forcing masks and the
//!   native signature table.
//! - `ns`, `compile`, `vm`: namespaces and tweak slots, the reactive
//!   dependency graph and top-level `Evaluator`, the bytecode compiler, and
//!   the VM with the core and domain natives.
//! - `pattern`, `clock`, `tex`: the pattern engine and signals, the cycle
//!   clock, and the visual chains with their shader and uniform plans.
//! - `sched`, `dsp`, `host`: the scheduler and slot table, the DSP graph and
//!   audio engine, and the capability hosts with their wire records
//!   (design 11, 12, 12.8).
//!
//! Core modules use no OS threads and no I/O, so the crate builds for
//! `wasm32-unknown-unknown`; host file and sample I/O stay behind the
//! `NoopHost` source loader until the host tasks.

/// Defines an id newtype over a raw integer (design 6.5.1).
///
/// Each id derives `Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug`
/// and has `new` and `get`. Nothing else.
macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident($raw:ty)) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $name($raw);

        impl $name {
            /// Wraps a raw id.
            #[must_use]
            pub const fn new(raw: $raw) -> Self {
                Self(raw)
            }

            /// Returns the raw id.
            #[must_use]
            pub const fn get(self) -> $raw {
                self.0
            }
        }
    };
}

pub mod clock;
pub mod compile;
pub mod dsp;
pub mod expand;
pub mod host;
pub mod ns;
pub mod pattern;
pub mod reader;
pub mod sched;
pub mod tex;
pub mod types;
pub mod value;
pub mod vm;

/// The counting allocator of the zero-allocation proof (design 12.8.9).
/// Test builds only: release, example and wasm builds never contain it.
#[cfg(test)]
#[global_allocator]
static ALLOC: dsp::alloc_probe::Counting = dsp::alloc_probe::Counting;
