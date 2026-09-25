//! Site tiers through the session (design 13, 14.5.6 "Tiers"; TASK-009
//! criterion 7): `reeval` rebuilds with override inheritance, `manual`
//! updates the slot with no replay, and `direct` sites (a cell control and
//! a probabilistic parameter) follow the 11.3 per-tier cell contract,
//! asserted separately for the native model (`NativeTransport`-style
//! shared cells) and the browser model (`BrowserTransport`).

use std::cell::RefCell;
use std::rc::Rc;

use super::support::{batches, site_at, Rig, DT};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::{AtomicCells, CellId, CellRead};
use crate::host::testing::{BrowserTransport, NativeTransport};
use crate::host::wire::{Ctl, CtlMsg, HostMsg};
use crate::ns::stage::SlotKey;
use crate::ns::tweak::TweakId;
use crate::sched::cells::{CellPort, Tier};
use crate::sched::runtime::RuntimeConfig;
use crate::session::protocol::WireTier;
use crate::session::session::SessionConfig;

/// The browser transport as the runtime's cell port.
struct Port(Rc<RefCell<BrowserTransport>>);

impl CellPort for Port {
    fn post(&mut self, msg: CtlMsg) {
        self.0.borrow_mut().post(msg);
    }
    fn post_batch(&mut self, seq: u32, entries: &[(CellId, u32, f32)]) {
        self.0.borrow_mut().post_batch(seq, entries);
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.0.borrow_mut().drain(out);
    }
}

/// A browser-tier session over a `delay`-tick transport.
fn browser(delay: u64) -> (Rig, Rc<RefCell<BrowserTransport>>) {
    let t = Rc::new(RefCell::new(BrowserTransport::new(16, delay)));
    let mut cfg = SessionConfig::new(CapabilitySet::browser());
    cfg.runtime = RuntimeConfig {
        tier: Tier::Browser(Box::new(Port(Rc::clone(&t)))),
        cell_pool: 16,
        ..RuntimeConfig::default()
    };
    (Rig::with(cfg), t)
}

/// A native-tier session whose cells are the `NativeTransport`'s shared
/// `AtomicCells` (the audio side reads the same table).
fn native() -> (Rig, AtomicCells) {
    let transport = NativeTransport::new(64);
    let cells = transport.audio_cells().clone();
    let mut cfg = SessionConfig::new(CapabilitySet::native());
    cfg.runtime = RuntimeConfig {
        tier: Tier::Native(cells.clone()),
        cell_pool: 64,
        ..RuntimeConfig::default()
    };
    (Rig::with(cfg), cells)
}

/// Ticks transport and session every `DT` up to `end`, recording the mirror
/// value of cell 0 after each transport tick.
fn run_browser(
    rig: &mut Rig,
    t: &Rc<RefCell<BrowserTransport>>,
    end: f64,
    log: &mut Vec<(f64, f32)>,
) {
    loop {
        t.borrow_mut().tick();
        log.push((rig.clock.now(), t.borrow().mirror().get(CellId::new(0))));
        rig.tick();
        let next = rig.clock.now() + DT;
        if next > end + 1e-9 {
            break;
        }
        rig.clock.set(next);
    }
}

fn mirror_at(log: &[(f64, f32)], at: f64) -> f32 {
    log.iter()
        .find(|(t, _)| (t - at).abs() < 1e-6)
        .map(|(_, v)| *v)
        .expect("a tick at that time")
}

fn event_at(rig: &Rig, time: f64) -> crate::host::wire::AudioEvent {
    rig.sent()
        .into_iter()
        .find(|e| (e.time - time).abs() < 1e-9)
        .unwrap_or_else(|| panic!("no event at {time}"))
}

fn gain_ctl(e: &crate::host::wire::AudioEvent) -> Option<Ctl> {
    let id = crate::dsp::controls::row("gain").expect("row").ctl;
    e.controls().iter().find(|(c, _)| *c == id).map(|(_, v)| *v)
}

const FOUR: &str = "s :analog > note [60 60 60 60] > gain 0.5 > d1\n";

#[test]
fn reeval_sites_rebuild_their_form_with_override_inheritance() {
    let mut rig = Rig::new();
    let src = "s :analog > note [60] > gain {* 0.5 1} > d1\n";
    let r = rig.ok(src, 1);
    let site = site_at(&r, src, "0.5").clone();
    assert_eq!(site.tier, WireTier::Reeval);
    rig.run_to(0.5);
    rig.clear();
    assert!(rig.set_tweak(&site, 0.8, 0).is_empty());
    let out = rig.tick();
    let bs = batches(&out);
    assert_eq!(bs.len(), 1, "the rebuild is one pass: {out:?}");
    let fresh = bs[0]
        .sites
        .iter()
        .find(|s| s.span == site.span)
        .expect("the rebuilt form's site");
    assert!(fresh.form_gen > site.form_gen, "a new generation");
    assert!((fresh.value - 0.8).abs() < 1e-6, "inherits the override");
    // After the rebind boundary the events carry the new gain.
    rig.run_to(4.5);
    let last = *rig.sent().last().expect("events");
    assert!((rig.ctl(&last, "gain").expect("gain") - 0.8).abs() < 1e-6);
}

