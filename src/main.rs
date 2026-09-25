//! vactrol - Vactrol scripting language
//!
//! The binary entry point (design 14.5.10, `command.md`). All the real
//! work is `vactrol::cli::main`, run with the process arguments and exit
//! code by `vactrol::cli::run`; wasm32 builds link an empty `main` so the
//! default-feature wasm32 build still produces a binary target.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    vactrol::cli::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
