//! `NativeAudioHost` logic without a device: `now` from the frame counter,
//! ack draining, graph and sample installs, ring overflow counting, and a
//! callback body that allocates nothing.

use std::sync::Arc;

use crate::dsp::alloc_probe::armed;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::EVENT_CAPACITY;
use crate::host::caps::{AudioHost, GraphHandle, SampleData};
use crate::host::native::audio::{AudioSide, GRAPH_RESOURCE_BASE, MAX_BLOCK};
use crate::host::native::NativeAudioHost;
use crate::host::wire::{AudioEvent, HostMsg, Release, SlotControl, SlotControlAck};
use crate::sched::slots::SlotId;
use crate::types::diag::DiagCode;

const SR: u32 = 48_000;

fn host() -> (NativeAudioHost, AudioSide) {
    let caps = CapabilitySet {
        max_voices: 8,
        ..CapabilitySet::native()
    };
    NativeAudioHost::headless(SR, caps, 64)
}

/// Renders `frames` stereo frames through the callback body, asserting
/// that it allocates nothing.
fn render(side: &mut AudioSide, frames: usize, channels: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * channels];
    let ((), allocs) = armed(|| side.render(&mut out, channels));
    assert_eq!(allocs, 0, "the callback allocated");
    out
}

fn drained(h: &mut NativeAudioHost) -> Vec<HostMsg> {
    let mut out = Vec::new();
    h.drain(&mut out);
    out
}

