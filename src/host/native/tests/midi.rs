//! `parse_midi` and the output queue's timing and 11.3 revocation.

use crate::host::caps::{MidiEvent, MidiInEvent};
use crate::host::native::midi::MidiMsg;
use crate::host::native::{parse_midi, MidiOutQueue};
use crate::host::wire::{Release, SlotControl};
use crate::sched::slots::SlotId;

const T: f64 = 1.5;

#[test]
fn note_on_and_off() {
    assert_eq!(
        parse_midi(&[0x90, 60, 100], T),
        Some(MidiInEvent::NoteOn {
            ch: 1,
            note: 60,
            vel: 100,
            time: T
        })
    );
    assert_eq!(
        parse_midi(&[0x83, 61, 64], T),
        Some(MidiInEvent::NoteOff {
            ch: 4,
            note: 61,
            time: T
        })
    );
}

#[test]
fn velocity_zero_is_note_off() {
    assert_eq!(
        parse_midi(&[0x9F, 72, 0], T),
        Some(MidiInEvent::NoteOff {
            ch: 16,
            note: 72,
            time: T
        })
    );
}

#[test]
fn control_change() {
    assert_eq!(
        parse_midi(&[0xB2, 74, 127], T),
        Some(MidiInEvent::Cc {
            ch: 3,
            controller: 74,
            value: 127,
            time: T
        })
    );
}

#[test]
fn realtime_messages() {
    assert_eq!(parse_midi(&[0xF8], T), Some(MidiInEvent::Clock { time: T }));
    assert_eq!(parse_midi(&[0xFA], T), Some(MidiInEvent::Start));
    assert_eq!(parse_midi(&[0xFC], T), Some(MidiInEvent::Stop));
    assert_eq!(parse_midi(&[0xFB], T), Some(MidiInEvent::Continue));
}

#[test]
fn running_status_and_malformed_input_are_ignored() {
    // Running status: data bytes with no status byte.
    assert_eq!(parse_midi(&[60, 100], T), None);
    assert_eq!(parse_midi(&[], T), None);
    // Short messages and data bytes with the high bit set.
    assert_eq!(parse_midi(&[0x90, 60], T), None);
    assert_eq!(parse_midi(&[0x90], T), None);
    assert_eq!(parse_midi(&[0xB0, 0x80, 1], T), None);
    // Other statuses: program change, pitch bend, sysex, active sensing.
    assert_eq!(parse_midi(&[0xC0, 5], T), None);
    assert_eq!(parse_midi(&[0xE0, 0, 64], T), None);
    assert_eq!(parse_midi(&[0xF0, 1, 2, 0xF7], T), None);
    assert_eq!(parse_midi(&[0xFE], T), None);
}

fn note(time: f64, gen: u32, n: u8, dur: f64) -> MidiEvent {
    MidiEvent::Note {
        time,
        slot: SlotId::new(1),
        gen,
        ch: 1,
        note: n,
        vel: 100,
        dur,
    }
}

fn due(q: &mut MidiOutQueue, now: f64) -> Vec<Vec<u8>> {
    let mut out: Vec<MidiMsg> = Vec::new();
    q.take_due(now, &mut out);
    out.iter().map(|m| m.as_bytes().to_vec()).collect()
}

#[test]
fn notes_go_out_on_and_off_at_their_times() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, 0.5));
    q.schedule(MidiEvent::Clock { time: 0.5 });
    assert_eq!(q.next_due(), Some(0.5));
    assert_eq!(due(&mut q, 0.9), vec![vec![0xF8]]);
    assert_eq!(due(&mut q, 1.0), vec![vec![0x90, 60, 100]]);
    assert!(due(&mut q, 1.49).is_empty());
    assert_eq!(due(&mut q, 1.5), vec![vec![0x80, 60, 0]]);
    assert!(q.is_empty());
}

#[test]
fn channel_sixteen_uses_nibble_fifteen() {
    let mut q = MidiOutQueue::new();
    q.schedule(MidiEvent::NoteOff {
        time: 0.0,
        slot: SlotId::new(1),
        gen: 1,
        ch: 16,
        note: 3,
    });
    assert_eq!(due(&mut q, 0.0), vec![vec![0x8F, 3, 0]]);
}

