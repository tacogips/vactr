//! The committed ledger (design 11.3 layer 2): the occurrences one
//! (slot, generation) has emitted, so an occurrence emits at most once no
//! matter how often overlapping queries return it. Bounded to the query
//! horizon: an entry expires when the position passes its whole span, and
//! the ledger drops with its lane.

use std::collections::BTreeMap;

use crate::pattern::occ::OccKey;
use crate::pattern::query::TimeSpan;
use crate::value::ratio::Ratio64;

/// Emitted occurrences of one lane, with their wholes.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    emitted: BTreeMap<OccKey, TimeSpan>,
}

impl Ledger {
    /// Records an emission.
    pub fn insert(&mut self, key: OccKey, whole: TimeSpan) {
        self.emitted.insert(key, whole);
    }

    /// True when `key` was emitted.
    #[must_use]
    pub fn contains(&self, key: &OccKey) -> bool {
        self.emitted.contains_key(key)
    }

    /// Forgets every entry whose whole ended at or before `pos`.
    pub fn expire(&mut self, pos: Ratio64) {
        self.emitted.retain(|_, w| w.end > pos);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.emitted.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.emitted.is_empty()
    }
}
