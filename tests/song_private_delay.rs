//! Real branch-private delay audio, physical admission and full-key ownership.
use std::sync::Arc;
use vactr::dsp::arena::{encode_bus, encode_inst, StoreKind};
use vactr::dsp::bus::{BusGraph, BusTemplate, SlotState};
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::{AtomicCells, CellId};
use vactr::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
use vactr::dsp::graph::{
    BankRef, BusDef, BusId, EffectKind, EffectSpec, InstDef, InstId, UGenSpec,
};
use vactr::dsp::ring::{
    self, ByteInbox, EngineIo, EventRing, Garbage, NativeInstall, NativeRecord, NativeSongInstall,
    SpscRing,
};
use vactr::dsp::ugen::Template;
use vactr::host::caps::SampleData;
use vactr::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg};
use vactr::sched::slots::{CtlId, SlotId};
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;
#[path = "song_private_delay/rig.rs"]
mod rig;
use rig::Rig;
struct Probe;
thread_local! { static CALLBACK_MEMORY: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::realloc(&std::alloc::System, ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
fn key(id: u32, kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(71),
        resource: SongResourceRef {
            id,
            generation: if id == 4 { 6 } else { 5 },
        },
        kind,
    }
}
fn effect(kind: EffectKind, name: &str, value: f32) -> EffectSpec {
    EffectSpec {
        kind,
        params: vec![(
            vactr::dsp::effects::param_ctl(kind, name).unwrap(),
            Ctl::Const(value),
        )]
        .into_boxed_slice(),
    }
}
fn linear_gain_chain(gain: f32) -> Box<[EffectSpec]> {
    assert!(gain.is_finite() && gain >= 0.);
    vec![if gain == 0. {
        effect(EffectKind::Mute, "on", 1.)
    } else {
        effect(
            EffectKind::Gain,
            "gain",
            vactr::dsp::effects::prim::gain_to_db(gain),
        )
    }]
    .into_boxed_slice()
}
fn private_need(caps: &CapabilitySet, sr: f32, template: &BusTemplate) -> usize {
    vactr::dsp::effects::mem_len(EffectKind::Room, sr, caps)
        + template.kinds[..template.n]
            .iter()
            .map(|kind| vactr::dsp::effects::mem_len(*kind, sr, caps))
            .sum::<usize>()
        + 2 * (4 * sr as usize + 4)
}
#[test]
fn native_and_arena_nonzero_private_delay_is_audible_from_measured_regions() {
    for bytes in [false, true] {
        let mut delayed = Rig::prepared(bytes, 0., 1., 1.);
        let mut silent = Rig::prepared(bytes, 0., 1., 1.);
        let start = delayed.frame;
        assert_eq!(start, silent.frame);
        for r in [&mut delayed, &mut silent] {
            r.activate(start);
        }
        delayed.event(0, start + 3, 1., Ctl::Const(0.001), Ctl::Const(0.));
        silent.event(0, start + 3, 0., Ctl::Const(0.001), Ctl::Const(0.));
        let out = delayed.audio(256, 17);
        let no_send = silent.audio(256, 17);
        assert!(no_send.iter().all(|v| *v == 0.));
        assert!(
            out[..(3 + 48) * 2].iter().all(|v| *v == 0.),
            "no dry/early delay through gain0"
        );
        assert!(
            out[(3 + 48) * 2..].iter().any(|v| v.abs() > 0.001),
            "actual delayed PCM through gain0"
        );
        let layout = delayed
            .engine
            .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
            .unwrap();
        let index = delayed
            .engine
            .buses()
            .slots
            .iter()
            .position(|s| s.resource == 3 && s.gen == 5 && s.state == SlotState::Live)
            .unwrap();
        assert_eq!(layout.left.slot as usize, index);
        assert!(index > 0);
        assert_eq!(
            layout.room.frames,
            vactr::dsp::effects::mem_len(EffectKind::Room, 48000., &delayed.caps) as u64
        );
        assert_eq!(layout.left.frames, 192004);
        assert_eq!(layout.right.frames, 192004);
        assert_eq!(layout.left.offset, layout.room.frames + layout.chain_frames);
        assert_eq!(layout.right.offset, layout.left.offset + layout.left.frames);
        assert!(layout.right.offset + layout.right.frames <= layout.storage_frames);
        assert_eq!(layout.send_frames_per_channel, 64);
        assert!(delayed
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Applied(SongActivation {
                epoch: SnapshotEpoch(71),
                frame: start
            }))));
    }
}
#[test]
fn private_delay_full_extent_one_short_and_fragmented_fit_are_honest() {
    let caps = CapabilitySet::browser();
    let cells = AtomicCells::new(8);
    let mut template = BusTemplate::new();
    template.bus = BusId::new(91);
    template.push(EffectKind::Delay).unwrap();
    let need = private_need(&caps, 48000., &template);
    for mem in [need - 1, need] {
        let mut buses = BusGraph::new(3, 64, mem, &cells, 48000., &caps);
        let before = buses.song_regions().to_vec();
        let boxed = Box::new(template);
        let pointer = std::ptr::from_ref(boxed.as_ref());
        let result = buses.stage_song_bus(
            key(3, SongResourceKind::PrivateFx),
            boxed,
            &cells,
            48000.,
            &caps,
        );
        if mem < need {
            assert!(
                buses.song_regions().iter().map(|r| r.frames).sum::<u64>() >= need as u64,
                "aggregate fits but no individual region"
            );
            let returned = result.unwrap_err();
            assert_eq!(std::ptr::from_ref(returned.as_ref()), pointer);
            assert_eq!(buses.song_regions(), before);
            assert_eq!(
                buses
                    .slots
                    .iter()
                    .filter(|s| s.state == SlotState::Staged)
                    .count(),
                0
            );
        } else {
            result.unwrap();
            assert_eq!(
                buses
                    .slots
                    .iter()
                    .filter(|s| s.state == SlotState::Staged)
                    .count(),
                1
            );
        }
    }
    let mut r = Rig::new(false, 4, 48000.5, 12.);
    let owner = key(3, SongResourceKind::PrivateFx);
    r.engine
        .begin_song_preparation(SongStagePreparation {
            preparation: SongPreparation {
                epoch: owner.epoch,
                branches: 1,
                resources: 1,
                required: SongHostCapacities {
                    sample_rate: 48000,
                    ..Default::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        })
        .unwrap();
    r.engine
        .reserve_song_resource(SongResourceReservation {
            epoch: owner.epoch,
            resource: owner.resource,
            kind: owner.kind,
        })
        .unwrap();
    let before = r.engine.song_remaining_capacities().unwrap();
    let boxed = Box::new(BusTemplate::new());
    let ptr = std::ptr::from_ref(boxed.as_ref());
    let returned = r
        .engine
        .stage_song_native(NativeSongInstall {
            lease: owner,
            payload: NativeInstall::Bus {
                resource: 3,
                gen: 5,
                master: false,
                template: boxed,
            },
        })
        .unwrap_err();
    match returned.payload {
        NativeInstall::Bus { template, .. } => {
            assert_eq!(std::ptr::from_ref(template.as_ref()), ptr)
        }
        _ => panic!("lost original bus"),
    }
    assert_eq!(r.engine.song_remaining_capacities().unwrap(), before);
}
#[test]
fn exact_frame_delay_cells_and_partitioned_audio_match() {
    for bytes in [false, true] {
        let mut reference = None;
        for partition in [1, 17, 64] {
            let mut r = Rig::prepared(bytes, 0., 1., 1.);
            let start = r.frame;
            r.activate(start);
            r.event(
                0,
                start + 7,
                1.,
                Ctl::Cell(CellId::new(42)),
                Ctl::Cell(CellId::new(43)),
            );
            let audio = r.audio(512, partition);
            assert!(audio.iter().any(|v| v.abs() > 0.001));
            if let Some(ref expected) = reference {
                assert_eq!(&audio, expected);
            } else {
                reference = Some(audio);
            }
        }
    }
}
#[test]
fn delayed_return_runs_through_track_and_master_after_private_fx() {
    for bytes in [false, true] {
        let mut unity = Rig::prepared(bytes, 0., 1., 1.);
        let mut downstream = Rig::prepared(bytes, 0., 0.5, 0.25);
        let start = unity.frame;
        assert_eq!(start, downstream.frame);
        for r in [&mut unity, &mut downstream] {
            r.activate(start);
            r.event(0, start, 1., Ctl::Const(0.001), Ctl::Const(0.));
        }
        let a = unity.audio(256, 17);
        let b = downstream.audio(256, 17);
        assert!(a.iter().any(|v| v.abs() > 0.001));
        assert!(a
            .iter()
            .zip(&b)
            .all(|(a, b)| (*a * 0.125 - *b).abs() < 0.000001));
    }
}
#[test]
fn hard_mute_reset_and_partial_return_never_touch_foreign_full_keys() {
    for bytes in [false, true] {
        let mut a = Rig::prepared(bytes, 0., 1., 1.);
        let mut b = Rig::prepared(bytes, 0., 1., 1.);
        let start = a.frame;
        for r in [&mut a, &mut b] {
            r.activate(start);
            r.event(1, start, 1., Ctl::Const(0.001), Ctl::Const(0.8));
        }
        a.event(0, start, 1., Ctl::Const(0.001), Ctl::Const(0.8));
        let _ = a.audio(128, 17);
        let _ = b.audio(128, 17);
        let frame = a.frame;
        a.command(SongCommand::Mute(SongMute {
            epoch: SnapshotEpoch(71),
            instrument: 0,
            muted: true,
            frame,
        }));
        let x = a.audio(256, 17);
        let y = b.audio(256, 17);
        assert_eq!(
            x, y,
            "private hard reset cannot alter sibling delay history"
        );
        assert!(y.iter().any(|v| v.abs() > 0.00001));
        a.command(SongCommand::Mute(SongMute {
            epoch: SnapshotEpoch(71),
            instrument: 0,
            muted: false,
            frame: a.frame,
        }));
        assert_eq!(
            a.audio(128, 17),
            b.audio(128, 17),
            "unmute never resurrects old delay"
        );
        let mut foreign = key(3, SongResourceKind::PrivateFx);
        foreign.resource.generation += 1;
        assert_eq!(
            a.engine.song_private_delay_layout(foreign),
            Err(SongRejectCode::StaleEpoch)
        );
        foreign = key(3, SongResourceKind::PrivateFx);
        foreign.epoch = SnapshotEpoch(999);
        assert_eq!(
            a.engine.song_private_delay_layout(foreign),
            Err(SongRejectCode::StaleEpoch)
        );
    }
}
#[test]
fn overlapping_generations_keep_private_tail_and_legacy_orbit_isolation() {
    for bytes in [false, true] {
        let mut a = Rig::prepared(bytes, 0., 1., 1.);
        let mut dry = Rig::prepared(bytes, 0., 1., 1.);
        let start = a.frame;
        for r in [&mut a, &mut dry] {
            r.activate(start);
        }
        a.event(0, start, 1., Ctl::Const(0.001), Ctl::Const(0.75));
        dry.event(0, start, 0., Ctl::Const(0.001), Ctl::Const(0.75));
        let _ = a.audio(128, 17);
        let _ = dry.audio(128, 17);
        a.command(SongCommand::Release(SongBranchRelease {
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(0),
            generation: 17,
            frame: a.frame,
            tail_deadline: a.frame + 1024,
        }));
        a.event(1, a.frame + 3, 1., Ctl::Const(0.002), Ctl::Const(0.5));
        dry.event(1, dry.frame + 3, 1., Ctl::Const(0.002), Ctl::Const(0.5));
        let x = a.audio(512, 17);
        let y = dry.audio(512, 17);
        assert!(
            x.iter().zip(&y).any(|(a, b)| (*a - *b).abs() > 0.00001),
            "old private tail remains audible with independent new generation"
        );
        assert_eq!(
            a.engine.buses().frames(a.engine.buses().master(), 1),
            dry.engine.buses().frames(dry.engine.buses().master(), 1),
            "no legacy orbit return"
        );
        let l = a
            .engine
            .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
            .unwrap();
        let r = a
            .engine
            .song_private_delay_layout(key(4, SongResourceKind::PrivateFx))
            .unwrap();
        assert_ne!(l.left.slot, r.left.slot);
    }
}
#[test]
fn real_delay_render_retirement_and_ack_pressure_allocate_and_deallocate_zero() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 0., 1., 1.);
        let start = r.frame;
        r.activate(start);
        r.event(0, start, 1., Ctl::Const(0.001), Ctl::Const(0.75));
        assert!(r.audio(128, 17).iter().any(|v| v.abs() > 0.001));
        for i in 0..r.acks.capacity() {
            r.command(SongCommand::RequestCapacity(SnapshotEpoch(900 + i as u64)));
            r.process(1, false);
        }
        let marker = Arc::new(SampleData {
            rate: 48000,
            channels: 1,
            frames: vec![0.; 1].into_boxed_slice(),
        });
        let pointer = Arc::as_ptr(&marker);
        r.garbage.push(Garbage::Sample(marker)).ok().unwrap();
        r.command(SongCommand::Release(SongBranchRelease {
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(0),
            generation: 17,
            frame: r.frame,
            tail_deadline: r.frame + 64,
        }));
        r.command(SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(71),
            arrangement: r.frame,
            tail_deadline: r.frame + 64,
        }));
        for _ in 0..8 {
            r.process(17, false);
        }
        assert!(
            r.engine
                .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
                .is_ok(),
            "full owners retained while garbage is blocked"
        );
        match r.returned.pop().unwrap() {
            Garbage::Sample(data) => assert_eq!(Arc::as_ptr(&data), pointer),
            _ => panic!("marker lost"),
        }
        for _ in 0..64 {
            r.process(1, true);
        }
        for owner in [
            key(3, SongResourceKind::PrivateFx),
            key(4, SongResourceKind::PrivateFx),
        ] {
            assert_eq!(
                r.receipts
                    .iter()
                    .filter(|m| **m == HostMsg::Song(SongHostAck::LeaseReturned(owner)))
                    .count(),
                1
            );
            assert_eq!(
                r.engine.song_private_delay_layout(owner),
                Err(SongRejectCode::StaleEpoch)
            );
        }
        assert_eq!(r.engine.song_remaining_capacities().unwrap(), r.baseline);
        if !bytes {
            assert_eq!(r.native_owners.len(), 7);
            assert_eq!(r.returned_owners.len(), r.native_owners.len());
            for original in &r.native_owners {
                assert_eq!(
                    r.returned_owners
                        .iter()
                        .filter(|returned| *returned == original)
                        .count(),
                    1
                );
                assert_eq!(
                    r.receipts
                        .iter()
                        .filter(|m| **m == HostMsg::Song(SongHostAck::LeaseReturned(original.0)))
                        .count(),
                    1
                );
            }
        } else {
            assert!(r.returned_owners.is_empty());
        }
    }
}