#[test]
fn manual_sites_update_the_slot_with_no_replay() {
    let mut rig = Rig::new();
    let src = "[{s :analog > note [60] > gain 0.5 > d1} {s :analog > once}]\n";
    let r = rig.ok(src, 1);
    let site = site_at(&r, src, "0.5").clone();
    assert_eq!(site.tier, WireTier::Manual, "the owner ran a one-shot");
    let f = rig
        .s
        .evaluator()
        .form_of_bind(SlotKey::D(1))
        .expect("the form owns d1");
    rig.clear();
    assert!(rig.set_tweak(&site, 0.9, 0).is_empty());
    let out = rig.tick();
    assert!(batches(&out).is_empty(), "no pass, no replay");
    let ev = rig.s.evaluator();
    assert_eq!(ev.graph().get(f).map(|r| r.runs), Some(1));
    let v = ev
        .ns()
        .tweaks()
        .borrow()
        .get(TweakId::new(site.id))
        .map(|s| s.slot.get().to_string());
    assert_eq!(v.as_deref(), Some("0.9"));
}

#[test]
fn direct_native_a_committed_voice_hears_the_write_within_the_commit_horizon() {
    let (mut rig, cells) = native();
    let r = rig.ok(FOUR, 1);
    let site = site_at(&r, FOUR, "0.5").clone();
    assert_eq!(site.tier, WireTier::Direct);
    rig.run_to(0.48);
    // The 0.5 s voice is already committed, with a cell.
    let Some(Ctl::Cell(cell)) = gain_ctl(&event_at(&rig, 0.5)) else {
        panic!("a late control commits as a cell natively");
    };
    assert!(rig.set_tweak(&site, 0.9, 0).is_empty());
    rig.clock.set(0.49);
    rig.tick();
    // The audio side reads the shared cell before the voice starts.
    assert!((cells.get(cell) - 0.9).abs() < 1e-6);
}

#[test]
fn direct_browser_the_write_is_heard_after_the_batch_applies() {
    let (mut rig, t) = browser(5);
    let r = rig.ok(FOUR, 1);
    let site = site_at(&r, FOUR, "0.5").clone();
    let mut log = Vec::new();
    run_browser(&mut rig, &t, 0.45, &mut log);
    assert!(rig.set_tweak(&site, 0.9, 0).is_empty());
    rig.clock.set(0.46);
    run_browser(&mut rig, &t, 1.1, &mut log);
    assert!(matches!(gain_ctl(&event_at(&rig, 0.5)), Some(Ctl::Cell(_))));
    // The batch leaves at the 0.46 s tick and lands five ticks later: the
    // 0.5 s voice plays the previous value, the 1.0 s voice the new one.
    assert!((mirror_at(&log, 0.5) - 0.5).abs() < 1e-6);
    assert!((mirror_at(&log, 0.51) - 0.9).abs() < 1e-6);
    assert!((mirror_at(&log, 1.0) - 0.9).abs() < 1e-6);
}

const MAYBE: &str = "let p maybe {s [:analog :analog :analog :analog]} 0\np > d1\n";

fn maybe_restores_uncommitted_steps(rig: &mut Rig, step: &mut dyn FnMut(&mut Rig, f64)) {
    let r = rig.ok(MAYBE, 1);
    let site = r
        .sites
        .iter()
        .find(|s| s.tier == WireTier::Direct)
        .expect("the probability site")
        .clone();
    step(rig, 0.45);
    assert!(rig.sent().is_empty(), "maybe 0 drops everything");
    assert!(rig.set_tweak(&site, 1.0, 0).is_empty());
    rig.clock.set(rig.clock.now() + DT);
    step(rig, 1.9);
    let times: Vec<f64> = rig.sent().iter().map(|e| e.time).collect();
    assert_eq!(
        times,
        vec![0.5, 1.0, 1.5],
        "the staged 0.5 s step is restored"
    );
}

#[test]
fn direct_probabilistic_parameter_native_model() {
    let (mut rig, _) = native();
    maybe_restores_uncommitted_steps(&mut rig, &mut |rig, end| {
        rig.run_to(end);
    });
}

#[test]
fn direct_probabilistic_parameter_browser_model() {
    let (mut rig, t) = browser(1);
    let mut log = Vec::new();
    maybe_restores_uncommitted_steps(&mut rig, &mut |rig, end| {
        run_browser(rig, &t, end, &mut log);
    });
}
