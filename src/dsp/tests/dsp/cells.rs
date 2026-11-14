//! Control cells on the audio side (11.3, 12.8.5): a voice reads a cell at
//! voice start; on the browser tier a `CellBatch` applied between two
//! quanta changes the next voice's value (earlier voices keep theirs); on
//! the native tier an `AtomicCells` store is visible to the next voice
//! start; template-default cells are control-rate (re-read every block).

use super::{chain, ctl, BrowserRig, NativeRig};
use crate::dsp::arena::encode_inst;
use crate::dsp::cells::CellId;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::{encode_batch, AudioEvent, Ctl, CtlMsg, HostMsg};
use crate::sched::slots::SlotId;

const C: CellId = CellId::new(5);

fn cell_event(time: f64) -> AudioEvent {
    let mut ev = AudioEvent::new(time, SlotId::new(1), 1, InstId::new(1));
    ev.push_ctl(ctl::FREQ, Ctl::Cell(C)).unwrap();
    ev.push_ctl(ctl::LEGATO, Ctl::Const(10.0)).unwrap();
    ev
}

/// The `freq` value each sounding voice started with, in pool order.
fn freqs<C2: crate::dsp::engine::CellStore, S: crate::dsp::ring::ControlSource>(
    rig: &super::Rig<C2, S>,
) -> Vec<f32> {
    let t = rig.engine.template(InstId::new(1)).unwrap();
    let k = t
        .params()
        .iter()
        .position(|(c, _)| *c == ctl::FREQ)
        .unwrap();
    rig.engine
        .voices()
        .voices
        .iter()
        .filter(|v| v.active)
        .map(|v| v.param(k))
        .collect()
}

#[test]
fn browser_batch_between_quanta_changes_the_next_voice_only() {
    let mut rig = BrowserRig::browser(1 << 20);
    let mut bytes = Vec::new();
    encode_inst(&chain(1, vec![UGenSpec::SinOsc]), &mut bytes).unwrap();
    let mut rec = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut rec);
    rig.push(&rec);
    rig.post(CtlMsg::CellInit {
        cell: C,
        epoch: 1,
        value: 440.0,
    });
    let _ = rig.step();
    let acks = rig.acks();
    assert!(acks.contains(&HostMsg::Installed {
        resource: 1,
        gen: 1
    }));
    assert!(acks.contains(&HostMsg::CellInitAck { cell: C, epoch: 1 }));
    let t = rig.engine.now();
    rig.send(cell_event(t));
    let _ = rig.step();
    assert_eq!(freqs(&rig), [440.0]);
    let mut b = vec![0u8; 64];
    let n = encode_batch(1, &[(C, 1, 880.0)], &mut b);
    rig.push(&b[..n]);
    let _ = rig.step();
    assert!(rig.acks().contains(&HostMsg::CellBatchAck { seq: 1 }));
    let t = rig.engine.now();
    rig.send(cell_event(t));
    let _ = rig.step();
    let mut f = freqs(&rig);
    f.sort_by(f32::total_cmp);
    assert_eq!(f, [440.0, 880.0], "the first voice kept its start value");
}

#[test]
fn at_most_one_batch_per_quantum() {
    let mut rig = BrowserRig::browser(1 << 20);
    rig.post(CtlMsg::CellInit {
        cell: C,
        epoch: 1,
        value: 1.0,
    });
    let mut b = vec![0u8; 64];
    for seq in 1..=2 {
        #[allow(clippy::cast_precision_loss)]
        let n = encode_batch(seq, &[(C, 1, seq as f32)], &mut b);
        rig.push(&b[..n]);
    }
    let _ = rig.step();
    let acks = rig.acks();
    assert!(acks.contains(&HostMsg::CellBatchAck { seq: 1 }));
    assert!(!acks.contains(&HostMsg::CellBatchAck { seq: 2 }), "held");
    let _ = rig.step();
    assert!(rig.acks().contains(&HostMsg::CellBatchAck { seq: 2 }));
}

#[test]
fn native_store_is_visible_to_the_next_voice_start() {
    let mut rig = NativeRig::native();
    rig.install(&chain(1, vec![UGenSpec::SinOsc]));
    rig.cells.set(C, 330.0);
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(cell_event(t));
    let _ = rig.step();
    rig.cells.set(C, 660.0);
    let t = rig.engine.now();
    rig.send(cell_event(t));
    let _ = rig.step();
    let mut f = freqs(&rig);
    f.sort_by(f32::total_cmp);
    assert_eq!(f, [330.0, 660.0]);
}

#[test]
fn template_default_cells_are_control_rate() {
    let mut rig = NativeRig::native();
    let def = InstDef {
        id: InstId::new(1),
        params: Box::new([(ctl::FREQ, Ctl::Cell(C))]),
        ..chain(1, vec![UGenSpec::SinOsc])
    };
    rig.install(&def);
    rig.cells.set(C, 100.0);
    let _ = rig.step();
    let t = rig.engine.now();
    let mut ev = AudioEvent::new(t, SlotId::new(1), 1, InstId::new(1));
    ev.push_ctl(ctl::LEGATO, Ctl::Const(10.0)).unwrap();
    rig.send(ev);
    let _ = rig.step();
    assert_eq!(freqs(&rig), [100.0]);
    rig.cells.set(C, 200.0);
    let _ = rig.step();
    assert_eq!(
        freqs(&rig),
        [200.0],
        "a sounding voice follows its default cell"
    );
}

