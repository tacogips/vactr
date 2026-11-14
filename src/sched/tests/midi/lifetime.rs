//! TASK-007 criterion 7 and the open-input half of criterion 10 (design
//! 11.7 NOTE LIFETIME): release authority is keyed by instance. Every
//! recorded priority-channel record is fed into a real `dsp::Engine` at its
//! arrival time and the engine's voice state is asserted.

use super::{off, on, EngineRig, MidiRig};
use crate::host::wire::{CtlMsg, HostMsg, Release, VoiceTag};
use crate::ns::stage::SlotKey;
use crate::types::diag::DiagCode;

/// A sustaining lane: attack 1 ms, release 50 ms.
const LANE: &str = "s :pluck > midi-notes channel: 1 > attack 0.001 > release 0.05 > d1";

/// A rig playing `LANE` and an engine that heard everything so far.
fn held() -> (MidiRig, EngineRig) {
    held_with(8)
}

fn held_with(voices: u8) -> (MidiRig, EngineRig) {
    let mut rig = MidiRig::new();
    assert!(rig.run(LANE).faults.is_empty());
    rig.run_to(2.1);
    let mut engine = EngineRig::new(voices);
    engine.feed(&rig);
    (rig, engine)
}

/// Presses `notes` in one tick; returns their tags in order.
fn press(rig: &mut MidiRig, engine: &mut EngineRig, notes: &[u8]) -> Vec<VoiceTag> {
    let before = rig.live_starts().len();
    for n in notes {
        rig.send(on(1, *n));
    }
    rig.step();
    engine.feed(rig);
    rig.live_starts()[before..].to_vec()
}

fn release(rig: &mut MidiRig, engine: &mut EngineRig, notes: &[u8]) {
    for n in notes {
        rig.send(off(1, *n));
    }
    rig.step();
    engine.feed(rig);
}

fn sounding(engine: &EngineRig, t: VoiceTag) -> bool {
    engine.voice(t).is_some_and(|v| !v.released())
}

#[test]
fn a_held_note_releases_on_its_note_off_via_its_tag() {
    let (mut rig, mut engine) = held();
    let tags = press(&mut rig, &mut engine, &[60]);
    assert_eq!(tags.len(), 1);
    assert!(sounding(&engine, tags[0]));
    release(&mut rig, &mut engine, &[60]);
    assert_eq!(rig.releases(), tags, "exactly its own tag");
    assert!(engine.voice(tags[0]).unwrap().released());
    engine.run_until(engine.engine.now() + 0.2);
    assert!(
        engine.voice(tags[0]).is_none(),
        "its release stage ended it"
    );
}

#[test]
fn repeated_same_pitch_note_ons_release_earliest_first() {
    let (mut rig, mut engine) = held();
    let a = press(&mut rig, &mut engine, &[60])[0];
    let b = press(&mut rig, &mut engine, &[60])[0];
    assert_ne!(a, b, "instances differ by seq");
    release(&mut rig, &mut engine, &[60]);
    assert_eq!(rig.releases(), vec![a]);
    assert!(engine.voice(a).unwrap().released());
    assert!(sounding(&engine, b), "the later instance still sounds");
    release(&mut rig, &mut engine, &[60]);
    assert_eq!(rig.releases(), vec![a, b]);
    assert!(engine.voice(b).unwrap().released());
}

#[test]
fn an_old_gen_voice_survives_a_rebind_and_its_note_off_ends_only_it() {
    let (mut rig, mut engine) = held();
    let old = press(&mut rig, &mut engine, &[60])[0];
    let old_gen = rig.rt.slot_gen(SlotKey::D(1)).unwrap();
    assert!(rig
        .run("s :pluck > midi-notes channel: 1 > attack 0.001 > gain 0.5 > d1")
        .faults
        .is_empty());
    // Past the rebind boundary (cycle 2 at 4 s).
    rig.run_to(4.2);
    engine.feed(&rig);
    let new_gen = rig.rt.slot_gen(SlotKey::D(1)).unwrap();
    assert!(new_gen > old_gen);
    let rebind = rig
        .slot_controls()
        .into_iter()
        .find(|(_, c)| c.new_gen == new_gen)
        .expect("the rebind's control")
        .1;
    assert_eq!(rebind.release, Release::None);
    assert!(sounding(&engine, old), "rebind lets the open voice ring");
    let new = press(&mut rig, &mut engine, &[64])[0];
    let nv = engine.voice(new).expect("a new-binding voice");
    assert_eq!(nv.gen, new_gen);
    assert_eq!(engine.voice(old).unwrap().gen, old_gen);
    // The old instance's note-off, after the rebind: only the old voice.
    release(&mut rig, &mut engine, &[60]);
    assert_eq!(rig.releases(), vec![old]);
    assert!(engine.voice(old).unwrap().released(), "no stuck note");
    assert!(
        sounding(&engine, new),
        "the new binding's voice is untouched"
    );
    release(&mut rig, &mut engine, &[64]);
    assert!(engine.voice(new).unwrap().released());
}

