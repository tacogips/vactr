//! TASK-007 completion-criteria coverage gaps closed against the real
//! instrument registry and prelude templates (design 11.2-11.7), mirroring
//! `src/sched/tests/sched.rs`'s rig and delivery model (that rig's `Rig` is
//! `pub(super)` and not reachable from here, so this file builds its own).
//!
//! - criterion 3: `stop`/`hush` on a `once` ephemeral slot, before it ever
//!   starts and while it is sounding.
//! - criterion 4: the future control's re-send after a lost delivery
//!   carries the same full boundary (`effective_time`) and release fields
//!   as its first (lost) send -- the gen-piggyback on the event batch
//!   carries neither.
//! - criterion 8: closed by repair R7 (`QueryVm::put_output`,
//!   `src/pattern/combinators/random.rs::query_degrade`), which fixed the
//!   defect a first repro surfaced -- a filter's dropped event still
//!   forwarded its captured `print` output once per query slice.
//!   `a_restage_that_removes_staged_events_drops_their_held_print` covers
//!   the filter-before-the-printing-control ordering (never regressed);
//!   `a_filter_wrapping_the_printing_control_drops_removed_work_output`
//!   is the regression test for the fixed ordering (filter wraps the
//!   printing control).
//! - criterion 9: the dry-run state snapshot additionally covers the
//!   per-slot control channel (`ControlChannel::outstanding`) and the
//!   `ControlCells` table, not only slot/staging counts.
//! - criterion 10: `stop` targets a texture slot alone; `hush` bounds new
//!   audio commits to the control tick (not the lookahead horizon) and
//!   reaches an OSC slot's untransmitted queue only.

use std::rc::Rc;

use crate::dsp::caps::CapabilitySet;
use crate::host::caps::Hosts;
use crate::host::noop::NoopHost;
use crate::host::testing::{
    MockClock, RecordingAudioHost, RecordingMidiHost, RecordingOscHost, RecordingRenderHost,
    RenderCall, SinkCall,
};
use crate::host::wire::{AudioEvent, HostMsg, Release, SlotControl, SlotControlAck};
use crate::ns::evaluator::{Evaluator, FormOutcome};
use crate::ns::insts::InstRegistry;
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, SlotKey, StagedEffect};
use crate::pattern::eval::{InputCells, QueryVm};
use crate::pattern::pat::Pat;
use crate::reader::span::FileId;
use crate::sched::control::ControlClass;
use crate::sched::dryrun::dry_run;
use crate::sched::runtime::{DrainReport, Runtime, RuntimeConfig, RuntimeSink, TickReport};
use crate::sched::slots::{Binding, SlotId};
use crate::value::intern::intern_kw;
use crate::value::value::Value;
use crate::vm::query_vm::VmQuery;

/// A source file id for this module's forms.
const FILE: FileId = FileId::new(9);
/// The tick period of every test, seconds.
const DT: f64 = 0.01;

/// A rig over one instrument registry: a real `Evaluator` and `Runtime` on
/// a mock clock with recording hosts, matching the sched rig's shape and
/// ack-delivery model (`src/sched/tests/sched.rs`) but resolved through the
/// real `InstRegistry` (the prelude synthesis templates, `:analog`
/// included) rather than a synthetic stub.
struct GapRig {
    ev: Evaluator,
    rt: Runtime,
    sink: RuntimeSink,
    clock: MockClock,
    audio: RecordingAudioHost,
    midi: RecordingMidiHost,
    osc: RecordingOscHost,
    render: RecordingRenderHost,
    ticks: Vec<TickReport>,
    /// Control delivery delay: every recorded `SlotControl` is acknowledged
    /// `ack_delay` after it was sent (`None`: never acknowledged).
    ack_delay: Option<f64>,
    acked: usize,
}

impl GapRig {
    fn new() -> GapRig {
        GapRig::with(RuntimeConfig::default())
    }

