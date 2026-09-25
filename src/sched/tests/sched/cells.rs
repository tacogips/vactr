//! TASK-007 criterion 5: control-cell semantics per tier (design 11.3).
//! Native: `NativeTransport`-style shared `AtomicCells` (the runtime's own
//! table is the one the callback reads). Browser: `BrowserTransport`, an
//! isolated `Mirror` behind a FIFO with delay, loss and stall, wired as the
//! runtime's `CellPort`. A voice starting at time `t` reads the mirror as
//! it is after the transport's tick at `t`. One cycle is 2 s.

use std::cell::RefCell;
use std::rc::Rc;

use super::{ctl, Rig, DT};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::{CellId, CellRead, CellState};
use crate::host::caps::MidiEvent;
use crate::host::testing::{BrowserTransport, SinkCall};
use crate::host::wire::{Ctl, CtlMsg, HostMsg};
use crate::sched::cells::{CellKey, CellPort, Tier};
use crate::sched::runtime::RuntimeConfig;
use crate::value::intern::intern_sym;

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

/// A browser-tier rig with a `delay`-tick transport and a cell pool.
fn browser(delay: u64, pool: usize) -> (Rig, Rc<RefCell<BrowserTransport>>) {
    let t = Rc::new(RefCell::new(BrowserTransport::new(pool, delay)));
    let cfg = RuntimeConfig {
        tier: Tier::Browser(Box::new(Port(Rc::clone(&t)))),
        cell_pool: pool,
        ..RuntimeConfig::default()
    };
    (Rig::with(cfg, CapabilitySet::browser()), t)
}

/// Ticks transport and runtime together every `DT` up to `end`, recording
/// the mirror value of `cell` after each transport tick.
fn run(rig: &mut Rig, t: &Rc<RefCell<BrowserTransport>>, end: f64, log: &mut Vec<(f64, f32)>) {
    let mut now = rig.clock.now();
    if !rig.ticks.is_empty() {
        now += DT;
        rig.clock.set(now);
    }
    loop {
        t.borrow_mut().tick();
        log.push((now, t.borrow().mirror().get(CellId::new(0))));
        rig.tick_at(now);
        if now + DT > end + 1e-9 {
            break;
        }
        now += DT;
        rig.clock.set(now);
    }
}

/// What the mirror held at time `at`.
fn at(log: &[(f64, f32)], at: f64) -> f32 {
    log.iter()
        .find(|(t, _)| (t - at).abs() < 1e-6)
        .map(|(_, v)| *v)
        .expect("a tick at that time")
}

/// The gain control of the event at `time`.
fn gain_at(rig: &Rig, time: f64) -> Ctl {
    let e = rig
        .sent()
        .into_iter()
        .find(|(_, e)| (e.time - time).abs() < 1e-9)
        .unwrap_or_else(|| panic!("no event at {time}"))
        .1;
    ctl(&e, "gain").expect("gain")
}

fn key(rig: &Rig, var: &str) -> CellKey {
    let slot = rig.ev.ns().session_slot(intern_sym(var)).expect("var");
    CellKey::Site {
        slot: slot.id(),
        ctl: crate::dsp::controls::row("gain").expect("row").ctl,
    }
}

#[test]
fn native_upd_is_heard_at_the_next_started_event_even_when_committed() {
    let mut rig = Rig::new();
    rig.run("var g 0.5\ns [:bd :sd :hh :cp] > gain g > d1");
    rig.run_to(0.48);
    // The 0.5 s event is committed with a cell, before the write.
    let Ctl::Cell(cell) = gain_at(&rig, 0.5) else {
        panic!("a late control commits as a cell natively");
    };
    rig.run("upd g 0.8");
    let cells = rig.rt.cells().native().expect("native tier");
    assert!(
        (cells.get(cell) - 0.8).abs() < 1e-6,
        "the voice at 0.5 s reads 0.8"
    );
}

#[test]
fn first_use_before_init_commits_a_constant_then_the_cell_path_resumes() {
    let (mut rig, t) = browser(1, 16);
    rig.run("var g 0.5\ns [:bd :sd :hh :cp] > gain g > d1");
    let mut log = Vec::new();
    run(&mut rig, &t, 0.6, &mut log);
    assert_eq!(gain_at(&rig, 0.0), Ctl::Const(0.5), "pre-ack downgrade");
    assert!(matches!(gain_at(&rig, 0.5), Ctl::Cell(_)), "after the ack");
    assert!((at(&log, 0.5) - 0.5).abs() < 1e-6);
}

