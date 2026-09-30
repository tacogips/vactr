//! MIDI clock slave and master, and the transport (design 11.1, 11.7,
//! 12.8.4, 12.8.12 "MIDI wiring").
//!
//! `use-clock :midi` slaves the clock to incoming MIDI clock over
//! `clock::MidiClockSync`: every 24-ppq pulse re-anchors the piecewise-linear
//! clock at an EXACT logical position; only the host-seconds anchor follows
//! the smoothed pulse period, so jitter never moves a `Ratio64` position.
//! `Start` restarts every binding at cycle 0, `Stop` freezes the scheduler
//! (staging cleared, `SlotControl` Natural to all sinks, open input notes
//! closed), `Continue` resumes from the frozen position. No pulse within the
//! timeout freewheels at the last smoothed tempo with `clock-lost`; the next
//! pulse re-syncs from the freewheeled position. `use-bpm` under `:midi` is
//! `clock-external` (the clock refuses the change).
//!
//! `midi-clock-out true` makes the clock a master: `Start`, 24-ppq `Clock`
//! and `Stop`/`Continue` go through `MidiHost` from the clock's logical
//! positions with the commit-horizon discipline of note events.

use std::rc::Rc;

use crate::clock::clock::{Clock, ClockSource, MidiClockSync, PULSES_PER_BEAT};
use crate::host::caps::MidiEvent;
use crate::host::wire::{Release, SlotControl};
use crate::ns::evaluator::Evaluator;
use crate::ns::stage::SlotKey;
use crate::sched::ledger::Ledger;
use crate::sched::midi_in::MidiIn;
use crate::sched::runtime::{ceil, grid, nowhere, Runtime, TickReport};
use crate::sched::slots::{Binding, SlotKind};
use crate::sched::staging::{Lane, Staging};
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::{name_of_kw, KwId};
use crate::value::ratio::Ratio64;

/// The most clock pulses one tick emits (a guard against a stalled host
/// clock, not a limit any tempo reaches).
const MAX_PULSES_PER_TICK: usize = 4096;

/// Clock-master output state.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Master {
    /// The next pulse position; `None` until the first one is chosen.
    next: Option<Ratio64>,
    /// `Start` was sent (with the first pulse, when it entered the commit
    /// horizon).
    started: bool,
}

/// Clock source, transport and clock-master state owned by the runtime.
#[derive(Clone, Debug, Default)]
pub struct MidiClockState {
    sync: Option<MidiClockSync>,
    /// Successful MIDI Starts, independent of clock position and polling cadence.
    restart_generation: u64,
    frozen: bool,
    /// Host time of the last pulse (or of the slave's (re)start).
    last_seen: f64,
    lost: bool,
    applied_source: Option<KwId>,
    master: Option<Master>,
}

impl MidiClockState {
    /// Changes once per successful MIDI Start, including repeated Starts at zero.
    #[must_use]
    pub fn restart_generation(&self) -> u64 {
        self.restart_generation
    }

    /// True under `use-clock :midi`.
    #[must_use]
    pub fn is_slave(&self) -> bool {
        self.sync.is_some()
    }

    /// True between a MIDI `Stop` and the next `Start`/`Continue`.
    #[must_use]
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// True while freewheeling after a clock loss.
    #[must_use]
    pub fn is_lost(&self) -> bool {
        self.lost
    }

    /// True under `midi-clock-out true`.
    #[must_use]
    pub fn is_master(&self) -> bool {
        self.master.is_some()
    }

    /// The smoothed external tempo, once two pulses arrived.
    #[must_use]
    pub fn smoothed_bpm(&self) -> Option<f64> {
        self.sync.as_ref().and_then(MidiClockSync::smoothed_bpm)
    }
}

impl Runtime {
    /// The MIDI input state (tests and the session layer).
    #[must_use]
    pub fn midi_in(&self) -> &MidiIn {
        &self.midi_in
    }

    /// The clock source and transport state.
    #[must_use]
    pub fn midi_clock(&self) -> &MidiClockState {
        &self.midi_clock
    }

