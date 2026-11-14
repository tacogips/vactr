//! Checked sample admission preserves original shared PCM on every refusal.
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
use browser_messages::{HostState, WasmAudioHost};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
use vactr::dsp::{
    arena::StoreKind,
    caps::CapabilitySet,
    cells::AtomicCells,
    engine::{Engine, EngineConfig, SongStagingConfig},
    ring::{self, ByteInbox, EngineIo, EventRing, Garbage, SpscRing},
};
use vactr::host::noop::NoopHost;
use vactr::host::{
    caps::{
        AudioHost, SampleData, SongHostPreparation, SongPreparationLimits, SongPreparationProgress,
        SongReadyBundle, SongSampleSenderCapacity, SongSubmitError,
    },
    native::audio::{AudioSide, NativeAudioHost},
    wire::HostMsg,
};
use vactr::song::{
    assets::{DecodedSongAssetFactory, SongAssetLimits},
    routing::*,
    PreparedSong, SnapshotEpoch, SongLimits,
};

static ARENA_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn key(id: u32) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(700),
        resource: SongResourceRef { id, generation: 1 },
        kind: SongResourceKind::Sample,
    }
}
fn sample(value: f32) -> Arc<SampleData> {
    Arc::new(SampleData {
        rate: 48000,
        channels: 1,
        frames: vec![value; 8].into_boxed_slice(),
    })
}
#[test]
fn unsupported_checked_and_legacy_refusals_retain_exact_arc_and_lease() {
    let mut host = NoopHost;
    let original = sample(0.5);
    let refusal = host
        .try_song_sample(key(1), Arc::clone(&original))
        .unwrap_err();
    assert_eq!(refusal.lease, key(1));
    assert!(Arc::ptr_eq(&refusal.data, &original));
    assert!(matches!(refusal.error, SongSubmitError::Unavailable));
    assert!(host.song_sample_sender_capacity().is_err());
    let returned = host
        .submit_song_sample(key(1), Arc::clone(&original))
        .unwrap_err();
    assert!(Arc::ptr_eq(&returned, &original));
}
#[test]
fn native_permanent_geometry_refusal_preserves_exact_arc_and_sender_observation() {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 1;
    let mut config = EngineConfig::new(
        &caps,
        48000.,
        vactr::host::native::audio::MAX_BLOCK,
        StoreKind::NativeArc,
    );
    config.bus_slots = 1;
    config.template_slots = 1;
    config.bus_seconds = 0.01;
    config.voice_seconds = 0.01;
    let (mut host, _side) = NativeAudioHost::headless_with_config(config, 1).unwrap();
    assert_eq!(
        host.song_sample_sender_capacity().unwrap(),
        SongSampleSenderCapacity::Unbounded
    );
    let original = sample(f32::NAN);
    for _ in 0..2 {
        let refusal = host
            .try_song_sample(key(2), Arc::clone(&original))
            .unwrap_err();
        assert_eq!(refusal.lease, key(2));
        assert!(Arc::ptr_eq(&refusal.data, &original));
        assert!(matches!(refusal.error, SongSubmitError::Invalid(_)));
    }
}