#[test]
fn a_rebind_revokes_unstarted_older_notes_only() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, 0.25)); // before the effective time
    q.schedule(note(2.0, 1, 62, 0.25)); // revoked
    q.schedule(note(2.0, 2, 64, 0.25)); // the new generation
    q.control(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: 1.5,
        release: Release::None,
    });
    let mut sent = due(&mut q, 1.0);
    sent.extend(due(&mut q, 10.0));
    assert_eq!(
        sent,
        vec![
            vec![0x90, 60, 100],
            vec![0x80, 60, 0],
            vec![0x90, 64, 100],
            vec![0x80, 64, 0],
        ]
    );
}

#[test]
fn a_hush_cuts_sounding_older_notes_at_the_effective_time() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, 4.0));
    assert_eq!(due(&mut q, 1.0), vec![vec![0x90, 60, 100]]);
    q.control(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: 1.2,
        release: Release::Panic,
    });
    assert_eq!(q.next_due(), Some(1.2));
    assert_eq!(due(&mut q, 1.2), vec![vec![0x80, 60, 0]]);
}

#[test]
fn a_stop_lets_sounding_notes_end_naturally() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, 4.0));
    let _ = due(&mut q, 1.0);
    q.control(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: 1.2,
        release: Release::Natural,
    });
    assert_eq!(q.next_due(), Some(5.0));
}

#[test]
fn other_slots_are_untouched() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(2.0, 1, 60, 0.5));
    q.control(SlotControl {
        slot: SlotId::new(9),
        new_gen: 5,
        effective_time: 0.0,
        release: Release::Panic,
    });
    assert_eq!(q.len(), 2);
}

#[test]
fn shutdown_releases_only_sounding_notes() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, 4.0));
    q.schedule(note(3.0, 1, 62, 4.0));
    let _ = due(&mut q, 1.0);
    let mut out = Vec::new();
    q.release_all(&mut out);
    let bytes: Vec<Vec<u8>> = out.iter().map(|m| m.as_bytes().to_vec()).collect();
    assert_eq!(bytes, vec![vec![0x80, 60, 0]]);
    assert!(q.is_empty());
}

#[test]
fn an_open_note_waits_for_its_explicit_note_off() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, f64::INFINITY));
    assert_eq!(due(&mut q, 1.0), vec![vec![0x90, 60, 100]]);
    assert!(due(&mut q, 1.0e9).is_empty());
    q.schedule(MidiEvent::NoteOff {
        time: 3.0,
        slot: SlotId::new(1),
        gen: 1,
        ch: 1,
        note: 60,
    });
    assert!(due(&mut q, 2.9).is_empty());
    assert_eq!(due(&mut q, 3.0), vec![vec![0x80, 60, 0]]);
    assert!(q.is_empty());
}

#[test]
fn repeated_open_notes_close_earliest_first() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, f64::INFINITY));
    q.schedule(note(2.0, 1, 60, f64::INFINITY));
    let _ = due(&mut q, 2.0);
    let off = |time| MidiEvent::NoteOff {
        time,
        slot: SlotId::new(1),
        gen: 1,
        ch: 1,
        note: 60,
    };
    q.schedule(off(3.0));
    assert_eq!(due(&mut q, 3.0), vec![vec![0x80, 60, 0]]);
    assert_eq!(q.len(), 1);
    q.schedule(off(4.0));
    assert_eq!(due(&mut q, 4.0), vec![vec![0x80, 60, 0]]);
    assert!(q.is_empty());
}

#[test]
fn a_panic_cuts_an_open_note() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, f64::INFINITY));
    let _ = due(&mut q, 1.0);
    q.control(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: 1.5,
        release: Release::Panic,
    });
    assert_eq!(due(&mut q, 1.5), vec![vec![0x80, 60, 0]]);
    assert!(q.is_empty());
}

#[test]
fn shutdown_releases_a_sounding_open_note() {
    let mut q = MidiOutQueue::new();
    q.schedule(note(1.0, 1, 60, f64::INFINITY));
    q.schedule(note(5.0, 1, 62, f64::INFINITY));
    let _ = due(&mut q, 1.0);
    let mut out = Vec::new();
    q.release_all(&mut out);
    let bytes: Vec<Vec<u8>> = out.iter().map(|m| m.as_bytes().to_vec()).collect();
    assert_eq!(bytes, vec![vec![0x80, 60, 0]]);
}
