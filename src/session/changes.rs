//! Editor change sets and span mapping (design 14.5.6).
//!
//! A `ChangeSet` is a sorted list of non-overlapping replacements in
//! base-revision byte offsets. `map_span` moves a stored span (a tweak site,
//! a definition, a directive) forward through the edit, or reports that the
//! edit touched it; composition applies the maps of successive change sets
//! in order. The session never guesses: a touched span is edit-invalidated.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One replacement: base bytes `from..to` become `insert_len` new bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Change {
    pub from: u32,
    pub to: u32,
    pub insert_len: u32,
}

impl Change {
    /// A pure insertion (`from == to`).
    #[must_use]
    pub const fn is_insertion(&self) -> bool {
        self.from == self.to
    }

    /// The length change this replacement causes.
    #[must_use]
    pub fn delta(&self) -> i64 {
        i64::from(self.insert_len) - (i64::from(self.to) - i64::from(self.from))
    }
}

/// Why a list of changes is not a valid `ChangeSet`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChangeError {
    /// Change `index` has `from > to`.
    Inverted { index: usize },
    /// Change `index` starts before the change preceding it.
    Unsorted { index: usize },
    /// Change `index` starts inside the range the preceding change replaces.
    Overlapping { index: usize },
}

impl fmt::Display for ChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeError::Inverted { index } => write!(f, "change {index} has from > to"),
            ChangeError::Unsorted { index } => {
                write!(f, "change {index} starts before the previous change")
            }
            ChangeError::Overlapping { index } => {
                write!(f, "change {index} overlaps the previous change")
            }
        }
    }
}

impl std::error::Error for ChangeError {}

/// Where a span went through an edit.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mapped {
    /// The span is intact at these new-revision offsets.
    Moved { start: u32, end: u32 },
    /// The edit overlapped or touched the span: it is edit-invalidated.
    Touched,
}

/// A validated, sorted, non-overlapping list of changes.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct ChangeSet {
    changes: Vec<Change>,
}

impl ChangeSet {
    /// Validates `changes`: each has `from <= to`, and each starts at or
    /// after the end of the one before it.
    ///
    /// # Errors
    /// `Inverted`, `Unsorted` or `Overlapping`, naming the offending change.
    pub fn new(changes: Vec<Change>) -> Result<ChangeSet, ChangeError> {
        for (index, c) in changes.iter().enumerate() {
            if c.from > c.to {
                return Err(ChangeError::Inverted { index });
            }
            if let Some(prev) = index.checked_sub(1).map(|k| changes[k]) {
                if c.from < prev.from {
                    return Err(ChangeError::Unsorted { index });
                }
                if c.from < prev.to {
                    return Err(ChangeError::Overlapping { index });
                }
            }
        }
        Ok(ChangeSet { changes })
    }

    /// The changes, in order.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Maps the base span `start..end` through this change set.
    ///
    /// A span entirely before a change is unchanged; a span entirely after
    /// it shifts by the change's delta. A replacement that overlaps or
    /// touches the span (shares an endpoint), or an insertion strictly
    /// inside it, makes it `Touched`.
    #[must_use]
    pub fn map_span(&self, start: u32, end: u32) -> Mapped {
        let mut delta: i64 = 0;
        for c in &self.changes {
            let before_span = if c.is_insertion() {
                c.from <= start
            } else {
                c.to < start
            };
            let after_span = if c.is_insertion() {
                c.from >= end
            } else {
                c.from > end
            };
            if before_span {
                delta += c.delta();
            } else if !after_span {
                return Mapped::Touched;
            } else {
                break;
            }
        }
        let shift = |x: u32| u32::try_from(i64::from(x) + delta).ok();
        match (shift(start), shift(end)) {
            (Some(start), Some(end)) => Mapped::Moved { start, end },
            _ => Mapped::Touched,
        }
    }

    /// This change set followed by `later` (whose offsets are in this set's
    /// NEW revision).
    #[must_use]
    pub fn compose(&self, later: &ChangeSet) -> ComposedMap {
        ComposedMap {
            sets: vec![self.clone(), later.clone()],
        }
    }
}

/// Successive change sets, applied in order.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct ComposedMap {
    sets: Vec<ChangeSet>,
}

impl ComposedMap {
    /// Appends a later change set.
    #[must_use]
    pub fn compose(mut self, later: &ChangeSet) -> ComposedMap {
        self.sets.push(later.clone());
        self
    }

