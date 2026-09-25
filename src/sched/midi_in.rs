//! MIDI input: `cc` cells, live `midi-notes` realization and note lifetime
//! (design 11.7, 12.8.3 "Input cells", 12.8.12 "MIDI wiring").
//!
//! The runtime drains `MidiInHost` once per tick on the evaluator thread.
//! `Cc` writes the `InputCells` the `cc` signal reads (value / 127). A
//! `NoteOn` is realized on every listening input lane (the bind-time
//! `input_lane_walk` of the bound pattern) with `realize_note`, committed at
//! once through the ordinary commit path (bypassing staging: live input
//! cannot be looked ahead) and posted on the sink's priority channel as
//! `CtlMsg::LiveNoteOn` with `VoiceTag = (slot, channel, pitch, seq)`.
//!
//! Note lifetime is keyed by INSTANCE: every `NoteOn` is recorded as an
//! arrival holding one `NoteInstance` per listening lane, filtered ones too.
//! A `NoteOff` matches the earliest unmatched arrival of its (channel,
//! pitch); a filtered or closed instance consumes it silently, a surviving
//! one sends `VoiceRelease { tag }` (one per voice it started) regardless
//! of any generation bump since. `stop` and `hush` close a slot's instances
//! (their voices get `Natural`/`Panic` through the `SlotControl` path; a
//! MIDI-routed one gets an immediate note-off).
//!
//! Channels are 1..16 on both sides, as in `cc channel:` and `midi-notes
//! channel:`; a `NoteOn` with velocity 0 is a `NoteOff` (MIDI convention).

use std::collections::VecDeque;
use std::rc::Rc;

use crate::host::caps::{MidiEvent, MidiInEvent, OscEvent};
use crate::host::wire::{CtlMsg, VoiceTag};
use crate::ns::evaluator::Evaluator;
use crate::ns::stage::SlotKey;
use crate::pattern::combinators::input::{input_lane_walk, realize_note, Lane, LanePlan, LiveNote};
use crate::pattern::eval::QueryCtx;
use crate::pattern::pat::Pat;
use crate::pattern::query::Event;
use crate::reader::span::Span;
use crate::sched::commit::{commit, CommitCx, Committed, EventCx};
use crate::sched::runtime::{nowhere, Runtime, TickReport};
use crate::sched::slots::{Binding, SlotId, SlotKind};
use crate::sched::telemetry::PlayingEvent;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;
use crate::vm::query_vm::VmQuery;

/// Open arrivals kept at most; the oldest is forgotten beyond it (a device
/// that never sends its note-offs cannot grow the table without bound).
const MAX_ARRIVALS: usize = 1024;

/// Where a surviving instance's voices went.
#[derive(Clone, PartialEq, Debug)]
pub enum LiveSink {
    /// Nothing to release (filtered, dropped, or an OSC message).
    None,
    /// `voices` audio voices share the instance's tag (a chord gives
    /// several).
    Audio { voices: u8 },
    /// MIDI-out notes `(channel, note)` held open until the note-off.
    Midi(Vec<(u8, u8)>),
}

/// One listening lane's record of one `NoteOn` (design 11.7).
#[derive(Clone, PartialEq, Debug)]
pub struct NoteInstance {
    pub key: SlotKey,
    pub slot: SlotId,
    pub gen: u32,
    pub seq: u32,
    /// Dropped by an input-lane filter (or a fault): its note-off is
    /// consumed silently.
    pub filtered: bool,
    pub channel: u8,
    pub pitch: u8,
    /// False once `stop`/`hush` closed it.
    pub open: bool,
    pub sink: LiveSink,
}

impl NoteInstance {
    /// The instance's voice tag.
    #[must_use]
    pub fn tag(&self) -> VoiceTag {
        VoiceTag {
            slot: self.slot,
            channel: self.channel,
            pitch: self.pitch,
            seq: self.seq,
        }
    }
}

/// One `NoteOn` and the instances it created (possibly none).
#[derive(Clone, PartialEq, Debug)]
pub struct Arrival {
    pub channel: u8,
    pub pitch: u8,
    pub instances: Vec<NoteInstance>,
}

/// A note event waiting for the live-note step of the tick.
#[derive(Clone, Copy, PartialEq, Debug)]
enum NoteMsg {
    On { channel: u8, pitch: u8, vel: u8 },
    Off { channel: u8, pitch: u8 },
}