#[test]
fn a_delayed_batch_is_heard_by_the_first_voice_after_it_applies() {
    let (mut rig, t) = browser(5, 16);
    rig.run("var g 0.5\ns [:bd :sd :hh :cp] > gain g > d1");
    let mut log = Vec::new();
    run(&mut rig, &t, 0.45, &mut log);
    rig.run("upd g 0.8");
    run(&mut rig, &t, 1.1, &mut log);
    assert!(matches!(gain_at(&rig, 0.5), Ctl::Cell(_)));
    // The batch leaves at the 0.46 s tick and lands 5 ticks later (0.51 s):
    // the voice at 0.5 s starts inside the hop and plays the previous
    // value; the next voice start (1.0 s) plays the new one.
    assert!((at(&log, 0.5) - 0.5).abs() < 1e-6);
    assert!((at(&log, 0.51) - 0.8).abs() < 1e-6);
    assert!((at(&log, 1.0) - 0.8).abs() < 1e-6);
}

#[test]
fn an_init_replay_after_an_update_is_acknowledge_only() {
    let (mut rig, t) = browser(1, 16);
    t.borrow_mut().drop_next_acks(1); // the first CellInitAck is lost
    rig.run("var g 0.5\ns [:bd :sd :hh :cp] > gain g > d1");
    let mut log = Vec::new();
    run(&mut rig, &t, 0.0, &mut log);
    rig.run("upd g 0.8");
    run(&mut rig, &t, 0.1, &mut log);
    // CellInit(E=1, 0.5) was re-sent after the batch set 0.8: v1 stays.
    let inits = t
        .borrow()
        .delivered()
        .iter()
        .filter(|m| matches!(m, CtlMsg::CellInit { .. }))
        .count();
    assert!(inits >= 2, "the init was replayed");
    let state = t.borrow().mirror().state(CellId::new(0));
    assert_eq!(
        state,
        Some(CellState::Live {
            epoch: 1,
            value: 0.8
        })
    );
}

#[test]
fn a_reused_id_initializes_its_new_epoch_and_old_updates_stay_inert() {
    let (mut rig, t) = browser(1, 1);
    rig.run("var g 0.5\nvar h 0.25\ns [:bd :sd :hh :cp] > gain g > d1");
    let mut log = Vec::new();
    run(&mut rig, &t, 0.1, &mut log);
    let gk = key(&rig, "g");
    assert_eq!(rig.rt.cells().cell_of(gk), Some((CellId::new(0), 1)));
    // `g`'s site goes away: retire; the retire ack frees the id.
    rig.run("stop :d1");
    let g = rig.ev.ns().session_slot(intern_sym("g")).expect("g").id();
    rig.rt.cells.retire_slot(g);
    rig.run("s [:bd :sd :hh :cp] > gain h > d2");
    run(&mut rig, &t, 2.1, &mut log);
    let hk = key(&rig, "h");
    assert_eq!(rig.rt.cells().cell_of(hk), Some((CellId::new(0), 2)));
    assert_eq!(
        t.borrow().mirror().state(CellId::new(0)),
        Some(CellState::Live {
            epoch: 2,
            value: 0.25
        })
    );
    // A stale update for the retired epoch 1 is inert.
    t.borrow_mut().post_batch(1000, &[(CellId::new(0), 1, 9.9)]);
    run(&mut rig, &t, 2.2, &mut log);
    assert!((t.borrow().mirror().get(CellId::new(0)) - 0.25).abs() < 1e-6);
}

#[test]
fn a_pre_ack_constant_plays_its_committed_value_after_a_newer_batch() {
    let (mut rig, t) = browser(2, 16);
    rig.run("var g 0.5\ns [nil :sd] > gain g > d1");
    let mut log = Vec::new();
    // The 1.0 s event commits at 0.97 s: the first reference, pre-ack.
    run(&mut rig, &t, 0.97, &mut log);
    rig.run("upd g 0.8");
    run(&mut rig, &t, 3.0, &mut log);
    assert_eq!(gain_at(&rig, 1.0), Ctl::Const(0.5));
    // The newer batch applied before that voice started; the voice still
    // plays its committed constant (the documented Const-fallback
    // exception), and the next commit is back on the cell path.
    assert!((at(&log, 1.0) - 0.8).abs() < 1e-6);
    assert!(matches!(gain_at(&rig, 3.0), Ctl::Cell(_)));
}

