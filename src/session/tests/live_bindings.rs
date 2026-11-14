//! Full-file Update removes absent sinks; incremental and failed edits do not.

use super::support::{Rig, DOC};
use crate::host::testing::AudioCall;
use crate::host::wire::{CtlMsg, Release};
use crate::ns::stage::SlotKey;
use crate::session::protocol::{ClientMsg, EvalBody, ServerMsg, WireSpan, WireValue};

fn idle(rig: &Rig, key: u8) -> bool {
    rig.s
        .runtime()
        .slots()
        .get(SlotKey::D(key))
        .is_none_or(|slot| slot.is_idle())
}

fn eval_file(rig: &mut Rig, file: &str, source: &str, revision: u64) -> bool {
    let (out, _) = rig.s.eval(source, file, revision, 0, None);
    !out.has_errors()
}

#[test]
fn commented_and_deleted_forms_stop_only_their_owned_slots() {
    for update in ["# s :analog > d1\ns :analog > d2", "s :analog > d2"] {
        let mut rig = Rig::new();
        assert!(eval_file(
            &mut rig,
            DOC,
            "s :analog > d1\ns :analog > d2",
            1
        ));
        assert!(eval_file(&mut rig, "other.vact", "s :analog > d3", 1));
        rig.run_to(1.2);
        let other_generation = rig.s.runtime().slot_gen(SlotKey::D(3));
        let calls = rig.audio.calls().len();
        assert!(eval_file(&mut rig, DOC, update, 2));
        assert!(idle(&rig, 1));
        assert!(!idle(&rig, 2));
        assert!(!idle(&rig, 3));
        assert_eq!(rig.s.runtime().slot_gen(SlotKey::D(3)), other_generation);
        let cleanup = rig.audio.calls();
        assert!(cleanup[calls..].iter().any(|(_, call)| matches!(call,
            AudioCall::Control(control) if control.slot.get() == 1 && control.release == Release::Natural)));
        assert!(!cleanup[calls..]
            .iter()
            .any(|(_, call)| matches!(call, AudioCall::Post(CtlMsg::OutputStop { .. }))));
        let sent = rig.sent().len();
        rig.run_to(4.2);
        let after = &rig.sent()[sent..];
        assert!(after.iter().all(|event| event.slot.get() != 1));
        assert!(after.iter().any(|event| event.slot.get() == 2));
        assert!(after.iter().any(|event| event.slot.get() == 3));
    }
}

#[test]
fn documents_without_sinks_stop_their_last_live_binding() {
    for update in ["", "# s :analog > d1\n", "let x 1"] {
        let mut rig = Rig::new();
        rig.ok("s :analog > d1", 1);
        rig.run_to(0.2);
        assert!(eval_file(&mut rig, DOC, update, 2));
        assert!(idle(&rig, 1));
        assert!(!rig.s.runtime().playback_active());
        let sent = rig.sent().len();
        rig.run_to(3.0);
        assert_eq!(rig.sent().len(), sent);
    }
}

#[test]
fn failed_parse_check_eval_and_runtime_bind_preserve_absent_slots() {
    for update in [
        "[",
        "let bad + 1 \"text\"",
        "let bad / 1 0",
        "s :not-a-sound > d2",
    ] {
        let mut rig = Rig::new();
        rig.ok("s :analog > d1\ns :analog > d2", 1);
        rig.run_to(0.2);
        assert!(!eval_file(&mut rig, DOC, update, 2), "{update}");
        assert!(!idle(&rig, 1), "{update}");
        assert!(!idle(&rig, 2), "{update}");
    }
}

#[test]
fn failed_other_file_takeover_and_tempo_change_do_not_transfer_audible_ownership() {
    let mut rig = Rig::new();
    assert!(eval_file(&mut rig, "other.vact", "s :analog > d1", 1));
    rig.run_to(0.2);
    assert!(!eval_file(&mut rig, DOC, "s :not-a-sound > d1", 1));
    // Tempo bumps scheduling generations, but never accepts another binding.
    assert!(eval_file(&mut rig, DOC, "use-bpm 140", 2));
    assert!(!idle(&rig, 1));
    assert!(eval_file(&mut rig, DOC, "", 3));
    assert!(!idle(&rig, 1));
    assert!(eval_file(&mut rig, "other.vact", "", 2));
    assert!(idle(&rig, 1));
}

#[test]
fn successful_same_slot_takeover_belongs_to_the_new_file() {
    let mut rig = Rig::new();
    assert!(eval_file(&mut rig, DOC, "s :analog > d1", 1));
    rig.run_to(0.2);
    assert!(eval_file(&mut rig, "other.vact", "s :bd > d1", 1));
    let generation = rig.s.runtime().slot_gen(SlotKey::D(1));
    assert!(eval_file(&mut rig, DOC, "", 2));
    assert!(!idle(&rig, 1));
    assert_eq!(rig.s.runtime().slot_gen(SlotKey::D(1)), generation);
    assert!(eval_file(&mut rig, "other.vact", "", 2));
    assert!(idle(&rig, 1));
}

