//! The protocol codec (design 14.5.6): every message round-trips, every
//! `protocol-error` code is produced by a malformed input, and truncated
//! frames are errors, never panics.

use super::support::Rig;
use crate::session::changes::Change;
use crate::session::codec::{decode, decode_as, encode, MAX_FRAME};
use crate::session::protocol::*;

fn span(a: u32, b: u32) -> WireSpan {
    WireSpan::new(a, b)
}

fn diag() -> WireDiag {
    WireDiag {
        code: "division-by-zero".to_string(),
        severity: "error".to_string(),
        message: "division by zero".to_string(),
        span: span(3, 9),
        file: "main.vact".to_string(),
        slot: Some("d1".to_string()),
        beat: Some([3, 2]),
    }
}

fn site() -> WireSite {
    WireSite {
        id: 4,
        span: span(10, 13),
        tier: WireTier::Reeval,
        origin: WireOrigin::PatternLiteral,
        value: 0.25,
        form_gen: 7,
        key: Some("drums.lpf.1.cutoff".to_string()),
    }
}

fn clients() -> Vec<ClientMsg> {
    vec![
        ClientMsg::Eval(EvalBody {
            file: "main.vact".to_string(),
            code: "s [:bd :sd] > d1\n# ünïcode \"quoted\"".to_string(),
            span: Some(span(0, 16)),
            doc_revision: 3,
            edit_epoch: 9,
        }),
        ClientMsg::Eval(EvalBody {
            file: "b.vact".to_string(),
            code: String::new(),
            span: None,
            doc_revision: 0,
            edit_epoch: 0,
        }),
        ClientMsg::Hush(Empty {}),
        ClientMsg::Stop(StopBody {
            slot: "d2".to_string(),
        }),
        ClientMsg::SetVar(SetVarBody {
            file: "main.vact".to_string(),
            name: "root".to_string(),
            value: WireValue::Int(-3),
            defining_form_gen: 2,
            edit_epoch: 1,
        }),
        ClientMsg::SetVar(SetVarBody {
            file: "main.vact".to_string(),
            name: "flag".to_string(),
            value: WireValue::Bool(true),
            defining_form_gen: 5,
            edit_epoch: 1,
        }),
        ClientMsg::SetTweak(SetTweakBody {
            file: "main.vact".to_string(),
            id: 4,
            form_gen: 7,
            value: WireNum::Float(0.5),
            edit_epoch: 2,
        }),
        ClientMsg::DocChanged(DocChangedBody {
            file: "main.vact".to_string(),
            doc_revision: 4,
            base_revision: 3,
            changes: vec![Change {
                from: 2,
                to: 5,
                insert_len: 1,
            }],
            dirty: vec![span(2, 3)],
            edit_epoch: 10,
        }),
        ClientMsg::Learn(LearnBody {
            file: "main.vact".to_string(),
            binding: LearnTarget::Key("drums.lpf.1.cutoff".to_string()),
            cc: 74,
            ch: Some(2),
            edit_epoch: 10,
        }),
        ClientMsg::Learn(LearnBody {
            file: "main.vact".to_string(),
            binding: LearnTarget::Id(4),
            cc: 0,
            ch: None,
            edit_epoch: 10,
        }),
        ClientMsg::Subscribe(SubscribeBody {
            telemetry: true,
            levels: false,
            diagnostics: true,
        }),
        ClientMsg::ManifestReq(Empty {}),
    ]
}

