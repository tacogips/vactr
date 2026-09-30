//! TASK-008 criterion 3 (DSP half): a `sample-play` event renders exactly
//! its begin/end region; speed and loop stretch and loop it; regions at the
//! sample bounds never read outside the sample; and the kernel reads no
//! controls beyond begin/end/speed/loop (plus the event's bank).

use super::{chain, ctl, event, NativeRig};
use crate::dsp::graph::{BankRef, UGenSpec};
use crate::dsp::ugen::catalog::SAMPLE;
use crate::dsp::ugen::sample::region;
use crate::dsp::ugen::Template;
use crate::sched::slots::CtlId;

const N: usize = 4096;
const BANK_ID: u32 = 7;

/// A ramp sample whose every value is distinct and non-zero.
fn ramp() -> Vec<f32> {
    #[allow(clippy::cast_precision_loss)]
    (0..N).map(|i| (i + 1) as f32 / N as f32).collect()
}

fn sampler(rig: &mut NativeRig) {
    rig.sample(BANK_ID, ramp(), 1);
    rig.install(&chain(1, vec![UGenSpec::SamplePlay(BankRef::new(BANK_ID))]));
    let _ = rig.step();
}

fn play(ctls: &[(CtlId, f32)], blocks: usize) -> (NativeRig, Vec<f32>) {
    let mut rig = NativeRig::native();
    sampler(&mut rig);
    let mut all = vec![(ctl::PAN, 0.0)];
    all.extend_from_slice(ctls);
    let t = rig.engine.now();
    rig.send(event(1, t, &all));
    let (l, _) = rig.run(blocks);
    (rig, l)
}

#[test]
fn renders_exactly_the_begin_end_region() {
    let (rig, l) = play(&[(ctl::BEGIN, 0.25), (ctl::END, 0.5)], 12);
    let data = ramp();
    let want = &data[N / 4..N / 2];
    assert_eq!(&l[..want.len()], want, "the region, sample for sample");
    assert!(
        l[want.len()..].iter().all(|v| *v == 0.0),
        "nothing after end"
    );
    assert_eq!(rig.engine.active_voices(), 0, "a one-shot ends at its end");
}

#[test]
fn speed_stretches_the_region() {
    let data = ramp();
    let (_, l) = play(
        &[(ctl::BEGIN, 0.5), (ctl::END, 0.75), (ctl::SPEED, 2.0)],
        12,
    );
    let want: Vec<f32> = data[N / 2..3 * N / 4].iter().step_by(2).copied().collect();
    assert_eq!(
        &l[..want.len()],
        &want[..],
        "double speed reads every other frame"
    );
    assert!(l[want.len()..].iter().all(|v| *v == 0.0));
    let (_, half) = play(&[(ctl::BEGIN, 0.0), (ctl::END, 0.01), (ctl::SPEED, 0.5)], 4);
    // Half speed: each source frame is held for two output frames (the odd
    // ones interpolate halfway).
    assert_eq!(half[0], data[0]);
    assert_eq!(half[2], data[1]);
    assert!((half[1] - 0.5 * (data[0] + data[1])).abs() < 1.0e-6);
}

#[test]
fn reverse_plays_from_end_to_begin() {
    let data = ramp();
    let (_, l) = play(
        &[(ctl::BEGIN, 0.0), (ctl::END, 0.25), (ctl::SPEED, -1.0)],
        12,
    );
    let want: Vec<f32> = data[..N / 4].iter().rev().copied().collect();
    assert_eq!(&l[..want.len()], &want[..]);
    assert!(l[want.len()..].iter().all(|v| *v == 0.0));
}

#[test]
fn loop_repeats_the_region() {
    let data = ramp();
    let (rig, l) = play(
        &[
            (ctl::BEGIN, 0.0),
            (ctl::END, 0.125),
            (ctl::LOOP, 1.0),
            (ctl::LEGATO, 10.0),
        ],
        12,
    );
    let len = N / 8;
    for k in 0..(l.len() / len) {
        assert_eq!(&l[k * len..(k + 1) * len], &data[..len], "loop pass {k}");
    }
    assert_eq!(rig.engine.active_voices(), 1, "a loop keeps sounding");
}

#[test]
fn regions_at_the_bounds_stay_inside_the_sample() {
    let data = ramp();
    let lo = *data.first().unwrap();
    let hi = *data.last().unwrap();
    for (b, e, speed, looping) in [
        (0.999, 1.0, 1.5, 1.0),
        (1.0, 1.0, 1.0, 0.0),
        (0.0, 0.0001, -3.0, 1.0),
        (-5.0, 7.0, 4.0, 1.0),
        (0.9, 0.1, 1.0, 0.0),
        (f32::NAN, f32::INFINITY, 2.7, 1.0),
    ] {
        let (_, l) = play(
            &[
                (ctl::BEGIN, b),
                (ctl::END, e),
                (ctl::SPEED, speed),
                (ctl::LOOP, looping),
            ],
            6,
        );
        assert!(
            l.iter().all(|v| *v == 0.0 || (lo..=hi).contains(v)),
            "every sample comes from the data (begin {b}, end {e}, speed {speed})"
        );
    }
    for frames in [0, 1, 2, 4096] {
        for (b, e) in [(0.0, 1.0), (1.0, 0.0), (0.5, 0.5), (-1.0, 2.0)] {
            let (start, stop) = region(frames, b, e);
            assert!(start <= stop && stop <= frames);
        }
    }
}

#[test]
fn sample_play_reads_only_its_region_controls() {
    let names: Vec<&str> = SAMPLE.iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        [
            "speed",
            "begin",
            "end",
            "loop",
            "region-start-low",
            "region-start-high",
            "region-stop-low",
            "region-stop-high"
        ]
    );
    let env = NativeRig::native().engine.build_env();
    let t = Template::from_inst(
        &chain(1, vec![UGenSpec::SamplePlay(BankRef::new(BANK_ID))]),
        &env,
    )
    .unwrap();
    let mut ids: Vec<u16> = t.params().iter().map(|(c, _)| c.get()).collect();
    ids.sort_unstable();
    let mut want = vec![
        ctl::SPEED.get(),
        ctl::BEGIN.get(),
        ctl::END.get(),
        ctl::LOOP.get(),
        158,
        159,
        160,
        161,
    ];
    want.sort_unstable();
    assert_eq!(
        ids, want,
        "only legacy playback controls and exact timestamp frame words"
    );
}

#[test]
fn bank_control_selects_the_resource() {
    let mut rig = NativeRig::native();
    sampler(&mut rig);
    rig.sample(9, vec![0.25; 256], 1);
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::PAN, 0.0), (ctl::BANK, 9.0)]));
    let (l, _) = rig.run(4);
    assert_eq!(&l[..256], &[0.25; 256][..]);
    assert!(l[256..].iter().all(|v| *v == 0.0));
}