/// The input lanes of one slot's bound pattern.
#[derive(Clone, Debug)]
struct LaneEntry {
    key: SlotKey,
    pat: Rc<Pat>,
    plan: LanePlan,
}

/// The input-lane walk of every bound pattern that has input lanes,
/// recomputed only when a slot's bound pattern changes.
#[derive(Clone, Debug, Default)]
pub struct LaneTable {
    entries: Vec<LaneEntry>,
}

impl LaneTable {
    /// The lanes of `key`, if its bound pattern has any.
    #[must_use]
    pub fn lanes(&self, key: SlotKey) -> Option<&[Lane]> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| &*e.plan.lanes)
    }
}

/// MIDI input state owned by the runtime.
#[derive(Clone, Debug, Default)]
pub struct MidiIn {
    lanes: LaneTable,
    pending: Vec<NoteMsg>,
    arrivals: VecDeque<Arrival>,
    next_arrival: u32,
    next_seq: u32,
    stolen_seen: u32,
}

impl MidiIn {
    /// The recorded arrivals not yet matched by a note-off, oldest first.
    #[must_use]
    pub fn arrivals(&self) -> &VecDeque<Arrival> {
        &self.arrivals
    }

    /// The lane table.
    #[must_use]
    pub fn lane_table(&self) -> &LaneTable {
        &self.lanes
    }

    fn arrival_seq(&mut self) -> u32 {
        self.next_arrival = self.next_arrival.wrapping_add(1);
        self.next_arrival
    }

    fn seq(&mut self) -> u32 {
        self.next_seq = self.next_seq.wrapping_add(1);
        self.next_seq
    }
}

/// A slot listening to live input, captured before realization.
struct Listener {
    key: SlotKey,
    slot: SlotId,
    name: KwId,
    gen: u32,
    span: Option<Span>,
    muted: bool,
    lane: Lane,
}

impl Runtime {
    /// Handles the MIDI input events of one poll: `cc` cells at once, clock
    /// and transport through the slave, notes queued for `play_live_notes`.
    pub(crate) fn take_midi_in(&mut self, host_now: f64, rep: &mut TickReport) {
        let events: Vec<MidiInEvent> = self.hosts.midi_in.poll().to_vec();
        for e in events {
            match e {
                MidiInEvent::Cc {
                    ch,
                    controller,
                    value,
                    ..
                } => self
                    .input
                    .set_cc(ch, controller, f32::from(value.min(127)) / 127.0),
                MidiInEvent::NoteOn { ch, note, vel, .. } if vel > 0 => {
                    self.midi_in.pending.push(NoteMsg::On {
                        channel: ch,
                        pitch: note,
                        vel,
                    });
                }
                MidiInEvent::NoteOn { ch, note, .. } | MidiInEvent::NoteOff { ch, note, .. } => {
                    self.midi_in.pending.push(NoteMsg::Off {
                        channel: ch,
                        pitch: note,
                    });
                }
                MidiInEvent::Clock { time } => self.clock_pulse(time),
                MidiInEvent::Start => self.transport_start(host_now),
                MidiInEvent::Stop => self.transport_stop(host_now),
                MidiInEvent::Continue => self.transport_continue(host_now),
            }
        }
        self.check_clock_lost(host_now, &mut rep.diags);
    }

    /// Realizes and releases the queued notes in arrival order (after the
    /// tick activated pending bindings).
    pub(crate) fn play_live_notes(&mut self, ev: &mut Evaluator, now: f64, rep: &mut TickReport) {
        let notes = std::mem::take(&mut self.midi_in.pending);
        if notes.is_empty() {
            return;
        }
        self.sync_lanes();
        for n in notes {
            match n {
                NoteMsg::On {
                    channel,
                    pitch,
                    vel,
                } => self.note_on(ev, channel, pitch, vel, now, rep),
                NoteMsg::Off { channel, pitch } => self.note_off(channel, pitch, now),
            }
        }
    }

