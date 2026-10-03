//! Real CLI process entrypoints and complete finite output, never fabricated ACKs.
#![cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
use std::{
    path::PathBuf,
    process::{Command, Output},
};
use vactr::cli::args::{parse, Command as CliCommand};
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vactr-song-cli-{stamp}-{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn score(&self, code: &str) -> PathBuf {
        let path = self.0.join("score.vact");
        std::fs::write(&path, code).unwrap();
        path
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_vactr"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const SONG: &str = "song {part [tone: {s :analog > gain 0.1}] duration: 1/7} bpm: 120 tail-seconds: 1/3 > play-song";
#[test]
fn render_arguments_validate_native_rate_and_complete_paths() {
    let args = |xs: &[&str]| xs.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        parse(&args(&["render", "a.vact", "a.wav", "--sample-rate=8000"])).unwrap(),
        CliCommand::Render {
            source: "a.vact".into(),
            output: "a.wav".into(),
            sample_rate: 8000
        }
    );
    for xs in [
        vec!["render", "a"],
        vec!["render", "a", "b", "--cycles", "1"],
        vec!["render", "a", "b", "--sample-rate", "7999"],
        vec!["render", "a", "b", "--host", "noop"],
        vec!["render", "a", "b", "--sample-rate=NaN"],
    ] {
        assert!(parse(&args(&xs)).is_err());
    }
}
#[test]
fn actual_render_writes_complete_fractional_song_and_empty_song() {
    let dir = Directory::new();
    dir.score(SONG);
    let result = dir.run(&["render", "score.vact", "song.wav", "--sample-rate", "8000"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let wav = std::fs::read(dir.0.join("song.wav")).unwrap();
    // 1/7 cycle * 2 seconds/cycle + 1/3 second, rounded once to 4952 frames.
    assert_eq!(wav.len(), 44 + 4952 * 4);
    assert!(wav[44..].iter().any(|byte| *byte != 0));
    assert!(String::from_utf8_lossy(&result.stdout).contains("state Ended"));
    dir.score("song {sequence []} tail-seconds: 0 > play-song");
    assert!(dir
        .run(&["render", "score.vact", "empty.wav"])
        .status
        .success());
    assert_eq!(
        std::fs::metadata(dir.0.join("empty.wav")).unwrap().len(),
        44
    );
}
#[test]
fn render_failures_preserve_existing_output_and_reject_mixed_entrypoints() {
    let dir = Directory::new();
    std::fs::write(dir.0.join("output.wav"), b"keep").unwrap();
    for code in [
        "s :analog > d1".to_owned(),
        format!("s :analog > d1\n{SONG}"),
        format!("{SONG}\n{SONG}"),
        format!("{SONG}\n/ 1 0"),
    ] {
        dir.score(&code);
        assert_eq!(
            dir.run(&["render", "score.vact", "output.wav"])
                .status
                .code(),
            Some(3)
        );
        assert_eq!(std::fs::read(dir.0.join("output.wav")).unwrap(), b"keep");
    }
    dir.score(SONG);
    let original = std::fs::read(dir.0.join("score.vact")).unwrap();
    assert_eq!(
        dir.run(&["render", "score.vact", "score.vact"])
            .status
            .code(),
        Some(3)
    );
    assert_eq!(std::fs::read(dir.0.join("score.vact")).unwrap(), original);
}
#[test]
fn finite_run_rejects_noop_and_manual_cycles_while_legacy_cycles_remain() {
    let dir = Directory::new();
    dir.score(SONG);
    let result = dir.run(&["run", "score.vact", "--host", "noop"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("native output clock"));
    assert_eq!(
        dir.run(&["run", "score.vact", "--host", "noop", "--cycles", "1"])
            .status
            .code(),
        Some(2)
    );
    dir.score("s :analog > d1");
    let result = dir.run(&["run", "score.vact", "--host", "noop", "--cycles", "1"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn generated_parts_file_renders_exact_fifty_seconds_without_cycles() {
    let dir = Directory::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = dir.0.join("generated.wav");
    let result = Command::new(env!("CARGO_BIN_EXE_vactr"))
        .current_dir(&root)
        .args(["render", "examples/song-mode/generated-parts.vact"])
        .arg(&output)
        .args(["--sample-rate", "8000"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let wav = std::fs::read(output).unwrap();
    assert_eq!(wav.len(), 44 + 50 * 8000 * 4);
    assert!(wav[44..].iter().any(|byte| *byte != 0));
    assert!(String::from_utf8_lossy(&result.stdout).contains("rendered 400000 frames"));
}
