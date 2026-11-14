//! Timestamp argument precision and sample-commit composition through the VM.
use super::{cval, Rig};
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::host::wire::HostMsg;
use crate::sched::runtime::RuntimeConfig;
use crate::vm::fail::Failure;
use std::sync::Arc;
struct Loader(u32);
impl SampleLoader for Loader {
    fn load(&mut self, _src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        Ok(Arc::new(SampleData {
            rate: self.0,
            channels: 1,
            frames: vec![0.0; self.0 as usize * 2].into_boxed_slice(),
        }))
    }
}
fn rig(rate: u32, source: &str) -> Rig {
    let mut rig = Rig::with_loader(
        RuntimeConfig::default(),
        CapabilitySet::native(),
        Box::new(Loader(rate)),
    );
    let report = rig.run(source);
    assert!(report.faults.is_empty(), "{:?}", report.faults);
    rig.audio.reply(HostMsg::Installed {
        resource: 1,
        gen: 0,
    });
    rig.run_to(2.1);
    rig
}
fn frame(event: &crate::host::wire::AudioEvent, prefix: &str) -> u32 {
    ((cval(event, &format!("{prefix}-high")).unwrap() as u32) << 16)
        | (cval(event, &format!("{prefix}-low")).unwrap() as u32)
}
#[test]
fn timestamp_literal_list_and_dynamic_block_preserve_exact_microseconds_at_commit() {
    for source in [
        "s :break > start-ms 123.456 > stop-ms 900 > d1",
        "s :break > start-ms [123.456 200.001] > stop-ms 900 > d1",
        "s :break > start-ms {+ 123.456 0} > stop-ms 900 > d1",
    ] {
        let rig = rig(48000, source);
        assert!(rig.faults().is_empty(), "{:?}", rig.faults());
        let sent = rig.sent();
        assert!(!sent.is_empty());
        assert_eq!(frame(&sent[0].1, "region-start"), 5926);
        assert_eq!(frame(&sent[0].1, "region-stop"), 43200);
    }
}
#[test]
fn timestamp_chop_slice_splice_fit_and_reverse_compose_inside_window() {
    for rate in [44100, 48000, 96000] {
        let chopped = rig(rate, "s :break > start-ms 100 > stop-ms 900 > chop 2 > d1");
        let sent = chopped.sent();
        assert!(sent.len() >= 2);
        assert_eq!(frame(&sent[0].1, "region-start"), rate / 10);
        assert_eq!(frame(&sent[0].1, "region-stop"), rate / 2);
        assert_eq!(frame(&sent[1].1, "region-start"), rate / 2);
        let sliced = rig(
            rate,
            "s :break > start-ms 100 > stop-ms 900 > slice 4 [3 0] > speed {- 0 1} > d1",
        );
        let sent = sliced.sent();
        assert_eq!(frame(&sent[0].1, "region-start"), 7 * rate / 10);
        assert_eq!(cval(&sent[0].1, "speed"), Some(-1.0));
        let spliced = rig(
            rate,
            "s :break > start-ms 100 > stop-ms 900 > splice 4 [3 0] > d1",
        );
        let sent = spliced.sent();
        assert!((cval(&sent[0].1, "speed").unwrap() - 0.2).abs() < 1e-6);
        let fitted = rig(rate, "s :break > start-ms 100 > stop-ms 900 > fit > d1");
        assert!((cval(&fitted.sent()[0].1, "speed").unwrap() - 0.4).abs() < 1e-6);
    }
}
#[test]
fn timestamp_invalid_precision_bounds_and_non_sample_uses_report_diagnostics() {
    for source in [
        "s :break > start-ms -1 > d1",
        "s :break > start-ms 1.0001 > d1",
        "s :break > start-ms 1000 > stop-ms 900 > d1",
        "s :break > stop-ms 2001 > d1",
        "s :bd > start-ms 0 > d1",
    ] {
        let mut rig = Rig::with_loader(
            RuntimeConfig::default(),
            CapabilitySet::native(),
            Box::new(Loader(48000)),
        );
        let report = rig.run(source);
        rig.audio.reply(HostMsg::Installed {
            resource: 1,
            gen: 0,
        });
        rig.run_to(2.1);
        assert!(
            !report.faults.is_empty() || !rig.faults().is_empty(),
            "{source} accepted"
        );
        assert!(rig.sent().is_empty());
    }
}
