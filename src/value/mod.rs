//! The core value model (design section 5, 6.5.3; TASK-001).

pub mod access;
pub mod dict;
pub mod eq;
pub mod intern;
pub mod key;
pub mod num;
pub mod print;
pub mod ratio;
pub mod sample;
// The `value/value.rs` layout is fixed by design section 4.
#[allow(clippy::module_inception)]
pub mod value;

pub use access::{first, get, index, len, truthy};
pub use dict::{dict_from_pairs, join, pairs, put};
pub use eq::deep_eq;
pub use intern::{intern_kw, intern_sym, name_of_kw, name_of_sym, Interner, KwId, SymId};
pub use key::{Key, NumKey};
pub use num::{widen, NumKind};
pub use ratio::Ratio64;
pub use value::{ListProv, ListVal, NativeId, RangeVal, StructVal, Value, VariantVal};

#[cfg(test)]
mod tests;
