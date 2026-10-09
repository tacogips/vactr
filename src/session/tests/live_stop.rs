//! LP-SESSION-STOP coverage through the session envelope path.

use super::support::Rig;
use crate::host::testing::AudioCall;
use crate::host::wire::{CtlMsg, HostMsg, OutputMode, OutputPhase, Release};
use crate::session::protocol::{ClientMsg, Empty, ServerMsg};

#[test]
fn stop_all_posts_gentle_output_stop() {
    let mut rig = Rig::new();
    rig.send(ClientMsg::StopAll(Empty {}));

    let stops: Vec<_> = rig
        .audio
        .calls()
        .into_iter()
        .filter_map(|(_, call)| match call {
            AudioCall::Post(CtlMsg::OutputStop { mode }) => Some(mode),
            _ => None,
        })
        .collect();
    assert_eq!(stops, [OutputMode::Gentle]);
}

#[test]
fn hush_posts_panic_controls_then_cut_output_stop() {
    let mut rig = Rig::new();
    rig.ok("s :bd > d1", 0);
    rig.send(ClientMsg::Hush(Empty {}));

    let calls = rig.audio.calls();
    let panic_at = calls
        .iter()
        .position(|(_, call)| matches!(call, AudioCall::Control(control) if control.release == Release::Panic))
        .expect("hush panic slot control");
    let cut_at = calls
        .iter()
        .position(|(_, call)| {
            matches!(
                call,
                AudioCall::Post(CtlMsg::OutputStop {
                    mode: OutputMode::Cut
                })
            )
        })
        .expect("hush output cut");
    assert!(panic_at < cut_at);
    assert_eq!(
        calls
            .iter()
            .filter(|(_, call)| matches!(call, AudioCall::Post(CtlMsg::OutputStop { .. })))
            .count(),
        1
    );
}

#[test]
fn output_telemetry_omits_unknown_state_and_publishes_draining() {
    let mut rig = Rig::new();
    let before = rig.tick();
    let before_sample = before.iter().find_map(|message| match message {
        ServerMsg::Tempo(body) => body.transport.as_ref(),
        _ => None,
    });
    let before_sample = before_sample.expect("subscribed transport sample");
    assert_eq!(before_sample.output, None);
    let before_json = serde_json::to_value(before_sample).expect("serialize transport sample");
    assert!(before_json.get("output").is_none());

    rig.audio.reply(HostMsg::OutputState {
        phase: OutputPhase::Draining,
        frame: 1,
    });
    rig.clock.set(0.1);
    let after = rig.tick();
    let after_sample = after.iter().find_map(|message| match message {
        ServerMsg::Tempo(body) => body.transport.as_ref(),
        _ => None,
    });
    assert_eq!(
        after_sample.and_then(|sample| sample.output.as_deref()),
        Some("draining")
    );
}
