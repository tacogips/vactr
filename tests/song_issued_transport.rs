//! Requirement-level coverage for issued Ready playback and export.
#![cfg(all(feature = "host-native", not(target_arch = "wasm32")))]

use std::collections::BTreeMap;
use std::path::PathBuf;
use vactr::dsp::arena::StoreKind;
use vactr::dsp::caps::CapabilitySet;
use vactr::host::caps::{
    AudioHost, SongHostPreparation, SongPreparationLimits, SongPreparationProgress,
};
use vactr::host::native::audio::{NativeAudioHost, MAX_BLOCK};
use vactr::host::wire::HostMsg;
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::export::{export_song, SongExportOptions};
use vactr::song::routing::{prepare_routes, SongHostCapacities};
use vactr::song::snapshot::FrozenSongEvent;
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;

const PROGRAM: &str = "bus :room:\n\tgain 0.75\nfn make:\n\tpart [drums: {s :analog > chord [:c :maj] > gain 0.2} hats: {s :analog > note {choose 60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75} > gain 0.2}] duration: 4\nfn make-slice:\n\tpart [hats: {s :analog > slow 64}] duration: 1\nfn remove-one p:\n\tlet events {part-events p :drums 0 4}\n\tlet handle {{first events} :handle}\n\tlet changed {delete-event p handle}\n\toverwrite-region changed :drums 1 2 {s :analog > note {choose 60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75} > gain 0.2}\nfn edit p:\n\tremove-one p\nfn indexed p:\n\tslice {beat -> p} 2 [0]\nlet original {make & []}\nlet edited {edit original}\nlet filtered {transform-instrument edited :drums :analog {p -> lpf p 900}}\nlet effected {instrument-fx filtered :drums :analog :room}\nlet slice-source {make-slice & []}\nlet sliced {transform-instrument slice-source :hats :analog indexed}\nlet arrangement sequence [sliced {part-repeat effected 2 seed-mode: :same} {part-repeat effected 2 seed-mode: :vary}]\nsong arrangement bpm: 120 cycle-beats: 4 seed: 42 tail-seconds: 2 > play-song";

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vactr-song-issued-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn candidate() -> PreparedSong {
    static NEXT_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(9000);
    let assets = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let context = vactr::session::song::CandidateBuildCtx {
        assets: &assets,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 1_000_000,
            max_source_files: 32,
            max_source_bytes: 100_000,
            max_banks: 32,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    prepare_song(
        vactr::session::song::evaluate_song_candidate(
            PROGRAM,
            "issued-transport.vact",
            42,
            SnapshotEpoch(NEXT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}

fn full_query(song: &mut PreparedSong) -> Vec<FrozenSongEvent> {
    let duration = song.snapshot().duration();
    song.query(
        vactr::pattern::TimeSpan::new(Ratio64::ZERO, duration).unwrap(),
        &SongLimits::default(),
    )
    .unwrap()
}

#[test]
fn issued_ready_transport_edits_repeats_slice_and_exports_exactly() {
    let mut prepared = candidate();
    let route_plan = prepare_routes(
        prepared.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &SongHostCapacities {
            sample_rate: 8000,
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
    )
    .unwrap();
    let reserved_generations = route_plan
        .branches
        .iter()
        .map(|route| route.reserved_generations)
        .sum::<u32>();
    let measured_bus_slots =
        u32::try_from(route_plan.tracks.len()).unwrap() + 1 + reserved_generations;
    assert_eq!(route_plan.required.bus_slots, measured_bus_slots);
    let available_bus_slots = u32::try_from(vactr::host::song_profile::SONG_BUS_SLOTS - 1).unwrap();
    eprintln!(
        "capacity evidence: tracks={}, reserved_generations={}, required_bus_slots={}, available={}",
        route_plan.tracks.len(),
        reserved_generations,
        measured_bus_slots,
        available_bus_slots
    );
    assert!(
        measured_bus_slots <= available_bus_slots,
        "fixture requires {measured_bus_slots} bus slots; song profile provides {available_bus_slots}"
    );
    let rows = full_query(&mut prepared);
    let drums = intern_kw("drums");
    let hats = intern_kw("hats");
    let at = |track, onset| {
        rows.iter()
            .filter(|row| row.track == track && row.part.begin == Ratio64::from_int(onset))
            .collect::<Vec<_>>()
    };
    let deleted_chord = at(drums, 1);
    assert_eq!(deleted_chord.len(), 2, "one chord tone was deleted");
    let overwritten = at(drums, 2);
    assert_eq!(overwritten.len(), 1);
    assert!((60.0..=75.0).contains(&overwritten[0].note.unwrap().to_f64()));
    assert_eq!(
        at(hats, 0).len(),
        1,
        "the Slice-sourced track remains present"
    );

    for (first_start, second_start, should_match) in [(1, 5, true), (9, 13, false)] {
        let signature = |start| {
            let mut events = rows
                .iter()
                .filter(|row| {
                    row.track == hats
                        && row.part.begin >= Ratio64::from_int(start)
                        && row.part.begin < Ratio64::from_int(start + 4)
                })
                .map(|row| {
                    (
                        row.part
                            .begin
                            .checked_sub(Ratio64::from_int(start))
                            .unwrap(),
                        row.note,
                        row.controls.clone(),
                    )
                })
                .collect::<Vec<_>>();
            events.sort_by(|a, b| a.0.cmp(&b.0));
            events
        };
        let first_signature = signature(first_start);
        let second_signature = signature(second_start);
        assert_eq!(
            first_signature == second_signature,
            should_match,
            "repeat signatures at {first_start} and {second_start}: {first_signature:?} vs {second_signature:?}"
        );
    }

    let directory = TempDir::new();
    let first_path = directory.file("first.wav");
    let second_path = directory.file("second.wav");
    let options = |output: PathBuf| SongExportOptions {
        output,
        sample_rate: 8000,
    };
    let first = export_song(candidate(), &options(first_path.clone())).unwrap();
    let second = export_song(candidate(), &options(second_path.clone())).unwrap();
    assert_eq!(
        first.final_state,
        vactr::sched::song::SongTransportState::Ended
    );
    assert_eq!(
        first.total_frames,
        first.arrangement_frames + first.tail_frames
    );
    let prepared = candidate();
    let snapshot = prepared.snapshot();
    let expected_frames = SongLimits::default()
        .frames_at(
            snapshot
                .duration()
                .checked_mul(snapshot.settings().seconds_per_cycle().unwrap())
                .unwrap()
                .checked_add(snapshot.settings().tail_seconds)
                .unwrap(),
            8000,
        )
        .unwrap();
    assert_eq!(first.total_frames, expected_frames);
    assert_eq!(
        std::fs::metadata(&first_path).unwrap().len(),
        44 + first.total_frames * 4
    );
    assert_eq!(
        first.tail_frames,
        SongLimits::default()
            .frames_at(Ratio64::from_int(2), 8000)
            .unwrap()
    );
    assert_eq!(first.total_frames, second.total_frames);
    assert_eq!(first.settings, second.settings);
    assert_eq!(
        std::fs::read(first_path).unwrap(),
        std::fs::read(second_path).unwrap()
    );
}

#[test]
fn preparation_refuses_before_upload_when_route_work_is_exhausted() {
    let caps = CapabilitySet::native();
    let config = vactr::host::song_profile::song_engine_config(
        8000.,
        MAX_BLOCK,
        caps,
        StoreKind::NativeArc,
        2,
    )
    .unwrap();
    let (mut host, mut side) = NativeAudioHost::headless_with_config(config, 4096).unwrap();
    let mut owner = SongHostPreparation::begin(
        candidate(),
        SongPreparationLimits {
            capabilities: caps,
            song: SongLimits::default(),
            max_resources: 256,
            max_pending_records: 4096,
            max_graph_bytes: 1_000_000,
            max_work: 1,
        },
    )
    .unwrap_or_else(|refusal| panic!("preparation refused: {}", refusal.failure));
    let mut silence = [0.0; MAX_BLOCK * 2];
    let mut observed_failure = None;
    for _ in 0..16 {
        if let Err(failure) = owner.submit(&mut host) {
            observed_failure = Some(failure);
            break;
        }
        assert!(!matches!(
            owner.progress(),
            SongPreparationProgress::Uploading
                | SongPreparationProgress::AwaitingReady
                | SongPreparationProgress::Ready
        ));
        side.render(&mut silence, 2);
        let mut messages = Vec::new();
        host.drain(&mut messages);
        for message in messages {
            if let HostMsg::Song(ack) = message {
                if let Err(unhandled) = owner.receive(ack) {
                    panic!("preparation returned an unowned acknowledgement: {unhandled:?}");
                }
            }
        }
    }
    let failure = observed_failure.expect("route preparation should exhaust its work budget");
    assert_eq!(failure.code, vactr::vm::fail::FailCode::FuelExhausted);
    assert!(!matches!(
        owner.progress(),
        SongPreparationProgress::Uploading
            | SongPreparationProgress::AwaitingReady
            | SongPreparationProgress::Ready
    ));
}
