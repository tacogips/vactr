//! Requirement-level finite song rendering through the production CLI.
#![cfg(all(feature = "host-native", not(target_arch = "wasm32")))]

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Output},
};
use vactr::{
    dsp::arena::StoreKind,
    dsp::caps::CapabilitySet,
    host::{
        caps::{AudioHost, SongHostPreparation, SongPreparationLimits, SongPreparationProgress},
        native::audio::{NativeAudioHost, MAX_BLOCK},
        wire::HostMsg,
    },
    song::{
        assets::{DecodedSongAssetFactory, SongAssetLimits},
        prepare_song, PreparedSong, SnapshotEpoch, SongLimits,
    },
};

const PROGRAM: &str = include_str!("../examples/song-mode/requirement-song.vact");
const SAMPLE_RATE: u32 = 8000;
const MAX_WORK: u32 = 1_000_000;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vactr-song-requirement-{}-{}-{}",
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

fn candidate_from(program: &str) -> PreparedSong {
    static NEXT_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(9_000_000);
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
            program,
            "requirement-song.vact",
            42,
            SnapshotEpoch(NEXT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}

fn candidate() -> PreparedSong {
    candidate_from(PROGRAM)
}

fn render(output: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vactr"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["render", "examples/song-mode/requirement-song.vact"])
        .arg(output)
        .args(["--sample-rate", "8000"])
        .output()
        .unwrap()
}

fn assert_warning_only_stderr(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty()
            || stderr
                .lines()
                .all(|line| line.starts_with("vactr: warning:")),
        "unexpected CLI stderr: {stderr}"
    );
}

fn expected_frames(song: &PreparedSong) -> u64 {
    let snapshot = song.snapshot();
    SongLimits::default()
        .frames_at(
            snapshot
                .duration()
                .checked_mul(snapshot.settings().seconds_per_cycle().unwrap())
                .unwrap()
                .checked_add(snapshot.settings().tail_seconds)
                .unwrap(),
            SAMPLE_RATE,
        )
        .unwrap()
}

#[test]
fn requirement_song_renders_through_cli_to_a_complete_terminating_wav() {
    let directory = TempDir::new();
    let first_path = directory.file("first.wav");
    let second_path = directory.file("second.wav");
    let program_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/song-mode/requirement-song.vact");
    let program = std::fs::read_to_string(program_path).unwrap();
    let expected = expected_frames(&candidate_from(&program));

    let first = render(&first_path);
    assert!(
        first.status.success(),
        "CLI render failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_warning_only_stderr(&first);
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(
        stdout.contains(&format!("rendered {expected} frames at {SAMPLE_RATE} Hz")),
        "CLI did not report the expected frame count {expected}: {stdout}"
    );
    assert!(
        stdout.contains("state Ended"),
        "CLI did not terminate: {stdout}"
    );

    let first_wav = std::fs::read(&first_path).unwrap();
    assert_eq!(first_wav.len() as u64, 44 + expected * 4);
    assert!(first_wav[44..].iter().any(|byte| *byte != 0));

    let second = render(&second_path);
    assert!(
        second.status.success(),
        "second CLI render failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_warning_only_stderr(&second);
    assert_eq!(first_wav, std::fs::read(second_path).unwrap());
}

fn reached_upload(owner: &SongHostPreparation) -> bool {
    matches!(
        owner.progress(),
        SongPreparationProgress::Uploading
            | SongPreparationProgress::AwaitingReady
            | SongPreparationProgress::Ready
    )
}

fn probe(max_work: u32) -> bool {
    let caps = CapabilitySet::native();
    let config = vactr::host::song_profile::song_engine_config(
        SAMPLE_RATE as f32,
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
            max_work,
        },
    )
    .unwrap_or_else(|refusal| panic!("preparation refused: {}", refusal.failure));
    let mut silence = [0.0; MAX_BLOCK * 2];

    for _ in 0..64 {
        if reached_upload(&owner) {
            return true;
        }
        if let Err(failure) = owner.submit(&mut host) {
            if reached_upload(&owner) {
                return true;
            }
            assert_eq!(
                failure.code,
                vactr::vm::fail::FailCode::FuelExhausted,
                "unexpected preparation error for max_work={max_work}: {failure}"
            );
            return false;
        }
        if reached_upload(&owner) {
            return true;
        }

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
    panic!("preparation made no terminal progress within 64 pumps for max_work={max_work}");
}

#[test]
fn requirement_song_route_work_fits_default_allowance() {
    assert!(!probe(1), "one unit of work unexpectedly reached upload");
    assert!(
        probe(MAX_WORK),
        "default work allowance did not reach upload"
    );

    let mut failing = 1;
    let mut passing = MAX_WORK;
    while passing - failing > 1 {
        let midpoint = failing + (passing - failing) / 2;
        if probe(midpoint) {
            passing = midpoint;
        } else {
            failing = midpoint;
        }
    }

    assert!(probe(passing), "minimum passing work value must pass");
    assert!(!probe(passing - 1), "one-less work value must fail");
    eprintln!("route work evidence: minimal_max_work={passing} allowance={MAX_WORK}");
    assert!(passing <= MAX_WORK);
}