#[test]
fn a_stalled_consumer_burst_stays_bounded_and_converges() {
    let (mut rig, t) = browser(1, 16);
    rig.run("var g 0.5\nvar h 0.5\ns [:bd :sd] > gain g > d1\ns [:hh :cp] > gain h > d2");
    let mut log = Vec::new();
    run(&mut rig, &t, 0.1, &mut log);
    t.borrow_mut().stall();
    let before = rig.rt.cells().stats().batches_sent;
    for i in 0..50 {
        rig.run(&format!(
            "upd g {}\nupd h {}",
            0.1 + f64::from(i) * 0.01,
            0.9
        ));
        let next = rig.clock.now() + DT;
        run(&mut rig, &t, next, &mut log);
        let (in_flight, pending) = rig.rt.cells().outstanding();
        assert!(in_flight <= 1 && pending <= 1);
        assert!(rig.rt.cells().pending_len() <= 2, "latest-wins per cell");
    }
    assert_eq!(
        rig.rt.cells().stats().batches_sent - before,
        1,
        "one batch in flight while stalled; the rest coalesce"
    );
    t.borrow_mut().resume();
    let next = rig.clock.now() + 0.3;
    run(&mut rig, &t, next, &mut log);
    let g = rig.rt.cells().cell_of(key(&rig, "g")).expect("g").0;
    let h = rig.rt.cells().cell_of(key(&rig, "h")).expect("h").0;
    assert!((t.borrow().mirror().get(g) - 0.59).abs() < 1e-6);
    assert!((t.borrow().mirror().get(h) - 0.9).abs() < 1e-6);
    assert_eq!(rig.rt.cells().outstanding(), (0, 0));
}

#[test]
fn a_reconnect_replays_the_snapshot_before_any_new_voice_reads_the_mirror() {
    let (mut rig, t) = browser(1, 16);
    rig.run("var g 0.5\ns [:bd :sd :hh :cp] > gain g > d1");
    let mut log = Vec::new();
    run(&mut rig, &t, 0.2, &mut log);
    // The port drops the next record (the update's batch) and reconnects.
    t.borrow_mut().drop_next(1);
    rig.run("upd g 0.9");
    // Checked before the in-flight batch's first re-send (3 ticks).
    run(&mut rig, &t, 0.22, &mut log);
    assert!((t.borrow().mirror().get(CellId::new(0)) - 0.5).abs() < 1e-6);
    rig.rt.resync_cells();
    run(&mut rig, &t, 1.1, &mut log);
    // Every voice after the reconnect read the snapshot value or carried
    // the committed constant: the first cell-carrying voice sees 0.9.
    let first_cell = rig
        .sent()
        .into_iter()
        .filter(|(arr, _)| *arr > 0.22)
        .find(|(_, e)| matches!(ctl(e, "gain"), Some(Ctl::Cell(_))))
        .expect("the cell path resumed")
        .1;
    assert!((at(&log, first_cell.time) - 0.9).abs() < 1e-6);
    // Only never-initialized incarnations get a CellInit on resync: none.
    let inits = t
        .borrow()
        .delivered()
        .iter()
        .filter(|m| matches!(m, CtlMsg::CellInit { .. }))
        .count();
    assert_eq!(inits, 1);
}

#[test]
fn midi_and_osc_bake_values_at_transmission() {
    let mut rig = Rig::new();
    rig.run("var v 0.5\ns {midi 1} > velocity v > d1\ns osc-a > gain v > d2");
    rig.run_to(0.1);
    rig.run("upd v 1.0");
    rig.run_to(2.1);
    let vels: Vec<u8> = rig
        .midi_calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            SinkCall::Send(MidiEvent::Note { vel, .. }) => Some(vel),
            _ => None,
        })
        .collect();
    assert_eq!(vels, vec![64, 127], "the sent note kept its baked value");
    let gains: Vec<String> = rig
        .osc_calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            SinkCall::Send(e) => Some(format!("{:?}", e.args)),
            SinkCall::Control(_) => None,
        })
        .collect();
    assert_eq!(gains.len(), 2);
    assert!(
        gains[0].contains("F(0.5)") && gains[1].contains("F(1.0)"),
        "{gains:?}"
    );
}
