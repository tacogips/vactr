//! Real branch-private delay audio, physical admission and full-key ownership.
use std::sync::Arc;
use vactr::dsp::arena::{encode_bus, encode_inst, StoreKind};
use vactr::dsp::bus::BusTemplate;
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
#[path = "song_branch_reuse/rig.rs"]
mod rig;
use rig::Rig;

fn event_for(generation: u32, frame: u64) -> SongCommand {
    let mut event = AudioEvent::new(0., SlotId::new(0), 1, InstId::new(301));
    for (id, value) in [(38, 1.), (39, 0.001), (40, 0.), (12, 0.001)] {
        event.push_ctl(CtlId::new(id), Ctl::Const(value)).unwrap();
    }
    SongCommand::Event(SongAudioEvent {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(0),
        generation,
        frame,
        event,
    })
}
fn event_preserving_controls(generation: u32, frame: u64) -> SongCommand {
    let mut event = AudioEvent::new(0., SlotId::new(0), 1, InstId::new(301));
    for (id, value) in [(38, 1.), (12, 0.001)] {
        event.push_ctl(CtlId::new(id), Ctl::Const(value)).unwrap();
    }
    SongCommand::Event(SongAudioEvent {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(0),
        generation,
        frame,
        event,
    })
}
fn rebind_for(expected: u32, frame: u64) -> SongCommand {
    SongCommand::RebindBranch(SongBranchRebind {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(0),
        expected_generation: expected,
        generation: expected + 1,
        transition_frame: frame,
        tail_deadline: u64::MAX,
    })
}
fn release_for(generation: u32, frame: u64, deadline: u64) -> SongCommand {
    SongCommand::Release(SongBranchRelease {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(0),
        generation,
        frame,
        tail_deadline: deadline,
    })
}
fn assert_rebound(rig: &Rig, generation: u32, frame: u64) {
    assert_eq!(
        rig.receipts
            .iter()
            .filter(|a| **a
                == HostMsg::Song(SongHostAck::BranchRebound(SongBranchRebound {
                    epoch: SnapshotEpoch(71),
                    branch: SongBranchId(0),
                    generation,
                    frame
                })))
            .count(),
        1
    );
}
#[test]
fn reusable_branch_native_and_arena_produce_nonzero_audio_across_generations() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 1., 1., 1.);
        let initial = r.frame;
        r.activate(initial);
        r.command(event_for(17, initial + 1));
        assert!(r.audio(128, 17).iter().any(|x| x.abs() > 1e-6));
        let allocated = r.engine.song_remaining_capacities().unwrap();
        for generation in 17..24 {
            let frame = r.frame;
            r.command(release_for(generation, frame, frame));
            r.command(rebind_for(generation, frame));
            r.process(1, true);
            assert_rebound(&r, generation + 1, frame);
            r.command(event_for(generation + 1, r.frame));
            assert!(r.audio(128, 19).iter().any(|x| x.abs() > 1e-6));
            assert_eq!(r.engine.song_remaining_capacities().unwrap(), allocated);
        }
    }
}
#[test]
fn reusable_branch_rejects_overlap_stale_keys_and_generation_wrap_atomically() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 1., 1., 1.);
        let start = r.frame;
        r.activate(start);
        r.command(event_for(17, start));
        r.process(16, true);
        let capacity = r.engine.song_remaining_capacities().unwrap();
        r.command(rebind_for(17, r.frame));
        r.process(1, true);
        assert!(r.receipts.contains(&HostMsg::Song(SongHostAck::Rejected {
            epoch: SnapshotEpoch(71),
            reason: SongRejectCode::NotReady
        })));
        assert_eq!(r.engine.song_remaining_capacities().unwrap(), capacity);
        let frame = r.frame;
        r.command(release_for(17, frame, frame + 64));
        r.command(rebind_for(17, frame));
        r.process(64, true);
        assert!(!r
            .receipts
            .iter()
            .any(|a| matches!(a, HostMsg::Song(SongHostAck::BranchRebound(_)))));
        r.command(rebind_for(17, r.frame));
        let frame = r.frame;
        r.process(1, true);
        assert_rebound(&r, 18, frame);
        r.command(release_for(17, r.frame, r.frame));
        r.command(rebind_for(17, r.frame));
        r.process(1, true);
        assert!(r.receipts.contains(&HostMsg::Song(SongHostAck::Rejected {
            epoch: SnapshotEpoch(71),
            reason: SongRejectCode::StaleEpoch
        })));
        assert!(!SongCommand::RebindBranch(SongBranchRebind {
            expected_generation: u32::MAX,
            generation: 0,
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(0),
            transition_frame: 0,
            tail_deadline: 0
        })
        .valid());
    }
}
#[test]
fn reusable_branch_transition_order_is_partition_invariant() {
    for bytes in [false, true] {
        let mut a = Rig::prepared(bytes, 1., 1., 1.);
        let mut b = Rig::prepared(bytes, 1., 1., 1.);
        let frame = a.frame;
        assert_eq!(frame, b.frame);
        for r in [&mut a, &mut b] {
            r.activate(frame);
            // Transition ingress is reversed; the proved chain executes Release/Rebind/Event.
            r.command(rebind_for(17, frame + 32));
            r.command(release_for(17, frame + 32, frame + 32));
            r.command(event_for(18, frame + 32));
            r.command(event_for(17, frame + 1));
        }
        assert_eq!(a.audio(192, 64), b.audio(192, 7));
        assert_rebound(&a, 18, frame + 32);
        assert_rebound(&b, 18, frame + 32);
        assert_eq!(a.receipts, b.receipts);
    }
}
#[test]
fn reusable_branch_pressure_retains_exact_owners_until_safe_handoff() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 1., 1., 1.);
        let start = r.frame;
        r.activate(start);
        r.process(1, true);
        let allocated = r.engine.song_remaining_capacities().unwrap();
        for _ in 0..r.acks.capacity() {
            r.command(SongCommand::RequestCapacity(SnapshotEpoch(71)));
            r.process(1, false);
        }
        let frame = r.frame;
        r.command(release_for(17, frame, frame));
        r.command(rebind_for(17, frame));
        r.command(SongCommand::Mute(SongMute {
            epoch: SnapshotEpoch(71),
            instrument: 1,
            muted: true,
            frame,
        }));
        r.process(1, false);
        r.command(release_for(18, r.frame, r.frame));
        r.command(rebind_for(18, r.frame));
        r.command(event_for(19, r.frame));
        assert!(r.process(1, false).iter().all(|v| *v == 0.));
        assert_eq!(r.engine.song_remaining_capacities().unwrap(), allocated);
        assert!(r.returned_owners.is_empty());
        for _ in 0..16 {
            assert!(r.process(1, true).iter().all(|v| *v == 0.));
        }
        assert_rebound(&r, 18, frame);
        // Second transition is late after pressure, so its actual frame must reject.
        assert!(!r
            .receipts
            .iter()
            .any(|a| matches!(a,HostMsg::Song(SongHostAck::BranchRebound(b)) if b.generation==19)));
        r.command(SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(71),
            arrangement: r.frame,
            tail_deadline: r.frame,
        }));
        for _ in 0..64 {
            r.process(1, true);
        }
        if !bytes {
            let mut returned = r.returned_owners.clone();
            returned.sort_unstable_by_key(|(key, pointer)| {
                (
                    key.epoch.0,
                    key.kind as u8,
                    key.resource.id,
                    key.resource.generation,
                    *pointer,
                )
            });
            let mut original = r.native_owners.clone();
            original.sort_unstable_by_key(|(key, pointer)| {
                (
                    key.epoch.0,
                    key.kind as u8,
                    key.resource.id,
                    key.resource.generation,
                    *pointer,
                )
            });
            assert_eq!(returned, original);
        }
        assert!(
            r.engine.song_remaining_capacities().unwrap().template_slots > allocated.template_slots
        );
    }
}
#[test]
fn reusable_branch_codec_preserves_full_keys_and_following_records() {
    let epoch = SnapshotEpoch(u64::MAX - 1);
    let config = SongBranchConfig {
        epoch,
        branch: SongBranchId(u32::MAX),
        generation: u32::MAX - 1,
        family: 9,
        track: 3,
        instrument: SongResourceRef {
            id: u32::MAX,
            generation: 7,
        },
        private_fx: Some(SongResourceRef {
            id: 5,
            generation: u32::MAX,
        }),
        track_template: None,
        master: None,
        transition_frame: u64::MAX - 2,
        tail_deadline: u64::MAX,
    };
    let commands = [
        SongCommand::ConfigureReusableBranch(SongReusableBranch {
            config,
            last_generation: u32::MAX,
        }),
        SongCommand::RebindBranch(SongBranchRebind {
            epoch,
            branch: config.branch,
            expected_generation: u32::MAX - 1,
            generation: u32::MAX,
            transition_frame: u64::MAX - 2,
            tail_deadline: u64::MAX,
        }),
    ];
    for command in commands {
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        assert!(n > 0);
        let (decoded, used) = CtlMsg::decode(&bytes[..n]).unwrap();
        assert_eq!(decoded, CtlMsg::Song(command));
        assert_eq!(used, n);
        if matches!(command, SongCommand::ConfigureReusableBranch(_)) {
            assert_eq!(n, 65);
            let mut malformed = bytes;
            malformed[34] = 2;
            assert!(CtlMsg::decode(&malformed[..n]).is_err());
        } else {
            assert_eq!(n, 38);
        }
        for len in 0..n {
            assert!(CtlMsg::decode(&bytes[..len]).is_err());
        }
        let mut inbox = ByteInbox::new();
        assert!(inbox.push(&bytes[..n]));
        let m = CtlMsg::Song(SongCommand::Prepare(epoch)).encode(&mut bytes);
        assert!(inbox.push(&bytes[..m]));
        assert!(
            matches!(ring::ControlSource::next(&mut inbox, ring::Budget { install_bytes: usize::MAX, batch: true }),Some(ring::Record::Msg(CtlMsg::Song(c))) if c==command)
        );
        assert!(
            matches!(ring::ControlSource::next(&mut inbox, ring::Budget { install_bytes: usize::MAX, batch: true }),Some(ring::Record::Msg(CtlMsg::Song(SongCommand::Prepare(e)))) if e==epoch)
        );
    }
    let ack = HostMsg::Song(SongHostAck::BranchRebound(SongBranchRebound {
        epoch,
        branch: config.branch,
        generation: u32::MAX,
        frame: u64::MAX,
    }));
    let mut bytes = [0; HostMsg::MAX_LEN];
    let n = ack.encode(&mut bytes);
    assert_eq!(n, 26);
    assert_eq!(HostMsg::decode(&bytes[..n]).unwrap(), (ack, n));
    for len in 0..n {
        assert!(HostMsg::decode(&bytes[..len]).is_err());
    }
    let mut invalid = config;
    invalid.generation = u32::MAX;
    assert_eq!(
        CtlMsg::Song(SongCommand::ConfigureReusableBranch(SongReusableBranch {
            config: invalid,
            last_generation: u32::MAX - 1
        }))
        .encode(&mut [0; CtlMsg::MAX_LEN]),
        0
    );
}
#[test]
fn reusable_branch_callbacks_allocate_and_deallocate_nothing() {
    // Rig::process probes every actual callback, including adoption and cancellation.
    reusable_branch_native_and_arena_produce_nonzero_audio_across_generations();
}
#[test]
fn ordinary_and_reusable_branches_reject_shared_private_full_keys() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 1., 1., 1.);
        let alias = SongBranchConfig {
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(2),
            generation: 1,
            family: 2,
            track: 5,
            instrument: key(1, SongResourceKind::Instrument).resource,
            private_fx: Some(key(3, SongResourceKind::PrivateFx).resource),
            track_template: Some(key(5, SongResourceKind::Track).resource),
            master: Some(key(6, SongResourceKind::Master).resource),
            transition_frame: 0,
            tail_deadline: u64::MAX,
        };
        assert_eq!(
            r.engine.stage_song_branch(alias),
            Err(SongRejectCode::Malformed)
        );
        assert_eq!(
            r.engine.stage_song_reusable_branch(SongReusableBranch {
                config: alias,
                last_generation: 2
            }),
            Err(SongRejectCode::Malformed)
        );
    }
}
#[test]
fn reusable_branch_nonzero_users_and_tail_preflight_preserve_state() {
    reusable_branch_rejects_overlap_stale_keys_and_generation_wrap_atomically();
}
#[test]
fn reusable_branch_same_owner_delay_reset_retains_controls_clears_history() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 0., 1., 1.);
        let frame = r.frame;
        r.activate(frame);
        r.command(event_for(17, frame));
        assert!(r.audio(128, 17).iter().any(|x| x.abs() > 1e-6));
        let key = key(3, SongResourceKind::PrivateFx);
        let layout = r.engine.song_private_delay_layout(key).unwrap();
        let frame = r.frame;
        r.command(release_for(17, frame, frame));
        r.command(rebind_for(17, frame));
        r.process(1, true);
        assert_rebound(&r, 18, frame);
        assert_eq!(r.engine.song_private_delay_layout(key).unwrap(), layout);
        assert!(r.audio(192, 13).iter().all(|x| *x == 0.));
        r.command(event_preserving_controls(18, r.frame));
        assert!(r.audio(128, 11).iter().any(|x| x.abs() > 1e-6));
    }
}