#[test]
fn stop_puts_open_input_voices_into_release_and_closes_their_records() {
    let (mut rig, mut engine) = held();
    let tags = press(&mut rig, &mut engine, &[60, 62]);
    assert!(tags.iter().all(|t| sounding(&engine, *t)));
    rig.run("stop :d1");
    let stop = rig.slot_controls().last().unwrap().1;
    assert_eq!(stop.release, Release::Natural);
    engine.feed(&rig);
    for t in &tags {
        let v = engine.voice(*t).expect("still in its release stage");
        assert!(v.released(), "Natural = key-up for an open voice");
    }
    for a in rig.rt.midi_in().arrivals() {
        assert!(a.instances.iter().all(|i| !i.open), "records closed");
    }
    release(&mut rig, &mut engine, &[60, 62]);
    assert!(rig.releases().is_empty(), "later note-offs are consumed");
    assert!(rig.rt.midi_in().arrivals().is_empty());
    engine.run_until(engine.engine.now() + 0.2);
    assert_eq!(engine.engine.active_voices(), 0, "no note sustains forever");
}

#[test]
fn a_filtered_note_on_consumes_its_own_note_off() {
    let mut rig = MidiRig::new();
    assert!(rig
        .run("s :pluck > midi-notes channel: 1 > degrade-by 0.5 > attack 0.001 > d1")
        .faults
        .is_empty());
    rig.run_to(2.1);
    let mut engine = EngineRig::new(32);
    engine.feed(&rig);
    for _ in 0..24 {
        rig.send(on(1, 60));
    }
    rig.step();
    engine.feed(&rig);
    let insts: Vec<_> = rig
        .rt
        .midi_in()
        .arrivals()
        .iter()
        .map(|a| a.instances[0].clone())
        .collect();
    assert_eq!(insts.len(), 24);
    let first_filtered = insts.iter().position(|i| i.filtered).expect("one dropped");
    let later_kept = insts[first_filtered..]
        .iter()
        .find(|i| !i.filtered)
        .expect("a later survivor")
        .tag();
    assert!(insts.iter().any(|i| !i.filtered));
    // One note-off per arrival, earliest first: a release is sent exactly
    // for the surviving instances matched so far, never for a later one.
    for k in 0..insts.len() {
        release(&mut rig, &mut engine, &[60]);
        let expected: Vec<VoiceTag> = insts[..=k]
            .iter()
            .filter(|i| !i.filtered)
            .map(|i| i.tag())
            .collect();
        assert_eq!(rig.releases(), expected, "after {} note-offs", k + 1);
        if k == first_filtered {
            assert!(
                sounding(&engine, later_kept),
                "the filtered instance's note-off left the same-pitch voice alone"
            );
        }
    }
}

#[test]
fn a_release_overtaking_its_start_hits_the_tombstone() {
    let mut rig = MidiRig::new();
    assert!(rig.run(LANE).faults.is_empty());
    rig.run_to(2.1);
    rig.send(on(1, 60));
    rig.send(off(1, 60));
    rig.step();
    let posts = rig.posts();
    let [(_, start @ CtlMsg::LiveNoteOn { tag, .. }), (_, rel @ CtlMsg::VoiceRelease { tag: r })] =
        posts.as_slice()
    else {
        panic!("start then release: {posts:?}");
    };
    assert_eq!(tag, r, "ordered on one channel");
    // A reordering transport (defense in depth): the release first.
    let mut engine = EngineRig::new(8);
    engine.post(*rel);
    engine.block();
    engine.post(*start);
    engine.block();
    assert!(engine.voice(*tag).is_none(), "the late start is dropped");
    assert_eq!(engine.engine.active_voices(), 0);
    assert_eq!(engine.engine.counters().dropped, 1);
}

#[test]
fn tag_map_exhaustion_steals_the_oldest_open_voice_with_a_diagnostic() {
    let (mut rig, mut engine) = held_with(4);
    let tags = press(&mut rig, &mut engine, &[60, 62, 64, 65]);
    assert!(tags.iter().all(|t| sounding(&engine, *t)));
    let extra = press(&mut rig, &mut engine, &[67])[0];
    // The victim's short gate (3 ms) ends; the waiting start takes its voice.
    engine.run_until(engine.engine.now() + 0.01);
    assert_eq!(engine.engine.counters().stolen, 1);
    assert!(engine.voice(tags[0]).is_none(), "the oldest was stolen");
    assert!(sounding(&engine, extra), "the new note got its voice");
    assert!(tags[1..].iter().all(|t| sounding(&engine, *t)));
    // The engine's counters reach the runtime: one voice-steal warning.
    for m in engine.acks() {
        if matches!(m, HostMsg::Counters { .. }) {
            rig.audio.reply(m);
        }
    }
    rig.step();
    rig.step();
    assert_eq!(rig.diags_with(DiagCode::VoiceSteal).len(), 1);
    // The stolen instance's note-off finds no voice: nothing else releases.
    release(&mut rig, &mut engine, &[60]);
    assert!(tags[1..].iter().all(|t| sounding(&engine, *t)));
    assert!(sounding(&engine, extra));
}

#[test]
fn hush_panic_gates_open_input_voices() {
    let (mut rig, mut engine) = held();
    let tags = press(&mut rig, &mut engine, &[60, 64]);
    rig.run("hush");
    assert_eq!(
        rig.slot_controls().last().unwrap().1.release,
        Release::Panic
    );
    engine.feed(&rig);
    // A short gate (3 ms), not the 50 ms release stage.
    engine.run_until(engine.engine.now() + 0.01);
    for t in &tags {
        assert!(engine.voice(*t).is_none(), "gated");
    }
    for a in rig.rt.midi_in().arrivals() {
        assert!(a.instances.iter().all(|i| !i.open));
    }
    release(&mut rig, &mut engine, &[60, 64]);
    assert!(rig.releases().is_empty());
}
