//! End-to-end song freezing and playback for tuning and strum patterns.

use std::collections::BTreeMap;
use std::sync::Arc;

use vactr::dsp::{arena::StoreKind, caps::CapabilitySet};
use vactr::host::caps::{
    AudioHost, GraphHandle, HostSigs, SampleData, SongCommandRefusal, SongHostPreparation,
    SongPreparationLimits, SongPreparationProgress, SongReadyBundle,
};
use vactr::host::native::audio::{AudioSide, NativeAudioHost};
use vactr::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use vactr::pattern::query::TimeSpan;
use vactr::sched::song::{SongTransport, SongTransportState};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::routing::{SongAudioEvent, SongCommand, SongHostAck, SongHostClock};
use vactr::song::snapshot::{FrozenControl, FrozenSongEvent};
use vactr::song::{self, PreparedSong, ResolvedNote, SnapshotEpoch, SongLimits};
use vactr::value::ratio::Ratio64;
use vactr::vm::fail::Failure;

const RATE: u32 = 48_000;

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
    let candidate = vactr::session::song::evaluate_song_candidate(
        code,
        "tuning.vact",
        42,
        SnapshotEpoch(1600),
        &context,
    )
    .expect("candidate evaluation");
    song::prepare_song(candidate).expect("song preparation")
}

fn span(begin: Ratio64, end: Ratio64) -> TimeSpan {
    TimeSpan::new(begin, end).expect("valid span")
}

fn frozen(code: &str) -> Vec<FrozenSongEvent> {
    let mut prepared = candidate(code);
    let duration = prepared.snapshot().duration();
    prepared
        .query(span(Ratio64::ZERO, duration), &SongLimits::default())
        .expect("frozen song events")
}

struct NativeRig {
    host: Box<dyn AudioHost>,
    side: Box<AudioSide>,
    frame: u64,
}

impl NativeRig {
    fn new() -> Self {
        let caps = CapabilitySet::native();
        let config = vactr::host::song_profile::song_engine_config(
            RATE as f32,
            vactr::host::native::audio::MAX_BLOCK,
            caps,
            StoreKind::NativeArc,
            2,
        )
        .expect("song engine config");
        let (host, side) = NativeAudioHost::headless_with_config(config, 1024)
            .expect("headless native audio host");
        Self {
            host: Box::new(host),
            side: Box::new(side),
            frame: 0,
        }
    }

    fn tick(&mut self) -> Vec<HostMsg> {
        let mut output = [0.0; 1024];
        self.side.render(&mut output, 2);
        self.frame += 512;
        let mut messages = Vec::new();
        self.host.drain(&mut messages);
        messages
    }

    fn complete(&mut self, owner: &mut SongHostPreparation) -> SongReadyBundle {
        for _ in 0..2048 {
            owner
                .submit(self.host.as_mut())
                .expect("submit preparation");
            for message in self.tick() {
                if let HostMsg::Song(ack) = message {
                    if let Err(unhandled) = owner.receive(ack) {
                        assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
                    }
                }
            }
            if owner.progress() == SongPreparationProgress::Ready {
                return owner.take_ready().expect("ready bundle");
            }
        }
        panic!(
            "song preparation did not become ready: {:?}",
            owner.progress()
        );
    }
}

struct CapturingHost<'a> {
    inner: &'a mut dyn AudioHost,
    commands: &'a mut Vec<SongCommand>,
}

impl AudioHost for CapturingHost<'_> {
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