    /// Maps the base span through every set in order; `Touched` as soon as
    /// any set touches it.
    #[must_use]
    pub fn map_span(&self, start: u32, end: u32) -> Mapped {
        map_through(&self.sets, start, end)
    }
}

/// Maps `start..end` through `sets` in order.
#[must_use]
pub fn map_through(sets: &[ChangeSet], start: u32, end: u32) -> Mapped {
    let mut at = Mapped::Moved { start, end };
    for set in sets {
        at = match at {
            Mapped::Moved { start, end } => set.map_span(start, end),
            Mapped::Touched => return Mapped::Touched,
        };
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(changes: &[(u32, u32, u32)]) -> ChangeSet {
        ChangeSet::new(
            changes
                .iter()
                .map(|&(from, to, insert_len)| Change {
                    from,
                    to,
                    insert_len,
                })
                .collect(),
        )
        .expect("valid change set")
    }

    #[test]
    fn insertion_above_moves() {
        let cs = set(&[(0, 0, 4)]);
        assert_eq!(cs.map_span(10, 12), Mapped::Moved { start: 14, end: 16 });
        // An insertion at the span start moves it; at its end leaves it.
        assert_eq!(
            set(&[(10, 10, 3)]).map_span(10, 12),
            Mapped::Moved { start: 13, end: 15 }
        );
        assert_eq!(
            set(&[(12, 12, 3)]).map_span(10, 12),
            Mapped::Moved { start: 10, end: 12 }
        );
    }

    #[test]
    fn edit_below_leaves_span() {
        let cs = set(&[(20, 25, 1)]);
        assert_eq!(cs.map_span(10, 12), Mapped::Moved { start: 10, end: 12 });
    }

    #[test]
    fn edit_inside_touches() {
        assert_eq!(set(&[(11, 12, 3)]).map_span(10, 14), Mapped::Touched);
        assert_eq!(set(&[(11, 11, 1)]).map_span(10, 14), Mapped::Touched);
        // A replacement sharing an endpoint touches.
        assert_eq!(set(&[(14, 16, 0)]).map_span(10, 14), Mapped::Touched);
        assert_eq!(set(&[(8, 10, 0)]).map_span(10, 14), Mapped::Touched);
    }

    #[test]
    fn deletion_spanning_touches() {
        assert_eq!(set(&[(5, 30, 0)]).map_span(10, 14), Mapped::Touched);
    }

    #[test]
    fn deletion_above_shifts_back() {
        assert_eq!(
            set(&[(0, 4, 0), (6, 6, 1)]).map_span(10, 14),
            Mapped::Moved { start: 7, end: 11 }
        );
    }

    #[test]
    fn two_insertions_composed_move_by_the_sum() {
        let first = set(&[(0, 0, 2)]);
        let second = set(&[(1, 1, 5)]);
        let map = first.compose(&second);
        assert_eq!(map.map_span(10, 12), Mapped::Moved { start: 17, end: 19 });
        assert_eq!(
            map_through(&[first, second], 10, 12),
            Mapped::Moved { start: 17, end: 19 }
        );
    }

    #[test]
    fn insertion_then_edit_of_shifted_span_touches() {
        let first = set(&[(0, 0, 2)]);
        // The span 10..12 is at 12..14 after `first`; the second edit hits it.
        let second = set(&[(12, 13, 1)]);
        assert_eq!(first.compose(&second).map_span(10, 12), Mapped::Touched);
        // The same offsets in base coordinates would have missed it.
        assert_eq!(
            second.map_span(20, 22),
            Mapped::Moved { start: 20, end: 22 }
        );
    }

    #[test]
    fn invalid_change_sets_are_rejected() {
        let c = |from, to| Change {
            from,
            to,
            insert_len: 0,
        };
        assert_eq!(
            ChangeSet::new(vec![c(5, 3)]),
            Err(ChangeError::Inverted { index: 0 })
        );
        assert_eq!(
            ChangeSet::new(vec![c(10, 12), c(2, 4)]),
            Err(ChangeError::Unsorted { index: 1 })
        );
        assert_eq!(
            ChangeSet::new(vec![c(2, 8), c(5, 9)]),
            Err(ChangeError::Overlapping { index: 1 })
        );
        assert!(ChangeSet::new(vec![c(2, 5), c(5, 9)]).is_ok());
        assert!(ChangeSet::new(Vec::new()).is_ok_and(|s| s.is_empty()));
    }
}