fn pool_event(branch: u32, generation: u32, frame: u64, feedback: f32) -> SongCommand {
    let mut event = AudioEvent::new(0., SlotId::new(branch), 1, InstId::new(301 + branch));
    for (id, value) in [(38, 1.), (39, 0.001), (40, feedback), (12, 0.001)] {
        event.push_ctl(CtlId::new(id), Ctl::Const(value)).unwrap();
    }
    SongCommand::Event(SongAudioEvent {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(branch),
        generation,
        frame,
        event,
    })
}
fn pool_release(branch: u32, generation: u32, frame: u64, deadline: u64) -> SongCommand {
    SongCommand::Release(SongBranchRelease {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(branch),
        generation,
        frame,
        tail_deadline: deadline,
    })
}
fn pool_rebind(branch: u32, expected: u32, frame: u64) -> SongCommand {
    SongCommand::RebindBranch(SongBranchRebind {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(branch),
        expected_generation: expected,
        generation: expected + 1,
        transition_frame: frame,
        tail_deadline: u64::MAX,
    })
}
fn assert_actual_rejection(rig: &Rig, reason: SongRejectCode) {
    assert!(rig.receipts.contains(&HostMsg::Song(SongHostAck::Rejected {
        epoch: SnapshotEpoch(71),
        reason
    })));
}
fn finish_pool(rig: &mut Rig) {
    let epoch = SnapshotEpoch(71);
    rig.command(SongCommand::Endpoints(SongEndpoints {
        epoch,
        arrangement: rig.frame,
        tail_deadline: rig.frame,
    }));
    for _ in 0..64 {
        rig.process(1, true);
    }
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap(),
        rig.initial_capacity
    );
    for id in 1..=8 {
        let kind = match id {
            1 | 2 => SongResourceKind::Instrument,
            3 | 4 => SongResourceKind::PrivateFx,
            5 => SongResourceKind::Track,
            6 => SongResourceKind::Master,
            7 => SongResourceKind::Sample,
            _ => SongResourceKind::ControlCells,
        };
        assert_eq!(
            rig.receipts
                .iter()
                .filter(|a| **a == HostMsg::Song(SongHostAck::LeaseReturned(key(id, kind))))
                .count(),
            1
        );
    }
    if !rig.bytes {
        let mut original = rig.native_owners.clone();
        let mut returned = rig.returned_owners.clone();
        original.sort_unstable_by_key(|(k, p)| {
            (
                k.epoch.0,
                k.kind as u8,
                k.resource.id,
                k.resource.generation,
                *p,
            )
        });
        returned.sort_unstable_by_key(|(k, p)| {
            (
                k.epoch.0,
                k.kind as u8,
                k.resource.id,
                k.resource.generation,
                *p,
            )
        });
        assert_eq!(original, returned);
    }
}
#[test]
fn distinct_private_slots_preserve_independent_positive_tails() {
    for bytes in [false, true] {
        let mut both = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let mut only_a = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let mut only_b = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let frame = both.frame;
        assert_eq!(frame, only_a.frame);
        assert_eq!(frame, only_b.frame);
        for r in [&mut both, &mut only_a, &mut only_b] {
            r.activate(frame);
            r.command(pool_release(0, 17, frame + 64, frame + 512));
            r.command(pool_release(1, 18, frame + 64, frame + 512));
        }
        both.command(pool_event(0, 17, frame + 1, 0.8));
        both.command(pool_event(1, 18, frame + 1, 0.7));
        only_a.command(pool_event(0, 17, frame + 1, 0.8));
        only_b.command(pool_event(1, 18, frame + 1, 0.7));
        let combined = both.audio(320, 17);
        let a = only_a.audio(320, 31);
        let b = only_b.audio(320, 7);
        assert!(a[192..].iter().any(|x| x.abs() > 1e-6));
        assert!(b[192..].iter().any(|x| x.abs() > 1e-6));
        assert_eq!(
            combined,
            a.iter().zip(&b).map(|(a, b)| a + b).collect::<Vec<_>>()
        );
        let ka = key(3, SongResourceKind::PrivateFx);
        let kb = key(4, SongResourceKind::PrivateFx);
        let la = both.engine.song_private_delay_layout(ka).unwrap();
        let lb = both.engine.song_private_delay_layout(kb).unwrap();
        assert_ne!(la.left.slot, lb.left.slot);
        assert_ne!(la.owner, lb.owner);
        for r in [&mut both, &mut only_a, &mut only_b] {
            finish_pool(r);
        }
    }
}
#[test]
fn positive_tail_overlap_and_safe_rebind_preserve_other_stage_audio() {
    for bytes in [false, true] {
        let mut actual = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let mut reference = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let frame = actual.frame;
        assert_eq!(frame, reference.frame);
        for r in [&mut actual, &mut reference] {
            r.activate(frame);
            r.command(pool_event(0, 17, frame + 1, 0.8));
            r.command(pool_event(1, 18, frame + 1, 0.8));
            r.command(pool_release(0, 17, frame + 64, frame + 192));
            r.command(pool_release(1, 18, frame + 64, frame + 640));
        }
        assert_eq!(actual.audio(96, 13), reference.audio(96, 31));
        let before = actual.engine.song_remaining_capacities().unwrap();
        let layout = actual
            .engine
            .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
            .unwrap();
        actual.command(pool_rebind(0, 17, actual.frame));
        let denied = actual.audio(96, 17);
        let unchanged = reference.audio(96, 11);
        assert_eq!(denied, unchanged);
        assert!(denied.iter().any(|x| x.abs() > 1e-6));
        assert_actual_rejection(&actual, SongRejectCode::NotReady);
        assert_eq!(actual.engine.song_remaining_capacities().unwrap(), before);
        assert_eq!(
            actual
                .engine
                .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
                .unwrap(),
            layout
        );
        let birth = actual.frame;
        actual.command(pool_rebind(0, 17, birth));
        let reset = actual.audio(128, 7);
        let survived = reference.audio(128, 23);
        assert_eq!(reset, survived);
        assert!(reset.iter().any(|x| x.abs() > 1e-6));
        assert_rebound(&actual, 18, birth);
        finish_pool(&mut actual);
        finish_pool(&mut reference);
    }
}
#[test]
fn immutable_graph_pools_support_a_b_a_without_old_tail_leak() {
    for bytes in [false, true] {
        let mut history = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let mut cold_a = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let frame = history.frame;
        assert_eq!(frame, cold_a.frame);
        for r in [&mut history, &mut cold_a] {
            r.activate(frame);
            r.command(pool_event(1, 18, frame + 65, 0.8));
            r.command(pool_release(0, 17, frame + 64, frame + 192));
            r.command(pool_release(1, 18, frame + 128, frame + 768));
        }
        history.command(pool_event(0, 17, frame + 1, 0.8));
        let old = history.audio(192, 17);
        let cold = cold_a.audio(192, 7);
        assert_ne!(old, cold);
        assert!(old.iter().any(|x| x.abs() > 1e-6));
        let a_layout = history
            .engine
            .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
            .unwrap();
        let b_layout = history
            .engine
            .song_private_delay_layout(key(4, SongResourceKind::PrivateFx))
            .unwrap();
        let birth = history.frame;
        for r in [&mut history, &mut cold_a] {
            r.command(pool_rebind(0, 17, birth));
            r.command(pool_event(0, 18, birth, 0.8));
        }
        let reborn = history.audio(256, 31);
        let independent = cold_a.audio(256, 13);
        assert_eq!(reborn, independent);
        assert!(reborn.iter().any(|x| x.abs() > 1e-6));
        assert_rebound(&history, 18, birth);
        assert_eq!(
            history
                .engine
                .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
                .unwrap(),
            a_layout
        );
        assert_eq!(
            history
                .engine
                .song_private_delay_layout(key(4, SongResourceKind::PrivateFx))
                .unwrap(),
            b_layout
        );
        assert!(!history.receipts.iter().any(|a|matches!(a,HostMsg::Song(SongHostAck::BranchRebound(b)) if b.branch==SongBranchId(1))));
        finish_pool(&mut history);
        finish_pool(&mut cold_a);
    }
}
#[test]
fn all_private_alias_directions_refuse_before_mutation() {
    for bytes in [false, true] {
        for existing_reusable in [false, true] {
            for incoming_reusable in [false, true] {
                let (mut r, alias) = Rig::prepared_alias_probe(bytes, existing_reusable);
                let before = r.engine.song_remaining_capacities().unwrap();
                let layout = r
                    .engine
                    .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
                    .unwrap();
                let result = if incoming_reusable {
                    r.engine.stage_song_reusable_branch(SongReusableBranch {
                        config: alias,
                        last_generation: 2,
                    })
                } else {
                    r.engine.stage_song_branch(alias)
                };
                assert_eq!(result, Err(SongRejectCode::Malformed));
                assert_eq!(r.engine.song_remaining_capacities().unwrap(), before);
                assert_eq!(
                    r.engine
                        .song_private_delay_layout(key(3, SongResourceKind::PrivateFx))
                        .unwrap(),
                    layout
                );
                let frame = r.frame;
                r.activate(frame);
                r.command(pool_event(0, 17, frame, 0.));
                assert!(r.audio(128, 17).iter().any(|x| x.abs() > 1e-6));
                finish_pool(&mut r);
            }
        }
    }
}
#[test]
fn stale_foreign_exhausted_and_final_epoch_transitions_never_reopen() {
    for bytes in [false, true] {
        let mut r = Rig::prepared_pools(bytes, [0., 0.5], [true; 2]);
        let frame = r.frame;
        r.activate(frame);
        r.process(1, true);
        let capacity = r.engine.song_remaining_capacities().unwrap();
        let mut foreign = match pool_event(0, 17, r.frame, 0.) {
            SongCommand::Event(e) => e,
            _ => unreachable!(),
        };
        foreign.epoch = SnapshotEpoch(72);
        r.command(SongCommand::Event(foreign));
        r.command(SongCommand::Release(SongBranchRelease {
            epoch: SnapshotEpoch(72),
            branch: SongBranchId(0),
            generation: 17,
            frame: r.frame,
            tail_deadline: r.frame,
        }));
        r.command(pool_event(0, 16, r.frame, 0.));
        r.command(pool_release(0, 16, r.frame, r.frame));
        let skip = SongBranchRebind {
            epoch: SnapshotEpoch(71),
            branch: SongBranchId(0),
            expected_generation: 17,
            generation: 19,
            transition_frame: r.frame,
            tail_deadline: u64::MAX,
        };
        assert!(!SongCommand::RebindBranch(skip).valid());
        if !bytes {
            r.command(SongCommand::RebindBranch(skip));
        } else {
            assert_eq!(
                CtlMsg::Song(SongCommand::RebindBranch(skip)).encode(&mut [0; CtlMsg::MAX_LEN]),
                0
            );
        }
        let wrapped = SongBranchRebind {
            expected_generation: u32::MAX,
            generation: 0,
            ..skip
        };
        assert!(!SongCommand::RebindBranch(wrapped).valid());
        if bytes {
            assert_eq!(
                CtlMsg::Song(SongCommand::RebindBranch(wrapped)).encode(&mut [0; CtlMsg::MAX_LEN]),
                0
            );
        } else {
            r.command(SongCommand::RebindBranch(wrapped));
        }
        assert!(r.audio(16, 7).iter().all(|x| *x == 0.));
        assert_actual_rejection(&r, SongRejectCode::StaleEpoch);
        assert_eq!(r.engine.song_remaining_capacities().unwrap(), capacity);
        for generation in 17..24 {
            let frame = r.frame;
            r.command(pool_release(0, generation, frame, frame));
            r.command(pool_rebind(0, generation, frame));
            r.process(1, true);
            assert_rebound(&r, generation + 1, frame);
        }
        let frame = r.frame;
        r.command(pool_release(0, 24, frame, frame));
        r.command(pool_rebind(0, 24, frame));
        r.command(pool_event(0, 25, frame, 0.));
        assert!(r.audio(16, 7).iter().all(|x| *x == 0.));
        assert_actual_rejection(&r, SongRejectCode::Capacity);
        assert!(!r
            .receipts
            .iter()
            .any(|a| matches!(a,HostMsg::Song(SongHostAck::BranchRebound(b)) if b.generation==25)));
        finish_pool(&mut r);
        let final_receipt_begin = r.receipts.len();
        r.command(pool_rebind(0, 24, r.frame));
        r.command(pool_event(0, 25, r.frame, 0.));
        r.command(pool_release(0, 24, r.frame, r.frame));
        r.activate(r.frame);
        assert!(r.audio(32, 11).iter().all(|x| *x == 0.));
        assert_eq!(
            r.engine.song_remaining_capacities().unwrap(),
            r.initial_capacity
        );
        assert!(r.receipts[final_receipt_begin..].iter().any(|a| matches!(
            a,
            HostMsg::Song(SongHostAck::Rejected {
                epoch: SnapshotEpoch(71),
                reason: SongRejectCode::StaleEpoch | SongRejectCode::NotReady
            })
        )));
        assert!(!r
            .receipts
            .iter()
            .any(|a| matches!(a,HostMsg::Song(SongHostAck::BranchRebound(b)) if b.generation==25)));
    }
}