fn playback_events(code: &str) -> Vec<SongAudioEvent> {
    let prepared = candidate(code);
    let mut rig = NativeRig::new();
    let mut owner = SongHostPreparation::begin(prepared, limits(CapabilitySet::native()))
        .map_err(|refusal| refusal.failure)
        .expect("begin song preparation");
    let ready = rig.complete(&mut owner);
    let duration = ready.prepared().snapshot().duration();
    let mut transport = SongTransport::new(ready, rig.frame + 512, SongLimits::default())
        .map_err(|refusal| refusal.failure)
        .expect("song transport");
    let mut commands = Vec::new();
    transport
        .submit_activation(&mut CapturingHost {
            inner: rig.host.as_mut(),
            commands: &mut commands,
        })
        .expect("submit activation");
    for _ in 0..16384 {
        transport
            .advance(
                &mut CapturingHost {
                    inner: rig.host.as_mut(),
                    commands: &mut commands,
                },
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: RATE,
                },
                span(Ratio64::ZERO, duration),
            )
            .expect("advance transport");
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                transport.receive(ack).expect("transport receipt");
            }
        }
        if transport.state() == SongTransportState::Ended {
            break;
        }
    }
    assert_eq!(transport.state(), SongTransportState::Ended);
    commands
        .into_iter()
        .filter_map(|command| match command {
            SongCommand::Event(event) => Some(event),
            _ => None,
        })
        .collect()
}

fn has_tuning(row: &FrozenSongEvent) -> bool {
    row.controls.iter().any(|(key, value)| {
        vactr::value::intern::name_of_kw(*key).as_ref() == "tuning"
            && matches!(value, FrozenControl::List(_))
    })
}

fn freq(event: &SongAudioEvent) -> f32 {
    let id = vactr::dsp::controls::row("freq").expect("freq control").ctl;
    match event.event.controls().iter().find(|(key, _)| *key == id) {
        Some((_, vactr::host::wire::Ctl::Const(value))) => *value,
        other => panic!("missing constant frequency: {other:?}"),
    }
}

#[test]
fn tuned_and_untuned_note_freeze_and_play_with_expected_frequency() {
    let tuned = "song {part [lead: {s :analog > tune {edo 19} > note :d > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
    let rows = frozen(tuned);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].note, Some(ResolvedNote::Int(63)));
    assert!(has_tuning(&rows[0]));
    let events = playback_events(tuned);
    assert_eq!(events.len(), 1);
    let expected = (vactr::sched::commit::note_to_freq(60.0) * (3.0_f64 / 19.0).exp2()) as f32;
    assert!(freq(&events[0]).to_bits().abs_diff(expected.to_bits()) <= 1);

    let untuned = "song {part [lead: {s :analog > note :d > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
    let rows = frozen(untuned);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].note, Some(ResolvedNote::Int(62)));
    let events = playback_events(untuned);
    assert_eq!(events.len(), 1);
    assert_eq!(
        freq(&events[0]).to_bits(),
        (vactr::sched::commit::note_to_freq(62.0) as f32).to_bits()
    );
}

#[test]
fn tuned_and_untuned_strum_songs_freeze_and_play_three_events() {
    let untuned = "song {part [lead: {s :analog > chord [:c :maj] > strum 1/32 > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
    let rows = frozen(untuned);
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().map(|row| row.note).collect::<Vec<_>>(),
        vec![
            Some(ResolvedNote::Int(60)),
            Some(ResolvedNote::Int(64)),
            Some(ResolvedNote::Int(67))
        ]
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.whole.expect("whole").begin)
            .collect::<Vec<_>>(),
        vec![
            Ratio64::ZERO,
            Ratio64::new(1, 32).unwrap(),
            Ratio64::new(2, 32).unwrap()
        ]
    );
    let events = playback_events(untuned);
    assert_eq!(events.len(), 3);

    let tuned = "song {part [lead: {s :analog > tune {edo 19} > chord [:c :maj] > strum 1/32 > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
    let rows = frozen(tuned);
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().map(|row| row.note).collect::<Vec<_>>(),
        vec![
            Some(ResolvedNote::Int(60)),
            Some(ResolvedNote::Int(66)),
            Some(ResolvedNote::Int(71))
        ]
    );
    let events = playback_events(tuned);
    assert_eq!(events.len(), 3);
    let anchor = vactr::sched::commit::note_to_freq(60.0);
    for (event, step) in events.iter().zip([0.0_f64, 6.0, 11.0]) {
        let expected = (anchor * (step / 19.0).exp2()) as f32;
        assert!(freq(event).to_bits().abs_diff(expected.to_bits()) <= 1);
    }
}
