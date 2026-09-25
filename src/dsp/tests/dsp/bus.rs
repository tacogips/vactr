//! TASK-008 criterion 4 (DSP half): a slot event routed to a bus reaches
//! the master through the bus chain; a bus swap retires the old chain only
//! at refcount zero, with zero allocation during the swap; `room` maps to
//! the bus reverb parameter; a `master` chain processes everything.

use super::{bus_def, chain, ctl, event, retired, NativeRig};
use crate::dsp::bus::SlotState;
use crate::dsp::effects::catalog::spec;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::host::wire::{Ctl, CtlMsg, HostMsg};

const BUS: u32 = 3;

fn dc_rig() -> NativeRig {
    let mut rig = NativeRig::native();
    rig.install(&chain(1, vec![UGenSpec::Const(0.5)]));
    let _ = rig.step();
    rig
}

fn gain(db: f32) -> crate::dsp::graph::EffectSpec {
    spec(EffectKind::Gain, &[("gain", Ctl::Const(db))]).unwrap()
}

fn held(rig: &NativeRig, bus: Option<f32>) -> crate::host::wire::AudioEvent {
    let mut ctls = vec![(ctl::PAN, 0.0), (ctl::LEGATO, 10.0)];
    if let Some(b) = bus {
        ctls.push((ctl::BUS, b));
    }
    event(1, rig.engine.now(), &ctls)
}

#[test]
fn a_routed_event_reaches_master_through_its_bus() {
    let mut plain = dc_rig();
    let ev = held(&plain, None);
    plain.send(ev);
    let (dry, _) = plain.run(4);

    let mut rig = dc_rig();
    rig.install_bus(&bus_def(BUS, vec![gain(-6.0206)]), false);
    let _ = rig.step();
    let ev = held(&rig, Some(3.0));
    rig.send(ev);
    let (wet, _) = rig.run(4);
    assert!(dry.iter().any(|v| *v != 0.0));
    for (w, d) in wet.iter().zip(&dry) {
        assert!((w - 0.5 * d).abs() < 1.0e-4, "the bus halves the signal");
    }
    let m = rig.engine.buses().master();
    let b = rig.engine.buses().route(Some(BusId::new(BUS)));
    assert_ne!(b, m, "the bus has its own slot");
    assert_eq!(rig.engine.buses().slots[b].users, 1);
}

#[test]
fn an_unknown_bus_routes_to_master() {
    let mut rig = dc_rig();
    let ev = held(&rig, Some(42.0));
    rig.send(ev);
    let (l, _) = rig.run(2);
    assert!(l.iter().any(|v| *v != 0.0));
}

#[test]
fn a_bus_swap_retires_the_old_chain_only_at_refcount_zero() {
    let mut rig = dc_rig();
    let old = rig.install_bus(&bus_def(BUS, vec![gain(0.0)]), false);
    let _ = rig.step();
    // A short voice routed to the old chain.
    let ev = event(
        1,
        rig.engine.now(),
        &[
            (ctl::BUS, 3.0),
            (ctl::ATTACK, 0.0),
            (ctl::DECAY, 0.02),
            (ctl::RELEASE, 0.01),
        ],
    );
    rig.send(ev);
    let _ = rig.step();
    let old_slot = rig.engine.buses().route(Some(BusId::new(BUS)));
    // Swap while it sounds; the install runs inside the probed process.
    let new = rig.install_bus(&bus_def(BUS, vec![gain(-6.0)]), false);
    let _ = rig.step();
    let s = &rig.engine.buses().slots[old_slot];
    assert_eq!(s.state, SlotState::Retiring, "the old chain retires");
    assert_eq!(s.users, 1, "a voice still uses it");
    let swap_acks = rig.acks();
    assert!(swap_acks.contains(&HostMsg::Installed {
        resource: new,
        gen: 1
    }));
    assert!(!retired(&swap_acks).contains(&old));
    assert_ne!(rig.engine.buses().route(Some(BusId::new(BUS))), old_slot);
    let _ = rig.run(20);
    let acks = rig.acks();
    assert!(
        retired(&acks).contains(&old),
        "retired once the voice ended"
    );
    assert!(!retired(&acks).contains(&new));
    assert_eq!(rig.engine.buses().slots[old_slot].state, SlotState::Free);
    // The garbage ring carries the handed-over templates back.
    assert!(rig.garbage_rx.pop().is_some());
}

#[test]
fn graph_retire_of_a_bus_waits_for_its_voices() {
    let mut rig = dc_rig();
    let id = rig.install_bus(&bus_def(BUS, vec![gain(0.0)]), false);
    let _ = rig.step();
    let ev = held(&rig, Some(3.0));
    rig.send(ev);
    let _ = rig.step();
    rig.post(CtlMsg::GraphRetire { id });
    let _ = rig.run(3);
    assert!(!retired(&rig.acks()).contains(&id), "held voice keeps it");
}

#[test]
fn room_maps_to_the_bus_reverb_parameter() {
    let mut rig = dc_rig();
    rig.install_bus(&bus_def(BUS, vec![]), false);
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::BUS, 3.0), (ctl::ROOM, 0.3)]));
    let _ = rig.run(2);
    let b = rig.engine.buses().route(Some(BusId::new(BUS)));
    assert!((rig.engine.buses().slots[b].room() - 0.3).abs() < 1.0e-6);
    let m = rig.engine.buses().master();
    assert_eq!(
        rig.engine.buses().slots[m].room(),
        0.0,
        "only the routed bus"
    );
}

#[test]
fn room_zero_is_bit_transparent() {
    let mut a = dc_rig();
    let ev = held(&a, None);
    a.send(ev);
    let (x, _) = a.run(6);
    let mut b = dc_rig();
    let mut ev = held(&b, None);
    ev.push_ctl(ctl::ROOM, Ctl::Const(0.0)).unwrap();
    b.send(ev);
    let (y, _) = b.run(6);
    assert_eq!(x, y);
}

#[test]
fn master_chain_processes_every_bus() {
    let mut rig = dc_rig();
    rig.install_bus(&bus_def(0, vec![gain(-6.0206)]), true);
    rig.install_bus(&bus_def(BUS, vec![]), false);
    let _ = rig.step();
    let ev = held(&rig, Some(3.0));
    rig.send(ev);
    let (l, _) = rig.run(3);
    let peak = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        (peak - 0.25).abs() < 1.0e-3,
        "0.5 through the -6 dB master: {peak}"
    );
}
