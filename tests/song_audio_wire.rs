//! Song POD transport preserves exact identity without changing legacy records.
use vactr::dsp::graph::InstId;
use vactr::host::wire::{AudioEvent, CtlMsg, HostMsg, WireError};
use vactr::sched::slots::SlotId;
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;
fn commands() -> Vec<SongCommand> {
    let epoch = SnapshotEpoch(u64::MAX);
    vec![
        SongCommand::Prepare(epoch),
        SongCommand::Activate(SongActivation {
            epoch,
            frame: u64::MAX,
        }),
        SongCommand::Mute(SongMute {
            epoch,
            instrument: u32::MAX,
            muted: true,
            frame: u64::MAX,
        }),
        SongCommand::Endpoints(SongEndpoints {
            epoch,
            arrangement: u64::MAX - 1,
            tail_deadline: u64::MAX,
        }),
        SongCommand::Release(SongBranchRelease {
            epoch,
            branch: SongBranchId(u32::MAX),
            generation: u32::MAX,
            frame: u64::MAX - 1,
            tail_deadline: u64::MAX,
        }),
        SongCommand::Event(SongAudioEvent {
            epoch,
            branch: SongBranchId(u32::MAX),
            generation: u32::MAX,
            frame: u64::MAX,
            event: AudioEvent::new(0., SlotId::new(3), 4, InstId::new(5)),
        }),
    ]
}
#[test]
fn commands_preserve_full_width_fields_and_reject_every_truncation() {
    for command in commands() {
        let message = CtlMsg::Song(command);
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let len = message.encode(&mut bytes);
        assert!(len > 0);
        assert_eq!(bytes[0], 0x1b);
        assert_eq!(CtlMsg::decode(&bytes[..len]).unwrap(), (message, len));
        for end in 0..len {
            assert!(CtlMsg::decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        assert_eq!(message.encode(&mut bytes[..len - 1]), 0);
    }
}
#[test]
fn acknowledgments_preserve_epoch_frame_and_strict_boolean_reason() {
    let epoch = SnapshotEpoch(u64::MAX);
    for ack in [
        SongHostAck::Ready(epoch),
        SongHostAck::Applied(SongActivation {
            epoch,
            frame: u64::MAX,
        }),
        SongHostAck::Muted(SongMute {
            epoch,
            instrument: u32::MAX,
            muted: true,
            frame: u64::MAX,
        }),
        SongHostAck::Rejected {
            epoch,
            reason: SongRejectCode::NotReady,
        },
    ] {
        let message = HostMsg::Song(ack);
        let mut bytes = [0; HostMsg::MAX_LEN];
        let len = message.encode(&mut bytes);
        assert!(len > 0);
        assert_eq!(bytes[0], 0x49);
        assert_eq!(HostMsg::decode(&bytes[..len]).unwrap(), (message, len));
        for end in 0..len {
            assert!(HostMsg::decode(&bytes[..end]).is_err());
        }
    }
    let mut bytes = [0; HostMsg::MAX_LEN];
    let n = HostMsg::Song(SongHostAck::Rejected {
        epoch,
        reason: SongRejectCode::NotReady,
    })
    .encode(&mut bytes);
    bytes[10] = 255;
    assert_eq!(HostMsg::decode(&bytes[..n]), Err(WireError::BadValue));
}
#[test]
fn malformed_mono_boolean_deadline_and_reserved_tag_are_rejected() {
    let mut bytes = [0; CtlMsg::MAX_LEN];
    let n = CtlMsg::Song(commands().pop().unwrap()).encode(&mut bytes);
    bytes[26] = 0;
    assert_eq!(CtlMsg::decode(&bytes[..n]), Err(WireError::BadValue));
    let n = CtlMsg::Song(SongCommand::Mute(SongMute {
        epoch: SnapshotEpoch(1),
        instrument: 0,
        muted: true,
        frame: 0,
    }))
    .encode(&mut bytes);
    bytes[14] = 2;
    assert_eq!(CtlMsg::decode(&bytes[..n]), Err(WireError::BadValue));
    assert_eq!(
        CtlMsg::Song(SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(1),
            arrangement: 2,
            tail_deadline: 1
        }))
        .encode(&mut bytes),
        0
    );
    assert_eq!(CtlMsg::decode(&[0x1a]), Err(WireError::BadTag(0x1a)));
}
#[test]
fn receipts_retain_stale_identity_and_return_exact_overflow_without_eviction() {
    let mut receipts = SongReceipts::default();
    let stale = SongHostAck::ResourceRetired {
        epoch: SnapshotEpoch(1),
        resource: SongResourceRef {
            id: 7,
            generation: 9,
        },
    };
    assert!(!receipts.record(stale).unwrap());
    assert!(receipts.expect(SnapshotEpoch(2)).is_err());
    assert_eq!(receipts.pop(), Some(stale));
    receipts.expect(SnapshotEpoch(2)).unwrap();
    assert!(!receipts.record(stale).unwrap());
    assert_eq!(receipts.pop(), Some(stale));
    for frame in 0..64 {
        assert!(receipts
            .record(SongHostAck::Applied(SongActivation {
                epoch: SnapshotEpoch(2),
                frame
            }))
            .unwrap());
    }
    let overflow = SongHostAck::Applied(SongActivation {
        epoch: SnapshotEpoch(2),
        frame: 64,
    });
    assert_eq!(receipts.record(overflow), Err(overflow));
    assert_eq!(receipts.iter().count(), 64);
    assert_eq!(
        receipts.pop(),
        Some(SongHostAck::Applied(SongActivation {
            epoch: SnapshotEpoch(2),
            frame: 0
        }))
    );
    assert!(receipts.record(overflow).unwrap());
    for frame in 1..=64 {
        assert_eq!(
            receipts.pop(),
            Some(SongHostAck::Applied(SongActivation {
                epoch: SnapshotEpoch(2),
                frame
            }))
        );
    }
    receipts.expect(SnapshotEpoch(3)).unwrap();
}

