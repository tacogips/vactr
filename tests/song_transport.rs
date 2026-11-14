//! Public Session ownership, original request correlation and finite playback.
use std::{collections::BTreeMap, rc::Rc};
use vactr::dsp::{arena::StoreKind, caps::CapabilitySet, engine::EngineConfig};
use vactr::host::{
    caps::{Hosts, SongPreparationLimits},
    native::audio::NativeAudioHost,
};
use vactr::sched::song::SongTransportState;
use vactr::session::{
    protocol::ApplySongBody, ClientMsg, Dest, Envelope, ServerMsg, Session, SessionConfig,
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

#[test]
fn original_session_candidate_repeats_changes_and_stops_with_correlated_acknowledgments() {
    let (mut session, mut side) = session_pair();
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nlet a {part [tone: {s :tone > n 60 > gain 0.2}] duration: 1}\nlet b {part [tone: {s :tone > n 67 > gain 0.2}] duration: 1}\nsong {sequence [{part-repeat a 2} b]} tail-seconds: 0 > play-song";
    let immediate = session.apply_from(
        23,
        Envelope::new(
            71,
            None,
            ClientMsg::ApplySong(ApplySongBody {
                file: "score.vact".into(),
                code: code.into(),
                doc_revision: 9,
                edit_epoch: 0,
            }),
        ),
    );
    assert!(
        immediate.is_empty(),
        "enqueue must not invent Ready or Applied"
    );
    let mut ready = None;
    let mut applied = None;
    let mut audible = false;
    let mut edited_active = false;
    let mut audible_after_edit = false;
    let mut frames = 0u64;
    for _ in 0..4096 {
        let output = session.tick_routed(frames as f64 / 32768.);
        for message in output {
            match message.env.body {
                ServerMsg::SongCandidateReady(body) => {
                    assert_eq!(message.to, Dest::Conn(23));
                    assert_eq!(message.env.re, Some(71));
                    assert_eq!(body.doc_revision, 9);
                    assert!(ready.replace(body.epoch).is_none());
                }
                ServerMsg::SongCandidateApplied(body) => {
                    assert_eq!(message.to, Dest::Conn(23));
                    assert_eq!(message.env.re, Some(71));
                    assert_eq!(Some(body.epoch), ready);
                    assert_eq!(body.doc_revision, 9);
                    assert!(applied.replace(body).is_none());
                }
                ServerMsg::SongCandidateFailed(body) => panic!("actual song failed: {body:?}"),
                _ => {}
            }
        }
        if applied.is_some() && !edited_active {
            use vactr::session::protocol::DocChangedBody;
            session.apply_from(
                23,
                Envelope::new(
                    72,
                    None,
                    ClientMsg::DocChanged(DocChangedBody {
                        file: "score.vact".into(),
                        doc_revision: 10,
                        base_revision: 9,
                        changes: Vec::new(),
                        dirty: Vec::new(),
                        edit_epoch: 1,
                    }),
                ),
            );
            edited_active = true;
        }
        let mut pcm = [0.; 512];
        side.render(&mut pcm, 2);
        let nonzero = pcm.iter().any(|sample| sample.abs() > 1e-5);
        audible |= nonzero;
        audible_after_edit |= edited_active && nonzero;
        frames += 256;
        if session.runtime().song_state() == Some(SongTransportState::Ended) {
            break;
        }
    }
    assert!(ready.is_some());
    assert!(applied.is_some());
    assert!(
        edited_active && audible_after_edit,
        "document edits must preserve acknowledged active audio"
    );
    assert!(audible, "actual Native oscillator output must be nonzero");
    assert_eq!(
        session.runtime().song_state(),
        Some(SongTransportState::Ended)
    );
    let mut silence = [1.; 512];
    side.render(&mut silence, 2);
    assert!(silence.iter().all(|sample| sample.abs() < 1e-7));
}

#[test]
fn document_change_cancels_original_delayed_preparation_and_allows_fresh_playback() {
    use vactr::session::protocol::DocChangedBody;
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > n 60 > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
    for callbacks in [0, 8] {
        let (mut session, mut side) = session_pair();
        let request = ApplySongBody {
            file: "score.vact".into(),
            code: code.into(),
            doc_revision: 9,
            edit_epoch: 0,
        };
        assert!(session
            .apply_from(
                23,
                Envelope::new(71, None, ClientMsg::ApplySong(request.clone()))
            )
            .is_empty());
        let mut frames = 0u64;
        for _ in 0..callbacks {
            let notices = session.tick_routed(frames as f64 / 32768.);
            assert!(
                !notices.iter().any(|m| matches!(
                    m.env.body,
                    ServerMsg::SongCandidateReady(_)
                        | ServerMsg::SongCandidateApplied(_)
                        | ServerMsg::SongCandidateFailed(_)
                )),
                "fixture must edit while the genuine host owner is still preparing: {notices:?}"
            );
            side.render(&mut [0.; 512], 2);
            frames += 256;
        }
        // Post the actual preparation request but delay its host callback across the edit.
        let notices = session.tick_routed(frames as f64 / 32768.);
        assert!(!notices.iter().any(|m| matches!(
            m.env.body,
            ServerMsg::SongCandidateReady(_) | ServerMsg::SongCandidateApplied(_)
        )));
        let change = DocChangedBody {
            file: "score.vact".into(),
            doc_revision: 10,
            base_revision: 9,
            changes: Vec::new(),
            dirty: Vec::new(),
            edit_epoch: 1,
        };
        session.apply_from(
            23,
            Envelope::new(72, None, ClientMsg::DocChanged(change.clone())),
        );
        // Duplicate notifications cannot replay the original cancellation outcome.
        session.apply_from(23, Envelope::new(73, None, ClientMsg::DocChanged(change)));
        let mut failed = 0;
        for _ in 0..256 {
            for message in session.tick_routed(frames as f64 / 32768.) {
                match message.env.body {
                    ServerMsg::SongCandidateFailed(body) => {
                        assert_eq!(message.to, Dest::Conn(23));
                        assert_eq!(message.env.re, Some(71));
                        assert_eq!(body.doc_revision, Some(9));
                        assert!(body.message.contains("invalidated"));
                        failed += 1;
                    }
                    ServerMsg::SongCandidateReady(_) | ServerMsg::SongCandidateApplied(_) => {
                        panic!("invalidated original candidate became playable")
                    }
                    _ => {}
                }
            }
            let mut pcm = [0.; 512];
            side.render(&mut pcm, 2);
            assert!(pcm.iter().all(|sample| sample.abs() < 1e-7));
            frames += 256;
        }
        assert_eq!(failed, 1);
        let mut fresh = request;
        fresh.doc_revision = 10;
        fresh.edit_epoch = 1;
        let reply = session.apply_from(23, Envelope::new(74, None, ClientMsg::ApplySong(fresh)));
        assert!(
            reply.is_empty(),
            "original owner cleanup must allow fresh admission: {reply:?}"
        );
        let mut applied = false;
        let mut audible = false;
        for _ in 0..1024 {
            for message in session.tick_routed(frames as f64 / 32768.) {
                match message.env.body {
                    ServerMsg::SongCandidateApplied(ack) => {
                        assert_eq!(ack.doc_revision, 10);
                        assert_eq!(message.env.re, Some(74));
                        applied = true;
                    }
                    ServerMsg::SongCandidateFailed(body) => panic!("fresh owner failed: {body:?}"),
                    _ => {}
                }
            }
            let mut pcm = [0.; 512];
            side.render(&mut pcm, 2);
            audible |= pcm.iter().any(|sample| sample.abs() > 1e-5);
            frames += 256;
            if session.runtime().song_state() == Some(SongTransportState::Ended) {
                break;
            }
        }
        assert!(applied && audible);
        assert_eq!(
            session.runtime().song_state(),
            Some(SongTransportState::Ended)
        );
    }
}

#[test]
fn direct_consuming_preparation_failure_has_finite_failed_state() {
    let (mut session, mut side) = session_pair_with_bus_slots(2);
    let factory =
        DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), Default::default());
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
    let candidate = vactr::session::song::evaluate_song_candidate(
        "song {part [drums: {s :analog}] duration: 1} tail-seconds: 0 > play-song",
        "direct.vact",
        1,
        vactr::song::SnapshotEpoch(99),
        &context,
    )
    .unwrap();
    let prepared = vactr::song::prepare_song(candidate).unwrap();
    session
        .submit_prepared_song(prepared)
        .unwrap_or_else(|e| panic!("direct owner refused: {}", e.failure.message));
    let mut frames = 0u64;
    for _ in 0..128 {
        session.tick_routed(frames as f64 / 32768.);
        let mut pcm = [0.; 512];
        side.render(&mut pcm, 2);
        assert!(pcm.iter().all(|sample| sample.abs() < 1e-7));
        frames += 256;
    }
    assert_eq!(
        session.runtime().song_state(),
        Some(SongTransportState::Failed)
    );
}