#[test]
fn malformed_delay_controls_do_not_start_or_modify_a_voice() {
    for bytes in [false, true] {
        for (send, time, feedback) in [
            (f32::NAN, 0.001, 0.5),
            (1., f32::INFINITY, 0.5),
            (1., 0.001, f32::NEG_INFINITY),
        ] {
            let mut subject = Rig::prepared(bytes, 0., 1., 1.);
            let mut reference = Rig::prepared(bytes, 0., 1., 1.);
            let start = subject.frame;
            for r in [&mut subject, &mut reference] {
                r.activate(start);
                r.event(1, start, 1., Ctl::Const(0.001), Ctl::Const(0.75));
            }
            let refused_before = subject.inbox.refused();
            subject.malformed_event(0, start + 3, send, time, feedback);
            let a = subject.audio(256, 17);
            let b = reference.audio(256, 17);
            assert_eq!(
                a, b,
                "rejected delay cannot mutate a voice or sibling history"
            );
            assert!(b.iter().any(|v| v.abs() > 0.001));
            if bytes {
                assert_eq!(
                    subject.inbox.refused(),
                    refused_before + 1,
                    "actual ByteInbox rejects the tampered nonfinite record"
                );
            } else {
                assert!(subject
                    .receipts
                    .contains(&HostMsg::Song(SongHostAck::Rejected {
                        epoch: SnapshotEpoch(71),
                        reason: SongRejectCode::Malformed,
                    })));
            }
        }
    }
}

