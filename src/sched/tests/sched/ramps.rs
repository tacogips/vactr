//! Session-owned cell-ramp transport tests.

use std::cell::RefCell;
use std::rc::Rc;

use crate::dsp::cells::{AtomicCells, CellId};
use crate::dsp::controls::row;
use crate::host::wire::{Ctl, CtlMsg, HostMsg};
use crate::sched::cells::{CellKey, CellMap, CellPort, ControlCells, Tier};
use crate::sched::ramps::{Capacity, RampSender, MAX_CELL_RAMPS};
use crate::value::value::Value;

#[derive(Default)]
struct Port(Rc<RefCell<Vec<CtlMsg>>>);

impl CellPort for Port {
    fn post(&mut self, message: CtlMsg) {
        self.0.borrow_mut().push(message);
    }
    fn post_batch(&mut self, _seq: u32, _entries: &[(CellId, u32, f32)]) {}
    fn drain(&mut self, _out: &mut Vec<HostMsg>) {}
}

fn native_cells() -> ControlCells {
    ControlCells::new(Tier::Native(AtomicCells::new(64)), 64, 3, 30)
}

#[test]
fn unacknowledged_ramp_is_retried_with_remaining_frames_until_ack() {
    let mut cells = native_cells();
    let control = row("gain").expect("gain row");
    let key = CellKey::External(CellId::new(2));
    let _ = cells.ctl_for(key, Some(control), CellMap::Direct, 0.5, &mut Vec::new());
    let (cell, epoch) = cells.cell_of(key).expect("cell");
    let mut sender = RampSender::default();
    let first = sender
        .post(cell, epoch, 0.9, 100, false, 10)
        .expect("within bound");
    let CtlMsg::CellRamp { seq, .. } = first else {
        panic!("ramp record")
    };
    assert!(
        sender.tick(20, 2, &cells).is_empty(),
        "wait one resend tick"
    );
    let resend = sender.tick(40, 2, &cells);
    assert!(
        matches!(resend.as_slice(), [CtlMsg::CellRamp { cell: got, epoch: e, seq: s, target, frames: 70, release: false }] if *got == cell && *e == epoch && *s == seq && (*target - 0.9).abs() < f32::EPSILON)
    );
    sender.on_ack(cell, seq);
    assert!(sender.tick(50, 2, &cells).is_empty(), "ack stops retries");
}

#[test]
fn browser_const_downgrade_uses_momentary_while_cell_is_unacked() {
    let records = Rc::new(RefCell::new(Vec::new()));
    let mut cells = ControlCells::new(Tier::Browser(Box::new(Port(Rc::clone(&records)))), 8, 3, 30);
    let control = row("gain").expect("gain row");
    let key = CellKey::Site {
        slot: 5,
        ctl: control.ctl,
    };
    assert_eq!(
        cells.ctl_for(key, Some(control), CellMap::Direct, 0.5, &mut Vec::new()),
        Ctl::Const(0.5)
    );
    let (cell, epoch) = cells.cell_of(key).expect("cell");
    assert_eq!(cells.cell_state(cell), Some((epoch, false, 0.5)));
    cells.set_momentary(cell, epoch, Some(0.8));
    assert_eq!(
        cells.ctl_for(key, Some(control), CellMap::Direct, 0.5, &mut Vec::new()),
        Ctl::Const(0.8)
    );
    assert!(records
        .borrow()
        .iter()
        .all(|message| !matches!(message, CtlMsg::CellRamp { .. })));
}

#[test]
fn the_thirty_third_concurrent_cell_ramp_is_rejected() {
    let mut sender = RampSender::default();
    for index in 0..MAX_CELL_RAMPS {
        sender
            .post(CellId::new(index as u32), 1, 0.5, 100, false, 0)
            .expect("first 32 fit");
    }
    assert_eq!(sender.len(), MAX_CELL_RAMPS);
    assert_eq!(
        sender.post(CellId::new(99), 1, 0.5, 100, false, 0),
        Err(Capacity)
    );
}

#[test]
fn slot_cell_update_keeps_the_stored_base_separate_from_the_ramp() {
    let mut cells = native_cells();
    let control = row("gain").expect("gain row");
    let key = CellKey::Site {
        slot: 8,
        ctl: control.ctl,
    };
    let _ = cells.ctl_for(key, Some(control), CellMap::Direct, 0.4, &mut Vec::new());
    let (cell, _) = cells.cell_of(key).expect("cell");
    cells.set_momentary(cell, 1, Some(0.9));
    assert_eq!(cells.encode_for(cell, &Value::Float(0.4)), Some(0.4));
}
