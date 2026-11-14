//! Complete finite composition through actual original Native/Arena ownership.
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
    engine::{Engine, SongStagingConfig},
    ring::{self, ByteInbox, EngineIo, EventRing, Garbage, SpscRing},
};
use vactr::host::{
    caps::{
        AudioHost, SampleData, SongHostPreparation, SongPreparationLimits, SongPreparationProgress,
        SongReadyBundle,
    },
    native::audio::{AudioSide, NativeAudioHost},
    wire::HostMsg,
};
use vactr::song::{
    assets::{DecodedSongAssetFactory, SongAssetLimits},
    routing::*,
    PreparedSong, SnapshotEpoch, SongLimits,
};

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
    last: [f32; 1024],
    receipts: Vec<SongHostAck>,
}
impl Rig {
    fn new(bytes: bool) -> Self {
        let caps = if bytes {
            CapabilitySet::browser()
        } else {
            CapabilitySet::native()
        };
        let (host, backend): (Box<dyn AudioHost>, Backend) = if !bytes {
            let cfg = vactr::host::song_profile::song_engine_config(
                48000.,
                vactr::host::native::audio::MAX_BLOCK,
                caps,
                StoreKind::NativeArc,
                2,
            )
            .unwrap();
            let (host, side) = NativeAudioHost::headless_with_config(cfg, 1024).unwrap();
            (Box::new(host), Backend::Native(Box::new(side)))
        } else {
            browser_abi::outbox_clear();
            browser_abi::fix_outbox(0);
            let cfg = vactr::host::song_profile::song_engine_config(
                48000.,
                512,
                caps,
                StoreKind::Arena { bytes: 1_000_000 },
                2,
            )
            .unwrap();
            let analysis_slots = u32::try_from(cfg.analysis_cells).unwrap();
            let control_slots =
                vactr::ns::insts::INST_CELL_BASE + vactr::ns::insts::INST_CELL_COUNT;
            let mut engine = Engine::with_config(cfg);
            let (acks, received) = SpscRing::split(1024);
            engine
                .configure_song_staging_for_transport(
                    SongStagingConfig {
                        preparations: 64,
                        leases: 256,
                        branches: 256,
                        control_slots,
                        analysis_slots,
                        native_pcm_bytes: 1_000_000,
                        critical_receipts: 2,
                    },
                    &acks,
                )
                .unwrap();
            let (_, events) = EventRing::split(ring::EVENT_CAPACITY);
            let (garbage, returned) = SpscRing::split(1024);
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
                    cells: AtomicCells::new(usize::try_from(control_slots).unwrap()),
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
            last: [0.; 1024],
            receipts: Vec::new(),
        }
    }
    fn tick(&mut self) -> Vec<HostMsg> {
        let mut out = [0.; 1024];
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
                        512,
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
        self.frame += 512;
        self.last = out;
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
}

use vactr::host::caps::{GraphHandle, HostSigs, SongCommandRefusal};
use vactr::host::wire::{AudioEvent, CtlMsg, SlotControl};
use vactr::pattern::TimeSpan;
use vactr::sched::song::{SongTransport, SongTransportState};
use vactr::song::snapshot::FrozenSongEvent;
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::vm::fail::Failure;

const RATE: u32 = 48000;
const CANONICAL: &str = include_str!("../examples/song-mode.vact");
static ARENA_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct SongIntegrationEvidence {
    arrangement_frames: u64,
    tail_frames: u64,
    native_browser_event_digest: String,
}
fn candidate(code: &str) -> PreparedSong {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let context = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 128,
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
    song::prepare_song(
        vactr::session::song::evaluate_song_candidate(
            code,
            "end-to-end.vact",
            42,
            SnapshotEpoch(1600),
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}
fn span(begin: Ratio64, end: Ratio64) -> TimeSpan {
    TimeSpan::new(begin, end).unwrap()
}

fn query_identity(prepared: &mut PreparedSong) -> Vec<FrozenSongEvent> {
    let duration = prepared.snapshot().duration();
    let limits = SongLimits::default();
    let mut full = prepared
        .query(span(Ratio64::ZERO, duration), &limits)
        .unwrap();
    full.sort_by(|a, b| a.handle.cmp(&b.handle));
    // Reordered, fractional partitions; reconstruct only caller-clipped parts.
    let cuts = [
        Ratio64::ZERO,
        Ratio64::new(1, 8).unwrap(),
        Ratio64::new(3, 2).unwrap(),
        Ratio64::new(9, 4).unwrap(),
        Ratio64::from_int(5),
        duration,
    ];
    let mut union = BTreeMap::<song::EventHandle, FrozenSongEvent>::new();
    for pair in cuts.windows(2).rev() {
        for row in prepared.query(span(pair[0], pair[1]), &limits).unwrap() {
            let expected = full
                .iter()
                .find(|other| other.handle == row.handle)
                .unwrap();
            let mut unclipped = row.clone();
            unclipped.part = expected.part;
            assert_eq!(
                &unclipped, expected,
                "all original identity/whole/note/control fields"
            );
            union
                .entry(row.handle.clone())
                .and_modify(|prior| {
                    prior.part = span(
                        prior.part.begin.min(row.part.begin),
                        prior.part.end.max(row.part.end),
                    );
                })
                .or_insert(row);
        }
    }
    assert_eq!(union.into_values().collect::<Vec<_>>(), full);
    full
}
struct RecordedHost<'a> {
    inner: &'a mut dyn AudioHost,
    commands: &'a mut Vec<SongCommand>,
}
impl AudioHost for RecordedHost<'_> {
    fn try_song_command(&mut self, command: SongCommand) -> Result<(), SongCommandRefusal> {
        self.inner.try_song_command(command)?;
        self.commands.push(command);
        Ok(())
    }
    fn song_clock(&self) -> Result<SongHostClock, Failure> {
        self.inner.song_clock()
    }
    fn send(&mut self, event: AudioEvent) {
        self.inner.send(event);
    }
    fn control(&mut self, command: SlotControl) {
        self.inner.control(command);
    }
    fn post(&mut self, command: CtlMsg) {
        self.inner.post(command);
    }
    fn drain(&mut self, output: &mut Vec<HostMsg>) {
        self.inner.drain(output);
    }
    fn now(&self) -> f64 {
        self.inner.now()
    }
    fn swap_graph(&mut self, graph: GraphHandle) {
        self.inner.swap_graph(graph);
    }
    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.inner.install_sample(id, data);
    }
    fn retire_sample(&mut self, id: u32) {
        self.inner.retire_sample(id);
    }
    fn analysis(&self) -> HostSigs {
        self.inner.analysis()
    }
}
struct Playback {
    commands: Vec<SongCommand>,
    pcm: Vec<f32>,
    endpoints: SongEndpoints,
    keys: Vec<SongLeaseKey>,
}
fn play(rig: &mut Rig, ready: SongReadyBundle, activation: u64) -> Playback {
    let keys = ready.resources().to_vec();
    for key in &keys {
        if matches!(
            key.kind,
            SongResourceKind::Instrument
                | SongResourceKind::PrivateFx
                | SongResourceKind::Track
                | SongResourceKind::Master
                | SongResourceKind::Sample
        ) {
            assert!(rig.receipts.contains(&SongHostAck::ResourceReady {
                epoch: key.epoch,
                resource: key.resource
            }));
        }
    }
    assert!(rig
        .receipts
        .contains(&SongHostAck::Ready(ready.prepared().epoch())));
    let duration = ready.prepared().snapshot().duration();
    let mut core = SongTransport::new(ready, activation, SongLimits::default())
        .ok()
        .unwrap();
    let endpoints = core.endpoints();
    let mut commands = Vec::new();
    core.submit_activation(&mut RecordedHost {
        inner: rig.host.as_mut(),
        commands: &mut commands,
    })
    .unwrap();
    let mut pcm = Vec::new();
    for _ in 0..16384 {
        core.advance(
            &mut RecordedHost {
                inner: rig.host.as_mut(),
                commands: &mut commands,
            },
            SongHostClock {
                frame: rig.frame,
                sample_rate: RATE,
            },
            span(Ratio64::ZERO, duration),
        )
        .unwrap();
        let before = rig.frame;
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                core.receive(ack).unwrap();
            }
        }
        if before >= activation && before < endpoints.tail_deadline {
            let frames = usize::try_from((endpoints.tail_deadline - before).min(512)).unwrap();
            pcm.extend_from_slice(&rig.last[..frames * 2]);
        }
        if core.state() == SongTransportState::Ended {
            break;
        }
    }
    assert_eq!(
        core.state(),
        SongTransportState::Ended,
        "actual transport: {:?}",
        core.failure()
    );
    assert!(core.failure().is_none());
    assert_eq!(
        core.applied_activation(),
        Some(SongActivation {
            epoch: SnapshotEpoch(1600),
            frame: activation
        })
    );
    assert_eq!(
        pcm.len(),
        usize::try_from(endpoints.tail_deadline - activation).unwrap() * 2
    );
    assert!(
        pcm.iter().any(|sample| sample.abs() > 1e-5),
        "actual finite PCM"
    );
    for key in &keys {
        assert_eq!(
            rig.receipts
                .iter()
                .filter(|ack| **ack == SongHostAck::LeaseReturned(*key))
                .count(),
            1,
            "every original full key returned once: {key:?}"
        );
    }
    assert_eq!(core.take_retired().unwrap().revision(), 42);
    assert!(core.take_retired().is_err());
    rig.tick();
    assert!(
        rig.last.iter().all(|sample| sample.abs() < 1e-7),
        "automatic finite silence"
    );
    Playback {
        commands,
        pcm,
        endpoints,
        keys,
    }
}

