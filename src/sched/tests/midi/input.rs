//! TASK-007 criterion 6 (input half, design 11.7): a `midi-notes` input
//! sounds a voice within one drain tick plus the commit path, bypassing
//! staging; `cc` writes its cell; `midi-notes` yields no events in dry runs;
//! `degrade-by 1` after `midi-notes` drops every note but still records the
//! filtered instances.

use super::{off, on, EngineRig, MidiRig};
use crate::host::caps::MidiInEvent;
use crate::host::wire::{Ctl, CtlMsg};
use crate::ns::stage::SlotKey;
use crate::pattern::eval::InputCells;
use crate::sched::dryrun::dry_run;
use crate::sched::slots::Binding;
use crate::sched::tests::sched::{cval, SYNTH};
use crate::value::value::Value;
use crate::vm::query_vm::VmQuery;

const LANE: &str = "s :pluck > midi-notes channel: 1 > d1";

#[test]
fn a_note_sounds_a_voice_within_one_drain_tick() {
    let mut rig = MidiRig::new();
    assert!(rig.run(LANE).faults.is_empty());
    rig.run_to(2.1);
    let mut engine = EngineRig::new(8);
    engine.feed(&rig);
    assert!(rig.live_starts().is_empty(), "no input yet, nothing plays");
    rig.send(on(1, 69));
    let rep = rig.step();
    assert_eq!(
        rep.committed, 1,
        "committed in the very tick that drained it"
    );
    let posts = rig.posts();
    let [(t, CtlMsg::LiveNoteOn { tag, ev })] = posts.as_slice() else {
        panic!("one live start: {posts:?}");
    };
    assert!((t - 2.11).abs() < 1e-9, "posted at the drain tick");
    assert_eq!((tag.channel, tag.pitch), (1, 69));
    let slot = rig.rt.slots().get(SlotKey::D(1)).expect("d1");
    assert_eq!(tag.slot, slot.id);
    assert_eq!(ev.gen, slot.gen);
    assert_eq!(ev.inst, SYNTH);
    let hz = cval(ev, "freq").expect("a constant freq");
    assert!((hz - 440.0).abs() < 0.01, "note 69 is 440 Hz, got {hz}");
    assert_eq!(rig.ring_events(), 0, "live input never uses the event ring");
    engine.feed(&rig);
    let v = engine.voice(*tag).expect("the voice sounds");
    assert!(!v.released(), "open duration: held until its note-off");
    // A note on another channel has no listener.
    rig.send(on(2, 60));
    assert_eq!(rig.step().committed, 0);
    assert_eq!(rig.live_starts().len(), 1);
}

#[test]
fn cc_writes_its_cell_and_the_signal_reads_it() {
    let mut rig = MidiRig::new();
    rig.send(MidiInEvent::Cc {
        ch: 2,
        controller: 74,
        value: 127,
        time: 0.0,
    });
    rig.send(MidiInEvent::Cc {
        ch: 1,
        controller: 7,
        value: 64,
        time: 0.0,
    });
    rig.tick_at(0.0);
    let cells = rig.rt.input_cells();
    assert!((cells.cc(2, 74) - 1.0).abs() < 1e-6);
    assert!((cells.cc(1, 7) - 64.0 / 127.0).abs() < 1e-6);
    assert!(
        cells.cc(1, 74).abs() < 1e-6,
        "cells are per (channel, controller)"
    );
    // A pattern reading the signal hears the cell at query time.
    assert!(rig
        .run("s [:bd :bd :bd :bd] > gain {cc 74 channel: 2} > d1")
        .faults
        .is_empty());
    rig.run_to(2.5);
    let gains: Vec<f32> = rig
        .audio
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            crate::host::testing::AudioCall::Send(e) => super::super::sched::ctl(&e, "gain"),
            _ => None,
        })
        .map(|c| match c {
            Ctl::Const(v) => v,
            Ctl::Cell(_) => panic!("an immediate signal value"),
        })
        .collect();
    assert!(!gains.is_empty());
    for g in gains {
        assert!((g - 1.0).abs() < 1e-6, "gain follows cc 74 on channel 2");
    }
}

#[test]
fn midi_notes_never_produces_events_in_dry_runs() {
    let mut rig = MidiRig::new();
    let out = rig.eval("s :pluck > midi-notes channel: 1");
    let Ok(v @ Value::Pattern(_)) = &out[0].value else {
        panic!("a pattern: {:?}", out[0].value);
    };
    let b = Binding::from_value(v).expect("binding");
    let cells = InputCells::new();
    let r = {
        let (vm, ns) = rig.ev.vm_and_ns();
        let mut h = VmQuery::new(vm, ns);
        dry_run(&b, &mut h, &cells, 0)
    };
    assert!(r.events.is_empty() && r.faults.is_empty());
    // Bound and played for two cycles with no input: nothing is sent.
    assert!(rig.run(LANE).faults.is_empty());
    rig.run_to(4.0);
    assert_eq!(rig.ring_events(), 0);
    assert!(rig.posts().is_empty());
}

#[test]
fn degrade_by_1_drops_every_note_but_records_filtered_instances() {
    let mut rig = MidiRig::new();
    assert!(rig
        .run("s :pluck > midi-notes channel: 1 > degrade-by 1 > d1")
        .faults
        .is_empty());
    rig.run_to(2.1);
    for n in 0..5 {
        rig.send(on(1, 60 + n));
    }
    assert_eq!(rig.step().committed, 0);
    assert!(rig.live_starts().is_empty(), "every note dropped");
    let arrivals = rig.rt.midi_in().arrivals();
    assert_eq!(arrivals.len(), 5, "every NoteOn recorded");
    for (i, a) in arrivals.iter().enumerate() {
        assert_eq!(a.pitch, 60 + u8::try_from(i).unwrap());
        assert_eq!(a.instances.len(), 1);
        assert!(a.instances[0].filtered, "recorded as filtered");
        assert!(a.instances[0].open);
    }
    for n in 0..5 {
        rig.send(off(1, 60 + n));
    }
    rig.step();
    assert!(
        rig.releases().is_empty(),
        "filtered notes consume their offs"
    );
    assert!(rig.rt.midi_in().arrivals().is_empty());
}