    /// True while a MIDI `Stop` holds the scheduler.
    pub(crate) fn transport_frozen(&self) -> bool {
        self.midi_clock.frozen
    }

    /// A tick while the transport is stopped: the position holds, nothing is
    /// queried or committed; live input, control re-sends and cell transport
    /// still run.
    pub(crate) fn frozen_tick(
        &mut self,
        ev: &mut Evaluator,
        host_now: f64,
        mut rep: TickReport,
    ) -> TickReport {
        self.pin_clock(host_now);
        self.play_live_notes(ev, host_now, &mut rep);
        rep.diags
            .extend(self.control.tick(host_now, &mut self.hosts));
        rep.diags.extend(self.cells.tick());
        self.last_now = Some(host_now);
        rep
    }

    /// Holds the clock line at the frozen position (drain-side positions
    /// then stay there too).
    fn pin_clock(&mut self, host: f64) {
        let tempo = self.clock.tempo();
        let _ = self.clock.anchor_at(host, self.pos, tempo);
        self.clock.set_pos(self.pos);
    }

    /// Applies the stored `use-clock` and `midi-clock-out` settings.
    pub(crate) fn apply_clock_settings(&mut self) {
        if self.clock_request != self.midi_clock.applied_source {
            self.midi_clock.applied_source = self.clock_request;
            let source = self
                .clock_request
                .and_then(|k| ClockSource::from_name(&name_of_kw(k)));
            match source {
                Some(ClockSource::MidiClock) => self.enter_slave(),
                Some(ClockSource::Internal) => self.leave_slave(),
                // `:link` is the checker's diagnostic (7.1.6).
                _ => {}
            }
        }
        if self.midi_clock_out != self.midi_clock.master.is_some() {
            if self.midi_clock_out {
                self.midi_clock.master = Some(Master {
                    next: None,
                    started: false,
                });
            } else {
                let started = self.midi_clock.master.is_some_and(|m| m.started);
                self.midi_clock.master = None;
                if started {
                    let time = self.hosts.audio.now() + self.commit_lead;
                    self.hosts.midi.send(MidiEvent::Stop { time });
                }
            }
        }
    }

    /// Replaces the clock with one of `source` continuing exactly at the
    /// current position from host time `host`.
    fn rebuild_clock(&mut self, source: ClockSource, host: f64) {
        let pos = self.pos;
        if let Ok(mut c) = Clock::new(source, self.clock.tempo(), host) {
            if c.reset(host, pos).is_ok() {
                c.set_pos(pos);
                self.clock = c;
            }
        }
    }

    fn enter_slave(&mut self) {
        if self.clock.source() == ClockSource::MidiClock {
            return;
        }
        let now = self.hosts.audio.now();
        let host = self.clock.to_host(self.pos);
        self.rebuild_clock(ClockSource::MidiClock, host);
        let mut sync = MidiClockSync::new(self.cfg.clock_smoothing);
        sync.resume(&self.clock);
        let st = &mut self.midi_clock;
        st.sync = Some(sync);
        st.last_seen = now;
        st.lost = false;
        st.frozen = false;
    }

    fn leave_slave(&mut self) {
        if self.clock.source() == ClockSource::Internal {
            return;
        }
        let now = self.hosts.audio.now();
        let frozen = self.midi_clock.frozen;
        let host = if frozen {
            now
        } else {
            self.clock.to_host(self.pos)
        };
        self.rebuild_clock(ClockSource::Internal, host);
        let st = &mut self.midi_clock;
        st.sync = None;
        st.lost = false;
        st.frozen = false;
        if frozen {
            self.master_resume(now, false);
        }
    }