    fn with(cfg: RuntimeConfig) -> GapRig {
        let clock = MockClock::new(0.0);
        let audio = RecordingAudioHost::new(clock.clone());
        let midi = RecordingMidiHost::new(clock.clone());
        let osc = RecordingOscHost::new(clock.clone());
        let render = RecordingRenderHost::new(clock.clone());
        let hosts = Hosts {
            audio: Box::new(audio.clone()),
            midi: Box::new(midi.clone()),
            osc: Box::new(osc.clone()),
            render: Box::new(render.clone()),
            midi_in: Box::new(NoopHost),
            samples: Box::new(NoopHost),
        };
        let reg = InstRegistry::shared();
        let resolver = Rc::new(Rc::clone(&reg));
        let (mut rt, sink) = Runtime::new(hosts, resolver, CapabilitySet::native(), cfg);
        let mut ev = Evaluator::with_insts(
            Prelude::core(),
            Box::new(NoopHost),
            Box::new(sink.clone()),
            Rc::clone(&reg),
        );
        assert!(
            reg.borrow().template_errors().is_empty(),
            "prelude template errors: {:?}",
            reg.borrow().template_errors()
        );
        let rep = rt.drain(&mut ev);
        assert!(
            rep.faults.is_empty(),
            "template install faults: {:?}",
            rep.faults
        );
        GapRig {
            ev,
            rt,
            sink,
            clock,
            audio,
            midi,
            osc,
            render,
            ticks: Vec::new(),
            ack_delay: Some(0.0),
            acked: 0,
        }
    }

    /// Evaluates `src` (every form must succeed) and drains the runtime.
    fn run(&mut self, src: &str) -> DrainReport {
        let out = self.eval(src);
        for o in &out {
            assert!(o.value.is_ok(), "{src:?}: {:?}", o.value);
        }
        self.rt.drain(&mut self.ev)
    }

    /// Evaluates `src` without asserting success and without draining.
    fn eval(&mut self, src: &str) -> Vec<FormOutcome> {
        self.ev
            .eval_str(src, FILE)
            .unwrap_or_else(|d| panic!("{src:?}: {d}"))
    }

    /// Releases one effect directly and drains (for effects source text
    /// cannot name, e.g. an auto-named ephemeral `once` slot).
    fn apply(&mut self, e: StagedEffect) -> DrainReport {
        self.sink.apply(e);
        self.rt.drain(&mut self.ev)
    }

    /// Acknowledges recorded controls whose delivery time has come.
    fn deliver_acks(&mut self) {
        let Some(delay) = self.ack_delay else {
            return;
        };
        let now = self.clock.now();
        let calls = self.audio.calls();
        let controls: Vec<(f64, SlotControl)> = calls
            .iter()
            .filter_map(|(t, c)| match c {
                crate::host::testing::AudioCall::Control(c) => Some((*t, *c)),
                _ => None,
            })
            .collect();
        while self.acked < controls.len() && controls[self.acked].0 + delay <= now + 1e-9 {
            let (_, c) = controls[self.acked];
            self.audio.reply(HostMsg::SlotControlAck(SlotControlAck {
                slot: c.slot,
                gen: c.new_gen,
            }));
            self.acked += 1;
        }
    }

    /// One tick at host time `t`.
    fn tick_at(&mut self, t: f64) {
        self.clock.set(t);
        self.deliver_acks();
        let rep = self.rt.tick(&mut self.ev, t);
        self.ticks.push(rep);
    }

    /// Ticks every `DT` from the current time up to and including `end`.
    fn run_to(&mut self, end: f64) {
        let mut t = self.clock.now();
        if self.ticks.is_empty() {
            self.tick_at(t);
        }
        while t + DT <= end + 1e-9 {
            t += DT;
            self.tick_at(t);
        }
    }

