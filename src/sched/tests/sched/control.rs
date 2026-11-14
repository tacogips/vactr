//! TASK-007 criteria 3, 4 and 10 (pattern and texture slots): stop, hush,
//! tempo and set-tweak across boundaries, `once`/`at`, the audio, MIDI and
//! OSC sinks with one (slot, gen) identity, the immediate-class monotone
//! merge, re-send, the gen piggyback and the transport threshold (design
//! 11.3). One cycle is 2 s; ticks are 10 ms.

use super::{played, started, Rig};
use crate::host::caps::MidiEvent;
use crate::host::testing::{RenderCall, SinkCall};
use crate::host::wire::{HostMsg, Release, SlotControlAck};
use crate::sched::control::ControlClass;
use crate::sched::runtime::RuntimeConfig;
use crate::sched::slots::SlotId;
use crate::types::diag::DiagCode;
use crate::value::value::Value;

const S1: SlotId = SlotId::new(1);

#[test]
fn stop_reaches_every_sink_with_one_identity_and_new_events_cease() {
    let l = 0.01;
    let mut rig = Rig::new();
    rig.ack_delay = Some(l);
    rig.run("s [:bd :sd :hh :cp] > d1");
    rig.run_to(0.9);
    rig.run("stop :d1");
    rig.run_to(3.0);
    let stop: Vec<_> = rig
        .controls()
        .into_iter()
        .filter(|(_, c)| c.release == Release::Natural)
        .collect();
    assert_eq!(stop.len(), 1);
    let c = stop[0].1;
    assert_eq!((c.slot, c.new_gen), (S1, 2));
    let midi = rig
        .midi_calls()
        .iter()
        .filter(|(_, x)| *x == SinkCall::Control(c))
        .count();
    let osc = rig
        .osc_calls()
        .iter()
        .filter(|(_, x)| *x == SinkCall::Control(c))
        .count();
    assert_eq!((midi, osc), (1, 1), "the same (slot, gen) on every sink");
    // Nothing new is committed after the stop, and nothing committed
    // before it starts after the control landed.
    assert!(rig.sent().iter().all(|(t, _)| *t <= 0.9 + 1e-9));
    let voices = started(&played(&rig, l, &[]));
    assert!(voices.iter().all(|v| v.ev.time < 0.9 + l));
    // Natural: the voice that started before the stop is not gated.
    assert!(voices.iter().all(|v| v.cut.is_none()));
}

#[test]
fn hush_panic_gates_sounding_voices_and_clears_texture_outputs() {
    let l = 0.01;
    let mut rig = Rig::new();
    rig.ack_delay = Some(l);
    rig.run("s [:bd :sd] > d1\ns :hh > d2\nosc 10 > out o0");
    rig.run_to(1.02);
    let programs = |rig: &Rig| {
        rig.render
            .calls()
            .into_iter()
            .filter_map(|(_, c)| match c {
                RenderCall::SetProgram(_, sh) => Some(sh.source.is_empty()),
                RenderCall::SetUniforms(..) => None,
            })
            .collect::<Vec<bool>>()
    };
    assert_eq!(programs(&rig), vec![false], "o0 activated once");
    rig.run("hush");
    rig.run_to(1.5);
    let panics: Vec<_> = rig
        .controls()
        .into_iter()
        .filter(|(_, c)| c.release == Release::Panic)
        .map(|(_, c)| c.slot)
        .collect();
    assert_eq!(panics.len(), 2, "one Panic per pattern slot");
    assert_eq!(programs(&rig), vec![false, true], "o0 cleared (black)");
    // The d1 voice that started at 1.0 is gated when the Panic lands.
    let voices = played(&rig, l, &[]);
    let v = voices
        .iter()
        .find(|v| v.ev.slot == S1 && (v.ev.time - 1.0).abs() < 1e-9)
        .expect("the 1.0 s voice");
    assert_eq!(v.start, Some(1.0));
    assert!((v.cut.expect("gated") - (1.02 + l)).abs() < 1e-9);
}