fn limits(capabilities: CapabilitySet) -> SongPreparationLimits {
    SongPreparationLimits {
        capabilities,
        song: SongLimits::default(),
        max_resources: 128,
        max_pending_records: 4096,
        max_graph_bytes: 65536,
        max_work: 8_000_000,
    }
}
enum Backend {
    Native(Box<AudioSide>),
    Arena {
        state: Rc<RefCell<HostState>>,
        engine: Box<Engine>,
        inbox: Box<ByteInbox>,
        pending: std::collections::VecDeque<Vec<u8>>,
        events: ring::EventConsumer,
        acks: ring::AckProducer,
        received: ring::AckConsumer,
        cells: AtomicCells,
        garbage: ring::Producer<Garbage>,
        returned: ring::Consumer<Garbage>,
    },
}
struct Rig {
    host: Box<dyn AudioHost>,
    backend: Backend,
    caps: CapabilitySet,
    frame: u64,
    receipts: Vec<SongHostAck>,
}
impl Rig {
    fn new(bytes: bool) -> Self {
        let mut caps = if bytes {
            CapabilitySet::browser()
        } else {
            CapabilitySet::native()
        };
        caps.max_voices = 2;
        let (host, backend): (Box<dyn AudioHost>, Backend) = if !bytes {
            let mut cfg = EngineConfig::new(
                &caps,
                32768.,
                vactr::host::native::audio::MAX_BLOCK,
                StoreKind::NativeArc,
            );
            cfg.bus_slots = 12;
            let (host, side) = NativeAudioHost::headless_with_config(cfg, 64).unwrap();
            (Box::new(host), Backend::Native(Box::new(side)))
        } else {
            browser_abi::outbox_clear();
            browser_abi::fix_outbox(0);
            let mut cfg =
                EngineConfig::new(&caps, 32768., 16, StoreKind::Arena { bytes: 1_000_000 });
            //17 distinct path families plus track/master and live legacy master.
            let graph_families = ring::INBOX_SLOTS + 1;
            cfg.template_slots = graph_families;
            cfg.bus_slots = graph_families + 3;
            cfg.bus_seconds = 20.;
            cfg.analysis_cells = 128;
            cfg.voice_seconds = 4.;
            let mut engine = Engine::with_config(cfg);
            let (acks, received) = SpscRing::split(128);
            engine
                .configure_song_staging_for_transport(
                    SongStagingConfig {
                        preparations: 4,
                        leases: 128,
                        branches: u32::try_from(graph_families).unwrap(),
                        control_slots: 64,
                        analysis_slots: 128,
                        native_pcm_bytes: 1_000_000,
                        critical_receipts: 2,
                    },
                    &acks,
                )
                .unwrap();
            let (_, events) = EventRing::split(32);
            let (garbage, returned) = SpscRing::split(128);
            let state = Rc::new(RefCell::new(HostState::new(1_000_000)));
            (
                Box::new(WasmAudioHost(Rc::clone(&state))),
                Backend::Arena {
                    state,
                    engine: Box::new(engine),
                    inbox: Box::new(ByteInbox::new()),
                    pending: std::collections::VecDeque::new(),
                    events,
                    acks,
                    received,
                    cells: AtomicCells::new(64),
                    garbage,
                    returned,
                },
            )
        };
        Self {
            host,
            backend,
            caps,
            frame: 0,
            receipts: Vec::new(),
        }
    }
    fn tick(&mut self) -> Vec<HostMsg> {
        let mut out = [0.; 32];
        match &mut self.backend {
            Backend::Native(side) => side.render(&mut out, 2),
            Backend::Arena {
                state,
                engine,
                inbox,
                pending,
                events,
                acks,
                received,
                cells,
                garbage,
                returned,
            } => {
                let length = usize::try_from(browser_abi::outbox_len()).unwrap();
                let bytes =
                    unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), length) }
                        .to_vec();
                browser_abi::outbox_clear();
                let mut at = 0;
                while at < bytes.len() {
                    let n =
                        usize::try_from(u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()))
                            .unwrap();
                    at += 4;
                    pending.push_back(bytes[at..at + n].to_vec());
                    at += n;
                }
                while let Some(record) = pending.front() {
                    if !inbox.push(record) {
                        break;
                    }
                    pending.pop_front();
                }
                {
                    engine.process(
                        &mut EngineIo {
                            events,
                            controls: inbox.as_mut(),
                            acks,
                            cells,
                            garbage: Some(garbage),
                        },
                        &mut out,
                        16,
                    )
                }
                while let Some(message) = received.pop() {
                    state.borrow_mut().on_msg(message);
                }
                while let Some(owner) = returned.pop() {
                    drop(owner);
                }
            }
        }
        self.frame += 16;
        let mut messages = Vec::new();
        // Native poll None can consume an internal report before later public ACKs.
        self.host.drain(&mut messages);
        for message in &messages {
            if let HostMsg::Song(ack) = message {
                self.receipts.push(*ack);
            }
        }
        messages
    }
    fn complete(&mut self, owner: &mut SongHostPreparation) -> SongReadyBundle {
        for _ in 0..2048 {
            owner.submit(self.host.as_mut()).unwrap();
            for message in self.tick() {
                if let HostMsg::Song(ack) = message {
                    if let Err(unhandled) = owner.receive(ack) {
                        assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
                    }
                }
            }
            if owner.progress() == SongPreparationProgress::Ready {
                return owner.take_ready().unwrap();
            }
        }
        panic!("actual owner did not reach Ready: {:?}", owner.progress());
    }
    fn cleanup(&mut self, owner: &mut SongHostPreparation) {
        for _ in 0..2048 {
            let _ = owner.submit(self.host.as_mut());
            for message in self.tick() {
                if let HostMsg::Song(ack) = message {
                    if let Err(unhandled) = owner.receive(ack) {
                        assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
                    }
                }
            }
            if matches!(
                owner.progress(),
                SongPreparationProgress::Cancelled
                    | SongPreparationProgress::Retired
                    | SongPreparationProgress::Failed
            ) {
                return;
            }
        }
        panic!("actual cleanup stalled: {:?}", owner.progress());
    }
}