    /// Every audio event sent, with its arrival (commit tick) time.
    fn sent(&self) -> Vec<(f64, AudioEvent)> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                crate::host::testing::AudioCall::Send(e) => Some((t, e)),
                _ => None,
            })
            .collect()
    }

    /// Every slot control the audio host received, with its arrival time.
    fn controls(&self) -> Vec<(f64, SlotControl)> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                crate::host::testing::AudioCall::Control(c) => Some((t, c)),
                _ => None,
            })
            .collect()
    }

    /// Every OSC call, with its arrival time.
    fn osc_calls(&self) -> Vec<(f64, SinkCall<crate::host::caps::OscEvent>)> {
        self.osc.calls()
    }

    /// Every console line any tick forwarded so far, in order.
    fn console(&self) -> Vec<String> {
        self.ticks
            .iter()
            .flat_map(|t| t.console.iter().map(|s| s.to_string()))
            .collect()
    }
}

/// The pattern bound to a slot by source text, e.g. `s :analog > note [:c4]`.
fn pat_of(rig: &mut GapRig, src: &str) -> Pat {
    let out = rig.eval(src);
    match &out.last().expect("a form").value {
        Ok(Value::Pattern(p)) => (**p).clone(),
        other => panic!("{src:?} is not a pattern: {other:?}"),
    }
}

/// A read-only dry run of `src`'s pattern (design 11.5), asserting it
/// staged nothing.
fn dry(rig: &mut GapRig, src: &str) -> crate::pattern::query::QueryResult {
    let p = pat_of(rig, src);
    let (vm, ns) = rig.ev.vm_and_ns();
    let before = vm.effects().len();
    let mut h = VmQuery::new(vm, ns);
    let cells = InputCells::new();
    let r = dry_run(
        &Binding::Pattern(Rc::new(p)),
        &mut h as &mut dyn QueryVm,
        &cells,
        0,
    );
    assert_eq!(
        rig.ev.vm_and_ns().0.effects().len(),
        before,
        "nothing staged"
    );
    r
}

/// A full state snapshot (design 11.5/10.4 "dry run leaves state
/// unchanged"), extending `src/sched/tests/sched/dryrun.rs`'s snapshot
/// (slot/lane/staging counts, host call counts, `at` queue) with the
/// per-slot control-channel outstanding entries and the `ControlCells`
/// table -- neither of which that snapshot reads.
fn snapshot(rig: &GapRig) -> String {
    let mut s = String::new();
    for slot in rig.rt.slots().iter() {
        s += &format!(
            "{:?} gen {} pending {} bound {} lanes {};",
            slot.key,
            slot.gen,
            slot.pending.is_some(),
            slot.bound.is_some(),
            slot.lanes().len()
        );
        for l in slot.lanes() {
            s += &format!(
                "lane {} q {:?} recs {} held {} ledger {};",
                l.gen,
                l.queried_to,
                l.staging.records().len(),
                l.staging.fragments().len(),
                l.ledger.len()
            );
        }
        for class in [ControlClass::Immediate, ControlClass::Future] {
            let e = rig.rt.control.outstanding(slot.id, class);
            s += &format!("ctl {class:?} {e:?};");
        }
    }
    s += &format!(
        "audio {} midi {} osc {} render {} queued {} at {};",
        rig.audio.calls().len(),
        rig.midi.calls().len(),
        rig.osc.calls().len(),
        rig.render.calls().len(),
        rig.rt.queued(),
        rig.rt.at.len()
    );
    let mut sites = rig.rt.cells().live_sites();
    sites.sort_by(|a, b| a.0.cmp(&b.0));
    for (key, id, epoch) in &sites {
        s += &format!(
            "cell {key:?} {id:?} epoch {epoch} val {:?};",
            rig.rt.cells().value(*key)
        );
    }
    let (in_flight, pending) = rig.rt.cells().outstanding();
    s += &format!(
        "cells inflight {in_flight} pending {pending} pendinglen {} stats {:?};",
        rig.rt.cells().pending_len(),
        rig.rt.cells().stats()
    );
    s
}

// ---------------------------------------------------------------------
// Criterion 3: stop/hush on a `once` ephemeral slot.
// ---------------------------------------------------------------------

