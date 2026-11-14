//! Live transport controls preserve bindings and report music activity truthfully.

use super::support::Rig;
use crate::host::testing::AudioCall;
use crate::host::wire::{HostMsg, OutputPhase, Release};
use crate::ns::stage::SlotKey;
use crate::session::protocol::{ClientMsg, Empty, ServerMsg, TransportSample};

fn sample(messages: &[ServerMsg]) -> &TransportSample {
    messages
        .iter()
        .find_map(|message| match message {
            ServerMsg::Tempo(body) => body.transport.as_ref(),
            _ => None,
        })
        .expect("transport telemetry")
}

#[test]
fn live_pause_preserves_position_and_bindings_then_resumes_without_replaying_gap() {
    let mut rig = Rig::new();
    rig.ok("s :analog > note [60 62 64 65] > d1", 0);
    rig.run_to(0.7);
    let before = rig.s.rt.clock().pos();
    let generation = rig.s.rt.slot_gen(SlotKey::D(1)).unwrap();
    assert!(rig.send(ClientMsg::TransportPause(Empty {})).is_empty());
    assert!(rig.s.rt.midi_clock().is_frozen());
    assert_eq!(rig.s.rt.slot_gen(SlotKey::D(1)), Some(generation + 1));
    let slot = rig.s.rt.slots().get(SlotKey::D(1)).unwrap();
    assert!(slot.bound.is_some());
    assert!(slot.lanes().iter().all(|lane| lane.staging.is_empty()));
    assert!(rig.audio.calls().iter().any(|(_, call)| {
        matches!(call, AudioCall::Control(control) if control.release == Release::Natural)
    }));
    rig.send(ClientMsg::TransportPause(Empty {}));
    assert_eq!(rig.s.rt.slot_gen(SlotKey::D(1)), Some(generation + 1));
    let sent = rig.sent().len();
    rig.run_to(5.0);
    assert_eq!(rig.sent().len(), sent);
    assert_eq!(rig.s.rt.clock().pos(), before);
    rig.clock.set(5.1);
    let paused = rig.tick();
    assert!(sample(&paused).active);
    assert!(!sample(&paused).running);
    rig.send(ClientMsg::TransportContinue(Empty {}));
    rig.send(ClientMsg::TransportContinue(Empty {}));
    assert!(!rig.s.rt.midi_clock().is_frozen());
    rig.run_to(6.0);
    let events = rig.sent();
    assert!(events.len() > sent);
    let resumed = &events[sent..];
    assert!(resumed.len() <= 3, "no catch-up burst: {resumed:?}");
    assert!(resumed.iter().all(|event| event.time >= 5.1));
    assert!(rig.s.rt.clock().pos() > before);
}

#[test]
fn stop_and_cut_while_paused_allow_subsequent_evaluation_to_play() {
    for stop in [ClientMsg::StopAll(Empty {}), ClientMsg::Hush(Empty {})] {
        let mut rig = Rig::new();
        rig.ok("s :analog > note [60] > d1", 0);
        rig.run_to(0.7);
        rig.send(ClientMsg::TransportPause(Empty {}));
        rig.run_to(2.0);
        rig.send(stop);
        assert!(!rig.s.rt.midi_clock().is_frozen());
        assert!(!rig.s.rt.playback_active());
        let sent = rig.sent().len();
        rig.ok("s :analog > note [64] > d1", 1);
        rig.run_to(5.0);
        assert!(rig.sent().len() > sent);
    }
}

#[test]
fn transport_activity_excludes_free_running_clock_and_output_tails() {
    let mut rig = Rig::new();
    rig.audio.reply(HostMsg::OutputState {
        phase: OutputPhase::Running,
        frame: 0,
    });
    let idle = rig.tick();
    assert!(sample(&idle).running);
    assert!(!sample(&idle).active);
    rig.ok("let answer 42", 0);
    rig.run_to(0.1);
    assert!(!rig.s.rt.playback_active());
    // A musical binding remains active across rests and pause, independent of amplitude.
    rig.ok("s :analog > note [60] > d1", 1);
    rig.clock.set(0.2);
    let playing = rig.tick();
    assert!(sample(&playing).active);
    rig.send(ClientMsg::StopAll(Empty {}));
    rig.audio.reply(HostMsg::OutputState {
        phase: OutputPhase::Draining,
        frame: 1,
    });
    rig.clock.set(0.3);
    let stopped = rig.tick();
    assert!(!sample(&stopped).active);
    assert_eq!(sample(&stopped).output.as_deref(), Some("draining"));
}

#[test]
fn finite_song_preparation_refuses_pause_with_protocol_error() {
    use crate::dsp::caps::CapabilitySet;
    use crate::host::caps::SongPreparationLimits;
    use crate::session::song::{evaluate_song_candidate, CandidateBuildCtx};
    use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
    use crate::song::SnapshotEpoch;
    use crate::song::SongLimits;

    let mut rig = Rig::new();
    let assets =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let context = CandidateBuildCtx {
        assets: &assets,
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
    let candidate = evaluate_song_candidate(
        "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone}] duration: 4} > play-song",
        "song.vact", 1, SnapshotEpoch(1), &context,
    ).unwrap();
    let prepared = crate::song::prepare_song(candidate).unwrap();
    rig.s
        .rt
        .prepare_song(
            prepared,
            SongPreparationLimits {
                capabilities: CapabilitySet::native(),
                song: SongLimits::default(),
                max_resources: 128,
                max_pending_records: 4096,
                max_graph_bytes: 65536,
                max_work: 8_000_000,
            },
        )
        .map_err(|refusal| refusal.failure)
        .unwrap();
    let replies = rig.send(ClientMsg::TransportPause(Empty {}));
    assert!(
        matches!(replies.as_slice(), [ServerMsg::ProtocolError(error)]
        if error.message.contains("finite-song"))
    );
    assert!(!rig.s.rt.midi_clock().is_frozen());
    assert!(rig.s.rt.owns_song_resources());
}