#[test]
fn hush_then_stop_keeps_panic() {
    let mut rig = Rig::new();
    rig.ack_delay = None;
    rig.run("s :bd > d1");
    rig.run_to(0.1);
    rig.run("hush\nstop :d1");
    let e = rig
        .rt
        .control
        .outstanding(S1, ControlClass::Immediate)
        .expect("outstanding");
    assert_eq!(e.ctl.release, Release::Panic);
    assert_eq!(e.ctl.new_gen, 3);
    let last = rig.controls().last().expect("sent").1;
    assert_eq!(last.release, Release::Panic, "never downgraded to Natural");
}

#[test]
fn hush_then_tempo_keeps_panic_with_the_tempo_generation() {
    let mut rig = Rig::new();
    rig.ack_delay = None;
    rig.run("s :bd > d1");
    rig.run_to(0.1);
    rig.run("hush\nuse-bpm 140");
    let e = rig
        .rt
        .control
        .outstanding(S1, ControlClass::Immediate)
        .expect("outstanding");
    assert_eq!(e.ctl.release, Release::Panic);
    assert_eq!(e.ctl.new_gen, 3, "the tempo's gen bump rides the entry");
}

#[test]
fn hush_then_rebind_keeps_both_entries_in_order() {
    let mut rig = Rig::new();
    rig.ack_delay = None;
    rig.run("s :bd > d1");
    rig.run_to(0.5);
    rig.run("hush\ns :sd > d1");
    let imm = rig
        .rt
        .control
        .outstanding(S1, ControlClass::Immediate)
        .expect("immediate");
    let fut = rig
        .rt
        .control
        .outstanding(S1, ControlClass::Future)
        .expect("future");
    assert_eq!(imm.ctl.release, Release::Panic);
    assert_eq!(fut.ctl.release, Release::None);
    assert!(imm.ctl.effective_time < fut.ctl.effective_time);
    assert!((fut.ctl.effective_time - 2.0).abs() < 1e-9);
    let sent: Vec<_> = rig.controls().into_iter().map(|(_, c)| c.release).collect();
    assert_eq!(sent, vec![Release::Panic, Release::None]);
    // Re-sends keep the effective-time order.
    rig.run_to(0.53);
    let sent: Vec<_> = rig.controls().into_iter().map(|(_, c)| c.release).collect();
    assert_eq!(
        sent,
        vec![Release::Panic, Release::None, Release::Panic, Release::None]
    );
}

#[test]
fn duplicate_delivery_is_idempotent_under_the_merge() {
    let l = 0.02;
    let mut rig = Rig::new();
    rig.ack_delay = None;
    rig.run("s [:bd :sd :hh :cp] > d1");
    rig.run_to(0.95);
    rig.run("stop :d1");
    rig.run_to(1.2);
    let controls = rig.controls();
    assert!(controls.len() >= 5, "re-sent while unacknowledged");
    assert!(controls.windows(2).all(|w| w[0].1 == w[1].1));
    // Applying the duplicates changes nothing: dropping every copy but the
    // first gives the same played voices.
    let dups: Vec<usize> = (1..controls.len()).collect();
    assert_eq!(played(&rig, l, &[]), played(&rig, l, &dups));
}

#[test]
fn a_stale_started_midi_note_gets_an_immediate_note_off() {
    let mut rig = Rig::new();
    rig.run("s {midi 1} > note [60 62 64 67] > d1");
    rig.run_to(0.98);
    // The note at 1.0 s is committed; the stop's effective time is 0.98 s.
    rig.run("stop :d1");
    let calls = rig.midi_calls();
    let note = calls
        .iter()
        .find_map(|(_, c)| match c {
            SinkCall::Send(MidiEvent::Note {
                time, note, gen, ..
            }) if (*time - 1.0).abs() < 1e-9 => Some((*note, *gen)),
            _ => None,
        })
        .expect("the 1.0 s note was committed");
    assert_eq!(note, (64, 1));
    let off = calls.iter().find_map(|(t, c)| match c {
        SinkCall::Send(MidiEvent::NoteOff {
            note, gen, time, ..
        }) => Some((*t, *note, *gen, *time)),
        _ => None,
    });
    let (sent_at, n, g, at) = off.expect("a note-off");
    assert!((sent_at - 0.98).abs() < 1e-9 && (at - 1.0).abs() < 1e-9);
    assert_eq!((n, g), (64, 1));
    // The control reached the MIDI sink before the note-off.
    let ctl_at = calls
        .iter()
        .position(|(_, c)| matches!(c, SinkCall::Control(_)))
        .expect("control");
    let off_at = calls
        .iter()
        .position(|(_, c)| matches!(c, SinkCall::Send(MidiEvent::NoteOff { .. })))
        .expect("off");
    assert!(ctl_at < off_at);
}

