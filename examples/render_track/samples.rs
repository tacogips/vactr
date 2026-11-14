//! Real native decoding/playback and canonical root admission regressions.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(format!(
                "renderer-samples-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn options(&self) -> Options {
        Options::parse([
            self.0.join("score.vact").to_string_lossy().into_owned(),
            self.0.join("out.wav").to_string_lossy().into_owned(),
            "--seconds".into(),
            "0.1".into(),
            "--tail-seconds".into(),
            "0".into(),
            "--sample-rate".into(),
            "8000".into(),
        ])
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn tone(float: bool) -> Vec<u8> {
    let frames = 4410;
    let mut header = wav::header(frames * if float { 2 } else { 1 }, 44100).unwrap();
    if float {
        header[20..22].copy_from_slice(&3u16.to_le_bytes());
        header[28..32].copy_from_slice(&(44100u32 * 8).to_le_bytes());
        header[32..34].copy_from_slice(&8u16.to_le_bytes());
        header[34..36].copy_from_slice(&32u16.to_le_bytes());
    }
    let mut bytes = header.to_vec();
    for frame in 0..frames {
        let sample = ((frame as f64 / 44100.0 * 440.0 * std::f64::consts::TAU).sin() * 0.4) as f32;
        for _ in 0..2 {
            if float {
                bytes.extend(sample.to_le_bytes());
            } else {
                bytes.extend(((sample * 32767.0).round() as i16).to_le_bytes());
            }
        }
    }
    bytes
}

#[test]
fn score_relative_pcm_and_float_samples_render_audibly() {
    for float in [false, true] {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("tone.wav"), tone(float)).unwrap();
        let options = fixture.options();
        let source = "use-bpm 120\ns {sample ./tone.wav} > gain 0.2 > d1\n";
        std::fs::write(&options.source, source).unwrap();
        let summary = render(&options, source).unwrap();
        assert_eq!(summary["events"], 1);
        assert_eq!(summary["frames"], 800);
        assert!(summary["rms"].as_f64().unwrap() > 0.005);
        assert!(summary["peak"].as_f64().unwrap() < 1.0);
        assert!(options.output.is_file());
    }
}

#[test]
fn missing_samples_and_paths_outside_roots_fail_without_output() {
    let fixture = Fixture::new();
    let options = fixture.options();
    let error = render(&options, "s {sample ./missing.wav} > d1\n").unwrap_err();
    assert!(error.contains("host-unavailable"), "{error}");
    assert!(!options.output.exists());
    // A valid, readable WAV outside cwd/project roots must fail admission.
    let outside = Fixture(std::env::temp_dir().join(format!(
        "vactr-outside-samples-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir_all(&outside.0).unwrap();
    let path = outside.0.join("tone.wav");
    std::fs::write(&path, tone(true)).unwrap();
    let source = format!("s {{sample {}}} > gain 0.2 > d1\n", path.display());
    let error = render(&options, &source).unwrap_err();
    assert!(error.contains("host-unavailable"), "{error}");
    assert!(!options.output.exists());
}

#[test]
fn source_loader_clones_keep_loaded_file_sample_paths() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.0.join("parts")).unwrap();
    std::fs::write(fixture.0.join("parts/tone.wav"), tone(true)).unwrap();
    std::fs::write(
        fixture.0.join("parts/part.vact"),
        "s {sample ./tone.wav} > gain 0.2 > d1\n",
    )
    .unwrap();
    let options = fixture.options();
    let source = "load ./parts/part.vact\n";
    std::fs::write(&options.source, source).unwrap();
    let summary = render(&options, source).unwrap();
    assert_eq!(summary["events"], 1);
    assert!(summary["rms"].as_f64().unwrap() > 0.005);
}