fn staged() -> Vec<SongCommand> {
    let epoch = SnapshotEpoch(42);
    let resource = SongResourceRef {
        id: u32::MAX,
        generation: u32::MAX,
    };
    vec![
        SongCommand::BeginPreparation(SongPreparation {
            epoch,
            branches: u32::MAX,
            resources: u32::MAX,
            required: SongHostCapacities {
                sample_rate: 192000,
                cell_slots: u32::MAX,
                voice_slots: u32::MAX,
                template_slots: u32::MAX,
                bus_slots: u32::MAX,
                sample_resources: u32::MAX,
                pcm_bytes: u64::MAX,
                voice_frames: u64::MAX,
                bus_frames: u64::MAX,
                ack_slots: u32::MAX,
            },
        }),
        SongCommand::ReserveResource(SongResourceReservation {
            epoch,
            resource,
            kind: SongResourceKind::Master,
        }),
        SongCommand::ConfigureBranch(SongBranchConfig {
            epoch,
            branch: SongBranchId(3),
            generation: 7,
            family: 8,
            track: 9,
            instrument: resource,
            private_fx: Some(resource),
            track_template: Some(resource),
            master: Some(resource),
            transition_frame: u64::MAX - 1,
            tail_deadline: u64::MAX,
        }),
        SongCommand::SealPreparation(epoch),
    ]
}
#[test]
fn staged_leases_round_trip_without_candidate_to_host_id_assumptions() {
    for command in staged() {
        let message = CtlMsg::Song(command);
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = message.encode(&mut bytes);
        assert!(n > 0);
        assert_eq!(CtlMsg::decode(&bytes[..n]).unwrap(), (message, n));
        for end in 0..n {
            assert!(CtlMsg::decode(&bytes[..end]).is_err());
        }
    }
    for ack in [
        SongHostAck::ResourceReady {
            epoch: SnapshotEpoch(u64::MAX),
            resource: SongResourceRef {
                id: u32::MAX,
                generation: u32::MAX,
            },
        },
        SongHostAck::ResourceRetired {
            epoch: SnapshotEpoch(u64::MAX),
            resource: SongResourceRef {
                id: u32::MAX,
                generation: u32::MAX,
            },
        },
    ] {
        let mut bytes = [0; HostMsg::MAX_LEN];
        let msg = HostMsg::Song(ack);
        let n = msg.encode(&mut bytes);
        assert_eq!(HostMsg::decode(&bytes[..n]).unwrap(), (msg, n));
    }
}
#[test]
fn native_and_byte_carriers_preserve_the_same_song_pod() {
    use vactr::dsp::ring::{Budget, ByteInbox, ControlSource, NativeRecord, Record, SpscRing};
    let (mut tx, mut native) = SpscRing::split(16);
    let mut browser = ByteInbox::new();
    for command in commands().into_iter().chain(staged()) {
        assert!(tx.push(NativeRecord::Msg(CtlMsg::Song(command))).is_ok());
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        assert!(browser.push(&bytes[..n]));
        let budget = Budget {
            install_bytes: 0,
            batch: true,
        };
        match (native.next(budget), browser.next(budget)) {
            (Some(Record::Msg(a)), Some(Record::Msg(b))) => assert_eq!(a, b),
            _ => panic!("typed carriers"),
        }
    }
}
fn small_engine() -> vactr::dsp::engine::Engine {
    use vactr::dsp::{arena::StoreKind, caps::CapabilitySet, ring::EngineConfig};
    let mut caps = CapabilitySet::browser();
    caps.max_voices = 1;
    let mut cfg = EngineConfig::new(&caps, 48000., 8, StoreKind::NativeArc);
    cfg.bus_seconds = 0.;
    cfg.voice_seconds = 0.;
    cfg.orbit_delay_seconds = 0.;
    cfg.bus_slots = 1;
    cfg.template_slots = 1;
    vactr::dsp::engine::Engine::with_config(cfg)
}
#[test]
fn full_ack_ring_retries_critical_rejection_and_stops_control_intake() {
    use vactr::dsp::cells::AtomicCells;
    use vactr::dsp::ring::{EngineIo, EventRing, SpscRing};
    let mut engine = small_engine();
    let (_events, mut events) = EventRing::split(1);
    let (mut tx, mut controls) = SpscRing::split(3);
    let (mut acks, mut rx) = SpscRing::split(1);
    let mut cells = AtomicCells::new(1);
    let mut out = [0.; 16];
    acks.push(HostMsg::CellBatchAck { seq: 99 }).unwrap();
    for epoch in [1, 2, 3] {
        tx.push_song(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame: 0,
        }))
        .unwrap();
    }
    let mut step = |engine: &mut vactr::dsp::engine::Engine,
                    controls: &mut vactr::dsp::ring::Consumer<CtlMsg>,
                    acks: &mut vactr::dsp::ring::AckProducer| {
        engine.process(
            &mut EngineIo {
                events: &mut events,
                controls,
                acks,
                cells: &mut cells,
                garbage: None,
            },
            &mut out,
            8,
        );
    };
    step(&mut engine, &mut controls, &mut acks);
    assert_eq!(controls.len(), 2);
    assert_eq!(rx.pop(), Some(HostMsg::CellBatchAck { seq: 99 }));
    for epoch in 1..=3 {
        step(&mut engine, &mut controls, &mut acks);
        assert_eq!(
            rx.pop(),
            Some(HostMsg::Song(SongHostAck::ActivationRejected {
                activation: SongActivation {
                    epoch: SnapshotEpoch(epoch),
                    frame: 0,
                },
                reason: SongRejectCode::NotReady
            }))
        );
    }
    assert!(controls.is_empty());
    assert_eq!(engine.active_voices(), 0);
}

