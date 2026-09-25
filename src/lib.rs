//! vactrol - Vactrol scripting language
//!
//! The core crate: value model, reader and the module skeleton laid out in
//! `design-docs/specs/design-implementation.md` section 4. Core modules use
//! no OS threads and no I/O so the crate builds for `wasm32-unknown-unknown`.

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

pub mod compile;
pub mod dsp;
pub mod expand;
pub mod ns;
pub mod pattern;
pub mod reader;
pub mod sched;
pub mod tex;
pub mod types;
pub mod value;
pub mod vm;
