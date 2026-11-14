//! The slot control channel (design 11.3 "Control channel: two ordered
//! classes per slot").
//!
//! Per slot at most two outstanding entries: one IMMEDIATE (stop, hush,
//! tempo; merged monotonically with `SlotControl::merge`, so a Panic is never
//! downgraded) and one FUTURE (a rebind at its boundary; a newer rebind
//! replaces only it). Entries are delivered in `effective_time` order to
//! every sink of a pattern slot (audio, MIDI, OSC) on their priority
//! channels, re-sent every `resend_ticks` until a `SlotControlAck` covers
//! them, and reported as `host-transport` after `diag_ticks`. An entry still
//! unacknowledged when the host clock passes its effective time is a missed
//! deadline, reported once.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::host::caps::{Hosts, MidiEvent};
use crate::host::wire::Release;
use crate::host::wire::{SlotControl, SlotControlAck};
use crate::ns::stage::SlotKey;
use crate::reader::span::{FileId, Span};
use crate::sched::runtime::{ceil, empty_program, Runtime};
use crate::sched::slots::SlotId;
use crate::sched::slots::{Binding, SlotKind};
use crate::sched::staging::Lane;
use crate::tex::shader::compile_tex;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::vm::fail::{FailCode, Failure};

/// Which class an entry belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlClass {
    Immediate,
    Future,
}

/// One outstanding control.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Entry {
    pub ctl: SlotControl,
    /// Ticks since it was last (re)sent without an acknowledgment.
    pub unacked: u32,
    pub sends: u32,
    reported: bool,
    deadline_reported: bool,
}