#[test]
fn osc_revocation_reaches_only_the_untransmitted_queue() {
    let l = 0.05;
    let mut rig = Rig::new();
    rig.ack_delay = Some(l);
    rig.run("s osc-a > d1");
    rig.run_to(1.98);
    rig.run("stop :d1");
    rig.run_to(2.5);
    let calls = rig.osc_calls();
    let sends: Vec<_> = calls
        .iter()
        .filter_map(|(t, c)| match c {
            SinkCall::Send(e) => Some((*t, e.clone())),
            SinkCall::Control(_) => None,
        })
        .collect();
    // Cycle 0 and the committed cycle-1 message (2.0 s) were handed over.
    assert_eq!(sends.len(), 2);
    let control_at = calls
        .iter()
        .find_map(|(t, c)| matches!(c, SinkCall::Control(_)).then_some(*t))
        .expect("the stop reached the OSC sink");
    let landed = control_at + l;
    // The 2.0 s message transmits before the control lands (2.03 s): it is
    // irrevocable and stays sent. Nothing is committed after the stop.
    let late = &sends[1].1;
    assert!((late.time - 2.0).abs() < 1e-9 && late.time < landed);
    assert!(sends.iter().all(|(t, _)| *t < control_at + 1e-9));
    assert_eq!(&*late.addr, "/a");
}

#[test]
fn a_structural_tweak_restores_and_removes_uncommitted_events() {
    // `maybe` 0 -> 1 restores events the previous staging had dropped.
    let mut rig = Rig::new();
    rig.run("let p maybe {s [:bd :sd :hh :cp]} 0\np > d1");
    // At 0.45 s the 0.5 s step is inside the query horizon (staged, and
    // dropped by `maybe 0`) but not yet committed.
    rig.run_to(0.45);
    assert_eq!(rig.sent().len(), 0);
    let site = rig.ev.sites_of("p")[0].clone();
    rig.ev
        .set_tweak(site.id, site.form_gen, Value::Int(1))
        .expect("tweak");
    rig.rt.drain(&mut rig.ev);
    rig.run_to(1.9);
    let times: Vec<f64> = rig.sent().iter().map(|(_, e)| e.time).collect();
    assert_eq!(times, vec![0.5, 1.0, 1.5]);

    // `degrade-by` 0 -> 1 removes events that were already staged.
    let mut rig = Rig::new();
    rig.run("var q 0\ndegrade-by {s [:bd :sd :hh :cp]} q > d1");
    rig.run_to(0.45);
    rig.run("upd q 1");
    rig.run_to(1.9);
    let times: Vec<f64> = rig.sent().iter().map(|(_, e)| e.time).collect();
    assert_eq!(times, vec![0.0], "0.5 s was staged, never committed");
}

#[test]
fn once_and_at_play_on_ephemeral_slots() {
    let mut rig = Rig::new();
    rig.run("s :bd > d1");
    rig.run_to(0.5);
    rig.run("s :crash > once\ns :crash > once at: 4\nat 2:\n\ts :sd > once");
    rig.run_to(3.5);
    let others: Vec<(u32, f64)> = rig
        .sent()
        .iter()
        .filter(|(_, e)| e.slot != S1)
        .map(|(_, e)| (e.slot.get(), e.time))
        .collect();
    assert_eq!(others.len(), 3, "{others:?}");
    // `once` starts at the commit horizon; `at: 4` four beats (one cycle)
    // from now; `at 2` runs its block two beats from now.
    assert!((others[0].1 - 0.53).abs() < 0.011, "{others:?}");
    // The `at` block runs when its beat (1.5 s) is reached; the `once` it
    // stages starts at that tick's commit horizon.
    assert!(
        others.iter().any(|(_, t)| (t - 1.53).abs() < 0.011),
        "{others:?}"
    );
    assert!(others.iter().any(|(_, t)| (t - 2.5).abs() < 0.011));
    // Each on its own ephemeral slot, removed after its cycle.
    let mut slots: Vec<u32> = others.iter().map(|(s, _)| *s).collect();
    slots.dedup();
    assert_eq!(slots.len(), 3);
    rig.run_to(5.0);
    assert_eq!(rig.rt.slots().iter().count(), 1, "only d1 is left");
}

