//! Reactive publication (design 14.5.5; TASK-009 criterion 1): every
//! TASK-005 trace, program text copied from `ns/tests/reactive_traces.rs`
//! and `reactive_recovery.rs`, driven through `set-var` + tick with a
//! subscriber that records every message. Exactly ONE `bindings` batch per
//! completed pass, after all rounds; no provisional value in any batch and
//! no staged effect of a rolled-back form at the recording host.

use super::support::{batches, Rig};
use crate::host::testing::AudioCall;
use crate::ns::depgraph::FormState;
use crate::ns::stage::SlotKey;
use crate::session::protocol::{BindingsBody, ServerMsg, WireFormState, WireState, WireValue};

/// The current defining generation of `name`.
fn gen(rig: &Rig, name: &str) -> u64 {
    let ev = rig.s.evaluator();
    ev.form_of(name)
        .and_then(|f| ev.graph().get(f))
        .map(|r| r.gen.get())
        .unwrap_or_else(|| panic!("`{name}` has no form"))
}

/// Queues `writes` as set-vars (one tick coalesces them into ONE pass) and
/// returns the batches of that tick.
fn upd(rig: &mut Rig, writes: &[(&str, WireValue)]) -> Vec<BindingsBody> {
    for (name, v) in writes {
        let g = gen(rig, name);
        let out = rig.set_var(name, *v, g, 0);
        assert!(out.is_empty(), "{name}: {out:?}");
    }
    let out = rig.tick();
    batches(&out)
}

fn int(n: i64) -> WireValue {
    WireValue::Int(n)
}

fn boolean(b: bool) -> WireValue {
    WireValue::Bool(b)
}

fn state<'a>(b: &'a BindingsBody, name: &str) -> &'a WireFormState {
    b.states
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no state for `{name}` in {b:?}"))
}

fn value_of(b: &BindingsBody, name: &str) -> Option<String> {
    b.changed
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.value.clone())
}

/// Every display string any batch in the log carries.
fn published_strings(log: &[ServerMsg]) -> Vec<String> {
    let mut out = Vec::new();
    for b in batches(log) {
        out.extend(b.changed.iter().map(|c| c.value.clone()));
        out.extend(b.states.iter().map(|s| s.value.clone()));
    }
    out
}

/// Every value published for `name`, in any batch.
fn published_for(log: &[ServerMsg], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for b in batches(log) {
        out.extend(
            b.changed
                .iter()
                .filter(|c| c.name == name)
                .map(|c| c.value.clone()),
        );
        out.extend(
            b.states
                .iter()
                .filter(|s| s.name == name)
                .map(|s| s.value.clone()),
        );
    }
    out
}

/// The number of recorded audio-host calls.
fn host_calls(rig: &Rig) -> usize {
    rig.audio.calls().len()
}

/// The controls and binds the host has seen for slot `d1`'s generation.
fn d1_gen(rig: &Rig) -> Option<u32> {
    rig.s.runtime().slot_gen(SlotKey::D(1))
}

#[test]
fn changing_edge_publishes_one_batch_after_all_rounds_with_final_values() {
    let mut rig = Rig::new();
    rig.ok(
        "var switch false\nvar root 10\nlet a d1 {if switch b root}\nlet b d2 {if switch root {+ a 1}}",
        1,
    );
    rig.clear();
    let bs = upd(&mut rig, &[("switch", boolean(true)), ("root", int(20))]);
    assert_eq!(bs.len(), 1, "exactly one batch for the pass: {bs:?}");
    let b = &bs[0];
    assert_eq!(value_of(b, "a").as_deref(), Some("20"));
    assert_eq!(value_of(b, "b").as_deref(), Some("20"));
    assert_eq!(state(b, "a").state, WireState::Ok);
    assert_eq!(state(b, "b").state, WireState::Ok);
    // Only final-round values: A's aborted round never shows.
    assert_eq!(published_for(&rig.log, "a"), vec!["20", "20"]);
    assert_eq!(batches(&rig.log).len(), 1, "nothing mid-pass or mid-round");
}