fn servers() -> Vec<ServerMsg> {
    vec![
        ServerMsg::EvalResult(EvalResultBody {
            file: "main.vact".to_string(),
            doc_revision: 3,
            forms: vec![
                WireForm {
                    span: span(0, 16),
                    value: Some("<pattern>".to_string()),
                    failure: None,
                    form_gen: 1,
                },
                WireForm {
                    span: span(17, 20),
                    value: None,
                    failure: Some(diag()),
                    form_gen: 2,
                },
            ],
            diagnostics: vec![diag()],
            sites: vec![site()],
            directives: WireDirectives {
                file_level: WireFileLevel { midi_ch: Some(1) },
                entries: vec![WireDirective {
                    span: span(20, 30),
                    kind: "positional".to_string(),
                    target: Some(span(0, 16)),
                    trailing: false,
                }],
                labels: vec![WireLabel {
                    name: "drums".to_string(),
                    spans: vec![span(1, 2)],
                    ambiguous: false,
                }],
                bindings: vec![WireBinding {
                    key: Some("drums.lpf.1.cutoff".to_string()),
                    span: span(5, 8),
                    param: "cutoff".to_string(),
                    cc: Some(74),
                    ch: None,
                    directive: span(20, 30),
                }],
            },
        }),
        ServerMsg::StaleBinding(StaleBindingBody {
            target: StaleTarget::Id(4),
            reason: StaleReason::StaleFormGen,
            current_form_gen: Some(8),
        }),
        ServerMsg::StaleBinding(StaleBindingBody {
            target: StaleTarget::Name("root".to_string()),
            reason: StaleReason::SupersededDefinition,
            current_form_gen: None,
        }),
        ServerMsg::StaleBinding(StaleBindingBody {
            target: StaleTarget::Id(1),
            reason: StaleReason::EditInvalidated,
            current_form_gen: None,
        }),
        ServerMsg::StaleBinding(StaleBindingBody {
            target: StaleTarget::Id(1),
            reason: StaleReason::UnreconciledEdit,
            current_form_gen: None,
        }),
        ServerMsg::DirectiveEdit(DirectiveEditBody {
            file: "main.vact".to_string(),
            doc_revision: 3,
            span: span(20, 30),
            expected: "#@ lpf".to_string(),
            text: "#@ lpf cc: 74".to_string(),
        }),
        ServerMsg::Manifest(ManifestBody {
            sounds: vec!["bd".to_string()],
            synths: vec!["analog".to_string()],
            controls: vec!["gain".to_string()],
        }),
        ServerMsg::ProtocolError(ProtocolError::new(ErrorCode::UnknownKind, "nope")),
        ServerMsg::Bindings(BindingsBody {
            pass: 2,
            changed: vec![WireChanged {
                name: "right".to_string(),
                value: "1".to_string(),
                form_gen: 3,
            }],
            sites: vec![site()],
            states: vec![
                WireFormState {
                    name: "left".to_string(),
                    state: WireState::Failed,
                    value: "1".to_string(),
                    blocked_on: None,
                    diagnostic: Some(diag()),
                },
                WireFormState {
                    name: "total".to_string(),
                    state: WireState::Blocked,
                    value: "3".to_string(),
                    blocked_on: Some("left".to_string()),
                    diagnostic: None,
                },
                WireFormState {
                    name: "right".to_string(),
                    state: WireState::Ok,
                    value: "1".to_string(),
                    blocked_on: None,
                    diagnostic: None,
                },
            ],
        }),
        ServerMsg::Diag(DiagBody {
            add: vec![diag()],
            clear: vec![WireClear {
                slot: "d1".to_string(),
            }],
        }),
        ServerMsg::Playing(PlayingBody {
            events: vec![WirePlaying {
                slot: "d1".to_string(),
                beat: [4, 1],
                time: 2.0,
                dur: [2, 1],
                src: Some(WireSrcRef {
                    file: "main.vact".to_string(),
                    span: span(3, 5),
                    doc_revision: 3,
                    form_gen: 1,
                }),
            }],
        }),
        ServerMsg::Levels(LevelsBody {
            levels: vec![WireLevel {
                source: ":master".to_string(),
                rms: 0.125,
            }],
        }),
        ServerMsg::Tempo(TempoBody {
            bpm: 120.0,
            beats_per_cycle: 4,
            cycle: [7, 2],
        }),
    ]
}

#[test]
fn every_client_and_server_message_round_trips() {
    for (k, m) in clients().into_iter().enumerate() {
        let env = Envelope::new(k as u64, None, m);
        let text = encode(&env);
        assert_eq!(decode(&text).as_ref(), Ok(&env), "{text}");
    }
    let kinds: Vec<&str> = clients().iter().map(ClientMsg::kind).collect();
    for k in ClientMsg::KINDS {
        assert!(kinds.contains(&k), "no sample of `{k}`");
    }
    for (k, m) in servers().into_iter().enumerate() {
        let env = Envelope::new(k as u64 + 100, Some(k as u64), m);
        let text = encode(&env);
        assert_eq!(decode_as::<ServerMsg>(&text).as_ref(), Ok(&env), "{text}");
    }
    let kinds: Vec<&str> = servers().iter().map(ServerMsg::kind).collect();
    for k in ServerMsg::KINDS {
        assert!(kinds.contains(&k), "no sample of `{k}`");
    }
}