/// `stop`/`hush` cannot name an ephemeral `once#N` slot from source text
/// (`#` is not a keyword character, design 6.5.4), so a targeted revoke is
/// released directly, as the scheduler itself would release one staged
/// from a MIDI CC or another program-driven source.
#[test]
fn revoking_an_uncommitted_once_slot_never_starts_it_and_removes_the_slot() {
    let mut rig = GapRig::new();
    rig.ack_delay = Some(0.01);
    rig.run("s :analog > note [:g4] > once");
    let key = SlotKey::Named(intern_kw("once#1"));
    let sid = rig.rt.slots().get(key).expect("staged").id;
    assert!(
        rig.rt.slots().get(key).is_some(),
        "the ephemeral slot exists once staged"
    );
    rig.apply(StagedEffect::Revoke(key));
    assert!(
        rig.rt.slots().get(key).is_none(),
        "revoke removes the ephemeral slot immediately"
    );
    rig.run_to(2.0);
    assert!(
        rig.sent().is_empty(),
        "the revoked once event never started: {:?}",
        rig.sent()
    );
    // The revoke is still bounded and well-formed even though the slot
    // never played: exactly one bumped-generation Natural control landed.
    let sent = rig.controls();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].1.slot, sid);
    assert_eq!(sent[0].1.release, Release::Natural);
    assert_eq!(sent[0].1.new_gen, 2);
}

/// `hush` reaches a *sounding* `once` voice exactly like a pattern slot's
/// (Panic release), and still removes the ephemeral slot.
#[test]
fn hush_gates_a_sounding_once_voice_and_removes_its_ephemeral_slot() {
    let mut rig = GapRig::new();
    rig.ack_delay = Some(0.01);
    rig.run("s :analog > note [:g4] > once");
    let key = SlotKey::Named(intern_kw("once#1"));
    let sid = rig.rt.slots().get(key).expect("staged").id;
    // Let it commit and start (commit_lead 0.03 s; well inside 0.3 s).
    rig.run_to(0.3);
    let started: Vec<_> = rig
        .sent()
        .into_iter()
        .filter(|(_, e)| e.slot == sid)
        .collect();
    assert_eq!(started.len(), 1, "the once event committed: {:?}", started);
    let hush_at = rig.clock.now();
    rig.run("hush");
    rig.run_to(1.0);
    let panics: Vec<_> = rig
        .controls()
        .into_iter()
        .filter(|(_, c)| c.slot == sid && c.release == Release::Panic)
        .collect();
    assert_eq!(panics.len(), 1, "{panics:?}");
    assert!(
        rig.rt.slots().get(key).is_none(),
        "hush removes the ephemeral slot too"
    );
    // Bounded: the control lands within its own ack delay, not the (much
    // larger) lookahead horizon.
    assert!((panics[0].0 - hush_at).abs() < 0.02, "{panics:?}");
}

// ---------------------------------------------------------------------
// Criterion 4: full boundary/release semantics survive loss and re-send.
// ---------------------------------------------------------------------

