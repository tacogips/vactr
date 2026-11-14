//! Actual owned finite transport output; no manual cycle rendering substitute.
#![cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
use std::{collections::BTreeMap, path::PathBuf};
use vactr::sched::song::SongTransportState;
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::export::{export_song, SongExportOptions};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vactr-song-export-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn candidate(code: &str, file: &str) -> PreparedSong {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let ctx = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 1_000_000,
            max_source_files: 64,
            max_source_bytes: 100_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    prepare_song(
        vactr::session::song::evaluate_song_candidate(code, file, 77, SnapshotEpoch(701), &ctx)
            .unwrap(),
    )
    .unwrap()
}
const TONE:&str="inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > gain 0.2}] duration: 1/7} tail-seconds: 1/3 > play-song";
#[test]
fn fractional_song_and_tail_write_exact_partial_block() {
    let dir = Directory::new();
    let output = dir.path("fractional.wav");
    let prepared = candidate(TONE, "export.vact");
    let settings = prepared.snapshot().settings();
    let duration = prepared.snapshot().duration();
    let seconds = duration
        .checked_mul(settings.seconds_per_cycle().unwrap())
        .unwrap();
    let limits = SongLimits::default();
    let expected = limits
        .frames_at(seconds.checked_add(settings.tail_seconds).unwrap(), 8000)
        .unwrap();
    let report = export_song(
        prepared,
        &SongExportOptions {
            output: output.clone(),
            sample_rate: 8000,
        },
    )
    .unwrap();
    assert_eq!(report.total_frames, expected);
    assert_eq!(
        report.arrangement_frames,
        limits.frames_at(seconds, 8000).unwrap()
    );
    assert_eq!(report.tail_frames, expected - report.arrangement_frames);
    assert_ne!(expected % vactr::host::native::audio::MAX_BLOCK as u64, 0);
    assert_eq!(report.duration, duration);
    assert_eq!(report.settings, settings);
    assert_eq!(report.revision, 77);
    assert_eq!(report.epoch, SnapshotEpoch(701));
    assert_eq!(report.final_state, SongTransportState::Ended);
    let wav = std::fs::read(output).unwrap();
    assert_eq!(wav.len() as u64, 44 + expected * 4);
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(
        u32::from_le_bytes(wav[40..44].try_into().unwrap()) as u64,
        expected * 4
    );
    assert!(wav[44..].iter().any(|byte| *byte != 0));
}
#[test]
fn empty_and_silent_scores_need_no_audibility() {
    let dir = Directory::new();
    for (name, code, frames) in [
        ("empty", "song {sequence []} tail-seconds: 0 > play-song", 0),
        (
            "silent",
            "song {part [tone: {s nil}] duration: 1/4} tail-seconds: 0 > play-song",
            4000,
        ),
    ] {
        let output = dir.path(name);
        let report = export_song(
            candidate(code, "silent.vact"),
            &SongExportOptions {
                output: output.clone(),
                sample_rate: 8000,
            },
        )
        .unwrap();
        assert_eq!(report.total_frames, frames);
        assert_eq!(report.final_state, SongTransportState::Ended);
        let bytes = std::fs::read(output).unwrap();
        assert_eq!(bytes.len() as u64, 44 + frames * 4);
        assert!(bytes[44..].iter().all(|b| *b == 0));
    }
}
#[test]
fn repeated_export_is_bit_exact() {
    let dir = Directory::new();
    let code="inst tone freq: float = 440:\n\tsin-osc freq > * amp\nlet a {part [tone: {s :tone > n 60 > gain 0.2}] duration: 1/4}\nlet b {part [tone: {s :tone > n 67 > gain 0.2}] duration: 1/4}\nsong {sequence [{part-repeat a 2} b]} tail-seconds: 0 > play-song";
    let a = dir.path("a.wav");
    let b = dir.path("b.wav");
    let first = export_song(
        candidate(code, "repeat.vact"),
        &SongExportOptions {
            output: a.clone(),
            sample_rate: 8000,
        },
    )
    .unwrap();
    let second = export_song(
        candidate(code, "repeat.vact"),
        &SongExportOptions {
            output: b.clone(),
            sample_rate: 8000,
        },
    )
    .unwrap();
    assert_eq!(first.total_frames, 12000);
    assert_eq!(first.seed, second.seed);
    assert_eq!(std::fs::read(a).unwrap(), std::fs::read(b).unwrap());
}
#[test]
fn source_alias_and_failed_export_preserve_existing_output() {
    let dir = Directory::new();
    let source = dir.path("score.vact");
    std::fs::write(&source, TONE).unwrap();
    let before = std::fs::read(&source).unwrap();
    assert!(export_song(
        candidate(TONE, source.to_str().unwrap()),
        &SongExportOptions {
            output: source.clone(),
            sample_rate: 8000
        }
    )
    .is_err());
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let output = dir.path("existing.wav");
    std::fs::write(&output, b"previous complete output").unwrap();
    assert!(export_song(
        candidate(TONE, "export.vact"),
        &SongExportOptions {
            output: output.clone(),
            sample_rate: 0
        }
    )
    .is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"previous complete output");
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 2);
}
#[test]
fn riff_overflow_leaves_existing_output_untouched() {
    let dir = Directory::new();
    let output = dir.path("existing.wav");
    std::fs::write(&output, b"old").unwrap();
    let code = "song {part [tone: {s nil}] duration: 1000000000} tail-seconds: 0 > play-song";
    assert!(export_song(
        candidate(code, "long.vact"),
        &SongExportOptions {
            output: output.clone(),
            sample_rate: 8000
        }
    )
    .is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"old");
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}