#[test]
fn arena_private_delay_rejects_short_actual_regions_without_adoption() {
    let mut r = Rig::new(true, 4, 48000., 0.25);
    let owner = key(3, SongResourceKind::PrivateFx);
    let def = BusDef {
        id: BusId::new(3),
        // Actual configured granular memory makes an empty private chain fit.
        // This genuine effect extent exceeds every available individual region.
        chain: vec![EffectSpec {
            kind: EffectKind::Delay,
            params: Box::new([]),
        }]
        .into_boxed_slice(),
    };
    let template = BusTemplate::from_def(&def).unwrap();
    let need = private_need(&r.caps, 48000., &template) as u64;
    assert!(r
        .engine
        .song_bus_regions()
        .iter()
        .all(|region| region.frames < need));
    assert!(
        r.engine
            .song_bus_regions()
            .iter()
            .map(|region| region.frames)
            .sum::<u64>()
            >= need
    );
    r.engine
        .begin_song_preparation(SongStagePreparation {
            preparation: SongPreparation {
                epoch: owner.epoch,
                branches: 1,
                resources: 1,
                required: SongHostCapacities {
                    sample_rate: 48000,
                    ..Default::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        })
        .unwrap();
    r.engine
        .reserve_song_resource(SongResourceReservation {
            epoch: owner.epoch,
            resource: owner.resource,
            kind: owner.kind,
        })
        .unwrap();
    let before = r.engine.song_remaining_capacities().unwrap();
    let regions = r.engine.song_bus_regions().to_vec();
    let mut graph = Vec::new();
    encode_bus(&def, false, &mut graph).unwrap();
    let mut bytes = vec![0; graph.len() + 64];
    let n = ring::encode_song_graph_record(owner, &graph, &mut bytes);
    assert!(n > 0 && r.inbox.push(&bytes[..n]));
    assert!(r.process(1, true).iter().all(|v| *v == 0.));
    assert!(r.receipts.contains(&HostMsg::Song(SongHostAck::Rejected {
        epoch: owner.epoch,
        reason: SongRejectCode::Capacity
    })));
    assert!(!r
        .receipts
        .contains(&HostMsg::Song(SongHostAck::ResourceReady {
            epoch: owner.epoch,
            resource: owner.resource
        })));
    assert_eq!(r.engine.song_remaining_capacities().unwrap(), before);
    assert_eq!(r.engine.song_bus_regions(), regions);
}
#[test]
fn same_frame_delay_controls_obey_fifo_and_partition_invariance() {
    for bytes in [false, true] {
        let mut reference = None;
        for partition in [1, 17, 64] {
            let mut subject = Rig::prepared(bytes, 0., 1., 1.);
            let start = subject.frame;
            subject.activate(start);
            subject.event(0, start + 3, 1., Ctl::Const(0.001), Ctl::Const(0.));
            subject.event(0, start + 3, 1., Ctl::Const(0.002), Ctl::Const(0.));
            let output = subject.audio(256, partition);
            assert!(output[..(3 + 96) * 2].iter().all(|v| *v == 0.));
            assert!(output[(3 + 96) * 2..].iter().any(|v| v.abs() > 0.001));
            if let Some(ref expected) = reference {
                assert_eq!(&output, expected);
            } else {
                reference = Some(output);
            }
        }
    }
}