fn allocation() -> SongHostCapacities {
    SongHostCapacities {
        sample_rate: 48000,
        cell_slots: 16,
        voice_slots: 8,
        template_slots: 9,
        bus_slots: 10,
        sample_resources: 11,
        pcm_bytes: 512,
        voice_frames: 1024,
        bus_frames: 2048,
        ack_slots: 12,
    }
}
#[test]
fn every_measured_capacity_admits_exact_bound_and_rejects_one_less() {
    let required = allocation();
    assert!(required.admit(required).is_ok());
    macro_rules! check {($($field:ident),*)=>{$(let mut available=required;available.$field-=1;assert!(available.admit(required).is_err(),stringify!($field));)*};}
    check!(
        cell_slots,
        voice_slots,
        template_slots,
        bus_slots,
        sample_resources,
        pcm_bytes,
        voice_frames,
        bus_frames,
        ack_slots
    );
    let mut rate = required;
    rate.sample_rate = 44100;
    assert!(rate.admit(required).is_err());
    rate.sample_rate = 0;
    assert!(rate.admit(rate).is_err());
}
#[test]
fn current_and_retiring_allocations_are_subtracted_with_overflow_errors() {
    let total = allocation();
    let mut current = total;
    current.cell_slots = 4;
    current.voice_slots = 2;
    current.template_slots = 2;
    current.bus_slots = 2;
    current.sample_resources = 2;
    current.pcm_bytes = 128;
    current.voice_frames = 256;
    current.bus_frames = 512;
    current.ack_slots = 2;
    let retiring = current;
    let available = total.after_reservations(current, retiring).unwrap();
    assert_eq!(available.cell_slots, 8);
    assert_eq!(available.voice_slots, total.voice_slots);
    assert_eq!(available.voice_frames, total.voice_frames);
    assert_eq!(available.template_slots, 5);
    assert_eq!(available.sample_resources, 7);
    assert_eq!(available.pcm_bytes, 256);
    assert_eq!(available.bus_frames, 1024);
    let mut overflow = current;
    overflow.pcm_bytes = u64::MAX;
    assert_eq!(
        total
            .after_reservations(overflow, retiring)
            .unwrap_err()
            .code,
        vactr::vm::fail::FailCode::Overflow
    );
    let mut exhausted = current;
    exhausted.bus_slots = total.bus_slots;
    assert!(total.after_reservations(exhausted, retiring).is_err());
}
#[test]
fn critical_backpressure_is_not_counted_as_a_dropped_ack() {
    use vactr::dsp::ring::SpscRing;
    let (mut tx, mut rx) = SpscRing::split(1);
    let ack = HostMsg::Song(SongHostAck::Rejected {
        epoch: SnapshotEpoch(1),
        reason: SongRejectCode::NotReady,
    });
    tx.push_critical(ack).unwrap();
    for _ in 0..10 {
        assert_eq!(tx.push_critical(ack), Err(ack));
    }
    assert_eq!(tx.dropped(), 0);
    assert_eq!(rx.pop(), Some(ack));
    tx.push_critical(ack).unwrap();
    assert_eq!(rx.pop(), Some(ack));
}
#[test]
fn legacy_wire_bytes_remain_stable() {
    use vactr::host::wire::SlotControlAck;
    let mut bytes = [0; HostMsg::MAX_LEN];
    let n = HostMsg::SlotControlAck(SlotControlAck {
        slot: SlotId::new(0x01020304),
        gen: 0x05060708,
    })
    .encode(&mut bytes);
    assert_eq!(&bytes[..n], &[0x40, 4, 3, 2, 1, 8, 7, 6, 5]);
    let n = CtlMsg::GraphInstall {
        id: 0x01020304,
        gen: 0x05060708,
    }
    .encode(&mut bytes);
    assert_eq!(&bytes[..n], &[0x16, 4, 3, 2, 1, 8, 7, 6, 5]);
}

