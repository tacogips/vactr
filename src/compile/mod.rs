//! Compiler (design section 8): kernel forms to `FnProto` bytecode.

pub mod compiler;
mod matchc;
pub mod proto;
mod sites;

pub use compiler::{compile, CompileCx};

#[cfg(test)]
mod tests;
