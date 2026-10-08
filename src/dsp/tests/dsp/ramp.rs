use crate::dsp::arena::encode_inst;
use crate::dsp::cells::{AtomicCells, CellId, CellRead};
use crate::dsp::graph::{InstId, UGenSpec};
use crate::dsp::ramp::{CellRamps, RampedCells, MAX_CELL_RAMPS};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::tests::dsp::{chain, BrowserRig};
use crate::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg};
use crate::sched::slots::{CtlId, SlotId};

#[test]
fn ramp_is_monotone_and_bounded_at_block_reads() {
    let cells = AtomicCells::new(64);
    let cell = CellId::new(3);
    let mut ramps = CellRamps::new(64);
    assert!(ramps.apply(cell, 1, 7, 1.0, 4_800, false, 0, cells.get(cell)));
    let mut previous = 0.0;
    for frame in (0..=4_800).step_by(128) {
        let value = RampedCells::new(&ramps, &cells, frame).get(cell);
        assert!(value >= previous);
        assert!(value - previous <= 128.0 / 4_800.0 + 1.0e-6);
        previous = value;
    }
    assert_eq!(RampedCells::new(&ramps, &cells, 4_800).get(cell), 1.0);
    assert_eq!(ramps.metadata(cell), Some((1, 7)));
}

#[test]
fn retarget_starts_at_current_value_and_release_reaps_to_cell() {
    let cells = AtomicCells::new(64);
    let cell = CellId::new(2);
    assert!(cells.set(cell, 0.25));
    let mut ramps = CellRamps::new(64);
    assert!(ramps.apply(cell, 1, 1, 1.0, 4_800, false, 0, cells.get(cell)));
    let current = RampedCells::new(&ramps, &cells, 1_280).get(cell);
    assert!(ramps.apply(cell, 1, 2, 0.0, 480, true, 1_280, current));
    assert_eq!(RampedCells::new(&ramps, &cells, 1_280).get(cell), current);
    let middle = RampedCells::new(&ramps, &cells, 1_520).get(cell);
    assert!(middle < current && middle > 0.0);
    ramps.reap(1_760);
    assert_eq!(RampedCells::new(&ramps, &cells, 1_760).get(cell), 0.25);
    assert_eq!(ramps.metadata(cell), None);
}

#[test]
fn fixed_capacity_and_zero_frame_release_are_bounded() {
    let cells = AtomicCells::new(64);
    let mut ramps = CellRamps::new(64);
    for index in 0..MAX_CELL_RAMPS {
        assert!(ramps.apply(CellId::new(index as u32), 1, 1, 0.5, 1_000, false, 0, 0.0));
    }
    assert!(!ramps.apply(CellId::new(40), 1, 1, 0.5, 1_000, false, 0, 0.0));
    assert_eq!(ramps.dropped(), 1);

    let cell = CellId::new(0);
    assert!(ramps.apply(cell, 1, 2, 0.75, 0, false, 0, 0.0));
    assert_eq!(RampedCells::new(&ramps, &cells, 0).get(cell), 0.75);
    assert!(ramps.apply(cell, 1, 3, 0.5, 0, true, 0, 0.75));
    assert_eq!(RampedCells::new(&ramps, &cells, 0).get(cell), 0.0);
    assert_eq!(ramps.metadata(cell), None);
}

#[test]
fn browser_epoch_ack_and_cell_retire_follow_engine_path() {
    let cell = CellId::new(3);
    let mut rig = BrowserRig::browser(1 << 20);
    let mut bytes = Vec::new();
    encode_inst(&chain(1, vec![UGenSpec::SinOsc]), &mut bytes).expect("graph encodes");
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    rig.push(&record);
    rig.post(CtlMsg::CellInit {
        cell,
        epoch: 1,
        value: 220.0,
    });
    let _ = rig.step();
    let _ = rig.step();
    assert!(rig.engine.template(InstId::new(1)).is_some());
    assert!(rig
        .acks()
        .contains(&HostMsg::CellInitAck { cell, epoch: 1 }));
    assert!(matches!(
        rig.cells.state(cell),
        Some(crate::dsp::cells::CellState::Live { epoch: 1, .. })
    ));

    rig.post(CtlMsg::CellRamp {
        cell,
        epoch: 2,
        seq: 4,
        target: 440.0,
        frames: 0,
        release: false,
    });
    let _ = rig.step();
    assert!(!rig.acks().contains(&HostMsg::CellRampAck { cell, seq: 4 }));

    rig.post(CtlMsg::CellRamp {
        cell,
        epoch: 1,
        seq: 5,
        target: 440.0,
        frames: 0,
        release: false,
    });
    let _ = rig.step();
    let acks = rig.acks();
    assert!(
        acks.contains(&HostMsg::CellRampAck { cell, seq: 5 }),
        "matching epoch ramp ack; acks={acks:?}, dropped={}, refused={}",
        rig.engine.counters().dropped,
        rig.controls.refused()
    );

    let mut event = AudioEvent::new(rig.engine.now(), SlotId::new(1), 1, InstId::new(1));
    event
        .push_ctl(CtlId::new(0), Ctl::Cell(cell))
        .expect("cell control fits");
    rig.send(event);
    let _ = rig.step();
    let template = rig.engine.template(InstId::new(1)).expect("installed");
    let freq = template
        .params()
        .iter()
        .position(|(id, _)| *id == CtlId::new(0))
        .expect("frequency cell parameter");
    let voice = rig
        .engine
        .voices()
        .voices
        .iter()
        .find(|voice| voice.active)
        .expect("cell-controlled voice started");
    assert_eq!(voice.param(freq), 440.0);

    rig.post(CtlMsg::CellRetire { cell, epoch: 1 });
    let _ = rig.step();
    let mut event = AudioEvent::new(rig.engine.now(), SlotId::new(1), 1, InstId::new(1));
    event
        .push_ctl(CtlId::new(0), Ctl::Cell(cell))
        .expect("cell control fits");
    rig.send(event);
    let _ = rig.step();
    let voices = rig
        .engine
        .voices()
        .voices
        .iter()
        .filter(|voice| voice.active)
        .collect::<Vec<_>>();
    assert_eq!(voices.last().expect("second voice").param(freq), 0.0);
}
