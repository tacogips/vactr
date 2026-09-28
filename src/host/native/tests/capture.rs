//! Device-free capture callback and clock-bridge checks.

use crate::dsp::alloc_probe::armed;
use crate::dsp::caps::CapabilitySet;
use crate::host::native::audio::{capture_channel_rank, render_captured, MAX_BLOCK};
use crate::host::native::capture;
use crate::host::native::{NativeAudioHost, NativeConfig};

#[test]
fn native_capture_stays_disabled_by_default() {
    assert!(!NativeConfig::default().audio_in);
    let (host, _) = NativeAudioHost::headless(48_000, CapabilitySet::native(), 64);
    assert_eq!(host.capture_stats(), None);
}

#[test]
fn input_configuration_rejects_zero_channels_and_prefers_stereo() {
    assert_eq!(capture_channel_rank(0), None);
    assert!(capture_channel_rank(2) < capture_channel_rank(1));
    assert!(capture_channel_rank(1) < capture_channel_rank(4));
}

#[test]
fn capture_counters_are_reported_incrementally_off_callback() {
    let (mut host, _) = NativeAudioHost::headless(48_000, CapabilitySet::native(), 64);
    let (mut producer, mut consumer, counters) = capture::pair();
    host.attach_capture_counters(counters);
    producer.push_interleaved(&[f32::NAN], 1);
    let mut output = [0.0; 4];
    consumer.fill(&mut output);
    let stats = host.capture_stats().unwrap();
    assert_eq!((stats.nonfinite, stats.underrun), (1, 1));
    let diagnostics = host.take_diagnostics();
    assert!(diagnostics
        .iter()
        .any(|diag| diag.message.contains("nonfinite: 1")));
    assert!(diagnostics
        .iter()
        .any(|diag| diag.message.contains("underrun: 1")));
    assert!(
        host.take_diagnostics().is_empty(),
        "counters report only new deltas"
    );
}

#[test]
fn input_normalizes_mono_stereo_wide_short_and_nonfinite() {
    let (mut producer, mut consumer, counters) = capture::pair();
    producer.push_interleaved(&[0.2, -0.3], 1);
    producer.push_interleaved(&[0.4, 0.5], 2);
    producer.push_interleaved(&[1.6, -1.7, 0.8, 0.9], 4);
    producer.push_interleaved(&[f32::NAN, f32::INFINITY, 0.3, 0.4], 3);
    let mut out = [1.0; 12];
    consumer.fill(&mut out);
    assert_eq!(
        out,
        [0.2, 0.2, -0.3, -0.3, 0.4, 0.5, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0]
    );
    let stats = counters.snapshot();
    assert_eq!(stats.nonfinite, 2);
    assert_eq!(stats.short, 1);
    assert_eq!(stats.underrun, 1);
}

#[test]
fn overflow_and_clock_backlog_are_bounded_without_replay() {
    let (mut producer, mut consumer, counters) = capture::pair();
    let many = vec![0.25; 2 * 8193];
    producer.push_interleaved(&many, 2);
    assert_eq!(counters.snapshot().overrun, 1);
    let mut out = [0.0; 2 * MAX_BLOCK];
    consumer.fill(&mut out);
    assert!(out.iter().all(|x| *x == 0.25));
    assert!(counters.snapshot().stale >= 6144);
    for _ in 0..4 {
        consumer.fill(&mut out);
    }
    assert!(out.iter().all(|x| *x == 0.0));
    assert!(counters.snapshot().underrun > 0);
}

#[test]
fn simulated_capture_callback_routes_stereo_and_quad_without_allocating() {
    for rate in [44_100, 48_000, 96_000] {
        for frames in [64, 256] {
            for channels in [2, 4] {
                let caps = CapabilitySet::native();
                let (_, mut side) = if channels == 4 {
                    NativeAudioHost::headless_quad(rate, caps, 64)
                } else {
                    NativeAudioHost::headless(rate, caps, 64)
                };
                let (mut producer, mut consumer, counters) = capture::pair();
                let mut source = vec![0.0; 2 * frames];
                for (i, pair) in source.chunks_exact_mut(2).enumerate() {
                    pair.copy_from_slice(&[i as f32 / frames as f32 * 0.2, -0.3]);
                }
                let mut scratch = [0.0; 2 * MAX_BLOCK];
                let mut output = vec![0.0; frames * channels];
                let (_, allocations) = armed(|| {
                    producer.push_interleaved(&source, 2);
                    render_captured(
                        &mut side,
                        &mut consumer,
                        &mut scratch,
                        &mut output,
                        channels,
                    );
                });
                assert_eq!(allocations, 0);
                for (actual, expected) in output.chunks_exact(channels).zip(source.chunks_exact(2))
                {
                    assert_eq!(&actual[..2], expected);
                    if channels == 4 {
                        assert_eq!(&actual[2..], &[0.0, 0.0]);
                    }
                }
                assert_eq!(counters.snapshot().underrun, 0);
                let (_, allocations) = armed(|| {
                    render_captured(
                        &mut side,
                        &mut consumer,
                        &mut scratch,
                        &mut output,
                        channels,
                    )
                });
                assert_eq!(allocations, 0);
                assert!(
                    output.iter().all(|x| *x == 0.0),
                    "disconnected input must not replay"
                );
            }
        }
    }
}
