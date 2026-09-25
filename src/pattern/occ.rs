//! Occurrence identity (design 10.2). `occ` is consumed only by the
//! scheduler's emission-uniqueness merge (11.3).

use crate::reader::span::NodeId;
use crate::value::ratio::Ratio64;

/// The stable logical identity of an event. `path` holds one
/// `(node id, ordinal)` entry per structure-introducing node the event
/// passed through; `anchor` is `whole.begin` (`part.begin` when `whole` is
/// `None`) and `cycle` its enclosing cycle. The same note returned by two
/// query windows has an equal key; identical twins from two stack branches
/// differ by their branch ordinal.
///
/// The design sketch uses a `SmallVec`; this crate adds no dependency, so
/// the path is a `Vec`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct OccKey {
    pub path: Vec<(NodeId, u32)>,
    pub anchor: Ratio64,
    pub cycle: i64,
}

impl OccKey {
    /// An empty key; `query` fills `anchor` and `cycle` at the end.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            path: Vec::new(),
            anchor: Ratio64::ZERO,
            cycle: 0,
        }
    }

    /// Appends one structural ordinal.
    pub fn push(&mut self, node: NodeId, ordinal: u32) {
        self.path.push((node, ordinal));
    }

    /// Sets the anchor and its cycle.
    pub fn anchor_at(&mut self, anchor: Ratio64) {
        self.anchor = anchor;
        self.cycle = anchor.floor();
    }
}

impl Default for OccKey {
    fn default() -> Self {
        Self::empty()
    }
}
