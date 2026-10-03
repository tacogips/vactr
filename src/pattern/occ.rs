//! Occurrence identity (design 10.2). `occ` is consumed only by the
//! scheduler's emission-uniqueness merge (11.3).

use crate::reader::span::NodeId;
use crate::value::ratio::Ratio64;

/// An explicit edge in a producer tree. These tags are separate from NodeId
/// hashes and from positions in a returned or filtered event vector.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ProducerKind {
    /// A statically named child edge of a pattern node.
    Child,
    /// An element of a source step list, including nested lists.
    NestedStep,
    /// A structure-generating replication or simultaneous branch.
    GeneratedBranch,
    /// A dynamically resolved source or transform result.
    DynamicExpansion,
    /// A timing-source section when a control supplies structure.
    TimingSource,
    /// A content-source section when a control supplies structure.
    ContentSource,
}

/// One collision-independent producer edge and its source ordinal.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ProducerStep {
    pub kind: ProducerKind,
    pub ordinal: u32,
}

/// Full source traversal identity, populated only by traced queries.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ProducerTrace {
    pub steps: Vec<ProducerStep>,
}

impl ProducerTrace {
    /// Appends an explicit source edge without compacting or hashing it.
    pub fn push(&mut self, kind: ProducerKind, ordinal: u32) {
        self.steps.push(ProducerStep { kind, ordinal });
    }

    /// Losslessly encodes typed edges as fixed tag/ordinal pairs for handles.
    #[must_use]
    pub fn ordinals(&self) -> Vec<u32> {
        self.steps
            .iter()
            .flat_map(|step| {
                let tag = match step.kind {
                    ProducerKind::Child => 0,
                    ProducerKind::NestedStep => 1,
                    ProducerKind::GeneratedBranch => 2,
                    ProducerKind::DynamicExpansion => 3,
                    ProducerKind::TimingSource => 4,
                    ProducerKind::ContentSource => 5,
                };
                [tag, step.ordinal]
            })
            .collect()
    }
}

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
