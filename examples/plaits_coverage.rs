//! Run with `CARGO_TERM_QUIET=true cargo run -q --example plaits_coverage`.

fn main() {
    println!("{}", vactrol::dsp::ported::plaits_coverage_summary());
}
