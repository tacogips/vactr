//! SS-ANALYSIS: the native live taps and captures (design 14.5.9) through
//! a headless host. The callback body allocates nothing with taps active
//! and a capture armed; a snapshot equals the last rendered frames; a
//! capture delivers exactly its frame count.

use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::alloc_probe::armed;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::controls;
use crate::dsp::graph::{BusDef, BusId, InstDef, InstId, UGenSpec};
use crate::host::caps::{
    AudioHost, CapturePoll, GraphHandle, InstResolver, Route, SignalInput, TapSrc,
};
use crate::host::native::audio::{AudioSide, MAX_BLOCK};
use crate::host::native::tap::TAP_FRAMES;
use crate::host::native::NativeAudioHost;
use crate::host::wire::{AudioEvent, Ctl};
use crate::sched::slots::SlotId;
use crate::value::intern::{intern_kw, KwId};
use crate::value::value::Sound;
use crate::vm::fail::{FailCode, Failure};

const SR: u32 = 48_000;

fn host() -> (NativeAudioHost, AudioSide) {
    NativeAudioHost::headless(SR, CapabilitySet::native(), 64)
}

/// Renders `frames` stereo frames, asserting the callback allocates
/// nothing.
fn render(side: &mut AudioSide, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; 2 * frames];
    let ((), allocs) = armed(|| side.render(&mut out, 2));
    assert_eq!(allocs, 0, "the callback allocated");
    out
}

/// A constant-1.0 instrument.
fn dc(id: u32) -> InstDef {
    InstDef {
        id: InstId::new(id),
        params: Box::new([]),
        nodes: Box::new([UGenSpec::Const(1.0)]),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

/// Names `:drums` as bus 1.
struct Names;

impl InstResolver for Names {
    fn route(&self, _: &Sound) -> Result<Route, Failure> {
        Err(Failure::new(FailCode::UnknownSound, "no sounds"))
    }
    fn inst(&self, _: InstId) -> Option<Arc<InstDef>> {
        None
    }
    fn signal_inputs(&self) -> Vec<SignalInput> {
        Vec::new()
    }
    fn bus(&self, name: KwId) -> Option<BusId> {
        (name == intern_kw("drums")).then_some(BusId::new(1))
    }
}

/// A host playing one DC voice into bus 1 (`:drums`) and the master.
fn sounding() -> (NativeAudioHost, AudioSide) {
    let (mut h, mut side) = host();
    h.set_bus_names(Rc::new(Names));
    h.swap_graph(GraphHandle::Inst {
        id: InstId::new(1),
        def: Arc::new(dc(1)),
    });
    h.swap_graph(GraphHandle::Bus {
        id: BusId::new(1),
        def: Arc::new(BusDef {
            id: BusId::new(1),
            chain: Box::new([]),
        }),
    });
    let _ = render(&mut side, 128);
    let mut ev = AudioEvent::new(h.now(), SlotId::new(1), 1, InstId::new(1));
    let row = controls::row("bus").expect("the bus row");
    ev.push_ctl(row.ctl, Ctl::Const(1.0)).expect("a control");
    h.send(ev);
    (h, side)
}

#[test]
fn a_snapshot_equals_the_last_rendered_frames() {
    let (mut h, mut side) = sounding();
    let out = render(&mut side, 3 * MAX_BLOCK + 100);
    let mut reader = h.tap_reader().expect("native taps");
    let mut snap = Vec::new();
    reader
        .snapshot(&TapSrc::Master, 256, &mut snap)
        .expect("a snapshot");
    let frames = out.len() / 2;
    let want: Vec<f32> = (frames - 256..frames)
        .map(|k| 0.5 * (out[2 * k] + out[2 * k + 1]))
        .collect();
    assert_eq!(snap, want);
    assert!(snap.iter().any(|x| *x != 0.0), "the voice sounds");
    // The bus block is tapped too (it feeds the master unchanged).
    reader
        .snapshot(&TapSrc::Bus(intern_kw("drums")), 256, &mut snap)
        .expect("a bus snapshot");
    assert_eq!(snap, want);
    // A longer request than the ring holds is capped at the ring.
    reader
        .snapshot(&TapSrc::Master, TAP_FRAMES + 10, &mut snap)
        .expect("capped");
    assert_eq!(snap.len(), TAP_FRAMES);
    // An unknown bus is a type failure; without bus names, host-unavailable.
    let e = reader
        .snapshot(&TapSrc::Bus(intern_kw("nope")), 8, &mut snap)
        .expect_err("unknown");
    assert_eq!(e.code, FailCode::Type);
    let (mut bare, _side) = host();
    let e = bare
        .tap_reader()
        .expect("taps")
        .snapshot(&TapSrc::Bus(intern_kw("drums")), 8, &mut snap)
        .expect_err("no names");
    assert_eq!(e.code, FailCode::HostUnavailable);
}

#[test]
fn a_capture_delivers_exactly_its_frames_with_no_callback_allocation() {
    let (mut h, mut side) = sounding();
    let start = h.now() + 0.001;
    let frames = 1500;
    let id = h
        .arm_capture(&TapSrc::Master, start, frames)
        .expect("armed");
    let mut got = Vec::new();
    let mut rendered = Vec::new();
    let mut done = false;
    for _ in 0..10 {
        // `render` asserts zero allocations with taps active and a capture
        // armed.
        rendered.extend(render(&mut side, MAX_BLOCK));
        match h.poll_capture(id, &mut got) {
            CapturePoll::Done => {
                done = true;
                break;
            }
            CapturePoll::Pending => {}
            CapturePoll::Failed(f) => panic!("{f}"),
        }
    }
    assert!(done, "the capture completed");
    assert_eq!(got.len(), 2 * frames);
    // The captured frames are the master output from the start frame on.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let first = (start * f64::from(SR)).round() as usize - 128;
    assert_eq!(&got[..], &rendered[2 * first..2 * (first + frames)]);
    // The slot is free again: a finished id is not pending any more.
    assert!(matches!(
        h.poll_capture(id, &mut got),
        CapturePoll::Failed(_)
    ));
}

#[test]
fn a_bus_capture_streams_the_bus_block() {
    let (mut h, mut side) = sounding();
    let id = h
        .arm_capture(&TapSrc::Bus(intern_kw("drums")), h.now(), 600)
        .expect("armed");
    let _ = render(&mut side, 2 * MAX_BLOCK);
    let mut got = Vec::new();
    assert_eq!(h.poll_capture(id, &mut got), CapturePoll::Done);
    assert_eq!(got.len(), 1200);
    assert!(got.iter().any(|x| *x != 0.0));
}