    /// One MIDI clock pulse at host time `time`.
    pub(crate) fn clock_pulse(&mut self, time: f64) {
        let resync = if self.midi_clock.lost {
            self.pulse_before(time)
        } else {
            None
        };
        let st = &mut self.midi_clock;
        let Some(sync) = st.sync.as_mut() else {
            return;
        };
        if st.frozen || !sync.is_running() {
            return;
        }
        if st.lost {
            // Re-sync from the freewheeled position: this pulse lands on the
            // pulse grid point nearest to where the freewheel put it.
            if let Some(p) = resync {
                self.clock.set_pos(p);
            }
            sync.resume(&self.clock);
            st.lost = false;
        }
        let _ = sync.pulse(&mut self.clock, time);
        st.last_seen = st.last_seen.max(time);
    }

    /// No pulse within the timeout: freewheel (the last anchor keeps the
    /// last smoothed tempo) and warn once per loss.
    pub(crate) fn check_clock_lost(&mut self, now: f64, diags: &mut Vec<Diagnostic>) {
        let timeout = self.cfg.midi_clock_timeout;
        let st = &mut self.midi_clock;
        let running = st.sync.as_ref().is_some_and(MidiClockSync::is_running);
        if !running || st.frozen || st.lost || now - st.last_seen <= timeout {
            return;
        }
        st.lost = true;
        let bpm = self.clock.tempo().bpm.to_f64();
        diags.push(Diagnostic {
            severity: DiagCode::ClockLost.default_severity(),
            ..Diagnostic::error(
                DiagCode::ClockLost,
                nowhere(),
                format!(
                    "no MIDI clock for {:.0} ms: freewheeling at {bpm:.1} bpm",
                    timeout * 1000.0
                ),
            )
        })
    }

    /// MIDI `Start`: every binding restarts at cycle 0 from `now`.
    pub(crate) fn transport_start(&mut self, now: f64) {
        let Some(sync) = self.midi_clock.sync.as_mut() else {
            return;
        };
        if sync.start(&mut self.clock, now).is_err() {
            return;
        }
        self.midi_clock.restart_generation = self.midi_clock.restart_generation.wrapping_add(1);
        self.pos = Ratio64::ZERO;
        self.clock.set_pos(Ratio64::ZERO);
        for k in self.slots.keys() {
            let Some(slot) = self.slots.get_mut(k) else {
                continue;
            };
            if slot.kind != SlotKind::Pattern || slot.ephemeral {
                continue;
            }
            let b = match slot.pending.take() {
                Some((b, _)) => Some(b),
                None => slot.bound.take(),
            };
            let Some(Binding::Pattern(p)) = &b else {
                slot.bound = b;
                continue;
            };
            let used = slot.gen > 0;
            slot.gen += 1;
            let (id, gen) = (slot.id, slot.gen);
            slot.lanes = vec![Lane::new(gen, Rc::clone(p), Ratio64::ZERO)];
            slot.bound = b;
            if used {
                self.control.cancel_future(id);
                let c = SlotControl {
                    slot: id,
                    new_gen: gen,
                    effective_time: now,
                    release: Release::None,
                };
                self.control.immediate(k, c, &mut self.hosts);
                self.midi_recovery(id, gen, now);
            }
        }
        let st = &mut self.midi_clock;
        st.frozen = false;
        st.lost = false;
        st.last_seen = now;
        if let Some(m) = st.master.as_mut() {
            // `Start` goes out with the pulse at cycle 0.
            m.next = Some(Ratio64::ZERO);
            m.started = false;
        }
    }