#[test]
fn the_wire_shape_matches_command_md() {
    let env = Envelope::new(
        12,
        Some(11),
        ServerMsg::StaleBinding(StaleBindingBody {
            target: StaleTarget::Id(3),
            reason: StaleReason::UnreconciledEdit,
            current_form_gen: None,
        }),
    );
    let v: serde_json::Value = serde_json::from_str(&encode(&env)).expect("json");
    assert_eq!(
        v,
        serde_json::json!({"v": 1, "seq": 12, "re": 11, "kind": "stale-binding",
            "body": {"target": 3, "reason": "unreconciled-edit"}})
    );
    let text = r#"{"v":1,"seq":1,"kind":"manifest?","body":{}}"#;
    assert_eq!(
        decode(text).map(|e| e.body),
        Ok(ClientMsg::ManifestReq(Empty {}))
    );
    let text = r#"{"v":1,"seq":2,"kind":"set-var","body":{"file":"a","name":"x","value":false,"defining_form_gen":1,"edit_epoch":0}}"#;
    assert!(matches!(
        decode(text).map(|e| e.body),
        Ok(ClientMsg::SetVar(SetVarBody {
            value: WireValue::Bool(false),
            ..
        }))
    ));
}

fn code_of(text: &str) -> ErrorCode {
    decode(text).expect_err(text).code
}

#[test]
fn each_protocol_error_code_comes_from_a_malformed_input() {
    assert_eq!(code_of("{"), ErrorCode::BadJson);
    assert_eq!(code_of("[1,2]"), ErrorCode::BadJson);
    assert_eq!(code_of("not json"), ErrorCode::BadJson);
    assert_eq!(
        code_of(r#"{"v":2,"seq":1,"kind":"hush","body":{}}"#),
        ErrorCode::UnsupportedVersion
    );
    assert_eq!(
        code_of(r#"{"v":"1","seq":1,"kind":"hush","body":{}}"#),
        ErrorCode::UnsupportedVersion
    );
    assert_eq!(
        code_of(r#"{"v":1,"seq":1,"kind":"eval-result","body":{}}"#),
        ErrorCode::UnknownKind
    );
    assert_eq!(
        code_of(r#"{"v":1,"seq":1,"kind":"launch","body":{}}"#),
        ErrorCode::UnknownKind
    );
    assert_eq!(
        code_of(r#"{"v":1,"seq":1,"kind":"stop","body":{"slot":3}}"#),
        ErrorCode::BadBody
    );
    assert_eq!(
        code_of(
            r#"{"v":1,"seq":1,"kind":"set-tweak","body":{"file":"a","id":1,"form_gen":1,"value":"x","edit_epoch":0}}"#
        ),
        ErrorCode::BadBody
    );
    assert_eq!(
        code_of(r#"{"v":1,"kind":"hush","body":{}}"#),
        ErrorCode::BadBody
    );
    assert_eq!(
        code_of(r#"{"seq":1,"kind":"hush","body":{}}"#),
        ErrorCode::BadBody
    );
    let huge = format!(
        r#"{{"v":1,"seq":1,"kind":"eval","body":{{"file":"a","code":"{}","doc_revision":1,"edit_epoch":0}}}}"#,
        "x".repeat(MAX_FRAME)
    );
    assert_eq!(code_of(&huge), ErrorCode::BadBody);
}

#[test]
fn truncating_valid_frames_at_every_byte_is_an_error_never_a_panic() {
    let mut frames: Vec<String> = clients()
        .into_iter()
        .map(|m| encode(&Envelope::new(1, None, m)))
        .collect();
    frames.extend(
        servers()
            .into_iter()
            .map(|m| encode(&Envelope::new(1, Some(1), m))),
    );
    for f in &frames {
        for cut in 0..f.len() {
            let Some(prefix) = f.get(..cut) else { continue };
            assert!(decode(prefix).is_err(), "{prefix:?}");
            assert!(decode_as::<ServerMsg>(prefix).is_err(), "{prefix:?}");
        }
    }
}

#[test]
fn the_session_answers_garbage_with_protocol_error_and_keeps_running() {
    let mut rig = Rig::new();
    for junk in [
        "",
        "{",
        "\u{0}\u{1}",
        r#"{"v":9}"#,
        r#"{"v":1,"seq":1,"kind":"x"}"#,
    ] {
        let out = rig.s.apply_text(7, junk);
        assert_eq!(out.len(), 1, "{junk:?}");
        assert!(matches!(&out[0].env.body, ServerMsg::ProtocolError(_)));
        assert_eq!(out[0].to, crate::session::Dest::Conn(7));
    }
    // A bad body inside a valid envelope is answered too, with `re`.
    let out = rig.s.apply_text(
        7,
        r#"{"v":1,"seq":5,"kind":"stop","body":{"slot":"d1\nhush"}}"#,
    );
    assert!(
        matches!(&out[0].env.body, ServerMsg::ProtocolError(e) if e.code == ErrorCode::BadBody)
    );
    assert_eq!(out[0].env.re, Some(5));
    // Still alive: an eval works.
    let r = rig.ok("let x 1", 1);
    assert_eq!(r.forms[0].value.as_deref(), Some("1"));
}
