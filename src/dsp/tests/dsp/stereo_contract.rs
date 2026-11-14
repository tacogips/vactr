//! MOD-004 bus-boundary contract: two independently panned instrument
//! inputs remain independent across a serialized stereo matrix, bus routing
//! and the master output. Voice-internal UGen graphs remain mono.

use super::{bus_def, chain, ctl, event, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_bus, encode_inst};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::catalog::spec;
use crate::dsp::graph::{AudioPortShape, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::Ctl;

fn swap() -> crate::dsp::graph::BusDef {
    let matrix = spec(
        EffectKind::Matrix,
        &[
            ("ll", Ctl::Const(0.0)),
            ("lr", Ctl::Const(1.0)),
            ("rl", Ctl::Const(1.0)),
            ("rr", Ctl::Const(0.0)),
        ],
    )
    .unwrap();
    bus_def(3, vec![matrix])
}

fn sources() -> [crate::dsp::graph::InstDef; 2] {
    [
        chain(1, vec![UGenSpec::Const(0.25)]),
        chain(2, vec![UGenSpec::Const(0.75)]),
    ]
}

fn send_pair<C: crate::dsp::engine::CellStore, S: crate::dsp::ring::ControlSource>(
    rig: &mut super::Rig<C, S>,
) {
    let t = rig.engine.now();
    for (id, pan) in [(1, 0.0), (2, 1.0)] {
        rig.send(event(
            id,
            t,
            &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 10.0)],
        ));
    }
}

fn assert_swapped(l: &[f32], r: &[f32]) {
    let a = l.iter().copied().sum::<f32>() / l.len() as f32;
    let b = r.iter().copied().sum::<f32>() / r.len() as f32;
    assert!(a > 0.65 && a < 0.85, "left receives former right: {a}");
    assert!(b > 0.20 && b < 0.35, "right receives former left: {b}");
    assert!(l.iter().chain(r).all(|x| x.is_finite()));
}

#[test]
fn native_bus_matrix_keeps_two_inputs_and_outputs_distinct() {
    assert_eq!(EffectKind::Matrix.bus_port_shape(), AudioPortShape::STEREO);
    let def = swap();
    assert_eq!(
        BusTemplate::from_def(&def).unwrap().port_shape(),
        AudioPortShape::STEREO
    );
    let mut rig = NativeRig::native();
    for source in sources() {
        rig.install(&source);
    }
    rig.install_bus(&def, false);
    rig.install_bus(&bus_def(0, vec![]), true);
    let _ = rig.step();
    send_pair(&mut rig);
    let (l, r) = rig.run(4);
    assert_swapped(&l[128..], &r[128..]);
}

#[test]
fn browser_graph_codec_keeps_stereo_bus_shape_and_audio() {
    let mut rig = BrowserRig::browser(4 << 20);
    for (resource, source) in sources().into_iter().enumerate() {
        let mut bytes = Vec::new();
        encode_inst(&source, &mut bytes).unwrap();
        let mut record = Vec::new();
        encode_graph_record(10 + resource as u32, 1, &bytes, &mut record);
        rig.push(&record);
    }
    for (resource, def, master) in [(20, swap(), false), (21, bus_def(0, vec![]), true)] {
        let mut bytes = Vec::new();
        encode_bus(&def, master, &mut bytes).unwrap();
        let mut record = Vec::new();
        encode_graph_record(resource, 1, &bytes, &mut record);
        rig.push(&record);
    }
    let _ = rig.run(6);
    send_pair(&mut rig);
    let (l, r) = rig.run(4);
    assert_swapped(&l[128..], &r[128..]);
}