#[test]
fn failed_diamond_publishes_failure_block_and_recovery() {
    let mut rig = Rig::new();
    rig.ok(
        "var root 1\nlet left / 1 root\nlet right + root 1\nlet total + left right\nd1 total",
        1,
    );
    rig.clear();
    let bs = upd(&mut rig, &[("root", int(0))]);
    assert_eq!(bs.len(), 1);
    let b = &bs[0];
    assert_eq!(value_of(b, "right").as_deref(), Some("1"));
    let left = state(b, "left");
    assert_eq!(left.state, WireState::Failed);
    assert_eq!(left.value, "1", "the restored committed value");
    assert_eq!(
        left.diagnostic.as_ref().map(|d| d.code.as_str()),
        Some("division-by-zero")
    );
    let total = state(b, "total");
    assert_eq!(total.state, WireState::Blocked);
    assert_eq!(total.blocked_on.as_deref(), Some("left"));
    assert_eq!(total.value, "3", "the previous committed total");
    // A subscriber never sees a partial set: the one batch holds every
    // scheduled form.
    for name in ["left", "right", "total"] {
        state(b, name);
    }
    // Recovery: the unblocked recomputation is published as `ok`.
    let bs = upd(&mut rig, &[("root", int(1))]);
    assert_eq!(bs.len(), 1);
    let b = &bs[0];
    assert_eq!(state(b, "left").state, WireState::Ok);
    assert!(state(b, "left").diagnostic.is_none());
    assert_eq!(state(b, "total").state, WireState::Ok);
    assert_eq!(state(b, "total").value, "3");
    assert_eq!(value_of(b, "right").as_deref(), Some("2"));
}

#[test]
fn provisional_rollback_publishes_restored_x_and_nothing_reaches_the_host() {
    let mut rig = Rig::new();
    rig.ok(
        "var n 0\nlet x d1 {if {> n 0} {/ 1 {- 12 y}} 7}\nlet z if {> n 0} 5 3\nlet y + z 7",
        1,
    );
    rig.run_to(0.5);
    let (calls, d1) = (host_calls(&rig), d1_gen(&rig));
    rig.clear();
    let bs = upd(&mut rig, &[("n", int(1))]);
    assert_eq!(bs.len(), 1);
    let b = &bs[0];
    let x = state(b, "x");
    assert_eq!((x.state, x.value.as_str()), (WireState::Failed, "7"));
    assert_eq!(state(b, "z").value, "5");
    assert_eq!(state(b, "y").value, "12");
    assert!(
        !published_strings(&rig.log).contains(&"1/2".to_string()),
        "the provisional 1/2 appears in no batch"
    );
    // No staged bind, revocation or cell update of X reached the host.
    assert_eq!(d1_gen(&rig), d1, "d1 was not rebound");
    let after: Vec<_> = rig.audio.calls()[calls..]
        .iter()
        .filter(|(_, c)| matches!(c, AudioCall::Control(_) | AudioCall::Post(_)))
        .cloned()
        .collect();
    assert!(after.is_empty(), "{after:?}");
}

#[test]
fn abort_retry_publishes_a_failed_with_its_previous_value_and_b_zero() {
    let mut rig = Rig::new();
    rig.ok(
        "var switch false\nvar root 2\nlet a d1 {if switch {/ 1 b} root}\nlet b if switch 0 {+ a 1}",
        1,
    );
    rig.clear();
    let bs = upd(&mut rig, &[("switch", boolean(true))]);
    assert_eq!(bs.len(), 1);
    let b = &bs[0];
    let a = state(b, "a");
    assert_eq!((a.state, a.value.as_str()), (WireState::Failed, "2"));
    assert_eq!(state(b, "b").value, "0");
    assert_eq!(value_of(b, "b").as_deref(), Some("0"));
    assert!(!published_strings(&rig.log).contains(&"1/3".to_string()));
    assert!(published_for(&rig.log, "a").iter().all(|v| v == "2"));
}

