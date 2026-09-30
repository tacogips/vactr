//! Exact timestamp frame words through native handovers and browser records.
use super::{caps, chain, config, ctl, event, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_inst, StoreKind};
use crate::dsp::graph::{BankRef, UGenSpec};
use crate::dsp::ring::{encode_graph_record, encode_sample_begin, encode_slice, NativeInstall};
use crate::dsp::ugen::sample::frame_region;
use crate::host::caps::SampleData;
use crate::host::wire::HostMsg;
use crate::sched::slots::CtlId;
use std::sync::Arc;

fn words(start: u32, stop: u32, speed: f32) -> Vec<(CtlId, f32)> {
    vec![
        (ctl::PAN, 0.0),
        (ctl::SPEED, speed),
        (CtlId::new(158), (start & 65535) as f32),
        (CtlId::new(159), (start >> 16) as f32),
        (CtlId::new(160), (stop & 65535) as f32),
        (CtlId::new(161), (stop >> 16) as f32),
        // Exact frames take precedence over these legacy fractions.
        (ctl::BEGIN, 0.0),
        (ctl::END, 0.01),
    ]
}

#[test]
fn timestamp_frame_words_preserve_long_source_indices_without_f32_rounding() {
    let editor = crate::dsp::meta::decl_for("sample-play").unwrap();
    assert_eq!(
        editor.params.len(),
        4,
        "transport words stay out of the editor"
    );
    for (start, stop) in [(33_600_001, 40_000_003), (u32::MAX - 257, u32::MAX)] {
        let controls = words(start, stop, 1.0);
        let raw = std::array::from_fn(|i| controls[i + 2].1);
        assert_eq!(
            frame_region(u32::MAX as usize, raw),
            Some((start as usize, stop as usize))
        );
    }
    assert_eq!(frame_region(100, [0.0, -1.0, 0.0, -1.0]), None);
}

#[test]
fn timestamp_native_browser_audio_matches_exact_exclusive_window_and_reverse_at_three_rates() {
    for rate in [44100u32, 48000, 96000] {
        let data: Vec<f32> = (0..2048).map(|i| (i + 1) as f32 / 4096.0).collect();
        // 1.234 and 9.876 milliseconds: select frames whose times are in [start, stop).
        let start = (1234u64 * u64::from(rate)).div_ceil(1_000_000) as u32;
        let stop = (9876u64 * u64::from(rate)).div_ceil(1_000_000) as u32;
        for speed in [1.0, -1.0] {
            let graph = chain(1, vec![UGenSpec::SamplePlay(BankRef::new(10))]);
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate as f32;
            let mut native = NativeRig::native_with(cfg);
            native.post_native(NativeInstall::Sample {
                resource: 10,
                gen: 1,
                data: Arc::new(SampleData {
                    rate,
                    channels: 1,
                    frames: data.clone().into_boxed_slice(),
                }),
            });
            native.install(&graph);
            let _ = native.step();
            assert!(native
                .acks()
                .iter()
                .any(|a| matches!(a, HostMsg::Installed { resource: 10, .. })));
            native.send(event(1, native.engine.now(), &words(start, stop, speed)));
            let (native_left, _) = native.run(10);

            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = rate as f32;
            let mut browser = BrowserRig::browser_with(cfg);
            browser.push(&encode_sample_begin(10, 1, data.len() as u32, 1, rate));
            let mut record = Vec::new();
            encode_slice(10, 0, &data, &mut record);
            browser.push(&record);
            let mut bytes = Vec::new();
            encode_inst(&graph, &mut bytes).unwrap();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(4);
            assert!(browser
                .acks()
                .iter()
                .any(|a| matches!(a, HostMsg::Installed { resource: 10, .. })));
            browser.send(event(1, browser.engine.now(), &words(start, stop, speed)));
            let (browser_left, _) = browser.run(10);
            let mut expected = data[start as usize..stop as usize].to_vec();
            if speed < 0.0 {
                expected.reverse();
            }
            for rendered in [&native_left, &browser_left] {
                assert_eq!(
                    &rendered[..expected.len()],
                    &expected,
                    "rate {rate}, speed {speed}"
                );
                assert!(rendered[expected.len()..].iter().all(|v| *v == 0.0));
            }
            assert_eq!(native_left, browser_left);
        }
    }
}