/// A rebind's future control carries `effective_time` and `release` from
/// its very first send; losing that delivery leaves only the default
/// gen-piggyback (the boundary batch's bare `gen`, no release or
/// effective-time field on `AudioEvent` at all) until the re-send lands
/// with the identical full fields, which the scheduler then acknowledges.
#[test]
fn the_resent_future_control_carries_the_same_full_boundary_semantics_as_its_first_send() {
    let cfg = RuntimeConfig {
        resend_ticks: 100,
        ..RuntimeConfig::default()
    };
    let mut rig = GapRig::with(cfg);
    rig.ack_delay = None; // every delivery is lost until acknowledged by hand
    rig.run("s :analog > note [:c4] > d1");
    let sid = SlotId::new(1);
    rig.run_to(1.5);
    rig.run("s :analog > note [:e4] > d1");
    let first = rig
        .controls()
        .into_iter()
        .find(|(_, c)| c.new_gen == 2)
        .expect("the first future send");
    assert_eq!(first.1.release, Release::None);
    assert!((first.1.effective_time - 2.0).abs() < 1e-9, "{:?}", first.1);
    // The boundary batch (new generation) commits before the lost control
    // ever lands: only the default piggyback (bare `gen`) governs so far.
    rig.run_to(2.2);
    assert!(
        rig.sent().iter().any(|(_, e)| e.gen == 2 && e.slot == sid),
        "the new generation's boundary batch committed"
    );
    // Re-send (every 100 ticks) delivers the same full control again.
    rig.run_to(2.6);
    let resends: Vec<_> = rig
        .controls()
        .into_iter()
        .filter(|(_, c)| c.new_gen == 2)
        .collect();
    assert!(
        resends.len() >= 2,
        "re-sent while unacknowledged: {resends:?}"
    );
    let landed = resends.last().expect("a resend").1;
    assert_eq!(
        landed.release,
        Release::None,
        "the re-send carries the real release, not a piggyback guess"
    );
    assert!(
        (landed.effective_time - 2.0).abs() < 1e-9,
        "and the real boundary: {landed:?}"
    );
    rig.audio.reply(HostMsg::SlotControlAck(SlotControlAck {
        slot: sid,
        gen: 2,
    }));
    rig.run_to(2.62);
    assert!(
        rig.rt
            .control
            .outstanding(sid, ControlClass::Future)
            .is_none(),
        "acknowledged and cleared once the full control landed"
    );
}

// ---------------------------------------------------------------------
// Criterion 8: a structurally removed staged event drops its held print.
// ---------------------------------------------------------------------

/// "a control write restaging the still-uncommitted span replaces the
/// held output (no duplicate, none for removed work)": `degrade-by` sits
/// BEFORE `gain loud` in the pipe, so an event the filter drops never
/// reaches the printing control at all -- unlike the repro recorded at the
/// end of this file, where the filter wrapped the printing control and a
/// dropped event's print still ran. Cycle 0 commits and prints normally;
/// by 1.9 s cycle 1's first onset (2.0 s) is staged with its print held
/// (queried inside the 0.12 s lookahead) but not yet committed
/// (commit_lead 0.03 s). Flipping `q` to 1 there and running through all
/// of cycle 1 must never forward that held print, or any of cycle 1's,
/// and must never duplicate what cycle 0 already printed.
#[test]
fn a_restage_that_removes_staged_events_drops_their_held_print() {
    let mut rig = GapRig::new();
    rig.run(
        "var q 0\nfn loud t:\n\tprint \"p\" t\n\t0.5\n\
         s :analog > note [:c4 :d4 :e4 :f4] > degrade-by q > gain loud > d1",
    );
    rig.run_to(1.9);
    let before = rig.console();
    assert_eq!(
        before.len(),
        4,
        "cycle 0's four events committed and printed: {before:?}"
    );
    rig.run("upd q 1");
    rig.run_to(3.9); // through the whole of cycle 1 (2.0-3.5 s)
    let after = rig.console();
    assert_eq!(
        after, before,
        "no held print for a removed onset reached the console, and \
         nothing already printed was duplicated: {after:?}"
    );
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for line in &after {
        *counts.entry(line.as_str()).or_insert(0) += 1;
    }
    assert!(
        counts.values().all(|&n| n == 1),
        "every committed-before-the-upd print appears exactly once: {counts:?}"
    );
    // The audio side agrees: cycle 1's events were filtered before commit
    // too (not just their held print).
    assert_eq!(
        rig.sent().iter().filter(|(_, e)| e.time >= 2.0).count(),
        0,
        "cycle 1 never committed"
    );
}
// Criterion 9: the dry-run snapshot covers the control channel and cells.
// ---------------------------------------------------------------------