#[test]
fn a_tempo_change_re_anchors_and_re_commits_under_a_new_generation() {
    let l = 0.01;
    let mut rig = Rig::new();
    rig.ack_delay = Some(l);
    rig.run("s [:bd :sd :hh :cp] > d1");
    rig.run_to(0.98);
    rig.run("use-bpm 60");
    rig.run_to(2.0);
    // Old gen: the 1.0 s event was committed but revoked (effective 0.98).
    let voices = played(&rig, l, &[]);
    let old = voices
        .iter()
        .find(|v| v.ev.gen == 1 && (v.ev.time - 1.0).abs() < 1e-9)
        .expect("committed before the change");
    assert!(old.start.is_none());
    // New gen: positions 1/2 and 3/4 re-committed at the new tempo (one
    // cycle = 4 s from the re-anchored position), each once.
    let new: Vec<f64> = rig
        .sent()
        .iter()
        .filter(|(_, e)| e.gen == 2)
        .map(|(_, e)| e.time)
        .collect();
    let clock = rig.rt.clock();
    let want = [clock.to_host(super::r(1, 2)), clock.to_host(super::r(3, 4))];
    assert_eq!(new, want);
    assert!(want[1] - want[0] > 0.99, "a quarter cycle is 1 s at 60 bpm");
}

#[test]
fn a_lost_future_control_leaves_only_the_default_piggyback_policy() {
    let cfg = RuntimeConfig {
        resend_ticks: 100,
        ..RuntimeConfig::default()
    };
    let mut rig = Rig::with(cfg, crate::dsp::caps::CapabilitySet::native());
    rig.ack_delay = None;
    rig.run("s [:bd :bd :bd :bd] > d1");
    rig.run_to(1.5);
    rig.run("s [:sd :sd :sd :sd] > d1");
    rig.run_to(2.2);
    // The first (only) delivery of the future control is lost; the new
    // generation's boundary batch arrives first (committed ~1.97 s).
    let lost = [0];
    let voices = played(&rig, 0.0, &lost);
    let batch_at = rig
        .sent()
        .iter()
        .find(|(_, e)| e.gen == 2)
        .map(|(t, _)| *t)
        .expect("new batch");
    assert!(batch_at < 2.0);
    // Over-eager but safe: the old 1.5 s voice is short-gated at the batch's
    // arrival, before the boundary (the effective time is unknown).
    let v = voices
        .iter()
        .find(|v| v.ev.gen == 1 && (v.ev.time - 1.5).abs() < 1e-9)
        .expect("old voice");
    assert_eq!(v.cut, Some(batch_at));
    // The re-send (every 100 ticks) delivers the full control later.
    rig.run_to(2.6);
    let futures = rig
        .controls()
        .into_iter()
        .filter(|(_, c)| c.new_gen == 2)
        .count();
    assert_eq!(futures, 2);
    rig.audio
        .reply(HostMsg::SlotControlAck(SlotControlAck { slot: S1, gen: 2 }));
    rig.run_to(2.62);
    assert!(rig
        .rt
        .control
        .outstanding(S1, ControlClass::Future)
        .is_none());
}

#[test]
fn repeated_loss_resends_and_raises_the_transport_diagnostic_once() {
    let mut rig = Rig::new();
    rig.ack_delay = None;
    rig.run("s :bd > d1");
    rig.run_to(0.1);
    rig.run("hush");
    rig.run_to(0.1 + 30.0 * 0.01);
    let sends = rig
        .controls()
        .iter()
        .filter(|(_, c)| c.release == Release::Panic)
        .count();
    assert_eq!(sends, 1 + 10, "the first send plus one per 3 ticks");
    let transport: Vec<_> = rig
        .diags()
        .into_iter()
        .filter(|d| d.code == DiagCode::HostTransport)
        .collect();
    assert_eq!(transport.len(), 1, "{transport:?}");
    assert!(transport[0]
        .message
        .contains("not acknowledged after 20 ticks"));
}