/// `broken` failed, `selected` blocked on it.
fn broken_and_selected() -> Rig {
    let mut rig = Rig::new();
    rig.ok(
        "var root 1\nvar switch true\nlet broken / 1 root\nlet selected d1 {if switch broken 7}",
        1,
    );
    let bs = upd(&mut rig, &[("root", int(0))]);
    assert_eq!(state(&bs[0], "broken").state, WireState::Failed);
    assert_eq!(state(&bs[0], "selected").state, WireState::Blocked);
    rig.clear();
    rig
}

fn is_failed(rig: &Rig, name: &str) -> bool {
    matches!(
        rig.s.evaluator().form_state(name),
        Some(FormState::Failed(_))
    )
}

#[test]
fn conditional_unblocking_clears_the_badge_and_broken_stays_failed() {
    let mut rig = broken_and_selected();
    let bs = upd(&mut rig, &[("switch", boolean(false))]);
    assert_eq!(bs.len(), 1);
    let sel = state(&bs[0], "selected");
    assert_eq!((sel.state, sel.value.as_str()), (WireState::Ok, "7"));
    assert!(sel.blocked_on.is_none() && sel.diagnostic.is_none());
    assert!(is_failed(&rig, "broken"));
    assert!(bs[0]
        .states
        .iter()
        .all(|s| s.name != "broken" || s.state == WireState::Failed));
}

#[test]
fn switch_toward_publishes_the_badge_and_repair_clears_it() {
    let mut rig = broken_and_selected();
    upd(&mut rig, &[("switch", boolean(false))]);
    let bs = upd(&mut rig, &[("switch", boolean(true))]);
    assert_eq!(bs.len(), 1);
    let sel = state(&bs[0], "selected");
    assert_eq!(sel.state, WireState::Blocked);
    assert_eq!(sel.blocked_on.as_deref(), Some("broken"));
    assert_eq!(sel.value, "7", "the last successful value survives");
    let bs = upd(&mut rig, &[("root", int(2))]);
    assert_eq!(bs.len(), 1);
    let sel = state(&bs[0], "selected");
    assert_eq!((sel.state, sel.value.as_str()), (WireState::Ok, "1/2"));
    assert_eq!(state(&bs[0], "broken").state, WireState::Ok);
}

#[test]
fn status_recovery_publishes_ok_badges_although_the_value_is_unchanged() {
    let mut rig = Rig::new();
    rig.ok(
        "var root 1\nvar switch false\nlet broken / 1 root\nlet total d1 {+ broken 1}\nlet selected d2 {if switch broken 7}",
        1,
    );
    upd(&mut rig, &[("root", int(0))]);
    let bs = upd(&mut rig, &[("switch", boolean(true))]);
    assert_eq!(state(&bs[0], "selected").state, WireState::Blocked);
    // 1 -> 0 -> 1: broken recovers to its retained value 1.
    let bs = upd(&mut rig, &[("root", int(1))]);
    assert_eq!(bs.len(), 1, "one recovery batch");
    let b = &bs[0];
    assert_eq!(state(b, "broken").state, WireState::Ok);
    assert_eq!(state(b, "broken").value, "1");
    assert_eq!(state(b, "total").state, WireState::Ok);
    assert_eq!(state(b, "total").value, "2");
    assert_eq!(state(b, "selected").state, WireState::Ok);
    assert_eq!(state(b, "selected").value, "1");
}