type Revisions = BTreeMap<song::PartRevision, song::PartRevision>;

// Parse the complete ordered source_trace group, not generic control framing.
fn source_frames(words: &[u32]) -> Option<(usize, Vec<Vec<u32>>)> {
    let mut offset: usize = 0;
    let mut payloads = Vec::new();
    for role in 0..8 {
        let header = words.get(offset..offset.checked_add(6)?)?;
        if header != [5, role, 5, header[3], 5, 0] {
            return None;
        }
        let count = usize::try_from(header[3]).ok()?;
        if matches!(role, 2 | 3 | 4 | 7) && count != 2 {
            return None;
        }
        if role == 5 && count != 1 {
            return None;
        }
        if role == 1 && count % 2 != 0 {
            return None;
        }
        offset += 6;
        let end = offset.checked_add(count.checked_mul(2)?)?;
        let payload = words.get(offset..end)?;
        if payload.chunks_exact(2).any(|pair| pair[0] != 2) {
            return None;
        }
        payloads.push(payload.chunks_exact(2).map(|pair| pair[1]).collect());
        offset = end;
    }
    Some((offset, payloads))
}
fn mapped_producer(words: &[u32], revisions: &Revisions) -> Vec<u32> {
    assert_eq!(words.len() % 2, 0);
    let mut output = Vec::with_capacity(words.len());
    let mut offset = 0;
    while offset < words.len() {
        if let Some((length, mut payloads)) = source_frames(&words[offset..]) {
            payloads[1] = mapped_producer(&payloads[1], revisions);
            let revision =
                song::PartRevision(u64::from(payloads[7][0]) | (u64::from(payloads[7][1]) << 32));
            let mapped = revisions
                .get(&revision)
                .expect("source revision belongs to actual copied DAG")
                .0;
            payloads[7] = vec![
                u32::try_from(mapped & u64::from(u32::MAX)).unwrap(),
                u32::try_from(mapped >> 32).unwrap(),
            ];
            for (role, payload) in payloads.into_iter().enumerate() {
                output.extend([
                    5,
                    u32::try_from(role).unwrap(),
                    5,
                    u32::try_from(payload.len()).unwrap(),
                    5,
                    0,
                ]);
                for word in payload {
                    output.extend([2, word]);
                }
            }
            offset += length;
        } else {
            output.extend_from_slice(&words[offset..offset + 2]);
            offset += 2;
        }
    }
    assert_eq!(output.len(), words.len());
    output
}
fn compare_handle(a: &song::EventHandle, b: &song::EventHandle, revisions: &Revisions) {
    assert_eq!(revisions.get(&a.revision()), Some(&b.revision()));
    assert_eq!(a.track(), b.track());
    assert_eq!(a.placement(), b.placement());
    assert_eq!(a.tone(), b.tone());
    assert_eq!(a.occurrence().cycle, b.occurrence().cycle);
    assert_eq!(a.occurrence().onset, b.occurrence().onset);
    assert_eq!(
        mapped_producer(&a.occurrence().producer_ordinals, revisions),
        b.occurrence().producer_ordinals
    );
}
fn compare_pattern(a: &song::snapshot::FrozenPattern, b: &song::snapshot::FrozenPattern) {
    assert_eq!(a.id, b.id);
    assert_eq!(a.named_buses, b.named_buses);
    assert_eq!(a.source_uses, b.source_uses);
    assert_eq!(a.index_timing(), b.index_timing());
    assert_eq!(a.sources.len(), b.sources.len());
    for (a, b) in a.sources.iter().zip(&b.sources) {
        assert_eq!(a.root_part, b.root_part);
        assert_eq!(a.track, b.track);
        assert_eq!(a.family, b.family);
    }
    assert_eq!(a.families.len(), b.families.len());
    for ((asound, aroute), (bsound, broute)) in a.families.iter().zip(&b.families) {
        assert_eq!(asound, bsound);
        assert_eq!(aroute.instrument, broute.instrument);
        assert_eq!(aroute.sample, broute.sample);
    }
}
fn revision_bijection(a: &PreparedSong, b: &PreparedSong) -> Revisions {
    use song::snapshot::{FrozenEdit as E, FrozenPartNode as N};
    let a = a.snapshot().routing();
    let b = b.snapshot().routing();
    assert_eq!(a.root_part, b.root_part);
    assert_eq!(a.parts.len(), b.parts.len());
    let mut revisions = Revisions::new();
    let mut reverse = Revisions::new();
    for (a, b) in a.parts.iter().zip(&b.parts) {
        if let Some(previous) = revisions.insert(a.revision, b.revision) {
            assert_eq!(previous, b.revision);
        }
        if let Some(previous) = reverse.insert(b.revision, a.revision) {
            assert_eq!(previous, a.revision);
        }
    }
    assert_eq!(revisions.len(), reverse.len());
    for (a, b) in a.parts.iter().zip(&b.parts) {
        assert_eq!(a.duration, b.duration);
        assert_eq!(a.tracks, b.tracks);
        match (&a.node, &b.node) {
            (N::Capture(a), N::Capture(b)) => {
                assert_eq!(a.len(), b.len());
                for ((at, ap), (bt, bp)) in a.iter().zip(b) {
                    assert_eq!(at, bt);
                    compare_pattern(ap, bp);
                }
            }
            (N::Sequence(a), N::Sequence(b)) => assert_eq!(a, b),
            (
                N::Repeat {
                    child: a,
                    count: ac,
                    seed_mode: as_,
                },
                N::Repeat {
                    child: b,
                    count: bc,
                    seed_mode: bs,
                },
            ) => {
                assert_eq!(a, b);
                assert_eq!(ac, bc);
                assert_eq!(as_, bs);
            }
            (
                N::Edit {
                    source: a,
                    edit: ae,
                },
                N::Edit {
                    source: b,
                    edit: be,
                },
            ) => {
                assert_eq!(a, b);
                match (ae, be) {
                    (
                        E::Replace {
                            track: a,
                            payload: ap,
                        },
                        E::Replace {
                            track: b,
                            payload: bp,
                        },
                    ) => {
                        assert_eq!(a, b);
                        compare_pattern(ap, bp);
                    }
                    (
                        E::Transform {
                            track: a,
                            family: af,
                            cutoff: ac,
                            payload: ap,
                        },
                        E::Transform {
                            track: b,
                            family: bf,
                            cutoff: bc,
                            payload: bp,
                        },
                    ) => {
                        assert_eq!(a, b);
                        assert_eq!(af, bf);
                        assert_eq!(revisions.get(ac), Some(bc));
                        compare_pattern(ap, bp);
                    }
                    (E::Delete(a), E::Delete(b)) => compare_handle(a, b, &revisions),
                    (
                        E::Overwrite {
                            track: a,
                            region: ar,
                            payload: ap,
                        },
                        E::Overwrite {
                            track: b,
                            region: br,
                            payload: bp,
                        },
                    ) => {
                        assert_eq!(a, b);
                        assert_eq!(ar, br);
                        compare_pattern(ap, bp);
                    }
                    (
                        E::InstrumentFx {
                            track: a,
                            family: af,
                            template: at,
                        },
                        E::InstrumentFx {
                            track: b,
                            family: bf,
                            template: bt,
                        },
                    ) => {
                        assert_eq!(a, b);
                        assert_eq!(af, bf);
                        assert_eq!(at, bt);
                    }
                    _ => panic!("different immutable edit contracts"),
                }
            }
            _ => panic!("different immutable Part topology"),
        }
    }
    revisions
}