impl Entry {
    fn new(ctl: SlotControl) -> Self {
        Self {
            ctl,
            unacked: 0,
            sends: 0,
            reported: false,
            deadline_reported: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SlotEntries {
    key: Option<SlotKey>,
    immediate: Option<Entry>,
    future: Option<Entry>,
}

/// The per-slot two-class channel.
#[derive(Clone, Debug, Default)]
pub struct ControlChannel {
    slots: BTreeMap<SlotId, SlotEntries>,
    resend_ticks: u32,
    diag_ticks: u32,
    pending_transport_diagnostic: Option<Diagnostic>,
}

fn nowhere() -> Span {
    Span::new(FileId::new(0), 0, 0)
}

fn send_all(hosts: &mut Hosts, c: SlotControl) {
    hosts.audio.control(c);
    hosts.midi.control(c);
    hosts.osc.control(c);
}

impl ControlChannel {
    #[must_use]
    pub fn new(resend_ticks: u32, diag_ticks: u32) -> Self {
        Self {
            slots: BTreeMap::new(),
            resend_ticks: resend_ticks.max(1),
            diag_ticks: diag_ticks.max(1),
            pending_transport_diagnostic: None,
        }
    }

    /// Queues one bounded output-transport diagnostic for the next tick.
    pub(crate) fn report_transport(&mut self, message: String) {
        self.pending_transport_diagnostic = Some(Diagnostic::error(
            DiagCode::HostTransport,
            nowhere(),
            message,
        ));
    }

    /// Queues an immediate control, merged into the outstanding one, and
    /// delivers the merged entry now.
    pub fn immediate(&mut self, key: SlotKey, c: SlotControl, hosts: &mut Hosts) {
        let s = self.slots.entry(c.slot).or_default();
        s.key = Some(key);
        let merged = match s.immediate {
            Some(e) => e.ctl.merge(c),
            None => c,
        };
        let mut e = Entry::new(merged);
        e.sends = 1;
        s.immediate = Some(e);
        send_all(hosts, merged);
    }

    /// Queues a future (boundary) control, replacing only the previous
    /// future entry, and delivers it now (after the immediate entry, which
    /// is earlier by construction).
    pub fn future(&mut self, key: SlotKey, c: SlotControl, hosts: &mut Hosts) {
        let s = self.slots.entry(c.slot).or_default();
        s.key = Some(key);
        let mut e = Entry::new(c);
        e.sends = 1;
        s.future = Some(e);
        send_all(hosts, c);
    }

    /// Drops the future entry of a slot (a stop or hush supersedes it).
    pub fn cancel_future(&mut self, slot: SlotId) {
        if let Some(s) = self.slots.get_mut(&slot) {
            s.future = None;
        }
    }

    /// Re-stamps an outstanding future entry with a newer generation (a
    /// tempo change) and re-sends it.
    pub fn restamp_future(&mut self, slot: SlotId, gen: u32, hosts: &mut Hosts) {
        if let Some(e) = self.slots.get_mut(&slot).and_then(|s| s.future.as_mut()) {
            e.ctl.new_gen = e.ctl.new_gen.max(gen);
            e.unacked = 0;
            e.sends += 1;
            send_all(hosts, e.ctl);
        }
    }

    /// An acknowledgment: every entry of the slot with a generation at or
    /// below the acknowledged one is delivered.
    pub fn ack(&mut self, a: SlotControlAck) {
        if let Some(s) = self.slots.get_mut(&a.slot) {
            if s.immediate.is_some_and(|e| e.ctl.new_gen <= a.gen) {
                s.immediate = None;
            }
            if s.future.is_some_and(|e| e.ctl.new_gen <= a.gen) {
                s.future = None;
            }
        }
        self.slots
            .retain(|_, s| s.immediate.is_some() || s.future.is_some());
    }

    /// One tick: re-sends unacknowledged entries in effective-time order,
    /// reports persistent loss and missed deadlines.
    pub fn tick(&mut self, host_now: f64, hosts: &mut Hosts) -> Vec<Diagnostic> {
        let mut diags = self
            .pending_transport_diagnostic
            .take()
            .into_iter()
            .collect::<Vec<_>>();
        let (resend, diag) = (self.resend_ticks, self.diag_ticks);
        for s in self.slots.values_mut() {
            let name = s.key.map_or_else(|| "?".to_string(), SlotKey::name);
            let mut es: Vec<(bool, &mut Entry)> = s
                .immediate
                .iter_mut()
                .map(|e| (false, e))
                .chain(s.future.iter_mut().map(|e| (true, e)))
                .collect();
            es.sort_by(|a, b| a.1.ctl.effective_time.total_cmp(&b.1.ctl.effective_time));
            for (future, e) in es {
                e.unacked += 1;
                if e.unacked % resend == 0 {
                    e.sends += 1;
                    send_all(hosts, e.ctl);
                }
                if e.unacked >= diag && !e.reported {
                    e.reported = true;
                    diags.push(Diagnostic::error(
                        DiagCode::HostTransport,
                        nowhere(),
                        format!(
                            "slot {name}: control for generation {} is not acknowledged \
                             after {} ticks",
                            e.ctl.new_gen, e.unacked
                        ),
                    ));
                }
                // Only a boundary control has a deadline: an immediate one
                // is due at once by definition.
                if future && host_now > e.ctl.effective_time && !e.deadline_reported {
                    e.deadline_reported = true;
                    diags.push(Diagnostic::error(
                        DiagCode::HostTransport,
                        nowhere(),
                        format!(
                            "slot {name}: control for generation {} missed its deadline \
                             ({:.1} ms late)",
                            e.ctl.new_gen,
                            (host_now - e.ctl.effective_time) * 1000.0
                        ),
                    ));
                }
            }
        }
        diags
    }

    /// Whether an outstanding slot control has already reported transport loss.
    pub(crate) fn transport_reported(&self) -> bool {
        self.slots.values().any(|slot| {
            slot.immediate.is_some_and(|entry| entry.reported)
                || slot.future.is_some_and(|entry| entry.reported)
        })
    }

    pub(crate) fn has_outstanding(&self) -> bool {
        self.slots
            .values()
            .any(|slot| slot.immediate.is_some() || slot.future.is_some())
    }

    /// The outstanding entry of a class.
    #[must_use]
    pub fn outstanding(&self, slot: SlotId, class: ControlClass) -> Option<Entry> {
        let s = self.slots.get(&slot)?;
        match class {
            ControlClass::Immediate => s.immediate,
            ControlClass::Future => s.future,
        }
    }
}

impl Runtime {
    /// Sets `pending` for the next cycle boundary and opens the new lane
    /// there (prospective activation): old staged work at and after the
    /// boundary is discarded and, for a slot that already played, revoked
    /// by the boundary control (11.2, 11.3).
    pub(crate) fn rebind(&mut self, key: SlotKey, binding: Binding) -> Result<(), Failure> {
        let (now, pos) = self.now_pos();
        let boundary = ceil(pos);
        let effective = self.clock.to_host(boundary).max(now);
        let slot = self.slots.get_or_insert(key);
        let used = slot.gen > 0;
        match &binding {
            Binding::Texture(t) => {
                if slot.out_id().is_none() {
                    return Err(Failure::new(
                        FailCode::Type,
                        "a texture plays on an output `o0`..`o3`",
                    ));
                }
                compile_tex(t)?;
                slot.kind = SlotKind::Texture;
                slot.gen += 1;
                slot.pending = Some((binding, boundary));
            }
            Binding::Pattern(p) => {
                slot.kind = SlotKind::Pattern;
                slot.gen += 1;
                let gen = slot.gen;
                // A newer rebind supersedes a lane not yet active.
                slot.lanes.retain(|l| l.from <= pos);
                for l in &mut slot.lanes {
                    l.until = Some(l.until.map_or(boundary, |u| u.min(boundary)));
                    l.staging.truncate_from(boundary);
                    l.queried_to = l.queried_to.min(boundary);
                    l.dirty.clear();
                }
                slot.lanes.push(Lane::new(gen, Rc::clone(p), boundary));
                slot.pending = Some((binding, boundary));
                if used {
                    let c = SlotControl {
                        slot: slot.id,
                        new_gen: gen,
                        effective_time: effective,
                        release: Release::None,
                    };
                    self.control.future(key, c, &mut self.hosts);
                    self.midi_recovery(c.slot, gen, effective);
                }
            }
        }
        Ok(())
    }

    /// `stop` (Natural) or `hush` (every slot, Panic) (11.3).
    pub(crate) fn revoke(&mut self, key: SlotKey) {
        let release = match key {
            SlotKey::All => Release::Panic,
            _ => Release::Natural,
        };
        let _ = self.revoke_with(key, release);
    }

    /// Revokes a slot or every slot using the requested voice release mode.
    pub(crate) fn revoke_with(&mut self, key: SlotKey, release: Release) -> f64 {
        let (now, _) = self.now_pos();
        let keys = match key {
            SlotKey::All => self.slots.keys(),
            k => vec![k],
        };
        for k in keys {
            let Some(slot) = self.slots.get_mut(k) else {
                continue;
            };
            let was_used = slot.gen > 0;
            slot.gen += 1;
            slot.bound = None;
            slot.pending = None;
            slot.lanes.clear();
            let (id, gen, kind, out) = (slot.id, slot.gen, slot.kind, slot.out_id());
            let ephemeral = slot.ephemeral;
            match kind {
                SlotKind::Texture => {
                    if let Some(o) = out {
                        self.hosts.render.set_program(o, empty_program());
                    }
                }
                SlotKind::Pattern if was_used => {
                    self.control.cancel_future(id);
                    let c = SlotControl {
                        slot: id,
                        new_gen: gen,
                        effective_time: now,
                        release,
                    };
                    self.control.immediate(k, c, &mut self.hosts);
                    self.midi_recovery(id, gen, now);
                }
                SlotKind::Pattern => {}
            }
            if ephemeral {
                self.slots.remove(k);
            }
        }
        now
    }

    /// The MIDI side of the always-on late-start recovery: every committed
    /// note of `slot` older than `gen` starting at or after `effective` gets
    /// an immediate note-off (a note the sink already dropped ignores it).
    pub(crate) fn midi_recovery(&mut self, slot: SlotId, gen: u32, effective: f64) {
        let now = self.hosts.audio.now();
        let mut keep = Vec::with_capacity(self.recent_midi.len());
        for e in std::mem::take(&mut self.recent_midi) {
            let MidiEvent::Note {
                time,
                slot: s,
                gen: g,
                ch,
                note,
                dur,
                ..
            } = e
            else {
                continue;
            };
            if time + dur < now {
                continue;
            }
            if s == slot && g < gen && time >= effective {
                self.hosts.midi.send(MidiEvent::NoteOff {
                    time: time.max(now),
                    slot,
                    gen: g,
                    ch,
                    note,
                });
            } else {
                keep.push(e);
            }
        }
        self.recent_midi = keep;
    }
}