#[test]
fn tick_time_set_var_repair_transfers_accepted_binding_ownership() {
    let mut rig = Rig::new();
    assert!(eval_file(&mut rig, "other.vact", "s :analog > d1", 1));
    rig.run_to(0.2);
    let (attempt, _) = rig.s.eval(
        "var broken true\ns {if broken :not-a-sound :analog} > d1",
        DOC,
        1,
        0,
        None,
    );
    assert!(attempt.has_errors());
    assert!(rig
        .set_var(
            "broken",
            WireValue::Bool(false),
            attempt.forms[0].form_gen.get(),
            0
        )
        .is_empty());
    rig.tick();
    let file = rig.s.lookup_file(DOC).unwrap();
    assert_eq!(
        rig.s
            .runtime()
            .slots()
            .get(SlotKey::D(1))
            .unwrap()
            .binding_file,
        Some(file)
    );
    let generation = rig.s.runtime().slot_gen(SlotKey::D(1));
    assert!(eval_file(&mut rig, "other.vact", "", 2));
    assert!(!idle(&rig, 1));
    assert_eq!(rig.s.runtime().slot_gen(SlotKey::D(1)), generation);
    assert!(eval_file(&mut rig, DOC, "", 2));
    assert!(idle(&rig, 1));
}

#[test]
fn deleting_failed_attempt_retires_its_reactive_owner_without_stopping_other_file() {
    let mut rig = Rig::new();
    assert!(eval_file(&mut rig, "other.vact", "s :analog > d1", 1));
    rig.run_to(0.2);
    assert!(!eval_file(
        &mut rig,
        DOC,
        "var broken true\ns {if broken :not-a-sound :analog} > d1",
        1
    ));
    assert!(eval_file(&mut rig, DOC, "", 2));
    assert!(!idle(&rig, 1));
    assert!(rig.s.evaluator().form_of_bind(SlotKey::D(1)).is_none());
    let generation = rig.s.runtime().slot_gen(SlotKey::D(1));
    assert!(!rig.s.eval_console("upd broken false").failed);
    assert!(!idle(&rig, 1));
    assert_eq!(rig.s.runtime().slot_gen(SlotKey::D(1)), generation);
    assert!(eval_file(&mut rig, "other.vact", "", 2));
    assert!(idle(&rig, 1));
}

#[test]
fn partial_form_evaluation_remains_incremental() {
    let mut rig = Rig::new();
    rig.ok("s :analog > d1\ns :analog > d2", 1);
    let source = "let value 1";
    rig.send(ClientMsg::Eval(EvalBody {
        file: DOC.to_string(),
        code: source.to_string(),
        span: Some(WireSpan::new(0, source.len() as u32)),
        doc_revision: 2,
        edit_epoch: 0,
    }));
    assert!(!idle(&rig, 1));
    assert!(!idle(&rig, 2));
}

#[test]
fn consecutive_console_entries_never_reconcile_previous_live_slots() {
    let mut rig = Rig::new();
    assert!(!rig.s.eval_console("s :analog > d1").failed);
    assert!(!rig.s.eval_console("s :analog > d2").failed);
    assert!(!rig.s.eval_console("+ 1 2").failed);
    assert!(!idle(&rig, 1));
    assert!(!idle(&rig, 2));
}

#[test]
fn removed_reactive_binding_cannot_resurrect_after_dependency_redefinition() {
    let mut rig = Rig::new();
    rig.ok("let p s :analog\np > d1", 1);
    rig.run_to(0.2);
    rig.ok("let p s :bd", 2);
    assert!(idle(&rig, 1));
    assert!(rig.s.evaluator().form_of_bind(SlotKey::D(1)).is_none());
    assert!(eval_file(&mut rig, "other.vact", "let p s :sd", 1));
    rig.run_to(3.0);
    assert!(idle(&rig, 1));
}

#[test]
fn removal_retracts_announcements_and_cancels_pending_rebinds() {
    let mut rig = Rig::new();
    rig.ok("s :analog > note [60 62 64 65] > d1", 1);
    let messages = rig.run_to(0.39);
    let pending = messages
        .iter()
        .filter_map(|msg| match msg {
            ServerMsg::Playing(body) => Some(&body.ahead),
            _ => None,
        })
        .flatten()
        .find(|event| event.time > 0.4)
        .and_then(|event| event.id)
        .expect("future preview");
    rig.ok("", 2);
    let messages = rig.run_to(0.41);
    assert!(messages
        .iter()
        .any(|msg| matches!(msg, ServerMsg::Playing(body) if body.retract.contains(&pending))));

    rig.ok("s :analog > d1", 3);
    rig.run_to(0.5);
    rig.ok("s :bd > d1", 4);
    assert!(rig
        .s
        .runtime()
        .slots()
        .get(SlotKey::D(1))
        .unwrap()
        .pending
        .is_some());
    rig.ok("# removed", 5);
    assert!(idle(&rig, 1));
    let sent = rig.sent().len();
    rig.run_to(3.0);
    assert_eq!(rig.sent().len(), sent);
}