#[test]
fn a_retired_cell_is_acknowledged_once_unused() {
    let mut rig = BrowserRig::browser(1 << 20);
    rig.post(CtlMsg::CellInit {
        cell: C,
        epoch: 1,
        value: 1.0,
    });
    rig.post(CtlMsg::CellRetire { cell: C, epoch: 1 });
    let _ = rig.step();
    assert!(rig
        .acks()
        .contains(&HostMsg::CellRetired { cell: C, epoch: 1 }));
}

fn cell_retired(acks: &[HostMsg], cell: CellId) -> bool {
    acks.iter()
        .any(|m| matches!(m, HostMsg::CellRetired { cell: c, .. } if *c == cell))
}

fn init(rig: &mut BrowserRig, cell: CellId) {
    rig.post(CtlMsg::CellInit {
        cell,
        epoch: 1,
        value: 800.0,
    });
}

#[test]
fn a_port_only_cell_is_not_retired_while_its_template_is_live_or_retiring() {
    let mut rig = BrowserRig::browser(1 << 20);
    let cutoff = crate::dsp::controls::row("cutoff").unwrap().ctl;
    let def = InstDef {
        node_params: Box::new([(1, cutoff, Ctl::Cell(C))]),
        ..chain(1, vec![UGenSpec::SinOsc, UGenSpec::Lpf])
    };
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut rec = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut rec);
    rig.push(&rec);
    init(&mut rig, C);
    let _ = rig.step();
    let t = rig.engine.now();
    let mut ev = AudioEvent::new(t, SlotId::new(1), 1, InstId::new(1));
    ev.push_ctl(ctl::LEGATO, Ctl::Const(0.2)).unwrap();
    rig.send(ev);
    let _ = rig.step();
    rig.post(CtlMsg::CellRetire { cell: C, epoch: 1 });
    let _ = rig.step();
    assert!(!cell_retired(&rig.acks(), C), "live template reads it");
    rig.post(CtlMsg::GraphRetire { id: 1 });
    let _ = rig.run(3);
    let acks = rig.acks();
    assert!(
        !cell_retired(&acks, C),
        "the retiring template's voice reads it"
    );
    assert!(!super::retired(&acks).contains(&1));
    let _ = rig.run(130);
    let acks = rig.acks();
    assert!(super::retired(&acks).contains(&1), "template collected");
    assert!(cell_retired(&acks, C), "then the cell retires");
}

#[test]
fn a_bus_only_cell_is_not_retired_while_its_chain_is_installed() {
    let mut rig = BrowserRig::browser(1 << 20);
    let gain = crate::dsp::effects::catalog::spec(
        crate::dsp::graph::EffectKind::Gain,
        &[("gain", Ctl::Cell(C))],
    )
    .unwrap();
    let mut bytes = Vec::new();
    crate::dsp::arena::encode_bus(&super::bus_def(3, vec![gain]), false, &mut bytes).unwrap();
    let mut rec = Vec::new();
    encode_graph_record(2, 1, &bytes, &mut rec);
    rig.push(&rec);
    init(&mut rig, C);
    let _ = rig.step();
    assert!(rig.acks().contains(&HostMsg::Installed {
        resource: 2,
        gen: 1
    }));
    rig.post(CtlMsg::CellRetire { cell: C, epoch: 1 });
    let _ = rig.run(2);
    assert!(!cell_retired(&rig.acks(), C), "the live bus chain reads it");
    rig.post(CtlMsg::GraphRetire { id: 2 });
    let _ = rig.step();
    let acks = rig.acks();
    assert!(super::retired(&acks).contains(&2));
    assert!(cell_retired(&acks, C), "acked once the chain is gone");
}

#[test]
fn an_impulse_response_is_held_while_a_bus_convolves_it() {
    let mut rig = NativeRig::native();
    rig.sample(20, vec![1.0, 0.5], 1);
    let conv = crate::dsp::effects::catalog::spec(
        crate::dsp::graph::EffectKind::Convolution,
        &[("ir", Ctl::Const(20.0))],
    )
    .unwrap();
    let bus = rig.install_bus(&super::bus_def(3, vec![conv]), false);
    let _ = rig.step();
    rig.post(CtlMsg::SampleRetire { resource: 20 });
    let _ = rig.run(2);
    assert!(
        !super::retired(&rig.acks()).contains(&20),
        "the bus still reads it"
    );
    rig.post(CtlMsg::GraphRetire { id: bus });
    let _ = rig.step();
    let acks = rig.acks();
    assert!(super::retired(&acks).contains(&bus));
    assert!(
        super::retired(&acks).contains(&20),
        "released with its last reader"
    );
}
