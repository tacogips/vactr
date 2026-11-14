//! The slot table (design 11.2, 12.8.3).
//!
//! One table for sound and visual sinks: `d1`..`d9` are slots 1..9, a named
//! `slot :name` or `out oN` gets the next free id from 10 on. A slot holds a
//! pattern or a texture binding (MIDI and OSC are instruments selected by
//! `s`, so a pattern slot routes each event to its sink, 12.8.3). A pattern
//! slot's staged work lives in LANES, one per generation: a rebind closes
//! the current lane at the next cycle boundary and opens a new one there
//! (prospective activation, 11.3).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ns::stage::SlotKey;
use crate::pattern::build::pattern_of;
use crate::pattern::pat::Pat;
use crate::sched::staging::Lane;
use crate::tex::texnode::{OutId, TexNode};
use crate::value::intern::{intern_kw, name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::call::kind_name;
use crate::vm::fail::{FailCode, Failure};

id_newtype!(
    /// A playing slot.
    SlotId(u32)
);

id_newtype!(
    /// A control id in the POD handoff (design 11.4).
    CtlId(u16)
);

/// What a slot plays.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SlotKind {
    Pattern,
    Texture,
}

/// A bound value (design 11.2).
#[derive(Clone, Debug)]
pub enum Binding {
    Pattern(Rc<Pat>),
    Texture(Rc<TexNode>),
}

impl Binding {
    /// The binding a slot sink received.
    ///
    /// # Errors
    /// `Type` for a value that is neither a pattern-like value nor a texture.
    pub fn from_value(v: &Value) -> Result<Binding, Failure> {
        match v {
            Value::Tex(t) => Ok(Binding::Texture(Rc::clone(t))),
            Value::Pattern(_) | Value::List(_) | Value::Signal(_) | Value::Range(_) => {
                Ok(Binding::Pattern(pattern_of(v, None)?))
            }
            other => Err(Failure::new(
                FailCode::Type,
                format!(
                    "a slot plays a pattern or a texture, got {}",
                    kind_name(other)
                ),
            )),
        }
    }

    /// The slot kind this binding needs.
    #[must_use]
    pub const fn kind(&self) -> SlotKind {
        match self {
            Binding::Pattern(_) => SlotKind::Pattern,
            Binding::Texture(_) => SlotKind::Texture,
        }
    }
}

/// One slot (design 11.2).
#[derive(Debug)]
pub struct Slot {
    pub key: SlotKey,
    pub id: SlotId,
    pub kind: SlotKind,
    /// The active binding.
    pub bound: Option<Binding>,
    /// A binding waiting for its cycle boundary.
    pub pending: Option<(Binding, Ratio64)>,
    /// The newest generation (bumped on rebind, stop, hush, tempo).
    pub gen: u32,
    pub orbit: u8,
    pub muted: bool,
    /// A `once` slot: removed when its only lane ends.
    pub ephemeral: bool,
    /// Pattern lanes, oldest first; at most one per generation.
    pub(crate) lanes: Vec<Lane>,
    /// Cycles whose queries reported a fault; the slot's diagnostics clear
    /// after a later clean cycle (10.3).
    pub(crate) fault_cycles: Vec<i64>,
    pub(crate) has_faults: bool,
}

impl Slot {
    fn new(key: SlotKey, id: SlotId) -> Self {
        Self {
            key,
            id,
            kind: SlotKind::Pattern,
            bound: None,
            pending: None,
            gen: 0,
            orbit: 0,
            muted: false,
            ephemeral: false,
            lanes: Vec::new(),
            fault_cycles: Vec::new(),
            has_faults: false,
        }
    }

    /// The slot's name as a keyword (`d1`, `drums`, `o0`): fault origins
    /// and telemetry carry it.
    #[must_use]
    pub fn name(&self) -> KwId {
        match self.key {
            SlotKey::Named(k) => k,
            other => intern_kw(&other.name()),
        }
    }

    /// The visual output of a texture slot (`o0`..`o3`).
    #[must_use]
    pub fn out_id(&self) -> Option<OutId> {
        let SlotKey::Named(k) = self.key else {
            return None;
        };
        let name = name_of_kw(k);
        let n = name.strip_prefix('o')?.parse::<u32>().ok()?;
        (n <= 3).then_some(OutId::new(n))
    }

    /// True when the slot plays nothing and has nothing pending.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.bound.is_none() && self.pending.is_none() && self.lanes.is_empty()
    }

    /// The lanes (read-only, tests and telemetry).
    #[must_use]
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }
}

/// Every slot, by key (design 11.2).
#[derive(Debug, Default)]
pub struct SlotTable {
    slots: Vec<Slot>,
    by_key: BTreeMap<SlotKey, usize>,
    next_named: u32,
}

impl SlotTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The slot for `key`, if it exists.
    #[must_use]
    pub fn get(&self, key: SlotKey) -> Option<&Slot> {
        self.by_key.get(&key).map(|&i| &self.slots[i])
    }

    /// The slot for `key`, mutable.
    pub fn get_mut(&mut self, key: SlotKey) -> Option<&mut Slot> {
        self.by_key.get(&key).map(|&i| &mut self.slots[i])
    }

    /// The slot for `key`, created on first use. `SlotKey::All` has no
    /// slot of its own; it maps to the table-wide id 0.
    pub fn get_or_insert(&mut self, key: SlotKey) -> &mut Slot {
        let i = match self.by_key.get(&key) {
            Some(&i) => i,
            None => {
                let id = match key {
                    SlotKey::D(n) => u32::from(n),
                    SlotKey::All => 0,
                    SlotKey::Named(_) => {
                        self.next_named += 1;
                        9 + self.next_named
                    }
                };
                self.slots.push(Slot::new(key, SlotId::new(id)));
                self.by_key.insert(key, self.slots.len() - 1);
                self.slots.len() - 1
            }
        };
        &mut self.slots[i]
    }

    /// Every slot, in creation order.
    pub fn iter(&self) -> impl Iterator<Item = &Slot> + '_ {
        self.slots.iter()
    }

    /// Every slot, mutable.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Slot> + '_ {
        self.slots.iter_mut()
    }

    /// The keys of every slot.
    #[must_use]
    pub fn keys(&self) -> Vec<SlotKey> {
        self.slots.iter().map(|s| s.key).collect()
    }

    /// Removes a slot (an ended `once`).
    pub fn remove(&mut self, key: SlotKey) {
        if let Some(i) = self.by_key.remove(&key) {
            self.slots.remove(i);
            for v in self.by_key.values_mut() {
                if *v > i {
                    *v -= 1;
                }
            }
        }
    }

    /// Unbinds every slot (the table side of `hush`; the runtime sends the
    /// controls and bumps the generations).
    pub fn hush_all(&mut self) {
        for s in &mut self.slots {
            s.bound = None;
            s.pending = None;
            s.lanes.clear();
        }
    }
}