    /// MIDI `Stop`: the scheduler freezes at its position; staging is
    /// cleared and every pattern slot gets `Natural` (open input voices
    /// enter their release stage).
    pub(crate) fn transport_stop(&mut self, now: f64) {
        let Some(sync) = self.midi_clock.sync.as_mut() else {
            return;
        };
        if self.midi_clock.frozen {
            return;
        }
        sync.stop();
        let pos = self.pos;
        for k in self.slots.keys() {
            let Some(slot) = self.slots.get_mut(k) else {
                continue;
            };
            if slot.kind != SlotKind::Pattern || slot.gen == 0 {
                continue;
            }
            slot.gen += 1;
            let (id, gen) = (slot.id, slot.gen);
            for l in &mut slot.lanes {
                l.gen = gen;
                l.staging = Staging::default();
                l.ledger = Ledger::default();
                l.dirty.clear();
                l.queried_to = l.from.max(pos);
            }
            let c = SlotControl {
                slot: id,
                new_gen: gen,
                effective_time: now,
                release: Release::Natural,
            };
            self.control.immediate(k, c, &mut self.hosts);
            self.midi_recovery(id, gen, now);
            self.control.restamp_future(id, gen, &mut self.hosts);
        }
        self.close_live_notes(SlotKey::All, now);
        self.midi_clock.frozen = true;
        self.pin_clock(now);
        if self.midi_clock.master.is_some_and(|m| m.started) {
            self.hosts.midi.send(MidiEvent::Stop { time: now });
        }
    }

    /// MIDI `Continue`: resumes from the frozen position.
    pub(crate) fn transport_continue(&mut self, now: f64) {
        if !self.midi_clock.frozen || self.midi_clock.sync.is_none() {
            return;
        }
        self.pin_clock(now);
        if let Some(sync) = self.midi_clock.sync.as_mut() {
            sync.resume(&self.clock);
        }
        let st = &mut self.midi_clock;
        st.frozen = false;
        st.lost = false;
        st.last_seen = now;
        self.master_resume(now, true);
    }

    /// The master's `Continue` after a freeze, pulses from the position on.
    fn master_resume(&mut self, now: f64, send: bool) {
        let next = self.pulse_ceil(self.pos);
        if let Some(m) = self.midi_clock.master.as_mut() {
            if m.started {
                m.next = Some(next);
                if send {
                    self.hosts.midi.send(MidiEvent::Continue { time: now });
                }
            }
        }
    }

    /// Pulses per cycle: 24 per beat.
    fn pulses_per_cycle(&self) -> Option<Ratio64> {
        self.clock
            .tempo()
            .beats_per_cycle
            .checked_mul(Ratio64::from_int(PULSES_PER_BEAT))
            .ok()
    }

    /// The pulse grid point one pulse before the one nearest to the clock's
    /// position at host time `time`.
    fn pulse_before(&self, time: f64) -> Option<Ratio64> {
        let per = self.pulses_per_cycle()?;
        let x = (self.clock.to_cycles(time) * per.to_f64()).round();
        if !x.is_finite() || x.abs() > 9.0e15 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        let n = x as i64;
        Ratio64::from_int(n.saturating_sub(1)).checked_div(per).ok()
    }

    /// The first pulse position at or after `pos`.
    fn pulse_ceil(&self, pos: Ratio64) -> Ratio64 {
        self.pulses_per_cycle()
            .and_then(|per| {
                let n = ceil(pos.checked_mul(per).ok()?);
                n.checked_div(per).ok()
            })
            .unwrap_or(pos)
    }

    /// Clock-master output (after the tick's commit): every pulse whose host
    /// time enters the commit horizon, the first one preceded by `Start`.
    pub(crate) fn emit_midi_clock(&mut self, now: f64) {
        let Some(mut m) = self.midi_clock.master else {
            return;
        };
        let Some(per) = self.pulses_per_cycle() else {
            return;
        };
        let Ok(step) = Ratio64::ONE.checked_div(per) else {
            return;
        };
        let horizon = now + self.commit_lead;
        let mut next = match m.next {
            Some(n) => n,
            None => {
                let at = grid(self.clock.to_cycles(horizon), true).max(self.pos);
                self.pulse_ceil(at)
            }
        };
        for _ in 0..MAX_PULSES_PER_TICK {
            let time = self.clock.to_host(next);
            if time > horizon {
                break;
            }
            if !m.started {
                self.hosts.midi.send(MidiEvent::Start { time });
                m.started = true;
            }
            self.hosts.midi.send(MidiEvent::Clock { time });
            match next.checked_add(step) {
                Ok(n) => next = n,
                Err(_) => break,
            }
        }
        m.next = Some(next);
        self.midi_clock.master = Some(m);
    }
}