impl Rig {
    fn command(&mut self, command: SongCommand) {
        self.host.try_song_command(command).unwrap();
    }
    fn report(&mut self, epoch: SnapshotEpoch) -> SongCapacityReport {
        self.command(SongCommand::RequestCapacity(epoch));
        for _ in 0..64 {
            self.tick();
            if let Some(report) = self.receipts.iter().rev().find_map(|ack| match ack {
                SongHostAck::CapacityReport(report) if report.epoch == epoch => Some(*report),
                _ => None,
            }) {
                return report;
            }
        }
        panic!("actual capacity report missing");
    }
    fn reserve_samples(&mut self, epoch: SnapshotEpoch, count: u32) -> Vec<SongLeaseKey> {
        self.reserve_samples_from(epoch, count, 1)
    }
    fn reserve_samples_from(
        &mut self,
        epoch: SnapshotEpoch,
        count: u32,
        first: u32,
    ) -> Vec<SongLeaseKey> {
        let report = self.report(epoch);
        assert!(report.available.sample_resources >= count);
        self.command(SongCommand::BeginPreparation(SongPreparation {
            epoch,
            branches: 0,
            resources: count,
            required: SongHostCapacities {
                sample_rate: 32768,
                sample_resources: count,
                pcm_bytes: u64::from(count) * 32,
                ..Default::default()
            },
        }));
        let keys: Vec<_> = (0..count)
            .map(|id| SongLeaseKey {
                epoch,
                resource: SongResourceRef {
                    id: first + id,
                    generation: 1,
                },
                kind: SongResourceKind::Sample,
            })
            .collect();
        for lease in &keys {
            self.command(SongCommand::ReserveResource(SongResourceReservation {
                epoch,
                resource: lease.resource,
                kind: lease.kind,
            }));
        }
        // Genuine command completion is observed by rendering and later upload ACKs.
        for _ in 0..8 {
            self.tick();
        }
        assert!(!self.receipts.iter().any(|ack| matches!(ack,
            SongHostAck::Rejected { epoch: e, .. } if *e == epoch)));
        keys
    }
    fn upload_ready(&mut self, keys: &[SongLeaseKey]) {
        for _ in 0..512 {
            self.tick();
            if keys.iter().all(|key| {
                self.receipts.iter().any(|ack| {
                    matches!(ack,
                SongHostAck::ResourceReady { epoch, resource }
                    if *epoch == key.epoch && *resource == key.resource)
                })
            }) {
                return;
            }
        }
        panic!(
            "actual sample upload did not become ready: {:?}",
            self.receipts
        );
    }
}