fn dc(id: u32) -> InstDef {
    InstDef {
        id: InstId::new(id),
        params: Box::new([]),
        nodes: Box::new([UGenSpec::Const(1.0)]),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

fn quad_dc(id: u32) -> InstDef {
    InstDef {
        id: InstId::new(id),
        params: Box::new([]),
        nodes: Box::new([
            UGenSpec::Const(0.1),
            UGenSpec::Const(0.2),
            UGenSpec::Const(0.3),
            UGenSpec::Const(0.4),
            UGenSpec::AuxOut,
            UGenSpec::Out3,
            UGenSpec::Out4,
        ]),
        edges: Box::new([
            Edge {
                from: 1,
                to: 4,
                port: 0,

                output: 0,
            },
            Edge {
                from: 2,
                to: 5,
                port: 0,

                output: 0,
            },
            Edge {
                from: 3,
                to: 6,
                port: 0,

                output: 0,
            },
        ]),
        node_params: Box::new([]),
    }
}

#[test]
fn headless_quad_callback_emits_four_distinct_lanes_without_allocation() {
    let caps = CapabilitySet {
        max_voices: 8,
        ..CapabilitySet::native()
    };
    let (mut h, mut side) = NativeAudioHost::headless_quad(SR, caps, 64);
    h.swap_graph(GraphHandle::Inst {
        id: InstId::new(1),
        def: Arc::new(quad_dc(1)),
    });
    let _ = render(&mut side, 128, 4);
    assert!(drained(&mut h)
        .iter()
        .any(|msg| matches!(msg, HostMsg::Installed { .. })));
    h.send(AudioEvent::new(h.now(), SlotId::new(1), 1, InstId::new(1)));
    let out = render(&mut side, 256, 4);
    for lane in 0..4 {
        let energy = out
            .chunks_exact(4)
            .map(|frame| frame[lane].abs())
            .sum::<f32>();
        assert!(energy > 0.01, "lane {lane} is silent");
    }
    for pair in out.chunks_exact(4) {
        assert!((pair[2] - pair[3]).abs() > 0.001);
    }
}

#[test]
fn now_follows_the_frame_counter() {
    let (h, mut side) = host();
    assert_eq!(h.now(), 0.0);
    let _ = render(&mut side, 480, 2);
    assert_eq!(h.now(), 0.01);
    let _ = render(&mut side, 3 * MAX_BLOCK, 2);
    assert_eq!(h.clock().frames(), 480 + 3 * MAX_BLOCK as u64);
}

#[test]
fn slot_controls_are_acknowledged_through_drain() {
    let (mut h, mut side) = host();
    h.control(SlotControl {
        slot: SlotId::new(3),
        new_gen: 2,
        effective_time: 0.0,
        release: Release::Panic,
    });
    assert!(
        drained(&mut h).is_empty(),
        "nothing before the callback runs"
    );
    let _ = render(&mut side, 128, 2);
    assert!(
        drained(&mut h).contains(&HostMsg::SlotControlAck(SlotControlAck {
            slot: SlotId::new(3),
            gen: 2
        }))
    );
}

#[test]
fn an_installed_instrument_sounds_and_a_replacement_retires_it() {
    let (mut h, mut side) = host();
    h.swap_graph(GraphHandle::Inst {
        id: InstId::new(1),
        def: Arc::new(dc(1)),
    });
    let _ = render(&mut side, 128, 2);
    assert!(drained(&mut h).contains(&HostMsg::Installed {
        resource: GRAPH_RESOURCE_BASE,
        gen: 1
    }));
    h.send(AudioEvent::new(h.now(), SlotId::new(1), 1, InstId::new(1)));
    let out = render(&mut side, 256, 2);
    assert!(out.iter().any(|v| *v != 0.0), "the voice sounds");
    // The replacement goes live; the first template retires once no
    // voice plays it, and its box comes back through the garbage ring.
    h.swap_graph(GraphHandle::Inst {
        id: InstId::new(1),
        def: Arc::new(dc(1)),
    });
    let mut acks = Vec::new();
    for _ in 0..400 {
        let _ = render(&mut side, 512, 2);
        acks.extend(drained(&mut h));
    }
    assert!(acks.contains(&HostMsg::Installed {
        resource: GRAPH_RESOURCE_BASE + 1,
        gen: 1
    }));
    assert!(acks.contains(&HostMsg::Retired {
        resource: GRAPH_RESOURCE_BASE
    }));
    assert!(h.take_diagnostics().is_empty());
}

#[test]
fn samples_install_and_retire_under_their_table_ids() {
    let (mut h, mut side) = host();
    let data = Arc::new(SampleData {
        rate: 44_100,
        channels: 1,
        frames: vec![0.5; 64].into_boxed_slice(),
    });
    h.install_sample(7, data);
    let _ = render(&mut side, 128, 2);
    assert!(drained(&mut h).contains(&HostMsg::Installed {
        resource: 7,
        gen: 1
    }));
    h.retire_sample(7);
    let _ = render(&mut side, 128, 2);
    assert!(drained(&mut h).contains(&HostMsg::Retired { resource: 7 }));
}

#[test]
fn a_full_event_ring_drops_counts_and_reports() {
    let (mut h, mut side) = host();
    let n = EVENT_CAPACITY + 5;
    for i in 0..n {
        #[allow(clippy::cast_precision_loss)]
        let t = 100.0 + i as f64;
        h.send(AudioEvent::new(t, SlotId::new(1), 0, InstId::new(9)));
    }
    assert_eq!(h.dropped(), 5);
    let dropped = h.dropped();
    let msgs = drained(&mut h);
    assert!(msgs.contains(&HostMsg::Counters {
        late: 0,
        dropped,
        stolen: 0,
        skipped: 0
    }));
    let diags = h.take_diagnostics();
    assert_eq!(diags.len(), dropped as usize);
    assert!(diags.iter().all(|d| d.code == DiagCode::RingOverflow));
    // Reported once: the next drain carries no new counter.
    let _ = render(&mut side, 128, 2);
    assert!(!drained(&mut h)
        .iter()
        .any(|m| matches!(m, HostMsg::Counters { dropped: d, .. } if *d > 0)));
}

#[test]
fn mono_and_wide_devices_render_without_allocating() {
    let (mut h, mut side) = host();
    h.swap_graph(GraphHandle::Inst {
        id: InstId::new(1),
        def: Arc::new(dc(1)),
    });
    let _ = render(&mut side, 128, 1);
    h.send(AudioEvent::new(h.now(), SlotId::new(1), 1, InstId::new(1)));
    let mono = render(&mut side, 2 * MAX_BLOCK + 3, 1);
    assert!(mono.iter().any(|v| *v != 0.0));
    let wide = render(&mut side, 100, 4);
    assert!(wide.chunks_exact(4).all(|f| f[2] == 0.0 && f[3] == 0.0));
    let none = render(&mut side, 0, 0);
    assert!(none.is_empty());
}

#[test]
fn analysis_reads_the_published_host_signals() {
    let (h, mut side) = host();
    let _ = render(&mut side, 1024, 2);
    let sigs = h.analysis();
    assert!(sigs.amp.is_finite());
    assert!(sigs.fft.iter().all(|v| v.is_finite()));
}
