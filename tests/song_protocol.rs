//! Song wire contracts preserve v1 envelopes and distinguish readiness/application.
use serde_json::{json, Value as Json};
use vactr::dsp::caps::CapabilitySet;
use vactr::host::caps::Hosts;
use vactr::session::codec::{decode, decode_as, encode};
use vactr::session::protocol::*;
use vactr::session::{Session, SessionConfig};
use vactr::song::{SnapshotEpoch, SongApplyAck};
use vactr::value::intern::intern_sym;

fn selector() -> WireInstrumentSelector {
    WireInstrumentSelector::new(vec![
        WireSongSound::Builtin { name: "bd".into() },
        WireSongSound::Instrument { id: u32::MAX },
        WireSongSound::Sample {
            path: "./音/loop.wav".into(),
            file: Some(7),
        },
        WireSongSound::Buffer { id: u64::MAX },
    ])
    .unwrap()
}
fn frame(kind: &str, body: Json) -> String {
    json!({"v":1,"seq":7,"kind":kind,"body":body}).to_string()
}
#[test]
fn all_new_messages_round_trip_exact_full_width_identities() {
    let clients = [
        ClientMsg::ApplySong(ApplySongBody {
            file: "song.vact".into(),
            code: "song arrangement > play-song".into(),
            doc_revision: u64::MAX,
            edit_epoch: u64::MAX,
        }),
        ClientMsg::MuteInstrument(SongInstrumentMuteBody {
            epoch: SnapshotEpoch(u64::MAX),
            selector: selector(),
            muted: true,
        }),
    ];
    for client in clients {
        let envelope = Envelope::new(9, None, client);
        let encoded = encode(&envelope);
        assert_eq!(decode(&encoded).unwrap(), envelope);
        let json: Json = serde_json::from_str(&encoded).unwrap();
        assert!(json["seq"].is_number());
        if json["kind"] == "mute-instrument" {
            assert_eq!(json["body"]["epoch"], u64::MAX.to_string());
            assert_eq!(
                json["body"]["selector"]["family"][3]["id"],
                u64::MAX.to_string()
            );
        }
    }
    let servers = [
        ServerMsg::SongCandidateReady(SongCandidateReadyBody {
            epoch: SnapshotEpoch(u64::MAX),
            doc_revision: u64::MAX,
        }),
        ServerMsg::SongCandidateApplied(SongApplyAck {
            epoch: SnapshotEpoch(u64::MAX),
            application_frame: u64::MAX,
            doc_revision: u64::MAX,
        }),
        ServerMsg::SongCandidateFailed(SongCandidateFailedBody {
            epoch: Some(SnapshotEpoch(u64::MAX)),
            doc_revision: Some(u64::MAX),
            code: "failed".into(),
            message: "failure".into(),
        }),
        ServerMsg::SongCandidateFailed(SongCandidateFailedBody {
            epoch: None,
            doc_revision: None,
            code: "host-unavailable".into(),
            message: "no candidate".into(),
        }),
    ];
    for server in servers {
        let envelope = Envelope::new(12, Some(9), server);
        let encoded = encode(&envelope);
        assert_eq!(decode_as::<ServerMsg>(&encoded).unwrap(), envelope);
        assert_eq!(envelope.body.routing(), Route::Requester);
        let json: Json = serde_json::from_str(&encoded).unwrap();
        assert!(json["seq"].is_number());
        assert!(json["re"].is_number());
        if json["kind"] == "song-candidate-ready" {
            assert!(json["body"].get("application_frame").is_none());
        }
        if json["kind"] == "song-candidate-applied" {
            assert_eq!(json["body"]["application_frame"], u64::MAX.to_string());
        }
    }
}
#[test]
fn decimal_epoch_frame_and_buffer_fields_reject_lossy_or_malformed_values() {
    for bad in [
        json!(1),
        json!(-1),
        json!(1.5),
        json!(null),
        json!(""),
        json!("+1"),
        json!("-1"),
        json!(" 1"),
        json!("01"),
        json!("1.0"),
        json!("1e3"),
        json!("18446744073709551616"),
    ] {
        let family = json!({"family":[{"kind":"builtin","name":"bd"}]});
        assert_eq!(
            decode(&frame(
                "mute-instrument",
                json!({"epoch":bad,"selector":family,"muted":true})
            ))
            .unwrap_err()
            .code,
            ErrorCode::BadBody
        );
        assert_eq!(
            decode_as::<ServerMsg>(&frame(
                "song-candidate-applied",
                json!({"epoch":"1","application_frame":bad,"doc_revision":0})
            ))
            .unwrap_err()
            .code,
            ErrorCode::BadBody
        );
        assert_eq!(
            decode(&frame(
                "mute-instrument",
                json!({"epoch":"1","selector":{"family":[{"kind":"buffer","id":bad}]},"muted":true})
            ))
            .unwrap_err()
            .code,
            ErrorCode::BadBody
        );
    }
    for good in ["0", "9007199254740993", "18446744073709551615"] {
        let result = decode_as::<ServerMsg>(&frame(
            "song-candidate-applied",
            json!({"epoch":good,"application_frame":good,"doc_revision":0}),
        ))
        .unwrap();
        let ServerMsg::SongCandidateApplied(ack) = result.body else {
            panic!()
        };
        assert_eq!(ack.epoch.0, good.parse::<u64>().unwrap());
        assert_eq!(ack.application_frame, ack.epoch.0);
    }
}
#[test]
fn selectors_validate_complete_family_and_never_fabricate_buffer_objects() {
    let valid = selector();
    assert_eq!(valid.family().len(), 4);
    assert!(matches!(
        valid.family()[3],
        WireSongSound::Buffer { id: u64::MAX }
    ));
    assert!(WireInstrumentSelector::new(Vec::new()).is_err());
    assert!(
        WireInstrumentSelector::new(vec![WireSongSound::Builtin { name: "bd".into() }; 257])
            .is_err()
    );
    assert!(WireInstrumentSelector::new(vec![WireSongSound::Buffer { id: 1 }; 2]).is_err());
    for bad in [
        json!({"family":[]}),
        json!({"family":[{"kind":"builtin","name":""}]}),
        json!({"family":[{"kind":"builtin","name":":bd"}]}),
        json!({"family":[{"kind":"builtin","name":"two words"}]}),
        json!({"family":[{"kind":"midi","channel":1}]}),
        json!({"family":[{"kind":"instrument","id":-1}]}),
        json!({"family":[{"kind":"instrument","id":4294967296_u64}]}),
        json!({"family":[{"kind":"sample","path":"","file":null}]}),
        json!({"family":[{"kind":"sample","path":"a\0b","file":null}]}),
        json!({"family":[{"kind":"builtin","name":"bd","extra":1}]}),
        json!({"family":[{"kind":"buffer","id":"3"},{"kind":"buffer","id":"3"}]}),
        json!({"family":[{"kind":"buffer","id":"3"}],"index":0}),
    ] {
        let text = frame(
            "mute-instrument",
            json!({"epoch":"1","selector":bad,"muted":true}),
        );
        assert_eq!(
            decode(&text).unwrap_err().code,
            ErrorCode::BadBody,
            "{text}"
        );
        assert!(serde_json::from_value::<WireInstrumentSelector>(bad).is_err());
    }
}
#[test]
fn new_bodies_reject_unknown_stale_field_names_and_wrong_frame_shape() {
    for body in [
        json!({"file":"score.vact","code":"","doc_revision":1,"edit_epoch":1,"span":{"start":0,"end":1}}),
        json!({"file":"score.vact","code":"","doc-revision":1,"edit_epoch":1}),
        json!({"file":"score.vact","code":"","doc_revision":-1,"edit_epoch":1}),
    ] {
        assert_eq!(
            decode(&frame("apply-song", body)).unwrap_err().code,
            ErrorCode::BadBody
        );
    }
    assert_eq!(
        decode(&frame(
            "mute-instrument",
            json!({"epoch":"1","selector":{"family":[{"kind":"builtin","name":"bd"}]},"muted":0})
        ))
        .unwrap_err()
        .code,
        ErrorCode::BadBody
    );
    assert_eq!(
        decode_as::<ServerMsg>(&frame(
            "song-candidate-ready",
            json!({"epoch":"1","doc_revision":1,"application_frame":"0"})
        ))
        .unwrap_err()
        .code,
        ErrorCode::BadBody
    );
    assert_eq!(
        decode_as::<ServerMsg>(&frame(
            "song-candidate-applied",
            json!({"epoch":"1","doc_revision":1})
        ))
        .unwrap_err()
        .code,
        ErrorCode::BadBody
    );
    assert_eq!(
        decode_as::<ServerMsg>(&frame(
            "song-candidate-failed",
            json!({"epoch":1,"doc_revision":1,"code":"failed","message":"failure"})
        ))
        .unwrap_err()
        .code,
        ErrorCode::BadBody
    );
}
#[test]
fn unsupported_requests_do_not_eval_code_register_files_or_activate_slots() {
    let mut session = Session::new(SessionConfig::new(CapabilitySet::native()), Hosts::noop());
    session.eval("var live 7", "live.vact", 1, 1, None);
    let before = session.runtime().clock().tempo();
    assert!(session.lookup_file("new-song.vact").is_none());
    let request = ClientMsg::ApplySong(ApplySongBody {
        file: "new-song.vact".into(),
        code: "upd live 99\nuse-bpm 240\ns :bd > d1".into(),
        doc_revision: 1,
        edit_epoch: 1,
    });
    let result = session.apply(Envelope::new(1, None, request));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].re, Some(1));
    let ServerMsg::SongCandidateFailed(failure) = &result[0].body else {
        panic!()
    };
    assert_eq!(failure.code, "host-unavailable");
    assert_eq!(failure.epoch, None);
    assert!(session.lookup_file("new-song.vact").is_none());
    assert_eq!(
        session
            .evaluator()
            .ns()
            .session_slot(intern_sym("live"))
            .unwrap()
            .get()
            .to_string(),
        "7"
    );
    assert_eq!(session.runtime().clock().tempo(), before);
    assert!(session.runtime().slots().iter().next().is_none());
    let result = session.apply(Envelope::new(
        2,
        None,
        ClientMsg::MuteInstrument(SongInstrumentMuteBody {
            epoch: SnapshotEpoch(999),
            selector: selector(),
            muted: true,
        }),
    ));
    let ServerMsg::SongCandidateFailed(failure) = &result[0].body else {
        panic!()
    };
    assert_eq!(failure.code, "type");
    assert_eq!(failure.epoch, Some(SnapshotEpoch(999)));
}
#[test]
fn stale_revision_and_edit_epoch_requests_are_rejected_before_unavailable_dispatch() {
    let mut session = Session::new(SessionConfig::new(CapabilitySet::native()), Hosts::noop());
    session.eval("var live 7", "live.vact", 4, 8, None);
    for (revision, epoch) in [(3, 8), (4, 7), (5, 8), (4, 9)] {
        let result = session.apply(Envelope::new(
            1,
            None,
            ClientMsg::ApplySong(ApplySongBody {
                file: "live.vact".into(),
                code: "upd live 99".into(),
                doc_revision: revision,
                edit_epoch: epoch,
            }),
        ));
        let ServerMsg::SongCandidateFailed(failure) = &result[0].body else {
            panic!()
        };
        assert_eq!(failure.code, "stale-song-revision");
    }
    assert_eq!(
        session
            .evaluator()
            .ns()
            .session_slot(intern_sym("live"))
            .unwrap()
            .get()
            .to_string(),
        "7"
    );
}
#[test]
fn legacy_numeric_envelopes_and_incremental_eval_are_unchanged() {
    let text = frame(
        "eval",
        json!({"file":"main.vact","code":"var old 4","doc_revision":1,"edit_epoch":2}),
    );
    let envelope = decode(&text).unwrap();
    assert!(matches!(&envelope.body, ClientMsg::Eval(_)));
    let encoded = encode(&envelope);
    let json: Json = serde_json::from_str(&encoded).unwrap();
    assert!(json["seq"].is_number());
    assert!(json["body"]["doc_revision"].is_number());
    assert!(json["body"]["edit_epoch"].is_number());
    let mut session = Session::new(SessionConfig::new(CapabilitySet::native()), Hosts::noop());
    let results = session.apply(envelope);
    assert!(results
        .iter()
        .any(|result| matches!(result.body, ServerMsg::EvalResult(_))));
    assert_eq!(
        session
            .evaluator()
            .ns()
            .session_slot(intern_sym("old"))
            .unwrap()
            .get()
            .to_string(),
        "4"
    );
}

