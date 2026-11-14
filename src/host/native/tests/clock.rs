use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::host::native::clock::{LatencyKind, OutputClock};

#[test]
fn measured_clock_reports_latency_and_buffer_uncertainty() {
    let clock = OutputClock::new(48_000);
    clock.set_running(true);
    let now = clock.now_nanos();
    clock.record_raw(48_000, now, Some(10_000_000));
    let reading = clock.read();
    assert!((1.0..=1.0 + 512.0 / 48_000.0).contains(&reading.processing_time));
    assert_eq!(reading.latency_kind, LatencyKind::Measured);
    assert_eq!(reading.latency_seconds, Some(0.01));
    assert_eq!(reading.uncertainty_seconds, Some(512.0 / 48_000.0));
}

#[test]
fn missing_playback_timestamp_uses_a_buffer_estimate() {
    let clock = OutputClock::new(48_000);
    clock.set_running(true);
    let now = clock.now_nanos();
    clock.record_raw(48_000, now, None);
    let reading = clock.read();
    assert_eq!(reading.latency_kind, LatencyKind::Estimate);
    assert_eq!(reading.latency_seconds, Some(512.0 / 48_000.0));
}

#[test]
fn absent_stopped_and_stale_samples_are_unavailable() {
    let clock = OutputClock::backdated(48_000, Duration::from_secs(5));
    assert_eq!(clock.read().latency_kind, LatencyKind::Unavailable);
    clock.set_running(true);
    let now = clock.now_nanos();
    assert!(now > 2_000_000_000);
    clock.record_raw(48_000, now - 2_000_000_000, Some(10_000_000));
    let stale = clock.read();
    assert_eq!(stale.latency_kind, LatencyKind::Unavailable);
    assert_eq!(stale.latency_seconds, None);

    clock.record_raw(48_000, now - 500_000_000, Some(10_000_000));
    let recent = clock.read();
    assert_eq!(recent.latency_kind, LatencyKind::Measured);
    assert_eq!(recent.latency_seconds, Some(0.01));

    clock.record_raw(48_000, now, Some(10_000_000));
    clock.set_running(false);
    let stopped = clock.read();
    assert_eq!(stopped.latency_kind, LatencyKind::Unavailable);
    assert_eq!(stopped.latency_seconds, None);
}

#[test]
fn concurrent_writer_never_publishes_backwards_or_nonfinite_time() {
    let clock = Arc::new(OutputClock::new(48_000));
    clock.set_running(true);
    let writer_clock = Arc::clone(&clock);
    let writer = thread::spawn(move || {
        for frame in 1..=10_000_u64 {
            let now = writer_clock.now_nanos();
            writer_clock.record_raw(frame, now, None);
        }
    });
    let mut last = 0.0;
    for _ in 0..10_000 {
        let reading = clock.read();
        assert!(reading.processing_time.is_finite());
        assert!(reading.processing_time >= last);
        last = reading.processing_time;
    }
    writer.join().unwrap();
}