struct Probe;
thread_local! {static ALLOCATIONS:std::cell::Cell<Option<u64>>=const {std::cell::Cell::new(None)};}
// SAFETY: all allocations are forwarded unchanged to System; the thread-local
// counter observes calls without allocating or affecting pointer ownership.
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|n| {
            if let Some(v) = n.get() {
                n.set(Some(v + 1));
            }
        });
        // SAFETY: forwarded GlobalAlloc caller layout contract.
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: std::alloc::Layout) {
        // SAFETY: pointer and layout are forwarded to the same allocator.
        unsafe { std::alloc::System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|n| {
            if let Some(v) = n.get() {
                n.set(Some(v + 1));
            }
        });
        // SAFETY: forwarded pointer/layout/size caller contract.
        unsafe { std::alloc::System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static GLOBAL: Probe = Probe;
#[test]
fn rejecting_song_commands_on_callback_allocates_nothing() {
    use vactr::dsp::cells::AtomicCells;
    use vactr::dsp::ring::{EngineIo, EventRing, SpscRing};
    let mut engine = small_engine();
    let (_tx, mut events) = EventRing::split(1);
    let (mut tx, mut controls) = SpscRing::split(16);
    let (mut acks, mut rx) = SpscRing::split(16);
    let mut cells = AtomicCells::new(1);
    let mut out = [0.; 16];
    for command in commands().into_iter().chain(staged()) {
        tx.push_song(command).unwrap();
    }
    ALLOCATIONS.with(|n| n.set(Some(0)));
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut controls,
            acks: &mut acks,
            cells: &mut cells,
            garbage: None,
        },
        &mut out,
        8,
    );
    let allocations = ALLOCATIONS.with(|n| n.replace(None).unwrap());
    assert_eq!(allocations, 0);
    let mut count = 0;
    while let Some(message) = rx.pop() {
        if let HostMsg::Song(ack) = message {
            if count == 1 {
                assert_eq!(
                    ack,
                    SongHostAck::ActivationRejected {
                        activation: SongActivation {
                            epoch: SnapshotEpoch(u64::MAX),
                            frame: u64::MAX,
                        },
                        reason: SongRejectCode::NotReady,
                    }
                );
            } else {
                assert!(matches!(
                    ack,
                    SongHostAck::Rejected {
                        reason: SongRejectCode::NotReady,
                        ..
                    }
                ));
            }
            count += 1;
        }
    }
    assert_eq!(count, 10);
    assert!(out.iter().all(|x| *x == 0.));
}
#[test]
fn malformed_byte_record_does_not_consume_the_following_valid_identity() {
    use vactr::dsp::ring::{Budget, ByteInbox, ControlSource, Record};
    let mut inbox = ByteInbox::new();
    let mut bytes = [0; CtlMsg::MAX_LEN];
    let message = CtlMsg::Song(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(91),
        frame: 17,
    }));
    let n = message.encode(&mut bytes);
    bytes[n] = 255;
    assert!(inbox.push(&bytes[..n + 1]));
    assert!(inbox.push(&bytes[..n]));
    let budget = Budget {
        install_bytes: 0,
        batch: true,
    };
    assert!(inbox.next(budget).is_none());
    assert_eq!(inbox.refused(), 1);
    match inbox.next(budget) {
        Some(Record::Msg(found)) => assert_eq!(found, message),
        _ => panic!("following identity was lost"),
    };
}