#[test]
fn acknowledged_mute_and_finite_state_keep_decimal_identity_and_routing() {
    let messages = [
        ServerMsg::SongInstrumentMuted(SongInstrumentMutedBody {
            epoch: SnapshotEpoch(u64::MAX),
            selector: selector(),
            muted: true,
            application_frame: u64::MAX,
        }),
        ServerMsg::SongTransportState(SongTransportStateBody {
            epoch: SnapshotEpoch(u64::MAX),
            state: WireSongTransportState::Ended,
            instruments: vec![selector()],
        }),
    ];
    for message in messages {
        let envelope = Envelope::new(9, Some(7), message.clone());
        let text = encode(&envelope);
        assert_eq!(decode_as::<ServerMsg>(&text).unwrap(), envelope);
        let json: Json = serde_json::from_str(&text).unwrap();
        assert_eq!(json["body"]["epoch"], u64::MAX.to_string());
        match message {
            ServerMsg::SongInstrumentMuted(_) => {
                assert_eq!(json["body"]["application_frame"], u64::MAX.to_string());
                assert_eq!(message.routing(), Route::Requester);
            }
            ServerMsg::SongTransportState(_) => {
                assert_eq!(message.routing(), Route::Broadcast(Topic::Telemetry))
            }
            _ => unreachable!(),
        }
    }
    for bad in [json!(1), json!("01"), json!("18446744073709551616")] {
        let body = json!({"epoch":"1","selector":selector(),"muted":true,"application_frame":bad});
        assert_eq!(
            decode_as::<ServerMsg>(&frame("song-instrument-muted", body))
                .unwrap_err()
                .code,
            ErrorCode::BadBody
        );
    }
    for state in ["prepared", "playing", "draining", "ended", "failed"] {
        assert!(decode_as::<ServerMsg>(&frame(
            "song-transport-state",
            json!({"epoch":"1","state":state})
        ))
        .is_ok());
    }
}
