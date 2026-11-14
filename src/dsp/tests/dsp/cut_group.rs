//! DDRUM-004B: cut-group choke (design-music.md "cut group": `s :break >
//! cut 1 > d1`). A new voice with a nonzero cut group quickly releases
//! (`Voice::short_gate`, the existing 3 ms click-free fade, 12.8.9) every
//! other active voice that shares both that cut group and its
//! `voice_hint` orbit (`Engine::choke_cut_group`); `cut == 0` chokes
//! nothing, and a different cut group or a different orbit leaves an
//! existing voice ringing.

use super::{chain, ctl, NativeRig};
use crate::dsp::graph::{Edge, InstId, UGenSpec};
use crate::host::wire::{AudioEvent, Ctl};
use crate::sched::slots::SlotId;

/// `sin-osc * env-adsr`, held open by a long `legato` so a test can
/// observe a steady voice across several blocks before and after a choke.
fn sustaining() -> NativeRig {
    let mut rig = NativeRig::native();
    let mut def = chain(1, vec![UGenSpec::EnvAdsr, UGenSpec::Mul]);
    def.nodes = Box::new([UGenSpec::SinOsc, UGenSpec::EnvAdsr, UGenSpec::Mul]);
    def.edges = Box::new([
        Edge {
            from: 0,
            to: 2,
            port: 0,
        },
        Edge {
            from: 1,
            to: 2,
            port: 1,
        },
    ]);
    rig.install(&def);
    let _ = rig.step();
    rig
}

/// A scheduled (non-live) hit at `orbit`/`cut`, `voice_hint` encoded
/// exactly as `sched::commit` encodes it (`orbit | cut << 8`).
fn hit(rig: &NativeRig, orbit: u32, cut: u32) -> AudioEvent {
    let mut ev = AudioEvent::new(rig.engine.now(), SlotId::new(1), 1, InstId::new(1));
    for (id, v) in [
        (ctl::FREQ, 440.0),
        (ctl::ATTACK, 0.001),
        (ctl::DECAY, 0.001),
        (ctl::SUSTAIN, 0.8),
        (ctl::RELEASE, 0.05),
        (ctl::LEGATO, 2.0),
    ] {
        ev.push_ctl(id, Ctl::Const(v)).unwrap();
    }
    ev.voice_hint = (orbit & 0xFF) | ((cut & 0xFF) << 8);
    ev
}

fn active_indices(rig: &NativeRig) -> Vec<usize> {
    rig.engine
        .voices()
        .voices
        .iter()
        .enumerate()
        .filter(|(_, v)| v.active)
        .map(|(i, _)| i)
        .collect()
}

/// Sends a hit at `orbit`/`cut`, renders one block, and returns the pool
/// index of the voice it started.
fn send_hit(rig: &mut NativeRig, orbit: u32, cut: u32) -> usize {
    let before = active_indices(rig);
    let ev = hit(rig, orbit, cut);
    rig.send(ev);
    let _ = rig.step();
    let after = active_indices(rig);
    *after
        .iter()
        .find(|i| !before.contains(i))
        .expect("the hit started a voice")
}

#[test]
fn same_orbit_and_cut_group_chokes_the_earlier_voice() {
    let mut rig = sustaining();
    let a = send_hit(&mut rig, 0, 1);
    assert!(
        rig.engine.voices().voices[a].fade.is_none(),
        "a rings freely before the second hit"
    );
    let b = send_hit(&mut rig, 0, 1);
    assert!(
        rig.engine.voices().voices[a].fade.is_some(),
        "the same cut group on the same orbit chokes the earlier voice"
    );
    assert!(
        rig.engine.voices().voices[b].fade.is_none(),
        "the new voice itself is not choked"
    );
    let _ = rig.run(2);
    assert!(
        !rig.engine.voices().voices[a].active,
        "a faded out within the short gate"
    );
    assert!(rig.engine.voices().voices[b].active, "b still rings");
}

#[test]
fn a_different_cut_group_does_not_choke() {
    let mut rig = sustaining();
    let a = send_hit(&mut rig, 0, 1);
    let _ = send_hit(&mut rig, 0, 2);
    assert!(
        rig.engine.voices().voices[a].fade.is_none(),
        "a different cut group leaves the earlier voice ringing"
    );
}

#[test]
fn cut_zero_chokes_nothing() {
    let mut rig = sustaining();
    let a = send_hit(&mut rig, 0, 1);
    let _ = send_hit(&mut rig, 0, 0);
    assert!(
        rig.engine.voices().voices[a].fade.is_none(),
        "cut 0 means no group: it chokes nothing"
    );
}

#[test]
fn a_zero_cut_voice_is_never_chokeable_either() {
    let mut rig = sustaining();
    let a = send_hit(&mut rig, 0, 0);
    let _ = send_hit(&mut rig, 0, 1);
    assert!(
        rig.engine.voices().voices[a].fade.is_none(),
        "a voice that started with cut 0 never joined a group"
    );
}

#[test]
fn a_different_orbit_does_not_choke() {
    let mut rig = sustaining();
    let a = send_hit(&mut rig, 0, 1);
    let _ = send_hit(&mut rig, 1, 1);
    assert!(
        rig.engine.voices().voices[a].fade.is_none(),
        "the same cut group on a different orbit leaves the earlier voice ringing"
    );
}