fn backend_parity(
    code: &str,
    expected_duration: Ratio64,
    tail: Ratio64,
) -> SongIntegrationEvidence {
    let mut native = Rig::new(false);
    let mut native_prepared = candidate(code);
    assert_eq!(native_prepared.snapshot().duration(), expected_duration);
    let native_rows = query_identity(&mut native_prepared);
    let mut arena = Rig::new(true);
    let mut arena_prepared = candidate(code);
    let arena_rows = query_identity(&mut arena_prepared);
    let revisions = revision_bijection(&native_prepared, &arena_prepared);
    assert_eq!(native_rows.len(), arena_rows.len());
    // These are separately issued revisions; compare full logical producer and
    // payload fields while each complete handle was already tested above.
    for (a, b) in native_rows.iter().zip(&arena_rows) {
        assert_eq!(a.track, b.track);
        compare_handle(&a.handle, &b.handle, &revisions);
        assert_eq!(a.whole, b.whole);
        assert_eq!(a.part, b.part);
        assert_eq!(a.instrument, b.instrument);
        assert_eq!(a.note, b.note);
        assert_eq!(a.controls, b.controls);
        assert_eq!(a.route, b.route);
    }
    let mut native_owner = SongHostPreparation::begin(native_prepared, limits(native.caps))
        .ok()
        .unwrap();
    let native_ready = native.complete(&mut native_owner);
    let mut arena_owner = SongHostPreparation::begin(arena_prepared, limits(arena.caps))
        .ok()
        .unwrap();
    let arena_ready = arena.complete(&mut arena_owner);
    let activation = native.frame.max(arena.frame) + 4096;
    let native_result = play(&mut native, native_ready, activation);
    let arena_result = play(&mut arena, arena_ready, activation);
    assert_eq!(native_result.endpoints, arena_result.endpoints);
    assert_eq!(native_result.keys, arena_result.keys);
    assert_eq!(
        native_result.commands, arena_result.commands,
        "complete accepted musical PODs"
    );
    assert_eq!(
        native_result.pcm, arena_result.pcm,
        "real same-rate DSP output"
    );
    let limits = SongLimits::default();
    let arrangement_frames = limits
        .frames_at(
            expected_duration.checked_mul(Ratio64::from_int(2)).unwrap(),
            RATE,
        )
        .unwrap();
    let tail_frames = limits.frames_at(tail, RATE).unwrap();
    assert_eq!(
        native_result.endpoints.arrangement,
        activation + arrangement_frames
    );
    assert_eq!(
        native_result.endpoints.tail_deadline,
        activation + arrangement_frames + tail_frames
    );
    let mut bytes = Vec::new();
    for command in &native_result.commands {
        let mut encoded = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(*command).encode(&mut encoded);
        assert!(n > 0);
        bytes.extend_from_slice(&encoded[..n]);
    }
    // Supplementary deterministic checksum follows direct full equality.
    let checksum = bytes.iter().fold(0xcbf29ce484222325u64, |sum, byte| {
        (sum ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    SongIntegrationEvidence {
        arrangement_frames,
        tail_frames,
        native_browser_event_digest: format!("{checksum:016x}"),
    }
}

#[test]
fn canonical_nested_parts_complete_exact_forty_eight_seconds_plus_eight_second_tail() {
    let _guard = ARENA_TEST.lock().unwrap();
    let evidence = backend_parity(CANONICAL, Ratio64::from_int(24), Ratio64::from_int(8));
    assert_eq!(evidence.arrangement_frames, 2_304_000);
    assert_eq!(evidence.tail_frames, 384_000);
    assert_eq!(evidence.native_browser_event_digest.len(), 16);
}

const EDITED: &str = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\ninst sibling freq: float = 880:\n\tsin-osc freq > * amp\nfn base:\n\tpart [drums: {s :tone > chord [:c :maj] > gain 0.2} hats: {s :sibling > note 72 > gain 0.2}] duration: 4\nfn inner p:\n\tlet events {part-events p :drums 0 4}\n\tlet handle {{first events} :handle}\n\tlet changed {delete-event p handle}\n\toverwrite-region changed :drums 1 2 {s :tone > note 67 > gain 0.2}\nfn outer p:\n\tinner p\nlet original {base & []}\nlet changed {outer original}\nsong {sequence [original {part-repeat changed 2}]} bpm: 120 cycle-beats: 4 seed: 42 tail-seconds: 0 > play-song";

fn segment(prepared: &mut PreparedSong, begin: i64, end: i64) -> Vec<FrozenSongEvent> {
    let mut rows = prepared
        .query(
            span(Ratio64::from_int(begin), Ratio64::from_int(end)),
            &SongLimits::default(),
        )
        .unwrap();
    rows.sort_by(|a, b| a.handle.cmp(&b.handle));
    rows
}

#[test]
fn nested_edits_delete_one_chord_tone_overwrite_and_preserve_repeated_siblings() {
    let _guard = ARENA_TEST.lock().unwrap();
    let mut prepared = candidate(EDITED);
    assert_eq!(prepared.snapshot().duration(), Ratio64::from_int(12));
    let original = segment(&mut prepared, 0, 4);
    let drums = intern_kw("drums");
    let hats = intern_kw("hats");
    let original_drums: Vec<_> = original.iter().filter(|row| row.track == drums).collect();
    let original_hats: Vec<_> = original.iter().filter(|row| row.track == hats).collect();
    assert_eq!(original_drums.len(), 12);
    assert_eq!(original_hats.len(), 4);
    let first_chord: Vec<_> = original_drums
        .iter()
        .filter(|row| row.part.begin == Ratio64::ZERO)
        .copied()
        .collect();
    assert_eq!(first_chord.len(), 3);
    let mut original_notes: Vec<_> = first_chord
        .iter()
        .map(|row| row.note.unwrap().to_f64())
        .collect();
    original_notes.sort_by(f64::total_cmp);
    assert_eq!(original_notes, vec![60.0, 64.0, 67.0]);
    let removed_tone = first_chord
        .iter()
        .map(|row| row.handle.tone())
        .min()
        .unwrap();
    for offset in [4, 8] {
        let changed = segment(&mut prepared, offset, offset + 4);
        let changed_drums: Vec<_> = changed.iter().filter(|row| row.track == drums).collect();
        let changed_hats: Vec<_> = changed.iter().filter(|row| row.track == hats).collect();
        assert_eq!(changed_drums.len(), 9);
        assert_eq!(changed_hats.len(), 4);
        let at = |local| Ratio64::from_int(offset + local);
        let kept: Vec<_> = changed_drums
            .iter()
            .filter(|row| row.part.begin == at(0))
            .collect();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|row| row.handle.tone() != removed_tone));
        let mut kept_notes: Vec<_> = kept.iter().map(|row| row.note.unwrap().to_f64()).collect();
        kept_notes.sort_by(f64::total_cmp);
        assert_eq!(kept_notes, vec![64.0, 67.0]);
        let overwritten: Vec<_> = changed_drums
            .iter()
            .filter(|row| row.part.begin == at(1))
            .collect();
        assert_eq!(overwritten.len(), 1);
        assert_eq!(overwritten[0].note.unwrap().to_f64(), 67.0);
        assert_eq!(overwritten[0].whole, Some(span(at(1), at(2))));
        for local in [2, 3] {
            let mut notes: Vec<_> = changed_drums
                .iter()
                .filter(|row| row.part.begin == at(local))
                .map(|row| row.note.unwrap().to_f64())
                .collect();
            notes.sort_by(f64::total_cmp);
            assert_eq!(notes, original_notes);
        }
        for sibling in changed_hats {
            let local = sibling
                .part
                .begin
                .checked_sub(Ratio64::from_int(offset))
                .unwrap();
            let source = original_hats
                .iter()
                .find(|row| row.part.begin == local)
                .unwrap();
            assert_eq!(sibling.instrument, source.instrument);
            assert_eq!(sibling.note, source.note);
            assert_eq!(sibling.controls, source.controls);
            assert_eq!(sibling.route, source.route);
            assert_eq!(sibling.handle.tone(), source.handle.tone());
            assert_eq!(
                sibling
                    .whole
                    .unwrap()
                    .begin
                    .checked_sub(Ratio64::from_int(offset))
                    .unwrap(),
                source.whole.unwrap().begin
            );
            assert_eq!(
                sibling
                    .whole
                    .unwrap()
                    .end
                    .checked_sub(Ratio64::from_int(offset))
                    .unwrap(),
                source.whole.unwrap().end
            );
        }
    }
    // Re-query the same original placement after all descendant edits/queries.
    assert_eq!(segment(&mut prepared, 0, 4), original);
    let evidence = backend_parity(EDITED, Ratio64::from_int(12), Ratio64::ZERO);
    assert_eq!(evidence.arrangement_frames, 1_152_000);
    assert_eq!(evidence.tail_frames, 0);
    assert_eq!(evidence.native_browser_event_digest.len(), 16);
}
