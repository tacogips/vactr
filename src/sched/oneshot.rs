//! `once` and `at` (design 11.2, design-music section 1).
//!
//! `once v at: t` plays exactly one iteration (one cycle) of `v` on an
//! ephemeral slot, starting at the commit horizon ("now") or `t` beats from
//! now; `at t: block` runs the block on the evaluator thread when the clock
//! reaches `t` beats from now, in Normal effect mode, and whatever it stages
//! (binds, `once`, `print`) is applied like any released effect.

use crate::clock::tempo::Tempo;
use crate::host::wire::{Release, SlotControl};
use crate::ns::evaluator::Evaluator;
use crate::ns::stage::StagedEffect;
use crate::ns::stage::{SlotKey, TempoChange};
use crate::pattern::eval::num_ratio;
use crate::sched::runtime::{grid, nowhere, Runtime};
use crate::sched::slots::{Binding, SlotKind};
use crate::sched::staging::Lane;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::{intern_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::fail::Failure;
use crate::vm::vm::EffectMode;
use std::rc::Rc;

/// Scheduled `at` thunks, by due position (cycles).
#[derive(Clone, Debug, Default)]
pub struct AtQueue {
    items: Vec<(Ratio64, u64, Value)>,
    seq: u64,
}

impl AtQueue {
    /// Schedules `body` at cycle position `due`.
    pub fn push(&mut self, due: Ratio64, body: Value) {
        self.seq += 1;
        self.items.push((due, self.seq, body));
    }

    /// Removes and returns the thunks due at or before `pos`, in due order
    /// (ties in scheduling order).
    pub fn take_due(&mut self, pos: Ratio64) -> Vec<Value> {
        let mut due: Vec<(Ratio64, u64, Value)> = Vec::new();
        self.items.retain(|it| {
            if it.0 <= pos {
                due.push(it.clone());
                false
            } else {
                true
            }
        });
        due.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        due.into_iter().map(|(_, _, v)| v).collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Drops every scheduled thunk (`hush`).
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// True for a value `at` runs rather than `once` plays.
#[must_use]
pub fn is_thunk(v: &Value) -> bool {
    matches!(v, Value::Fn(_) | Value::Thunk(_) | Value::Native(_))
}

/// Runs an `at` body in Normal effect mode and returns what it staged. On
/// failure the staged effects are dropped (the whole-form rule).
///
/// # Errors
/// The body's failure.
pub fn run_thunk(ev: &mut Evaluator, body: &Value) -> Result<Vec<StagedEffect>, Failure> {
    let (vm, ns) = ev.vm_and_ns();
    let mark = vm.effects().len();
    let r = vm.with_effect_mode(EffectMode::Normal, |vm| {
        vm.call_value(ns, body, Vec::new(), Vec::new())
    });
    let staged: Vec<StagedEffect> = vm.effects_mut().take();
    let (keep, mine) = staged.split_at(mark.min(staged.len()));
    let mine = mine.to_vec();
    for e in keep {
        vm.effects_mut().push(e.clone());
    }
    r.map(|_| mine)
}

impl Runtime {
    /// `use-bpm` / `use-cycle`: re-anchor, bump every generation effective
    /// now, clear staging and re-query from the current position (11.3).
    pub(crate) fn tempo(
        &mut self,
        t: TempoChange,
        diags: &mut Vec<Diagnostic>,
        faults: &mut Vec<Failure>,
    ) {
        if self.owns_song_resources() && !matches!(t, TempoChange::MidiClockOut(_)) {
            faults.push(Failure::new(
                FailCode::BeyondCapability,
                "live tempo or clock changes are unavailable while a finite song owns resources",
            ));
            return;
        }
        let (value, bpm) = match t {
            TempoChange::Bpm(v) => (v, true),
            TempoChange::Cycle(v) => (v, false),
            TempoChange::Clock(k) => {
                self.clock_request = Some(k);
                return;
            }
            TempoChange::MidiClockOut(on) => {
                self.midi_clock_out = on;
                return;
            }
        };
        let old = self.clock.tempo();
        let tempo = num_ratio(&value)
            .ok_or_else(|| Failure::new(FailCode::Type, "a tempo is a number"))
            .and_then(|r| {
                if bpm {
                    Tempo::new(r, old.beats_per_cycle)
                } else {
                    Tempo::new(old.bpm, r)
                }
            });
        let tempo = match tempo {
            Ok(t) => t,
            Err(f) => {
                faults.push(f);
                return;
            }
        };
        let (now, pos) = self.now_pos();
        if self.clock.set_tempo(tempo, pos).is_err() {
            diags.push(Diagnostic::error(
                DiagCode::ClockExternal,
                nowhere(),
                "the clock follows an external source: `use-bpm` has no effect",
            ));
            return;
        }
        let from = grid(self.clock.to_cycles(now), true).max(pos);
        let keys = self.slots.keys();
        for k in keys {
            let Some(slot) = self.slots.get_mut(k) else {
                continue;
            };
            // Every pattern slot that ever played (a hushed one too, so a
            // tempo change after a hush rides the merged Panic entry).
            if slot.kind != SlotKind::Pattern || slot.gen == 0 || slot.ephemeral {
                continue;
            }
            slot.gen += 1;
            let (id, gen) = (slot.id, slot.gen);
            for l in &mut slot.lanes {
                l.gen = gen;
                l.staging = crate::sched::staging::Staging::default();
                l.ledger = crate::sched::ledger::Ledger::default();
                l.dirty.clear();
                l.queried_to = l.from.max(from);
            }
            let c = SlotControl {
                slot: id,
                new_gen: gen,
                effective_time: now,
                release: Release::None,
            };
            self.control.immediate(k, c, &mut self.hosts);
            self.midi_recovery(id, gen, now);
            self.control.restamp_future(id, gen, &mut self.hosts);
        }
    }

    /// `once` (an ephemeral one-cycle slot) or `at` (a scheduled thunk).
    pub(crate) fn one_shot(
        &mut self,
        at: Option<Ratio64>,
        value: Value,
        overrides: Vec<(KwId, Value)>,
        faults: &mut Vec<Failure>,
    ) {
        if self.owns_song_resources() {
            faults.push(Failure::new(
                FailCode::BeyondCapability,
                "legacy one-shot scheduling is unavailable while a finite song owns resources",
            ));
            return;
        }
        let (now, pos) = self.now_pos();
        let bpc = self.clock.tempo().beats_per_cycle;
        let offset = at.and_then(|b| b.checked_div(bpc).ok());
        let horizon = grid(self.clock.to_cycles(now + self.commit_lead), true).max(pos);
        let start = match offset.and_then(|o| pos.checked_add(o).ok()) {
            Some(s) => s.max(horizon),
            None => horizon,
        };
        if is_thunk(&value) {
            self.at.push(start, value);
            return;
        }
        let pat = match Binding::from_value(&value) {
            Ok(Binding::Pattern(p)) => p,
            Ok(Binding::Texture(_)) => {
                faults.push(Failure::new(FailCode::Type, "`once` plays a pattern"));
                return;
            }
            Err(f) => {
                faults.push(f);
                return;
            }
        };
        self.once_seq += 1;
        let key = SlotKey::Named(intern_kw(&format!("once#{}", self.once_seq)));
        let slot = self.slots.get_or_insert(key);
        slot.ephemeral = true;
        slot.gen = 1;
        let mut lane = Lane::new(1, Rc::clone(&pat), start);
        lane.until = start.checked_add(Ratio64::ONE).ok();
        lane.offset = start;
        lane.overrides = overrides;
        slot.lanes.push(lane);
        slot.bound = Some(Binding::Pattern(pat));
    }
}