#[test]
fn late_failure_publishes_x_blocked_on_y_and_never_the_provisional_two() {
    let mut rig = Rig::new();
    rig.ok(
        "var n 0\nlet x d1 {if {> n 0} {+ y 1} 7}\nlet z if {> n 0} 0 1\nlet y / 1 z",
        1,
    );
    rig.run_to(0.5);
    let (calls, d1) = (host_calls(&rig), d1_gen(&rig));
    rig.clear();
    let bs = upd(&mut rig, &[("n", int(1))]);
    assert_eq!(bs.len(), 1);
    let b = &bs[0];
    let x = state(b, "x");
    assert_eq!((x.state, x.value.as_str()), (WireState::Blocked, "7"));
    assert_eq!(x.blocked_on.as_deref(), Some("y"));
    let y = state(b, "y");
    assert_eq!((y.state, y.value.as_str()), (WireState::Failed, "1"));
    assert_eq!(value_of(b, "z").as_deref(), Some("0"));
    assert!(
        published_for(&rig.log, "x").iter().all(|v| v == "7"),
        "the provisional 2 appears nowhere"
    );
    assert_eq!(d1_gen(&rig), d1);
    assert!(rig.audio.calls()[calls..]
        .iter()
        .all(|(_, c)| !matches!(c, AudioCall::Control(_) | AudioCall::Post(_))));
}

#[test]
fn newly_discovered_selector_publishes_selected_and_clears_its_badge() {
    let mut rig = Rig::new();
    rig.ok(
        "var root 1\nlet broken / 1 root\nvar a false\nvar b true\nlet selected d1 {if a {if b broken 7} 5}",
        1,
    );
    upd(&mut rig, &[("root", int(0))]);
    let bs = upd(&mut rig, &[("a", boolean(true))]);
    assert_eq!(state(&bs[0], "selected").state, WireState::Blocked);
    rig.clear();
    let bs = upd(&mut rig, &[("b", boolean(false))]);
    assert_eq!(bs.len(), 1);
    let sel = state(&bs[0], "selected");
    assert_eq!((sel.state, sel.value.as_str()), (WireState::Ok, "7"));
    assert!(sel.blocked_on.is_none());
    assert!(is_failed(&rig, "broken"), "broken stays failed");
}

#[test]
fn ordinary_failure_publishes_the_failure_then_the_repair_without_re_eval() {
    let mut rig = Rig::new();
    rig.ok("var a false\nvar b 0\nlet selected d1 {if a {/ 1 b} 5}", 1);
    rig.clear();
    let bs = upd(&mut rig, &[("a", boolean(true))]);
    assert_eq!(bs.len(), 1);
    let sel = state(&bs[0], "selected");
    assert_eq!((sel.state, sel.value.as_str()), (WireState::Failed, "5"));
    assert_eq!(
        sel.diagnostic.as_ref().map(|d| d.code.as_str()),
        Some("division-by-zero")
    );
    let bs = upd(&mut rig, &[("b", int(1))]);
    assert_eq!(bs.len(), 1);
    let sel = state(&bs[0], "selected");
    assert_eq!((sel.state, sel.value.as_str()), (WireState::Ok, "1"));
    assert!(sel.diagnostic.is_none(), "the diagnostic is cleared");
    let evals = rig
        .log
        .iter()
        .filter(|m| matches!(m, ServerMsg::EvalResult(_)))
        .count();
    assert_eq!(evals, 0, "no manual re-eval in between");
}

#[test]
fn a_pass_that_schedules_no_form_publishes_nothing() {
    let mut rig = Rig::new();
    rig.ok("var g 0.5\ns :analog > gain g > d1", 1);
    rig.clear();
    let bs = upd(&mut rig, &[("g", WireValue::Float(0.25))]);
    assert!(bs.is_empty(), "{bs:?}");
}

fn transport_samples(msgs: &[ServerMsg]) -> Vec<crate::session::protocol::TransportSample> {
    msgs.iter()
        .filter_map(|m| match m {
            ServerMsg::Tempo(t) => t.transport.clone(),
            _ => None,
        })
        .collect()
}