/// Extends `sched::tests::sched::dryrun`'s state-unchanged assertion with
/// the two pieces its snapshot never reads: an outstanding future control
/// entry (from an unacknowledged rebind) and a live `ControlCells` site
/// (from a `Late`-bound `gain` control) must both still be present, and
/// bit-for-bit unchanged, after a successful and a failing dry run.
#[test]
fn dry_run_leaves_the_control_channel_and_control_cells_unchanged() {
    let mut rig = GapRig::new();
    rig.ack_delay = None; // the rebind's future control stays outstanding
    rig.run("var g 0.5\ns :analog > note [:c4 :e4] > gain g > d1");
    rig.run_to(0.9);
    rig.run("s :analog > note [:g4] > gain g > d1");
    rig.run_to(0.95);
    let sid = SlotId::new(1);
    assert!(
        rig.rt
            .control
            .outstanding(sid, ControlClass::Future)
            .is_some(),
        "the rig actually exercises an outstanding future entry"
    );
    assert!(
        !rig.rt.cells().live_sites().is_empty(),
        "the rig actually exercises a live control cell"
    );
    let before = snapshot(&rig);
    let ok = dry(&mut rig, "s :analog > note [:a4] > gain g");
    assert!(ok.faults.is_empty(), "{:?}", ok.faults);
    assert_eq!(
        snapshot(&rig),
        before,
        "a successful dry run changes nothing"
    );
    let failed = dry(&mut rig, "s :nope");
    assert!(!failed.faults.is_empty());
    assert_eq!(snapshot(&rig), before, "a failing dry run changes nothing");
}

// ---------------------------------------------------------------------
// Criterion 10: stop on a texture slot; hush bounded on audio and OSC.
// ---------------------------------------------------------------------