    /// Recomputes the lane walk of every slot whose bound pattern changed.
    fn sync_lanes(&mut self) {
        let mut entries = Vec::new();
        for slot in self.slots.iter() {
            let Some(Binding::Pattern(p)) = &slot.bound else {
                continue;
            };
            let known = self
                .midi_in
                .lanes
                .entries
                .iter()
                .find(|e| e.key == slot.key && Rc::ptr_eq(&e.pat, p));
            if let Some(e) = known {
                entries.push(e.clone());
                continue;
            }
            // A lane error was already rejected by the bind-time walk.
            if let Ok(plan) = input_lane_walk(p) {
                if !plan.lanes.is_empty() {
                    entries.push(LaneEntry {
                        key: slot.key,
                        pat: Rc::clone(p),
                        plan,
                    });
                }
            }
        }
        self.midi_in.lanes.entries = entries;
    }

    fn listeners(&self, channel: u8) -> Vec<Listener> {
        let mut out = Vec::new();
        for e in &self.midi_in.lanes.entries {
            let Some(slot) = self.slots.get(e.key) else {
                continue;
            };
            // The generation of the lane playing this binding (a pending
            // rebind already bumped the slot's newest generation).
            let gen = slot
                .lanes
                .iter()
                .find(|l| Rc::ptr_eq(&l.pat, &e.pat))
                .map_or(slot.gen, |l| l.gen);
            for lane in &e.plan.lanes {
                if lane.channel.is_some_and(|c| c != channel) {
                    continue;
                }
                out.push(Listener {
                    key: slot.key,
                    slot: slot.id,
                    name: slot.name(),
                    gen,
                    span: e.pat.span,
                    muted: slot.muted,
                    lane: lane.clone(),
                });
            }
        }
        out
    }

    fn note_on(
        &mut self,
        ev: &mut Evaluator,
        channel: u8,
        pitch: u8,
        vel: u8,
        now: f64,
        rep: &mut TickReport,
    ) {
        let arrival = self.midi_in.arrival_seq();
        let mut rec = Arrival {
            channel,
            pitch,
            instances: Vec::new(),
        };
        for l in self.listeners(channel) {
            let seq = self.midi_in.seq();
            let mut inst = NoteInstance {
                key: l.key,
                slot: l.slot,
                gen: l.gen,
                seq,
                filtered: true,
                channel,
                pitch,
                open: true,
                sink: LiveSink::None,
            };
            let note = LiveNote {
                channel,
                note: pitch,
                velocity: vel,
                seq: u64::from(arrival),
                at: self.pos,
            };
            let realized = {
                let (vm, ns) = ev.vm_and_ns();
                let mut h = VmQuery::new(vm, ns);
                let mut cx = QueryCtx::new(&mut h, &self.input, self.cfg.seed);
                cx.tempo = self.clock.tempo();
                realize_note(&l.lane, &note, &mut cx)
            };
            match realized {
                Ok(Some(e)) if !l.muted => match self.commit_live(&e, &l, inst.tag(), now, rep) {
                    Ok(LiveSink::None) => {}
                    Ok(sink) => {
                        inst.filtered = false;
                        inst.sink = sink;
                    }
                    Err(f) => rep.faults.push(f),
                },
                Ok(_) => {}
                Err(mut f) => {
                    f.origin.slot = f.origin.slot.or(Some(l.name));
                    rep.faults.push(f);
                }
            }
            rec.instances.push(inst);
        }
        self.midi_in.arrivals.push_back(rec);
        while self.midi_in.arrivals.len() > MAX_ARRIVALS {
            self.midi_in.arrivals.pop_front();
        }
    }