#[derive(Default)]
struct AckGate {
    hold: bool,
    forwarded: bool,
    delayed: Vec<vactr::host::wire::HostMsg>,
}
struct GatedNative {
    inner: NativeAudioHost,
    gate: Rc<std::cell::RefCell<AckGate>>,
}
impl vactr::host::caps::AudioHost for GatedNative {
    fn song_clock(&self) -> Result<vactr::song::routing::SongHostClock, vactr::vm::fail::Failure> {
        self.inner.song_clock()
    }
    fn try_song_command(
        &mut self,
        command: vactr::song::routing::SongCommand,
    ) -> Result<(), vactr::host::caps::SongCommandRefusal> {
        self.inner.try_song_command(command)
    }
    fn submit_song_native(
        &mut self,
        install: vactr::dsp::ring::NativeSongInstall,
    ) -> Result<(), vactr::dsp::ring::NativeSongInstall> {
        self.inner.submit_song_native(install)
    }
    fn materialize_song_native(
        &self,
        lease: vactr::song::routing::SongLeaseKey,
        graph: &vactr::host::caps::GraphHandle,
    ) -> Result<Option<vactr::dsp::ring::NativeSongInstall>, vactr::vm::fail::Failure> {
        self.inner.materialize_song_native(lease, graph)
    }
    fn send(&mut self, event: vactr::host::wire::AudioEvent) {
        self.inner.send(event);
    }
    fn control(&mut self, control: vactr::host::wire::SlotControl) {
        self.inner.control(control);
    }
    fn post(&mut self, control: vactr::host::wire::CtlMsg) {
        self.inner.post(control);
    }
    fn drain(&mut self, out: &mut Vec<vactr::host::wire::HostMsg>) {
        use vactr::{host::wire::HostMsg, song::routing::SongHostAck};
        let mut gate = self.gate.borrow_mut();
        if !gate.hold {
            out.append(&mut gate.delayed);
        }
        let mut actual = Vec::new();
        self.inner.drain(&mut actual);
        for message in actual {
            if gate.hold && matches!(message, HostMsg::Song(SongHostAck::Muted(_))) {
                if gate.forwarded {
                    gate.delayed.push(message);
                    continue;
                }
                gate.forwarded = true;
            }
            out.push(message);
        }
    }
    fn now(&self) -> f64 {
        self.inner.now()
    }
    fn swap_graph(&mut self, graph: vactr::host::caps::GraphHandle) {
        self.inner.swap_graph(graph);
    }
    fn install_sample(&mut self, id: u32, data: std::sync::Arc<vactr::host::caps::SampleData>) {
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
fn complete_family_mute_waits_for_every_actual_ack_and_unmute_only_reopens_future_audio() {
    use vactr::session::protocol::{
        SongInstrumentMuteBody, WireInstrumentSelector, WireSongTransportState,
    };
    let mut caps = CapabilitySet::native();
    caps.max_voices = 4;
    let mut engine = EngineConfig::new(
        &caps,
        32768.,
        vactr::host::native::audio::MAX_BLOCK,
        StoreKind::NativeArc,
    );
    engine.bus_slots = 12;
    let (audio, mut side) = NativeAudioHost::headless_with_config(engine, 64).unwrap();
    let gate = Rc::new(std::cell::RefCell::new(AckGate::default()));
    let mut hosts = Hosts::noop();
    hosts.audio = Box::new(GatedNative {
        inner: audio,
        gate: Rc::clone(&gate),
    });
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
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\ninst other freq: float = 220:\n\tsin-osc freq > * amp\nsong {part [a: {s :tone > n 60 > gain 0.2} b: {s :other > n 60 > gain 0.2}] duration: 4} tail-seconds: 0 > play-song";
    let response = session.apply_from(
        23,
        Envelope::new(
            71,
            None,
            ClientMsg::ApplySong(ApplySongBody {
                file: "mute.vact".into(),
                code: code.into(),
                doc_revision: 1,
                edit_epoch: 0,
            }),
        ),
    );
    assert!(
        response.is_empty(),
        "actual initial Apply response: {response:?}"
    );
    let mut frames = 0u64;
    let mut epoch = None;
    let mut instruments = Vec::new();
    let mut audible = false;
    for _ in 0..256 {
        for message in session.tick_routed(frames as f64 / 32768.) {
            match message.env.body {
                ServerMsg::SongCandidateApplied(ack) => epoch = Some(ack.epoch),
                ServerMsg::SongTransportState(body) => {
                    assert_eq!(
                        epoch,
                        Some(body.epoch),
                        "actual Applied must precede its state"
                    );
                    instruments = body.instruments;
                }
                ServerMsg::SongCandidateFailed(body) => panic!("preparation failed: {body:?}"),
                _ => {}
            }
        }
        let mut pcm = [0.; 512];
        side.render(&mut pcm, 2);
        frames += 256;
        audible |= pcm.iter().any(|sample| sample.abs() > 1e-5);
        if epoch.is_some() && !instruments.is_empty() && audible {
            break;
        }
    }
    assert!(audible);
    let epoch = epoch.unwrap();
    let family = instruments
        .iter()
        .flat_map(|s| s.family().iter().cloned())
        .collect();
    let selector = WireInstrumentSelector::new(family).unwrap();
    assert!(selector.family().len() >= 2);
    gate.borrow_mut().hold = true;
    assert!(session
        .apply_from(
            23,
            Envelope::new(
                72,
                None,
                ClientMsg::MuteInstrument(SongInstrumentMuteBody {
                    epoch,
                    selector: selector.clone(),
                    muted: true,
                })
            )
        )
        .is_empty());
    for _ in 0..32 {
        let output = session.tick_routed(frames as f64 / 32768.);
        assert!(
            !output
                .iter()
                .any(|m| matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))),
            "one actual family ACK is insufficient"
        );
        side.render(&mut [0.; 512], 2);
        frames += 256;
        if !gate.borrow().delayed.is_empty() {
            break;
        }
    }
    assert!(
        gate.borrow().forwarded && !gate.borrow().delayed.is_empty(),
        "retain a genuine second DSP ACK"
    );
    gate.borrow_mut().hold = false;
    let output = session.tick_routed(frames as f64 / 32768.);
    let muted: Vec<_> = output
        .iter()
        .filter_map(|m| {
            if let ServerMsg::SongInstrumentMuted(body) = &m.env.body {
                assert_eq!(m.to, Dest::Conn(23));
                assert_eq!(m.env.re, Some(72));
                Some(body)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(muted.len(), 1);
    assert_eq!(muted[0].selector, selector);
    assert!(muted[0].muted);
    for _ in 0..4 {
        session.tick_routed(frames as f64 / 32768.);
        side.render(&mut [0.; 512], 2);
        frames += 256;
    }
    let mut silence = [1.; 512];
    side.render(&mut silence, 2);
    frames += 256;
    assert!(silence.iter().all(|sample| sample.abs() < 1e-7));
    assert!(session
        .apply_from(
            23,
            Envelope::new(
                73,
                None,
                ClientMsg::MuteInstrument(SongInstrumentMuteBody {
                    epoch,
                    selector: selector.clone(),
                    muted: false,
                })
            )
        )
        .is_empty());
    let mut reopened = false;
    let mut unmuted = false;
    let mut ended = false;
    for _ in 0..2048 {
        for message in session.tick_routed(frames as f64 / 32768.) {
            match message.env.body {
                ServerMsg::SongInstrumentMuted(body) => {
                    assert_eq!(message.env.re, Some(73));
                    assert!(!body.muted);
                    assert_eq!(body.selector, selector);
                    unmuted = true;
                }
                ServerMsg::SongTransportState(body)
                    if body.state == WireSongTransportState::Ended =>
                {
                    assert_eq!(body.epoch, epoch);
                    ended = true;
                }
                ServerMsg::SongCandidateFailed(body) => panic!("active mute failed: {body:?}"),
                _ => {}
            }
        }
        let mut pcm = [0.; 512];
        side.render(&mut pcm, 2);
        frames += 256;
        reopened |= pcm.iter().any(|sample| sample.abs() > 1e-5);
        if ended {
            break;
        }
    }
    assert!(unmuted && reopened && ended);
    let reply = session.apply_from(
        23,
        Envelope::new(
            74,
            None,
            ClientMsg::MuteInstrument(SongInstrumentMuteBody {
                epoch,
                selector,
                muted: true,
            }),
        ),
    );
    assert!(
        matches!(&reply[0].env.body, ServerMsg::SongCandidateFailed(_)),
        "retired epochs cannot acknowledge new mute success"
    );
}
