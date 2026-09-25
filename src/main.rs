//! vactrol - Vactrol scripting language
//!
//! This is the main entry point for the vactrol binary. The CLI belongs to
//! TASK-009; for now the binary only reports its version.

fn main() {
    println!("vactrol {}", env!("CARGO_PKG_VERSION"));
}
