//! Exact rendered clocks through the actual Native and portable browser senders.
pub use vactr::{dsp, reader, sched, song, types, value, vm};
#[path = "../src/host/wasm/abi.rs"]
pub mod browser_abi;
#[path = "../src/host/wasm/messages.rs"]
pub mod browser_messages;
pub mod host {
    pub use vactr::host::{caps, wire};
    pub mod wasm {
        pub use crate::browser_abi as abi;
        pub use crate::browser_messages as messages;
    }
}
use std::cell::RefCell;
use std::rc::Rc;
use vactr::dsp::arena::StoreKind;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::AtomicCells;
use vactr::dsp::engine::{Engine, EngineConfig};
use vactr::dsp::ring::{self, ByteInbox, EngineIo, EventRing, SpscRing};
use vactr::host::caps::{AudioHost, SongSubmitError};
use vactr::host::wire::{CtlMsg, HostMsg};
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;

fn request(n: u64) -> SongClockRequest {
    SongClockRequest {
        epoch: SnapshotEpoch(u64::MAX),
        request: n,
    }
}
fn report(n: u64, frame: u64, rate: u32) -> SongHostAck {
    SongHostAck::ClockReport(SongClockReport {
        request: request(n),
        clock: SongHostClock {
            frame,
            sample_rate: rate,
        },
    })
}
fn browser() -> (
    browser_messages::WasmAudioHost,
    Rc<RefCell<browser_messages::HostState>>,
) {
    browser_abi::fix_outbox(4096);
    let state = Rc::new(RefCell::new(browser_messages::HostState::new(4096)));
    (browser_messages::WasmAudioHost(state.clone()), state)
}
struct Rig {
    engine: Engine,
    inbox: ByteInbox,
    _events_tx: ring::EventProducer,
    events: ring::EventConsumer,
    acks: ring::AckProducer,
    received: ring::AckConsumer,
    cells: AtomicCells,
}
impl Rig {
    fn new(rate: f32, ack_slots: usize) -> Self {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 1;
        let mut cfg = EngineConfig::new(&caps, rate, 16, StoreKind::Arena { bytes: 1_000_000 });
        cfg.template_slots = 1;
        cfg.bus_slots = 1;
        cfg.voice_seconds = 0.01;
        cfg.bus_seconds = 0.01;
        let (_events_tx, events) = EventRing::split(1);
        let (acks, received) = SpscRing::split(ack_slots);
        Self {
            engine: Engine::with_config(cfg),
            inbox: ByteInbox::new(),
            _events_tx,
            events,
            acks,
            received,
            cells: AtomicCells::new(1),
        }
    }
    fn transfer(&mut self) {
        let len = usize::try_from(browser_abi::outbox_len()).unwrap();
        // The real ABI outbox remains stable while copying its framed bytes.
        let bytes = unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), len) }.to_vec();
        browser_abi::for_each_record(&bytes, |record| assert!(self.inbox.push(record)));
        browser_abi::fix_outbox(4096);
    }
    fn step(&mut self) {
        let mut output = [0.; 32];
        self.engine.process(
            &mut EngineIo {
                events: &mut self.events,
                controls: &mut self.inbox,
                acks: &mut self.acks,
                cells: &mut self.cells,
                garbage: None,
            },
            &mut output,
            16,
        );
        assert!(output.iter().all(|x| *x == 0.));
    }
    fn deliver(&mut self, state: &Rc<RefCell<browser_messages::HostState>>) -> Vec<HostMsg> {
        let mut out = Vec::new();
        while let Some(message) = self.received.pop() {
            state.borrow_mut().on_msg(message);
            out.push(message);
        }
        out
    }
}
#[test]
fn full_width_clock_codec_and_all_truncations_preserve_existing_bytes() {
    let command = CtlMsg::Song(SongCommand::RequestClock(request(u64::MAX)));
    let mut bytes = [0; CtlMsg::MAX_LEN];
    let n = command.encode(&mut bytes);
    assert_eq!(n, 18);
    assert_eq!(&bytes[..2], &[0x1B, 17]);
    assert_eq!(CtlMsg::decode(&bytes[..n]).unwrap(), (command, n));
    for end in 0..n {
        assert!(CtlMsg::decode(&bytes[..end]).is_err());
    }
    for ack in [
        report(u64::MAX, u64::MAX, 192000),
        SongHostAck::ClockRejected(SongClockFailure {
            request: request(u64::MAX),
            reason: SongRejectCode::HostFault,
        }),
    ] {
        let message = HostMsg::Song(ack);
        let mut bytes = [0; HostMsg::MAX_LEN];
        let n = message.encode(&mut bytes);
        assert_eq!(
            n,
            if matches!(ack, SongHostAck::ClockReport(_)) {
                30
            } else {
                19
            }
        );
        assert_eq!(HostMsg::decode(&bytes[..n]).unwrap(), (message, n));
        for end in 0..n {
            assert!(HostMsg::decode(&bytes[..end]).is_err());
        }
        if matches!(ack, SongHostAck::ClockRejected(_)) {
            bytes[n - 1] = 255;
            assert!(HostMsg::decode(&bytes[..n]).is_err());
        }
    }
    for (command, tag) in [
        (SongCommand::Prepare(SnapshotEpoch(7)), 0),
        (SongCommand::RequestCapacity(SnapshotEpoch(7)), 15),
        (SongCommand::CancelPreparation(SnapshotEpoch(7)), 13),
    ] {
        let n = CtlMsg::Song(command).encode(&mut bytes);
        assert_eq!(&bytes[..n], &[0x1B, tag, 7, 0, 0, 0, 0, 0, 0, 0]);
    }
    let mut rig = Rig::new(48000., 4);
    let mut malformed = [0; CtlMsg::MAX_LEN];
    let n = command.encode(&mut malformed);
    assert!(rig.inbox.push(&malformed[..n - 1]));
    assert!(rig.inbox.push(&malformed[..n]));
    rig.step();
    let mut messages = Vec::new();
    while let Some(message) = rig.received.pop() {
        messages.push(message);
    }
    assert!(messages.is_empty());
    assert_eq!(rig.inbox.refused(), 1);
    rig.step();
    assert_eq!(
        rig.received.pop(),
        Some(HostMsg::Song(report(u64::MAX, 16, 48000)))
    );
    assert!(rig.received.pop().is_none());
    rig.step();
    assert!(rig.received.pop().is_none());
    assert_eq!(rig.inbox.refused(), 1);
}
#[test]
fn actual_browser_sender_engine_frame_and_rate_are_not_wall_time() {
    let (mut host, state) = browser();
    state.borrow_mut().now = 1e30;
    assert!(host.song_clock().is_err());
    let mut rig = Rig::new(44100., 4);
    for (nonce, frame) in [(1, 0), (2, 16), (3, 32)] {
        host.try_song_command(SongCommand::RequestClock(request(nonce)))
            .unwrap();
        assert!(host.song_clock().is_err());
        rig.transfer();
        rig.step();
        assert_eq!(
            rig.deliver(&state),
            vec![HostMsg::Song(report(nonce, frame, 44100))]
        );
        assert_eq!(
            host.song_clock().unwrap(),
            SongHostClock {
                frame,
                sample_rate: 44100
            }
        );
        assert_eq!(
            host.poll_msg().unwrap(),
            Some(HostMsg::Song(report(nonce, frame, 44100)))
        );
    }
}
#[test]
fn browser_only_admitted_nonce_changes_cache_and_foreign_receipts_survive() {
    let (mut host, state) = browser();
    host.try_song_command(SongCommand::RequestClock(request(1)))
        .unwrap();
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(report(1, 100, 48000)));
    let old = host.song_clock().unwrap();
    browser_abi::fix_outbox(1);
    let command = SongCommand::RequestClock(request(2));
    let refused = host.try_song_command(command).unwrap_err();
    assert_eq!(refused.command, command);
    assert!(matches!(refused.error, SongSubmitError::Backpressure));
    assert_eq!(host.song_clock().unwrap(), old);
    browser_abi::fix_outbox(4096);
    host.try_song_command(command).unwrap();
    for foreign in [
        report(1, 200, 48000),
        SongHostAck::ClockReport(SongClockReport {
            request: SongClockRequest {
                epoch: SnapshotEpoch(1),
                request: 2,
            },
            clock: old,
        }),
    ] {
        state.borrow_mut().on_msg(HostMsg::Song(foreign));
        assert!(host.song_clock().is_err());
    }
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(report(2, 101, 48000)));
    assert_eq!(host.song_clock().unwrap().frame, 101);
    assert_eq!(state.borrow().acks.len(), 4); // Every actual/foreign reply is forwarded.
    assert!(host.try_song_command(command).is_err());
    assert_eq!(host.song_clock().unwrap().frame, 101);
}
#[test]
fn browser_rate_regression_failure_and_nonce_exhaustion_are_explicit() {
    let (mut host, state) = browser();
    for (nonce, ack, text) in [
        (1, report(1, 100, 48000), None),
        (2, report(2, 101, 44100), Some("sample rate mismatch")),
        (3, report(3, 99, 48000), Some("frame regressed")),
        (
            4,
            SongHostAck::ClockRejected(SongClockFailure {
                request: request(4),
                reason: SongRejectCode::NotReady,
            }),
            Some("request rejected"),
        ),
    ] {
        host.try_song_command(SongCommand::RequestClock(request(nonce)))
            .unwrap();
        state.borrow_mut().on_msg(HostMsg::Song(ack));
        if let Some(text) = text {
            assert!(host.song_clock().unwrap_err().to_string().contains(text));
        } else {
            assert_eq!(host.song_clock().unwrap().frame, 100);
        }
    }
    host.try_song_command(SongCommand::RequestClock(request(u64::MAX)))
        .unwrap();
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(report(u64::MAX, 101, 48000)));
    let previous = host.song_clock().unwrap();
    let error = host
        .try_song_command(SongCommand::RequestClock(request(u64::MAX)))
        .unwrap_err();
    assert!(
        matches!(error.error, SongSubmitError::Invalid(ref f) if f.code == vactr::vm::fail::FailCode::Overflow)
    );
    assert_eq!(host.song_clock().unwrap(), previous);
}
#[test]
fn actual_engine_retains_clock_under_ack_pressure_without_reconsuming_request() {
    let (mut host, state) = browser();
    let mut rig = Rig::new(48000., 1);
    // Genuine engine-generated clock reports fill the measured ACK ring.
    host.try_song_command(SongCommand::RequestClock(request(1)))
        .unwrap();
    rig.transfer();
    rig.step();
    host.try_song_command(SongCommand::RequestClock(request(2)))
        .unwrap();
    rig.transfer();
    rig.step();
    host.try_song_command(SongCommand::RequestClock(request(3)))
        .unwrap();
    rig.transfer();
    rig.step();
    assert_eq!(
        rig.deliver(&state),
        vec![HostMsg::Song(report(1, 0, 48000))]
    );
    assert!(host.song_clock().is_err());
    rig.step();
    assert_eq!(
        rig.deliver(&state),
        vec![HostMsg::Song(report(2, 16, 48000))]
    );
    assert!(host.song_clock().is_err());
    rig.step();
    assert_eq!(
        rig.deliver(&state),
        vec![HostMsg::Song(report(3, 48, 48000))]
    );
    assert_eq!(host.song_clock().unwrap().frame, 48);
    rig.step();
    assert!(rig.deliver(&state).is_empty());
}
#[test]
fn actual_engine_clock_failure_retains_nonce_and_has_no_ready_dependency() {
    let (mut host, state) = browser();
    let mut rig = Rig::new(48000.5, 1);
    host.try_song_command(SongCommand::RequestClock(request(1)))
        .unwrap();
    rig.transfer();
    rig.step();
    assert_eq!(
        rig.deliver(&state),
        vec![HostMsg::Song(SongHostAck::ClockRejected(
            SongClockFailure {
                request: request(1),
                reason: SongRejectCode::Malformed
            }
        ))]
    );
    assert!(host.song_clock().is_err());
    assert!(vactr::host::noop::NoopHost.song_clock().is_err());
}
#[cfg(feature = "host-native")]
#[test]
fn native_trait_clock_reads_rendered_integer_frames_and_preserves_capacity_cache() {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 1;
    let (mut host, mut side) = vactr::host::native::NativeAudioHost::headless(48000, caps, 1);
    assert_eq!(
        AudioHost::song_clock(&host).unwrap(),
        SongHostClock {
            frame: 0,
            sample_rate: 48000
        }
    );
    side.render(&mut [0.; 32], 2);
    while host.poll_msg().unwrap().is_some() {}
    let capacity = host.song_remaining_capacities().unwrap();
    let clock = AudioHost::song_clock(&host).unwrap();
    assert_eq!(clock.frame, 16);
    host.try_song_command(SongCommand::RequestClock(request(u64::MAX)))
        .unwrap();
    assert_eq!(host.song_remaining_capacities().unwrap(), capacity);
    side.render(&mut [0.; 14], 2);
    assert_eq!(AudioHost::song_clock(&host).unwrap().frame, 23);
    assert_eq!(
        host.poll_msg().unwrap(),
        Some(HostMsg::Song(report(u64::MAX, 16, 48000)))
    );
    assert_eq!(host.song_remaining_capacities().unwrap(), capacity);
    let mut refused = None;
    for _ in 0..10000 {
        match host.try_song_command(SongCommand::RequestClock(request(9))) {
            Ok(()) => {}
            Err(error) => {
                refused = Some(error);
                break;
            }
        }
    }
    let refused = refused.expect("actual finite native control ring fills");
    assert_eq!(refused.command, SongCommand::RequestClock(request(9)));
    assert!(matches!(refused.error, SongSubmitError::Backpressure));
    assert_eq!(host.song_remaining_capacities().unwrap(), capacity);
}
