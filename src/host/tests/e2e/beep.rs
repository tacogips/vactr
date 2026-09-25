//! The `examples/beep.rs` program, headless (design 12.8.10, 12.8.12): the
//! automated proxy for TASK-008's manual criterion 10.

use super::{all_finite, peak, rms, E2e};

/// `s :analog > note [:a4] > once`, one second in: non-silent, finite and
/// bounded, zero callback allocation, at least one event committed, no
/// faults.
#[test]
fn the_beep_program_renders_headlessly() {
    let mut e = E2e::new();
    e.eval("s :analog > note [:a4] > once");
    let left = e.run_for(1.0);

    assert!(all_finite(&left), "every sample is finite");
    let level = rms(&left);
    assert!(level > 1.0e-3, "the beep sounds (rms {level})");
    assert!(peak(&left) < 4.0, "bounded (peak {})", peak(&left));
    assert!(e.committed > 0, "at least one event committed");
    assert!(e.faults.is_empty(), "no faults: {:?}", e.faults);
}