#[test]
fn periodic_transport_uses_matching_host_time_and_runtime_cycle_with_rate_ceiling() {
    let mut rig = Rig::new();
    let mut samples = Vec::new();
    for n in 0..=1000 {
        let time = f64::from(n) / 1000.0;
        rig.clock.set(time);
        for sample in transport_samples(&rig.tick()) {
            assert_eq!(sample.sample_time, time);
            assert_eq!(
                sample.cycle,
                crate::session::publish::ratio_pair(rig.s.runtime().clock().pos())
            );
            assert!(sample.running);
            assert_eq!(sample.latency_kind, "unavailable");
            assert_eq!(sample.latency_seconds, None);
            samples.push(sample);
        }
    }
    assert!(
        samples.len() >= 19 && samples.len() <= 21,
        "{}",
        samples.len()
    );
    for pair in samples.windows(2) {
        assert!(pair[1].sample_time - pair[0].sample_time >= 0.05);
        assert_eq!(pair[0].epoch, pair[1].epoch);
    }
    let before = samples.last().unwrap().epoch.clone();
    rig.clock.set(0.0);
    let restarted = transport_samples(&rig.tick());
    assert_ne!(restarted[0].epoch, before);
    assert_eq!(restarted[0].sample_time, 0.0);
}

#[test]
fn transport_pause_and_lost_external_clock_hold_position_and_invalidate_epoch() {
    let mut rig = Rig::new();
    rig.ok("use-clock :midi", 1);
    rig.clock.set(0.1);
    let running = transport_samples(&rig.tick())[0].clone();
    rig.s.rt.transport_stop(0.1);
    rig.clock.set(0.2);
    let paused = transport_samples(&rig.tick())[0].clone();
    assert!(!paused.running);
    assert_ne!(paused.epoch, running.epoch);
    rig.clock.set(0.3);
    let held = transport_samples(&rig.tick())[0].clone();
    assert_eq!(held.cycle, paused.cycle);
    rig.s.rt.transport_continue(0.3);
    rig.clock.set(0.4);
    let resumed = transport_samples(&rig.tick())[0].clone();
    assert!(resumed.running);
    assert_ne!(resumed.epoch, paused.epoch);
    rig.clock.set(1.0);
    let lost = transport_samples(&rig.tick())[0].clone();
    assert!(!lost.running);
    assert_ne!(lost.epoch, resumed.epoch);
    rig.clock.set(1.2);
    let still_lost = transport_samples(&rig.tick())[0].clone();
    assert_eq!(still_lost.cycle, lost.cycle);
    assert_eq!(still_lost.epoch, lost.epoch);
}

#[test]
fn scheduled_end_time_uses_original_seconds_and_preserves_source_revision() {
    use crate::ns::namespace::FormGen;
    use crate::reader::span::{FileId, Span, SrcRef};
    use crate::sched::slots::SlotKind;
    use crate::sched::telemetry::PlayingEvent;
    use crate::value::intern::intern_kw;
    use crate::value::ratio::Ratio64;
    let event = PlayingEvent {
        slot: intern_kw("d1"),
        beat: Ratio64::ZERO,
        time: 2.0,
        dur: 0.75,
        src: Some(SrcRef {
            span: Span::new(FileId::new(0), 3, 9),
            doc_revision: 42,
            form_gen: FormGen::new(7),
        }),
        kind: SlotKind::Pattern,
        reduced_lead: false,
    };
    let files = vec![std::rc::Rc::from("main.vact")];
    let first = crate::session::publish::playing_wire(&event, &files, 120.0, &|_| 42);
    let changed = crate::session::publish::playing_wire(&event, &files, 240.0, &|_| 42);
    assert_eq!(first.end_time, Some(2.75));
    assert_eq!(changed.end_time, first.end_time);
    assert_ne!(
        first.dur, changed.dur,
        "legacy beat duration still follows current BPM"
    );
    assert_eq!(changed.src.unwrap().doc_revision, 42);
}

