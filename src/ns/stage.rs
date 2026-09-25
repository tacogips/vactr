//! Staged effects (design 7.1.3 "Staged effects", 10.4).
//!
//! Every host-visible effect is a `StagedEffect`. A form collects its
//! effects in an `EffectBuffer`; they reach an `EffectSink` only when the
//! whole form succeeded, and are dropped when it failed (the bind-then-fail
//! case). The only sink in this issue is the test `RecordingSink`; TASK-007
//! adds the slot-table sink. The reactive layer builds pass-level staging on
//! top of this buffer.

use std::rc::Rc;

use crate::host::caps::{GraphHandle, TapSrc};
use crate::ns::namespace::VarSlotRef;
use crate::ns::tweak::TweakId;
use crate::reader::span::Span;
use crate::value::intern::{name_of_kw, KwId, SymId};
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::value::value::Value;

/// A playing slot: `d1`..`d9`, a named `slot :name`, or every slot (`hush`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SlotKey {
    D(u8),
    Named(KwId),
    All,
}

impl SlotKey {
    /// The slot's display name (`d1`, `drums`, `*`).
    #[must_use]
    pub fn name(self) -> String {
        match self {
            SlotKey::D(n) => format!("d{n}"),
            SlotKey::Named(k) => name_of_kw(k).to_string(),
            SlotKey::All => "*".to_string(),
        }
    }
}

/// A tempo or clock change.
#[derive(Clone, Debug)]
pub enum TempoChange {
    Bpm(Value),
    Cycle(Value),
    Clock(KwId),
    MidiClockOut(bool),
}

/// One host-visible effect.
#[derive(Clone, Debug)]
pub enum StagedEffect {
    /// Bind a pattern or texture (any value in this wave) to a slot.
    SlotBind {
        slot: SlotKey,
        value: Value,
    },
    /// Stop a slot (`stop :name`, `hush` is `Revoke(All)`).
    Revoke(SlotKey),
    /// A control-cell update for a late-bound slot (11.3).
    CellUpdate {
        slot: VarSlotRef,
        value: Value,
    },
    /// A tweak site's value changed.
    TweakRefresh(TweakId),
    /// The display `bindings` batch: names and their new values.
    Bindings(Vec<(SymId, Value)>),
    Tempo(TempoChange),
    /// `once` (with `at:` and control overrides) or `at t body`.
    OneShot {
        at: Option<Ratio64>,
        value: Value,
        overrides: Vec<(KwId, Value)>,
    },
    /// A console line from `print`.
    Console(Rc<str>),
    /// Install a graph on the audio side: released by an `inst`, `bus` or
    /// `master` definition. BE-INST stages it and BE-SCHED applies it
    /// (12.8.6).
    Install(GraphHandle),
    /// `capture :src cycles`: record the next `cycles` cycles of `src` into
    /// the `Pending` buffer `buf`, from the next cycle boundary (14.5.9).
    /// SS-ANALYSIS's runtime handling consumes it.
    Capture {
        buf: Rc<SampleBuf>,
        src: TapSrc,
        cycles: Ratio64,
        origin: Span,
    },
    /// `render cycles`: render `cycles` cycles offline into the `Pending`
    /// buffer `buf` (14.5.9). SS-ANALYSIS's runtime handling consumes it.
    Render {
        buf: Rc<SampleBuf>,
        cycles: Ratio64,
        origin: Span,
    },
}

/// Receives released effects in order.
pub trait EffectSink {
    fn apply(&mut self, effect: StagedEffect);
}

/// A sink that records every effect in order (tests).
#[derive(Debug, Default)]
pub struct RecordingSink {
    pub effects: Vec<StagedEffect>,
}

impl EffectSink for RecordingSink {
    fn apply(&mut self, effect: StagedEffect) {
        self.effects.push(effect);
    }
}

impl RecordingSink {
    /// The console lines, in order.
    #[must_use]
    pub fn console(&self) -> Vec<String> {
        self.effects
            .iter()
            .filter_map(|e| match e {
                StagedEffect::Console(s) => Some(s.to_string()),
                _ => None,
            })
            .collect()
    }
}

/// The effects one form (or one pass) staged, in order.
#[derive(Clone, Debug, Default)]
pub struct EffectBuffer {
    items: Vec<StagedEffect>,
}

impl EffectBuffer {
    /// Stages an effect.
    pub fn push(&mut self, effect: StagedEffect) {
        self.items.push(effect);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Drops the effects staged after the first `len` (a failed form).
    pub fn truncate(&mut self, len: usize) {
        self.items.truncate(len);
    }

    /// The staged effects.
    #[must_use]
    pub fn items(&self) -> &[StagedEffect] {
        &self.items
    }

    /// Releases every effect to `sink`, in order, and empties the buffer.
    /// Call only when the whole form succeeded.
    pub fn release(&mut self, sink: &mut dyn EffectSink) {
        for effect in self.items.drain(..) {
            sink.apply(effect);
        }
    }

    /// Drops every effect (the form failed).
    pub fn drop_all(&mut self) {
        self.items.clear();
    }

    /// Takes the effects, leaving the buffer empty.
    #[must_use]
    pub fn take(&mut self) -> Vec<StagedEffect> {
        std::mem::take(&mut self.items)
    }
}
