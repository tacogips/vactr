//! TASK-008 criterion 8 (the engine-level `VoiceRelease` half, 11.7): a
//! release reaches exactly the tagged voice even after a slot-generation
//! bump; a release before its start hits the tombstone and the late start
//! is dropped; pool exhaustion steals the oldest open voice and counts it;
//! `Natural` puts open voices into their release stage; `Panic` short-gates
//! them.

use super::{chain, ctl, NativeRig};
use crate::dsp::graph::{InstId, UGenSpec};
use crate::host::wire::{AudioEvent, Ctl, CtlMsg, Release, SlotControl, VoiceTag};
use crate::sched::slots::SlotId;

fn tag(pitch: u8, seq: u32) -> VoiceTag {
    VoiceTag {
        slot: SlotId::new(1),
        channel: 0,
        pitch,
        seq,
    }
}

fn note(rig: &NativeRig, gen: u32, t: VoiceTag) -> CtlMsg {
    let mut ev = AudioEvent::new(rig.engine.now(), SlotId::new(1), gen, InstId::new(1));
    for (id, v) in [
        (ctl::ATTACK, 0.001),
        (ctl::DECAY, 0.01),
        (ctl::SUSTAIN, 0.8),
        (ctl::RELEASE, 0.05),
    ] {
        ev.push_ctl(id, Ctl::Const(v)).unwrap();
    }
    CtlMsg::LiveNoteOn { tag: t, ev }
}

/// A sustaining instrument: `sin-osc * env-adsr`.
fn pad() -> NativeRig {
    let mut rig = NativeRig::native();
    let mut def = chain(1, vec![UGenSpec::EnvAdsr, UGenSpec::Mul]);
    def.nodes = Box::new([UGenSpec::SinOsc, UGenSpec::EnvAdsr, UGenSpec::Mul]);
    def.edges = Box::new([
        crate::dsp::graph::Edge {
            from: 0,
            to: 2,
            port: 0,
        },
        crate::dsp::graph::Edge {
            from: 1,
            to: 2,
            port: 1,
        },
    ]);
    rig.install(&def);
    let _ = rig.step();
    rig
}

fn voice_of(rig: &NativeRig, t: VoiceTag) -> Option<&crate::dsp::voice::Voice> {
    rig.engine
        .voices()
        .voices
        .iter()
        .find(|v| v.active && v.tag == Some(t))
}

#[test]
fn release_reaches_exactly_the_tagged_voice_after_a_gen_bump() {
    let mut rig = pad();
    let (a, b) = (tag(60, 1), tag(60, 2));
    let m = note(&rig, 1, a);
    rig.post(m);
    let _ = rig.step();
    rig.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: rig.engine.now(),
        release: Release::None,
    }));
    let m = note(&rig, 2, b);
    rig.post(m);
    let _ = rig.step();
    assert!(
        !voice_of(&rig, a).unwrap().released(),
        "rebind lets it ring"
    );
    rig.post(CtlMsg::VoiceRelease { tag: a });
    let _ = rig.step();
    assert!(
        voice_of(&rig, a).unwrap().released(),
        "the old-gen voice released"
    );
    assert!(
        !voice_of(&rig, b).unwrap().released(),
        "the other tag untouched"
    );
    let _ = rig.run(30);
    assert!(voice_of(&rig, a).is_none(), "its release stage ended it");
    assert!(voice_of(&rig, b).is_some(), "still held");
}

#[test]
fn release_before_start_hits_the_tombstone() {
    let mut rig = pad();
    let c = tag(64, 9);
    rig.post(CtlMsg::VoiceRelease { tag: c });
    let _ = rig.step();
    let m = note(&rig, 1, c);
    rig.post(m);
    let _ = rig.step();
    assert!(voice_of(&rig, c).is_none(), "the late start is dropped");
    assert_eq!(rig.engine.active_voices(), 0);
    assert_eq!(rig.engine.counters().dropped, 1);
}

#[test]
fn pool_exhaustion_steals_the_oldest_open_voice() {
    let mut rig = pad();
    for seq in 0..8 {
        let m = note(&rig, 1, tag(40 + u8::try_from(seq).unwrap(), seq));
        rig.post(m);
        let _ = rig.step();
    }
    assert_eq!(rig.engine.active_voices(), 8);
    let late = tag(90, 100);
    let m = note(&rig, 1, late);
    rig.post(m);
    let _ = rig.step();
    assert_eq!(rig.engine.counters().stolen, 1);
    let _ = rig.run(3);
    assert!(
        voice_of(&rig, tag(40, 0)).is_none(),
        "the oldest was gated out"
    );
    assert!(voice_of(&rig, late).is_some(), "the new note got its slot");
    assert!(voice_of(&rig, tag(41, 1)).is_some());
}

#[test]
fn natural_releases_open_voices_and_panic_gates_them() {
    let mut rig = pad();
    let a = tag(50, 1);
    let m = note(&rig, 1, a);
    rig.post(m);
    let _ = rig.run(4);
    rig.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: rig.engine.now(),
        release: Release::Natural,
    }));
    let _ = rig.step();
    let v = voice_of(&rig, a).expect("still in its release stage");
    assert!(
        v.released() && v.fade.is_none(),
        "Natural: release stage, no gate"
    );
    let _ = rig.run(30);
    assert!(voice_of(&rig, a).is_none());

    let b = tag(51, 2);
    let m = note(&rig, 2, b);
    rig.post(m);
    let _ = rig.run(4);
    rig.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 3,
        effective_time: rig.engine.now(),
        release: Release::Panic,
    }));
    let _ = rig.step();
    assert!(
        voice_of(&rig, b).is_some_and(|v| v.fade.is_some()),
        "Panic: short gate"
    );
    let _ = rig.run(2);
    assert!(voice_of(&rig, b).is_none(), "gone within 3 ms");
}