#[test]
fn arena_exact_metadata_boundary_final_slice_and_full_key_return_preserve_charge() {
    let _guard = ARENA_TEST.lock().unwrap();
    let mut rig = Rig::new(true);
    let epoch = SnapshotEpoch(710);
    let report = rig.report(epoch);
    let count = u32::try_from(ring::INBOX_SLOTS).unwrap();
    assert!(report.available.sample_resources > count);
    let keys = rig.reserve_samples(epoch, count);
    let baseline = rig.host.song_sample_sender_capacity().unwrap();
    let originals: Vec<_> = keys.iter().map(|_| sample(0.25)).collect();
    for (key, data) in keys.iter().zip(&originals) {
        rig.host.try_song_sample(*key, Arc::clone(data)).unwrap();
    }
    let exhausted = rig.host.song_sample_sender_capacity().unwrap();
    let (
        SongSampleSenderCapacity::Bounded {
            pcm_bytes: before, ..
        },
        SongSampleSenderCapacity::Bounded {
            pcm_bytes: after, ..
        },
    ) = (baseline, exhausted)
    else {
        panic!("real bounded sender expected")
    };
    assert_eq!(before - after, u64::from(count) * 32);
    assert!(matches!(
        exhausted,
        SongSampleSenderCapacity::Bounded { resources: 0, .. }
    ));
    let extra = sample(0.5);
    for _ in 0..2 {
        let refused = rig
            .host
            .try_song_sample(key(999), Arc::clone(&extra))
            .unwrap_err();
        assert_eq!(refused.lease, key(999));
        assert!(Arc::ptr_eq(&extra, &refused.data));
        assert!(matches!(refused.error, SongSubmitError::Invalid(_)));
        assert_eq!(rig.host.song_sample_sender_capacity().unwrap(), exhausted);
    }
    rig.upload_ready(&keys);
    assert_eq!(rig.host.song_sample_sender_capacity().unwrap(), exhausted);
    assert!(keys
        .iter()
        .all(|key| rig.receipts.iter().any(|ack| matches!(ack,
        SongHostAck::SliceAccepted { lease, offset: 0 } if lease == key))));
    // Observation does not rewrite the independently retained physical report.
    assert_eq!(
        rig.receipts.iter().find_map(|ack| match ack {
            SongHostAck::CapacityReport(r) if *r == report => Some(*r),
            _ => None,
        }),
        Some(report)
    );
    rig.command(SongCommand::Mute(SongMute {
        epoch,
        instrument: 999,
        muted: true,
        frame: rig.frame,
    }));
    for _ in 0..8 {
        rig.tick();
    }
    assert!(rig
        .receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Rejected { epoch: e, .. } if *e == epoch)));
    assert_eq!(
        rig.host.song_sample_sender_capacity().unwrap(),
        exhausted,
        "generic rejection is not a full-key return"
    );
    rig.command(SongCommand::CancelPreparation(epoch));
    for _ in 0..512 {
        rig.tick();
        if keys
            .iter()
            .all(|key| rig.receipts.contains(&SongHostAck::LeaseReturned(*key)))
        {
            break;
        }
    }
    assert!(keys
        .iter()
        .all(|key| rig.receipts.contains(&SongHostAck::LeaseReturned(*key))));
    assert_eq!(rig.host.song_sample_sender_capacity().unwrap(), baseline);
}

#[test]
fn native_actual_control_ring_pressure_returns_original_then_upload_progresses() {
    let mut rig = Rig::new(false);
    let keys = rig.reserve_samples(SnapshotEpoch(720), 1);
    let mut admitted = 0;
    for request in 1..=u64::try_from(vactr::host::native::audio::CONTROL_CAPACITY + 1).unwrap() {
        let command = SongCommand::RequestClock(SongClockRequest {
            epoch: SnapshotEpoch(721),
            request,
        });
        match rig.host.try_song_command(command) {
            Ok(()) => admitted += 1,
            Err(refused) => {
                assert_eq!(refused.command, command);
                assert!(matches!(refused.error, SongSubmitError::Backpressure));
                break;
            }
        }
    }
    assert!(
        (1..=vactr::host::native::audio::CONTROL_CAPACITY).contains(&admitted),
        "actual ring also carries the provider's internal capacity request"
    );
    let original = sample(0.25);
    let refused = rig
        .host
        .try_song_sample(keys[0], Arc::clone(&original))
        .unwrap_err();
    assert_eq!(refused.lease, keys[0]);
    assert!(Arc::ptr_eq(&refused.data, &original));
    assert!(matches!(refused.error, SongSubmitError::Backpressure));
    // Drain real control records, retaining the exact refused PCM for retry.
    rig.tick();
    rig.host
        .try_song_sample(keys[0], Arc::clone(&original))
        .unwrap();
    rig.upload_ready(&keys);
    rig.command(SongCommand::CancelPreparation(keys[0].epoch));
    for _ in 0..512 {
        rig.tick();
        if rig.receipts.contains(&SongHostAck::LeaseReturned(keys[0])) {
            break;
        }
    }
    assert!(rig.receipts.contains(&SongHostAck::LeaseReturned(keys[0])));
}

