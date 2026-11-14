//! Live legacy isolation follows actual finite ownership and full-key retirement.
use std::{collections::BTreeMap, rc::Rc};
use vactr::dsp::{arena::StoreKind, caps::CapabilitySet, engine::EngineConfig};
use vactr::host::{
    caps::{Hosts, SongPreparationLimits},
    native::audio::NativeAudioHost,
};
use vactr::sched::song::SongTransportState;
use vactr::session::{
    protocol::ApplySongBody, ClientMsg, Envelope, ServerMsg, Session, SessionConfig,
};
use vactr::song::{
    assets::{DecodedSongAssetFactory, SongAssetLimits},
    SongLimits,
};

fn session_pair() -> (Session, vactr::host::native::audio::AudioSide) {
    session_pair_with_bus_slots(12)
}
fn session_pair_with_bus_slots(
    bus_slots: usize,
) -> (Session, vactr::host::native::audio::AudioSide) {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 4;
    let mut engine = EngineConfig::new(
        &caps,
        32768.,
        vactr::host::native::audio::MAX_BLOCK,
        StoreKind::NativeArc,
    );
    engine.bus_slots = bus_slots;
    let (audio, side) = NativeAudioHost::headless_with_config(engine, 64).unwrap();
    let mut hosts = Hosts::noop();
    hosts.audio = Box::new(audio);
    let mut config = SessionConfig::new(caps);
    config.song_assets = Some(Rc::new(DecodedSongAssetFactory::new(
        BTreeMap::new(),
        BTreeMap::new(),
        Default::default(),
    )));
    let mut session = Session::new(config, hosts);
    session
        .set_song_asset_limits(SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 16,
            max_source_bytes: 100_000,
            max_banks: 16,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        })
        .unwrap();
    session
        .set_song_preparation_limits(SongPreparationLimits {
            capabilities: caps,
            song: SongLimits::default(),
            max_resources: 128,
            max_pending_records: 4096,
            max_graph_bytes: 65536,
            max_work: 8_000_000,
        })
        .unwrap();
    (session, side)
}

