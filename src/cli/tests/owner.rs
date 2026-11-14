use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::cli::args::HostChoice;
use crate::cli::owner::{AudioSessionEvent, SessionOwner};
use crate::session::codec::decode_as;
use crate::session::protocol::{ServerMsg, TempoBody};

fn fresh_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "vactr-session-owner-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temporary owner directory");
    path
}

fn tempo_epoch(text: &str) -> Option<String> {
    decode_as::<ServerMsg>(text)
        .ok()
        .and_then(|env| match env.body {
            ServerMsg::Tempo(TempoBody {
                transport: Some(sample),
                ..
            }) => Some(sample.epoch),
            _ => None,
        })
}

#[test]
fn owner_answers_probe_routes_discontinuity_and_joins() {
    let cwd = fresh_dir();
    let owner = SessionOwner::spawn(HostChoice::Noop, cwd.clone()).expect("session owner");
    let (tx, rx) = mpsc::channel();
    let id = owner.connect(tx);
    owner.send(
        id,
        r#"{"v":1,"seq":9,"kind":"clock-probe","body":{"page_send":12.5}}"#.into(),
    );
    let reply = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("clock-probe reply");
    let value: serde_json::Value = serde_json::from_str(&reply).expect("valid JSON");
    assert_eq!(value["kind"], "clock-probe");
    assert_eq!(value["re"], 9);
    assert_eq!(value["body"]["page_send"], 12.5);
    assert_eq!(value["body"]["latency_kind"], "unavailable");
    let initial = value["body"]["epoch"].as_str().expect("epoch").to_string();

    owner.send(
        id,
        r#"{"v":1,"seq":10,"kind":"subscribe","body":{"telemetry":true,"levels":false,"diagnostics":false}}"#.into(),
    );
    owner.audio_event(AudioSessionEvent::RouteChanged);
    let deadline = Instant::now() + Duration::from_secs(2);
    let next_epoch = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "next tempo telemetry did not arrive");
        if let Ok(text) = rx.recv_timeout(remaining) {
            if let Some(epoch) = tempo_epoch(&text) {
                if epoch != initial {
                    break epoch;
                }
            }
        }
    };
    assert_ne!(next_epoch, initial);

    let started = Instant::now();
    owner.shutdown();
    assert!(started.elapsed() < Duration::from_secs(2));
    std::fs::remove_dir_all(cwd).expect("remove temporary owner directory");
}