    /// Commits one realized live note and hands it to its sink at once.
    fn commit_live(
        &mut self,
        e: &Event,
        l: &Listener,
        tag: VoiceTag,
        now: f64,
        rep: &mut TickReport,
    ) -> Result<LiveSink, Failure> {
        let ecx = EventCx {
            slot: l.slot,
            slot_name: l.name,
            gen: l.gen,
            span: l.span,
            overrides: &[],
        };
        let mut diags = Vec::new();
        let mut cx = CommitCx {
            clock: &self.clock,
            cells: &mut self.cells,
            resolver: self.resolver.as_ref(),
            samples: &mut self.samples,
            hosts: &mut self.hosts,
            caps: &self.caps,
            diags: &mut diags,
        };
        let c = commit(e, &mut cx, &ecx);
        rep.diags.extend(diags);
        let sink = match c? {
            Committed::Audio(evs) => {
                let voices = u8::try_from(evs.len()).unwrap_or(u8::MAX);
                for mut a in evs {
                    a.time = now;
                    self.hosts.audio.post(CtlMsg::LiveNoteOn { tag, ev: a });
                }
                LiveSink::Audio { voices }
            }
            Committed::Midi(evs) => {
                let mut held = Vec::with_capacity(evs.len());
                for m in evs {
                    if let MidiEvent::Note {
                        slot,
                        gen,
                        ch,
                        note,
                        vel,
                        ..
                    } = m
                    {
                        // Open duration: the note-off ends it.
                        self.hosts.midi.send(MidiEvent::Note {
                            time: now,
                            slot,
                            gen,
                            ch,
                            note,
                            vel,
                            dur: f64::INFINITY,
                        });
                        held.push((ch, note));
                    }
                }
                LiveSink::Midi(held)
            }
            Committed::Osc(o) => {
                self.hosts.osc.send(OscEvent { time: now, ..o });
                rep.committed += 1;
                return Ok(LiveSink::None);
            }
            Committed::Dropped => return Ok(LiveSink::None),
        };
        rep.committed += 1;
        let beat = self
            .clock
            .tempo()
            .beats_at(self.pos)
            .unwrap_or(Ratio64::ZERO);
        let ctls: Vec<(KwId, Value)> = e.controls.iter().map(|(k, v)| (*k, v.clone())).collect();
        self.telemetry.publish(
            PlayingEvent {
                slot: l.name,
                beat,
                time: now,
                src: e.src,
                dur: 0.0,
                kind: SlotKind::Pattern,
                reduced_lead: false,
            },
            &ctls,
        );
        Ok(sink)
    }

    /// A note-off: the earliest unmatched arrival of (channel, pitch).
    fn note_off(&mut self, channel: u8, pitch: u8, now: f64) {
        let Some(i) = self
            .midi_in
            .arrivals
            .iter()
            .position(|a| a.channel == channel && a.pitch == pitch)
        else {
            return;
        };
        let Some(a) = self.midi_in.arrivals.remove(i) else {
            return;
        };
        for inst in &a.instances {
            if !inst.filtered && inst.open {
                self.release_instance(inst, now);
            }
        }
    }

    fn release_instance(&mut self, inst: &NoteInstance, now: f64) {
        match &inst.sink {
            LiveSink::None => {}
            LiveSink::Audio { voices } => {
                for _ in 0..*voices {
                    self.hosts
                        .audio
                        .post(CtlMsg::VoiceRelease { tag: inst.tag() });
                }
            }
            LiveSink::Midi(notes) => {
                for &(ch, note) in notes {
                    self.hosts.midi.send(MidiEvent::NoteOff {
                        time: now,
                        slot: inst.slot,
                        gen: inst.gen,
                        ch,
                        note,
                    });
                }
            }
        }
    }

    /// `stop` (one slot) or `hush` / transport stop (`SlotKey::All`):
    /// closes the open instances so their later note-offs are consumed
    /// silently. Audio voices follow the `SlotControl` just sent; MIDI-out
    /// notes get their note-off now.
    pub(crate) fn close_live_notes(&mut self, key: SlotKey, now: f64) {
        let mut offs = Vec::new();
        for a in &mut self.midi_in.arrivals {
            for inst in &mut a.instances {
                if inst.open && (key == SlotKey::All || inst.key == key) {
                    inst.open = false;
                    if !inst.filtered && matches!(inst.sink, LiveSink::Midi(_)) {
                        offs.push(inst.clone());
                    }
                }
            }
        }
        for inst in &offs {
            self.release_instance(inst, now);
        }
    }

    /// The engine's cumulative steal counter: a `voice-steal` warning for
    /// every increase (11.7: pool exhaustion steals the oldest open voice).
    pub(crate) fn note_stolen(&mut self, stolen: u32, diags: &mut Vec<Diagnostic>) {
        if stolen <= self.midi_in.stolen_seen {
            return;
        }
        let n = stolen - self.midi_in.stolen_seen;
        self.midi_in.stolen_seen = stolen;
        diags.push(Diagnostic {
            severity: DiagCode::VoiceSteal.default_severity(),
            ..Diagnostic::error(
                DiagCode::VoiceSteal,
                nowhere(),
                format!("the voice pool is full: {n} open input voice(s) stolen"),
            )
        });
    }
}