fn pcm_candidate(count: usize, epoch: SnapshotEpoch) -> PreparedSong {
    let samples: BTreeMap<_, _> = (0..count)
        .map(|i| (format!("./p{i}.wav"), sample(0.25)))
        .collect();
    let steps = (0..count)
        .map(|i| format!("{{s {{sample ./p{i}.wav}}}}"))
        .collect::<Vec<_>>()
        .join(" ");
    let code = format!("song {{part [drums: [{steps}]] duration: 1}} tail-seconds: 0 > play-song");
    let factory = DecodedSongAssetFactory::new(samples, Default::default(), Default::default());
    let context = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 16,
            max_source_bytes: 100_000,
            max_banks: 16,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    let prepared = song::prepare_song(
        vactr::session::song::evaluate_song_candidate(&code, "samples.vact", 17, epoch, &context)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(prepared.snapshot().resource_count(), count);
    prepared
}

#[test]
fn complete_pcm_union_exact_sender_boundary_reaches_ready_and_one_over_fails_before_reserve() {
    let _guard = ARENA_TEST.lock().unwrap();
    let count = ring::INBOX_SLOTS;
    let mut rig = Rig::new(true);
    let baseline = rig.host.song_sample_sender_capacity().unwrap();
    let mut owner =
        SongHostPreparation::begin(pcm_candidate(count, SnapshotEpoch(730)), limits(rig.caps))
            .ok()
            .unwrap();
    let ready = rig.complete(&mut owner);
    assert_eq!(ready.routes().branches.len(), count);
    assert_eq!(ready.pools().len(), count);
    assert_eq!(
        ready.routes().required.template_slots,
        u32::try_from(count).unwrap()
    );
    assert_eq!(
        ready.routes().required.bus_slots,
        u32::try_from(count + 2).unwrap()
    );
    assert_eq!(
        ready
            .resources()
            .iter()
            .filter(|key| key.kind == SongResourceKind::Sample)
            .count(),
        count
    );
    assert!(matches!(
        rig.host.song_sample_sender_capacity().unwrap(),
        SongSampleSenderCapacity::Bounded { resources: 0, .. }
    ));
    let mut cleanup = SongHostPreparation::retire(ready);
    rig.cleanup(&mut cleanup);
    assert!(cleanup.take_cancelled().is_ok());
    assert_eq!(rig.host.song_sample_sender_capacity().unwrap(), baseline);
    let epoch = SnapshotEpoch(731);
    let mut over = SongHostPreparation::begin(pcm_candidate(count + 1, epoch), limits(rig.caps))
        .ok()
        .unwrap();
    let mut failure = None;
    for _ in 0..64 {
        if let Err(error) = over.submit(rig.host.as_mut()) {
            failure = Some(error);
            break;
        }
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                if ack.epoch() == epoch {
                    over.receive(ack).unwrap();
                }
            }
        }
    }
    assert_eq!(failure.unwrap().code, vm::fail::FailCode::HostUnavailable);
    assert_eq!(rig.host.song_sample_sender_capacity().unwrap(), baseline);
    assert!(!rig.receipts.iter().any(|ack| matches!(ack,
        SongHostAck::ResourceReady { epoch: e, .. } | SongHostAck::Ready(e) if *e == epoch)));
    rig.cleanup(&mut over);
    assert_eq!(
        over.take_cancelled().unwrap().snapshot().resource_count(),
        count + 1
    );
}