fn live(
    session: &mut Session,
    code: &str,
    revision: u64,
) -> vactr::session::protocol::EvalResultBody {
    let messages = session.apply_from(
        17,
        Envelope::new(
            revision,
            None,
            ClientMsg::Eval(vactr::session::protocol::EvalBody {
                file: format!("live-{revision}.vact"),
                code: code.into(),
                span: None,
                doc_revision: revision,
                edit_epoch: 0,
            }),
        ),
    );
    messages
        .into_iter()
        .find_map(|message| match message.env.body {
            ServerMsg::EvalResult(body) => Some(body),
            _ => None,
        })
        .expect("actual incremental evaluation reply")
}
fn rejected(session: &mut Session, code: &str, revision: u64) {
    let reply = live(session, code, revision);
    assert!(
        reply
            .diagnostics
            .iter()
            .any(|diag| diag.code == "beyond-capability"
                && diag.message.contains("finite song owns resources")),
        "expected actual ownership refusal for {code}: {reply:?}"
    );
    assert!(!session
        .runtime()
        .slots()
        .iter()
        .any(|slot| slot.bound.is_some() || slot.pending.is_some()));
}
fn step(
    session: &mut Session,
    side: &mut vactr::host::native::audio::AudioSide,
    frame: &mut u64,
) -> (Vec<vactr::session::Outgoing>, [f32; 512]) {
    let messages = session.tick_routed(*frame as f64 / 32768.);
    let mut pcm = [0.; 512];
    side.render(&mut pcm, 2);
    *frame += 256;
    (messages, pcm)
}
fn apply(session: &mut Session) {
    let code =
        "song {part [tone: {s :analog > gain 0.1}] duration: 1/8} tail-seconds: 1/8 > play-song";
    let messages = session.apply_from(
        17,
        Envelope::new(
            1,
            None,
            ClientMsg::ApplySong(ApplySongBody {
                file: "finite.vact".into(),
                code: code.into(),
                doc_revision: 1,
                edit_epoch: 0,
            }),
        ),
    );
    assert!(
        messages.is_empty(),
        "actual initial candidate response: {messages:?}"
    );
}
#[test]
fn actual_song_ownership_rejects_live_clock_and_legacy_work_until_retirement() {
    let (mut session, mut side) = session_pair();
    let (mut twin, mut twin_side) = session_pair();
    apply(&mut session);
    apply(&mut twin);
    let original_tempo = session.runtime().clock().tempo();
    let original_clock = session.runtime().clock().source();
    rejected(&mut session, "use-bpm 90", 2); // actual preparing owner
    let mut frame = 0;
    let mut twin_frame = 0;
    let mut applied = None;
    let mut catalog = Vec::new();
    for _ in 0..128 {
        let (messages, pcm) = step(&mut session, &mut side, &mut frame);
        let (_, reference) = step(&mut twin, &mut twin_side, &mut twin_frame);
        assert_eq!(
            pcm, reference,
            "preparation refusal must preserve actual audio"
        );
        for message in messages {
            match message.env.body {
                ServerMsg::SongCandidateApplied(body) => applied = Some(body),
                ServerMsg::SongTransportState(body) if !body.instruments.is_empty() => {
                    catalog = body.instruments
                }
                ServerMsg::SongCandidateFailed(body) => panic!("song failed: {body:?}"),
                _ => {}
            }
        }
        if applied.is_some() {
            break;
        }
    }
    let activation = applied.expect("actual Native Applied");
    assert!(!catalog.is_empty(), "actual frozen family inventory");
    let mut audible = false;
    for (index, command) in [
        "use-bpm 90",
        "use-cycle 3",
        "use-clock :internal",
        "s :analog > d1",
        "once {s :analog}",
        "at 0 {s :analog > d2}",
    ]
    .iter()
    .enumerate()
    {
        rejected(&mut session, command, 10 + u64::try_from(index).unwrap());
        assert_eq!(session.runtime().clock().tempo(), original_tempo);
        assert_eq!(session.runtime().clock().source(), original_clock);
        let (messages, pcm) = step(&mut session, &mut side, &mut frame);
        let (_, reference) = step(&mut twin, &mut twin_side, &mut twin_frame);
        assert_eq!(pcm, reference, "live refusal changed PCM for {command}");
        audible |= pcm.iter().any(|value| value.abs() > 1e-6);
        for message in messages {
            if let ServerMsg::SongTransportState(body) = message.env.body {
                assert_eq!(body.epoch, activation.epoch);
                assert_eq!(body.instruments, catalog);
            }
        }
    }
    let midi = live(&mut session, "midi-clock-out true", 30);
    assert!(
        midi.diagnostics.iter().all(|diag| diag.severity != "error"),
        "output-only MIDI: {midi:?}"
    );
    let mut checked_draining = false;
    for _ in 0..128 {
        if session.runtime().song_state() == Some(SongTransportState::Draining) && !checked_draining
        {
            rejected(&mut session, "at 0 {s :analog > d2}", 31);
            checked_draining = true;
        }
        let (_, pcm) = step(&mut session, &mut side, &mut frame);
        let (_, reference) = step(&mut twin, &mut twin_side, &mut twin_frame);
        assert_eq!(pcm, reference, "draining refusal changed actual tail");
        if session.runtime().song_state() == Some(SongTransportState::Ended) {
            break;
        }
    }
    assert!(audible && checked_draining);
    assert_eq!(
        session.runtime().song_state(),
        Some(SongTransportState::Ended)
    );
    assert_eq!(twin.runtime().song_state(), Some(SongTransportState::Ended));
    // Ended is published only after actual guarded full-key resource returns.
    for (index, command) in [
        "use-bpm 90",
        "use-cycle 3",
        "use-clock :internal",
        "once {s :analog}",
        "at 0 {s :analog > d2}",
        "s :analog > d1",
    ]
    .iter()
    .enumerate()
    {
        let reply = live(&mut session, command, 40 + u64::try_from(index).unwrap());
        assert!(
            reply
                .diagnostics
                .iter()
                .all(|diag| diag.severity != "error"),
            "legacy command after genuine cleanup must retain its semantics: {command}: {reply:?}"
        );
    }
    assert_eq!(session.runtime().clock().tempo().bpm.to_string(), "90");
    assert_eq!(
        session
            .runtime()
            .clock()
            .tempo()
            .beats_per_cycle
            .to_string(),
        "3"
    );
    assert!(session
        .runtime()
        .slots()
        .iter()
        .any(|slot| slot.bound.is_some() || slot.pending.is_some()));
}