#[derive(Clone, Default)]
struct QueueState {
    messages: std::rc::Rc<std::cell::RefCell<std::collections::VecDeque<HostMsg>>>,
    polls: std::rc::Rc<std::cell::Cell<usize>>,
    drains: std::rc::Rc<std::cell::Cell<usize>>,
}
struct QueueAudio {
    state: QueueState,
    supported: bool,
}
impl vactr::host::caps::AudioHost for QueueAudio {
    fn send(&mut self, _: AudioEvent) {}
    fn control(&mut self, _: vactr::host::wire::SlotControl) {}
    fn post(&mut self, _: CtlMsg) {}
    fn poll_msg(&mut self) -> Result<Option<HostMsg>, vactr::vm::fail::Failure> {
        self.state.polls.set(self.state.polls.get() + 1);
        if !self.supported {
            return Err(vactr::vm::fail::Failure::new(
                vactr::vm::fail::FailCode::HostUnavailable,
                "unsupported poll",
            ));
        }
        Ok(self.state.messages.borrow_mut().pop_front())
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.state.drains.set(self.state.drains.get() + 1);
        out.extend(self.state.messages.borrow_mut().drain(..));
    }
    fn now(&self) -> f64 {
        0.
    }
    fn swap_graph(&mut self, _: vactr::host::caps::GraphHandle) {}
    fn install_sample(&mut self, _: u32, _: std::sync::Arc<vactr::host::caps::SampleData>) {}
    fn retire_sample(&mut self, _: u32) {}
    fn analysis(&self) -> vactr::host::caps::HostSigs {
        vactr::host::caps::HostSigs::default()
    }
}
fn runtime_with(
    audio: Box<dyn vactr::host::caps::AudioHost>,
) -> (
    vactr::sched::runtime::Runtime,
    vactr::ns::evaluator::Evaluator,
) {
    use vactr::host::{caps::Hosts, noop::NoopHost};
    let mut hosts = Hosts::noop();
    hosts.audio = audio;
    let (runtime, sink) = vactr::sched::runtime::Runtime::new(
        hosts,
        std::rc::Rc::new(NoopHost),
        vactr::dsp::caps::CapabilitySet::native(),
        vactr::sched::runtime::RuntimeConfig::default(),
    );
    let evaluator = vactr::ns::evaluator::Evaluator::new(
        vactr::ns::namespace::Prelude::core(),
        Box::new(NoopHost),
        Box::new(sink),
    );
    (runtime, evaluator)
}
fn resource_ack(id: u32) -> SongHostAck {
    SongHostAck::ResourceReady {
        epoch: SnapshotEpoch(5),
        resource: SongResourceRef {
            id,
            generation: id + 7,
        },
    }
}
#[test]
fn actual_runtime_saturation_retains130_identities_once_and_blocks_polling() {
    let state = QueueState::default();
    for id in 0..130 {
        state
            .messages
            .borrow_mut()
            .push_back(HostMsg::Song(resource_ack(id)));
    }
    let (mut runtime, mut ev) = runtime_with(Box::new(QueueAudio {
        state: state.clone(),
        supported: true,
    }));
    runtime.expect_song_receipts(SnapshotEpoch(5)).unwrap();
    assert!(runtime.tick(&mut ev, 0.).faults.is_empty());
    assert_eq!(runtime.song_receipts().iter().count(), 64);
    assert_eq!(state.polls.get(), 65);
    assert_eq!(state.messages.borrow().len(), 65);
    assert!(runtime.expect_song_receipts(SnapshotEpoch(6)).is_err());
    runtime.tick(&mut ev, 0.);
    assert_eq!(state.polls.get(), 65);
    assert_eq!(state.drains.get(), 0);
    let mut seen = Vec::new();
    while let Some(ack) = runtime.pop_song_receipt() {
        seen.push(ack);
    }
    assert!(runtime.expect_song_receipts(SnapshotEpoch(6)).is_err());
    while seen.len() < 130 {
        assert!(runtime.tick(&mut ev, 0.).faults.is_empty());
        while let Some(ack) = runtime.pop_song_receipt() {
            seen.push(ack);
        }
    }
    assert_eq!(seen, (0..130).map(resource_ack).collect::<Vec<_>>());
    assert!(state.messages.borrow().is_empty());
    assert_eq!(state.drains.get(), 0);
    runtime.expect_song_receipts(SnapshotEpoch(6)).unwrap();
    assert!(runtime.pending_song().is_none());
}
#[test]
fn stale_retirement_is_retained_and_non_song_analysis_still_processed() {
    let state = QueueState::default();
    let stale = SongHostAck::ResourceRetired {
        epoch: SnapshotEpoch(4),
        resource: SongResourceRef {
            id: 7,
            generation: 8,
        },
    };
    state.messages.borrow_mut().extend([
        HostMsg::Song(stale),
        HostMsg::AnalysisCell {
            id: 12,
            value: 0.75,
        },
        HostMsg::Song(SongHostAck::Ready(SnapshotEpoch(5))),
    ]);
    let (mut runtime, mut ev) = runtime_with(Box::new(QueueAudio {
        state: state.clone(),
        supported: true,
    }));
    runtime.expect_song_receipts(SnapshotEpoch(5)).unwrap();
    let report = runtime.tick(&mut ev, 0.);
    assert_eq!(report.faults.len(), 1);
    assert_eq!(report.faults[0].code, vactr::vm::fail::FailCode::Type);
    assert_eq!(runtime.pop_song_receipt(), Some(stale));
    assert_eq!(
        runtime.pop_song_receipt(),
        Some(SongHostAck::Ready(SnapshotEpoch(5)))
    );
    assert_eq!(
        runtime
            .input_cells()
            .analyzer(vactr::pattern::eval::AnalyzerId::new(12)),
        0.75
    );
    assert!(runtime.pending_song().is_none());
}
#[test]
fn unsupported_poll_has_no_legacy_fallback_but_true_legacy_mode_keeps_drain() {
    let state = QueueState::default();
    state
        .messages
        .borrow_mut()
        .push_back(HostMsg::AnalysisCell { id: 12, value: 0.5 });
    let (mut runtime, mut ev) = runtime_with(Box::new(QueueAudio {
        state: state.clone(),
        supported: false,
    }));
    let report = runtime.tick(&mut ev, 0.);
    assert!(report.faults.is_empty());
    assert_eq!(state.drains.get(), 1);
    assert_eq!(state.polls.get(), 0);
    assert_eq!(
        runtime
            .input_cells()
            .analyzer(vactr::pattern::eval::AnalyzerId::new(12)),
        0.5
    );
    runtime.expect_song_receipts(SnapshotEpoch(5)).unwrap();
    state
        .messages
        .borrow_mut()
        .push_back(HostMsg::Song(resource_ack(1)));
    let report = runtime.tick(&mut ev, 0.);
    assert_eq!(
        report.faults[0].code,
        vactr::vm::fail::FailCode::HostUnavailable
    );
    assert_eq!(state.drains.get(), 1);
    assert_eq!(state.messages.borrow().len(), 1);
    assert_eq!(runtime.song_receipts().iter().count(), 0);
}
#[test]
fn unsolicited_song_reply_in_legacy_mode_is_a_protocol_fault_not_ownership() {
    let state = QueueState::default();
    state
        .messages
        .borrow_mut()
        .push_back(HostMsg::Song(SongHostAck::Ready(SnapshotEpoch(1))));
    let (mut runtime, mut ev) = runtime_with(Box::new(QueueAudio {
        state,
        supported: true,
    }));
    let report = runtime.tick(&mut ev, 0.);
    assert_eq!(report.faults.len(), 1);
    assert_eq!(report.faults[0].code, vactr::vm::fail::FailCode::Type);
    assert!(runtime.pop_song_receipt().is_none());
    assert!(runtime.pending_song().is_none());
}
#[cfg(feature = "host-native")]
#[test]
fn actual_native_host_runtime_pipeline_preserves_stale_rejections_under_saturation() {
    use vactr::host::caps::AudioHost;
    let mut caps = vactr::dsp::caps::CapabilitySet::native();
    caps.max_voices = 1;
    let (mut host, mut side) = vactr::host::native::NativeAudioHost::headless(48000, caps, 1);
    for epoch in 1..=130 {
        host.post_song(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame: 0,
        }));
    }
    side.render(&mut [0.; 32], 2);
    let (mut runtime, mut ev) = runtime_with(Box::new(host));
    runtime.expect_song_receipts(SnapshotEpoch(1)).unwrap();
    let mut seen = Vec::new();
    while seen.len() < 130 {
        runtime.tick(&mut ev, 0.);
        while let Some(ack) = runtime.pop_song_receipt() {
            seen.push(ack);
        }
    }
    assert_eq!(
        seen,
        (1..=130)
            .map(|epoch| SongHostAck::ActivationRejected {
                activation: SongActivation {
                    epoch: SnapshotEpoch(epoch),
                    frame: 0,
                },
                reason: SongRejectCode::NotReady
            })
            .collect::<Vec<_>>()
    );
    assert!(runtime.pending_song().is_none());
}