struct RaceHost {
    inner: Box<dyn AudioHost>,
    state: Rc<RefCell<HostState>>,
    foreign: SongLeaseKey,
    fired: std::cell::Cell<bool>,
}
impl AudioHost for RaceHost {
    fn song_sample_sender_capacity(&self) -> Result<SongSampleSenderCapacity, vm::fail::Failure> {
        let observed = self.inner.song_sample_sender_capacity()?;
        if !self.fired.replace(true) {
            WasmAudioHost(Rc::clone(&self.state))
                .try_song_sample(self.foreign, sample(0.5))
                .unwrap();
        }
        Ok(observed)
    }
    fn try_song_sample(
        &mut self,
        key: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), vactr::host::caps::SongSampleRefusal> {
        self.inner.try_song_sample(key, data)
    }
    fn try_song_command(
        &mut self,
        command: SongCommand,
    ) -> Result<(), vactr::host::caps::SongCommandRefusal> {
        self.inner.try_song_command(command)
    }
    fn try_song_graph(
        &mut self,
        key: SongLeaseKey,
        graph: &vactr::host::caps::GraphHandle,
    ) -> Result<(), SongSubmitError> {
        self.inner.try_song_graph(key, graph)
    }
    fn song_clock(&self) -> Result<SongHostClock, vm::fail::Failure> {
        self.inner.song_clock()
    }
    fn drain(&mut self, output: &mut Vec<HostMsg>) {
        self.inner.drain(output);
    }
    fn now(&self) -> f64 {
        self.inner.now()
    }
    fn send(&mut self, event: vactr::host::wire::AudioEvent) {
        self.inner.send(event);
    }
    fn control(&mut self, command: vactr::host::wire::SlotControl) {
        self.inner.control(command);
    }
    fn post(&mut self, command: vactr::host::wire::CtlMsg) {
        self.inner.post(command);
    }
    fn swap_graph(&mut self, graph: vactr::host::caps::GraphHandle) {
        self.inner.swap_graph(graph);
    }
    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.inner.install_sample(id, data);
    }
    fn retire_sample(&mut self, id: u32) {
        self.inner.retire_sample(id);
    }
    fn analysis(&self) -> vactr::host::caps::HostSigs {
        self.inner.analysis()
    }
}
#[test]
fn non_atomic_sender_observation_race_fails_once_and_cleans_every_accepted_key() {
    let _guard = ARENA_TEST.lock().unwrap();
    let mut rig = Rig::new(true);
    let foreign = rig.reserve_samples_from(SnapshotEpoch(740), 1, 200)[0];
    let state = match &rig.backend {
        Backend::Arena { state, .. } => Rc::clone(state),
        _ => unreachable!(),
    };
    let inner = std::mem::replace(&mut rig.host, Box::new(NoopHost));
    rig.host = Box::new(RaceHost {
        inner,
        state,
        foreign,
        fired: std::cell::Cell::new(false),
    });
    let epoch = SnapshotEpoch(741);
    let mut owner =
        SongHostPreparation::begin(pcm_candidate(ring::INBOX_SLOTS, epoch), limits(rig.caps))
            .ok()
            .unwrap();
    let mut failures = 0;
    for _ in 0..2048 {
        if let Err(error) = owner.submit(rig.host.as_mut()) {
            assert_eq!(error.code, vm::fail::FailCode::HostUnavailable);
            failures += 1;
        }
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                if ack.epoch() == epoch {
                    if let Err(other) = owner.receive(ack) {
                        assert!(matches!(other, SongHostAck::SliceAccepted { .. }));
                    }
                }
            }
        }
        if owner.progress() == SongPreparationProgress::Cancelled {
            break;
        }
    }
    assert_eq!(failures, 1, "permanent refusal must not retry indefinitely");
    assert_eq!(
        owner.take_cancelled().unwrap().snapshot().resource_count(),
        ring::INBOX_SLOTS
    );
    let returned = rig
        .receipts
        .iter()
        .filter(|ack| matches!(ack, SongHostAck::LeaseReturned(key) if key.epoch == epoch))
        .count();
    assert!(returned >= ring::INBOX_SLOTS - 1);
    assert!(
        matches!(rig.host.song_sample_sender_capacity().unwrap(), SongSampleSenderCapacity::Bounded { resources, .. } if resources == u32::try_from(ring::INBOX_SLOTS - 1).unwrap())
    );
    rig.command(SongCommand::CancelPreparation(foreign.epoch));
    for _ in 0..512 {
        rig.tick();
        if rig.receipts.contains(&SongHostAck::LeaseReturned(foreign)) {
            break;
        }
    }
    assert!(rig.receipts.contains(&SongHostAck::LeaseReturned(foreign)));
    assert!(
        matches!(rig.host.song_sample_sender_capacity().unwrap(), SongSampleSenderCapacity::Bounded { resources, .. } if resources == u32::try_from(ring::INBOX_SLOTS).unwrap())
    );
}