/// `stop :o0` targets only the texture output slot: it clears the
/// program, and never touches the unrelated audio slot playing
/// concurrently (only `hush`, which revokes every slot, was covered).
#[test]
fn stop_targets_only_a_texture_slot_and_clears_its_output() {
    let mut rig = GapRig::new();
    rig.ack_delay = Some(0.01);
    rig.run("s :analog > note [:c4] > d1\nosc 20 > out o0");
    rig.run_to(0.5);
    let programs = |rig: &GapRig| {
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
    rig.run("stop :o0");
    rig.run_to(0.6);
    assert_eq!(
        programs(&rig),
        vec![false, true],
        "o0 cleared by the targeted stop"
    );
    assert!(
        rig.controls().is_empty(),
        "stop :o0 never touched d1's audio slot"
    );
    rig.run_to(2.5);
    assert!(!rig.sent().is_empty(), "d1 kept playing after stop :o0");
}

/// `hush` bounds new audio commits to the control tick, not the (larger)
/// lookahead horizon -- covered only for `stop` before -- and reaches an
/// OSC slot's untransmitted queue exactly as `stop` does, with the Panic
/// release class.
#[test]
fn hush_bounds_new_audio_events_and_revokes_only_the_untransmitted_osc_queue() {
    let l = 0.05;
    let mut rig = GapRig::new();
    rig.ack_delay = Some(l);
    rig.run("s :analog > note [:c4] > d1\ns {osc \"/x\"} > d2");
    rig.run_to(1.98);
    let hush_at = rig.clock.now();
    rig.run("hush");
    rig.run_to(2.5);
    assert!(
        rig.sent().iter().all(|(t, _)| *t <= hush_at + 1e-9),
        "no new commit past the control tick, not the lookahead horizon: {:?}",
        rig.sent()
    );
    let osc_calls = rig.osc_calls();
    let sends: Vec<(f64, f64)> = osc_calls
        .iter()
        .filter_map(|(t, c)| match c {
            SinkCall::Send(e) => Some((*t, e.time)),
            SinkCall::Control(_) => None,
        })
        .collect();
    assert!(
        !sends.is_empty(),
        "at least one message transmitted before hush"
    );
    let control_at = osc_calls
        .iter()
        .find_map(|(t, c)| matches!(c, SinkCall::Control(_)).then_some(*t))
        .expect("hush reached the osc sink");
    assert!(
        sends.iter().all(|(t, _)| *t < control_at + 1e-9),
        "already-transmitted OSC is irrevocable: {sends:?}"
    );
    let panic_ctl = osc_calls
        .iter()
        .find_map(|(_, c)| match c {
            SinkCall::Control(sc) => Some(*sc),
            SinkCall::Send(_) => None,
        })
        .expect("a control reached osc");
    assert_eq!(panic_ctl.release, Release::Panic, "hush's release class");
}

// ---------------------------------------------------------------------
// Criterion 8, R7 regression: a filter wrapping the printing control.
// ---------------------------------------------------------------------

/// R7's regression test (design 10.4, `query_degrade`'s `put_output`
/// bookkeeping in `src/pattern/combinators/random.rs`). Before R7 this
/// failed deterministically: the filter WRAPS the printing control
/// (`degrade-by` queries its whole subject subtree -- including
/// `gain loud`, which prints -- before deciding which events to keep), so
/// a control evaluated on an event a later filter drops still ran its
/// print; each small per-tick re-query slice over that still-forming
/// (ultimately dropped) event's span then forwarded the print's captured
/// output as a fresh keyless staging fragment (no `OccKey`, since the
/// occurrence never survives) -- once per query slice instead of at most
/// once ever. See this task's report for the exact pre-fix console vector
/// observed with the fix hunk in `random.rs` reverted.
#[test]
fn a_filter_wrapping_the_printing_control_drops_removed_work_output() {
    let mut rig = GapRig::new();
    rig.run(
        "var q 0\nfn loud t:\n\tprint \"p\" t\n\t0.5\n\
         degrade-by {s :analog > note [:c4 :d4 :e4 :f4] > gain loud} q > d1",
    );
    // 0.0 s committed by 0.45 s; 0.5 s is staged (queried inside the
    // lookahead window) but held: its commit is still ~0.02 s away
    // (commit_lead 0.03 s).
    rig.run_to(0.45);
    let before = rig.console();
    assert_eq!(
        before.len(),
        1,
        "only the 0.0 s event's held print committed: {before:?}"
    );
    rig.run("upd q 1");
    // At least two full cycles (2 s each) past the upd.
    rig.run_to(4.45);
    let after = rig.console();
    assert_eq!(
        after, before,
        "no held print for a removed onset ever reached the console, and \
         nothing already printed was duplicated: {after:?}"
    );
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for line in &after {
        *counts.entry(line.as_str()).or_insert(0) += 1;
    }
    assert!(
        counts.values().all(|&n| n == 1),
        "every committed-before-the-upd print appears exactly once: {counts:?}"
    );
}

/// Serial repair R7 fixed `degrade-by`/`maybe`; `sometimes-by`'s two
/// branches (the untransformed pass-through and the transformed subtree)
/// had the same bug: each branch queried its child, then filtered the
/// events away without dropping the filtered-out events' held `print`
/// output, so a control evaluated on removed work (`gain loud`, which
/// prints) still forwarded its captured output once per query slice
/// (90 lines for 20 committed events). `query_filtered` in `random.rs` now
/// backs both `query_degrade` and both `query_sometimes` branches.
/// R7's bookkeeping is per query, not per event: when a filter keeps some
/// of a query's events and drops others, the dropped twin's print still
/// rides with the kept onset (22 lines for 20 events). That residual is
/// bounded by one extra print per committed onset and is asserted as such.
#[test]
fn a_sometimes_filter_drops_removed_work_output() {
    let mut rig = GapRig::new();
    rig.run(
        "fn loud t:\n\tprint \"p\" t\n\t0.5\n\
         sometimes-by {s :analog > note [:c4 :d4 :e4 :f4] > gain loud} 0.5 {p -> fast p 2} > d1",
    );
    rig.run_to(6.0);
    let console = rig.console().len();
    let sent = rig.sent().len();
    assert!(sent > 0, "the pattern committed events");
    assert!(
        console >= sent,
        "every committed event printed: console={console} sent={sent}"
    );
    assert!(
        console <= 2 * sent,
        "no per-slice forwarding of removed work: console={console} sent={sent}"
    );
}