fn staged_boundaries() -> Vec<SongCommand> {
    let epoch = SnapshotEpoch(u64::MAX);
    let mut out = Vec::new();
    for command in staged() {
        match command {
            SongCommand::BeginPreparation(mut p) => {
                p.epoch = epoch;
                out.push(SongCommand::BeginPreparation(p));
            }
            SongCommand::ReserveResource(mut r) => {
                r.epoch = epoch;
                for kind in [
                    SongResourceKind::Instrument,
                    SongResourceKind::PrivateFx,
                    SongResourceKind::Track,
                    SongResourceKind::Master,
                    SongResourceKind::Sample,
                ] {
                    r.kind = kind;
                    out.push(SongCommand::ReserveResource(r));
                }
            }
            SongCommand::ConfigureBranch(mut b) => {
                b.epoch = epoch;
                b.branch = SongBranchId(u32::MAX);
                b.generation = u32::MAX;
                b.family = u32::MAX;
                b.track = u32::MAX;
                for mask in 0..8 {
                    b.private_fx = (mask & 1 != 0).then_some(b.instrument);
                    b.track_template = (mask & 2 != 0).then_some(b.instrument);
                    b.master = (mask & 4 != 0).then_some(b.instrument);
                    out.push(SongCommand::ConfigureBranch(b));
                }
            }
            SongCommand::SealPreparation(_) => out.push(SongCommand::SealPreparation(epoch)),
            _ => panic!("unexpected staged command"),
        }
    }
    out
}
#[test]
fn every_staged_resource_and_optional_combination_has_full_width_carrier_parity() {
    use vactr::dsp::ring::{Budget, ByteInbox, ControlSource, NativeRecord, Record, SpscRing};
    let (mut tx, mut native) = SpscRing::split(2);
    let mut browser = ByteInbox::new();
    let budget = Budget {
        install_bytes: 0,
        batch: true,
    };
    let commands = staged_boundaries();
    assert_eq!(commands.len(), 15);
    for command in commands {
        let message = CtlMsg::Song(command);
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = message.encode(&mut bytes);
        assert!(n > 0);
        assert_eq!(command.epoch(), SnapshotEpoch(u64::MAX));
        assert_eq!(CtlMsg::decode(&bytes[..n]).unwrap(), (message, n));
        assert!(tx.push(NativeRecord::Msg(message)).is_ok());
        assert!(browser.push(&bytes[..n]));
        match (native.next(budget), browser.next(budget)) {
            (Some(Record::Msg(a)), Some(Record::Msg(b))) => {
                assert_eq!(a, message);
                assert_eq!(a, b);
            }
            _ => panic!("staged identity lost"),
        }
        for end in 0..n {
            assert!(
                CtlMsg::decode(&bytes[..end]).is_err(),
                "staged prefix {end}/{n}"
            );
        }
        assert_eq!(message.encode(&mut bytes[..n - 1]), 0);
    }
}
fn assert_staged_refusal_preserves_following_identity(malformed: &[u8]) {
    use vactr::dsp::ring::{Budget, ByteInbox, ControlSource, Record};
    let mut inbox = ByteInbox::new();
    let resource = SongResourceRef {
        id: u32::MAX,
        generation: u32::MAX,
    };
    let valid = CtlMsg::Song(SongCommand::ConfigureBranch(SongBranchConfig {
        epoch: SnapshotEpoch(u64::MAX),
        branch: SongBranchId(u32::MAX),
        generation: u32::MAX,
        family: u32::MAX,
        track: u32::MAX,
        instrument: resource,
        private_fx: Some(resource),
        track_template: Some(resource),
        master: Some(resource),
        transition_frame: u64::MAX - 1,
        tail_deadline: u64::MAX,
    }));
    let mut bytes = [0; CtlMsg::MAX_LEN];
    let n = valid.encode(&mut bytes);
    assert!(inbox.push(malformed));
    assert!(inbox.push(&bytes[..n]));
    let budget = Budget {
        install_bytes: 0,
        batch: true,
    };
    assert!(inbox.next(budget).is_none());
    assert_eq!(inbox.refused(), 1);
    match inbox.next(budget) {
        Some(Record::Msg(message)) => assert_eq!(message, valid),
        _ => panic!("valid staged epoch following malformed record was lost"),
    }
    assert!(inbox.next(budget).is_none());
}
#[test]
fn invalid_staged_kind_and_optional_flags_refuse_without_losing_next_identity() {
    for command in staged_boundaries() {
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        let flags = match command {
            SongCommand::ReserveResource(_) => vec![18],
            SongCommand::ConfigureBranch(b) => {
                let second = 35 + usize::from(b.private_fx.is_some()) * 8;
                let third = second + 1 + usize::from(b.track_template.is_some()) * 8;
                vec![34, second, third]
            }
            _ => continue,
        };
        for flag in flags {
            let invalid_values = if matches!(command, SongCommand::ReserveResource(_)) {
                [5, 255]
            } else {
                [2, 255]
            };
            for invalid in invalid_values {
                let mut bad = bytes;
                bad[flag] = invalid;
                assert_eq!(CtlMsg::decode(&bad[..n]), Err(WireError::BadValue));
                assert_staged_refusal_preserves_following_identity(&bad[..n]);
            }
        }
    }
}
#[test]
fn truncated_staged_byte_records_preserve_the_following_full_width_epoch() {
    for command in staged_boundaries() {
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        // Empty records are rejected by ByteInbox::push; decoder empty-prefix
        // rejection is already exercised above. Every nonempty truncation is queued.
        for end in 1..n {
            assert_staged_refusal_preserves_following_identity(&bytes[..end]);
        }
    }
}