fn published_playing(msgs: &[ServerMsg]) -> Vec<crate::session::protocol::WirePlaying> {
    msgs.iter()
        .filter_map(|m| match m {
            ServerMsg::Playing(p) => Some(p.events.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn midi_restart_at_zero_invalidates_playing_inside_snapshot_cadence() {
    let mut rig = Rig::new();
    rig.ok("use-clock :midi\ns :analog > d1", 41);
    let first = transport_samples(&rig.tick())[0].clone();
    assert_eq!(first.cycle, [0, 1]);
    rig.s.rt.transport_start(0.0);
    rig.clock.set(0.01);
    let immediate = rig.tick();
    assert!(
        transport_samples(&immediate).is_empty(),
        "cadence is preserved"
    );
    let playing = published_playing(&immediate);
    assert!(!playing.is_empty());
    let epoch = playing[0].epoch.clone();
    assert_ne!(epoch.as_deref(), Some(first.epoch.as_str()));
    for event in &playing {
        assert_eq!(event.epoch, epoch);
        assert_eq!(event.src.as_ref().unwrap().doc_revision, 41);
        assert_eq!(event.src.as_ref().unwrap().file, "main.vact");
        assert!((event.end_time.unwrap() - event.time - 2.0).abs() < 1e-9);
    }
    rig.clock.set(0.05);
    let next = transport_samples(&rig.tick());
    assert_eq!(next.len(), 1);
    assert_eq!(Some(next[0].epoch.as_str()), epoch.as_deref());
}

#[test]
fn midi_restarts_between_samples_invalidate_epoch_with_nondecreasing_position() {
    let mut rig = Rig::new();
    rig.ok("use-clock :midi\ns :analog > d1", 42);
    let first = transport_samples(&rig.tick())[0].clone();
    // Multiple successful Starts occur without an intervening publisher observation.
    rig.s.rt.transport_start(0.02);
    rig.s.rt.transport_start(0.04);
    rig.clock.set(0.1);
    let out = rig.tick();
    let next = transport_samples(&out)[0].clone();
    assert!(next.cycle[0] >= first.cycle[0]);
    assert_ne!(next.epoch, first.epoch);
    let playing = published_playing(&out);
    assert!(!playing.is_empty());
    for event in playing {
        assert_eq!(event.epoch.as_deref(), Some(next.epoch.as_str()));
        assert_eq!(event.src.unwrap().doc_revision, 42);
        assert!((event.end_time.unwrap() - event.time - 2.0).abs() < 1e-9);
    }
}

#[test]
fn restart_generation_counts_successful_starts_and_preserves_neutral_transitions() {
    let mut rig = Rig::new();
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 0);
    rig.s.rt.transport_start(0.0); // Internal clock rejects MIDI Start.
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 0);
    let initial = transport_samples(&rig.tick())[0].clone();
    rig.s.rt.transport_continue(0.0);
    rig.clock.set(0.05);
    assert_eq!(transport_samples(&rig.tick())[0].epoch, initial.epoch);
    rig.ok("use-clock :midi", 1);
    rig.tick();
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 0);
    for expected in 1..=3 {
        rig.s.rt.transport_start(0.05);
        assert_eq!(rig.s.rt.midi_clock().restart_generation(), expected);
    }
    rig.s.rt.clock_pulse(0.06);
    rig.clock.set(0.1);
    rig.tick();
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 3);
    rig.s.rt.transport_stop(0.1);
    rig.s.rt.transport_continue(0.1);
    rig.s.rt.transport_continue(0.1);
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 3);
    rig.ok("use-clock :internal", 2);
    rig.tick();
    rig.s.rt.transport_start(0.1);
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 3);
    rig.ok("use-clock :midi", 3);
    rig.tick();
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 3);
    rig.s.rt.transport_start(0.1);
    assert_eq!(rig.s.rt.midi_clock().restart_generation(), 4);
}

#[test]
fn invalid_host_samples_do_not_advance_or_publish_and_unsubscribed_samples_are_absent() {
    let mut rig = Rig::new();
    for time in [f64::NAN, f64::INFINITY, -1.0] {
        assert!(rig.s.tick(time).is_empty());
    }
    rig.send(crate::session::protocol::ClientMsg::Subscribe(
        crate::session::protocol::SubscribeBody {
            telemetry: false,
            levels: false,
            diagnostics: false,
        },
    ));
    assert!(transport_samples(&rig.tick()).is_empty());
}
