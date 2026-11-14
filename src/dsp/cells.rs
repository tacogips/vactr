//! Control cells on the audio side (design 11.3, 12.8.5).
//!
//! A late-bound control reaches a voice as `Ctl::Cell(CellId)`, and the voice
//! reads the cell through `CellRead`. The transport differs per tier:
//!
//! - native: `AtomicCells`, shared process memory (`f32` bit patterns,
//!   release store / acquire load), so no cell message is ever sent;
//! - browser: the worklet's `Mirror`, an isolated table maintained by the
//!   ordered `CellInit` / `CellBatch` / `CellRetire` protocol of 11.3.
//!
//! Both are fixed-capacity: nothing here allocates after construction.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::host::wire::HostMsg;

id_newtype!(
    /// A control cell (design 11.3, 11.4).
    CellId(u32)
);

impl CellId {
    /// The cell's index in a fixed-capacity table.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Reads a cell's current value on the audio side.
pub trait CellRead {
    /// The cell's value; `0.0` for a cell that is out of range or not live.
    fn get(&self, cell: CellId) -> f32;
}

/// The native cell table: atomic `f32` bit patterns in shared memory.
///
/// Cloning shares the table; the evaluator writes and the audio callback
/// reads the same cells.
#[derive(Clone, Debug)]
pub struct AtomicCells {
    cells: Arc<[AtomicU32]>,
}

impl AtomicCells {
    /// A table of `capacity` cells, all `0.0`.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            cells: (0..capacity).map(|_| AtomicU32::new(0)).collect(),
        }
    }

    /// The number of cells.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cells.len()
    }

    /// Writes a cell (release store). Returns `false` for an id out of
    /// range, which writes nothing.
    pub fn set(&self, cell: CellId, value: f32) -> bool {
        match self.cells.get(cell.index()) {
            Some(slot) => {
                slot.store(value.to_bits(), Ordering::Release);
                true
            }
            None => false,
        }
    }
}

impl CellRead for AtomicCells {
    fn get(&self, cell: CellId) -> f32 {
        self.cells
            .get(cell.index())
            .map_or(0.0, |slot| f32::from_bits(slot.load(Ordering::Acquire)))
    }
}

/// One mirror cell's incarnation state (design 11.3).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CellState {
    Vacant,
    Live { epoch: u32, value: f32 },
    Retiring { epoch: u32 },
}

#[derive(Clone, Copy, Debug)]
struct MirrorCell {
    state: CellState,
    /// The highest epoch ever seen for this id, surviving `Vacant`. Only a
    /// `CellInit` advances it, so it is also the highest INITIALIZED epoch.
    /// Epochs start at 1; 0 means "never initialized".
    last_epoch: u32,
}

/// The browser worklet's cell table (design 11.3 "Incarnation state
/// machine"). Only `CellInit` with a greater epoch changes an incarnation;
/// every other message is checked against the current `Live` epoch.
#[derive(Clone, Debug)]
pub struct Mirror {
    cells: Box<[MirrorCell]>,
    /// The last applied `CellBatch` sequence number.
    last_seq: Option<u32>,
}

impl Mirror {
    /// A mirror of `capacity` vacant cells.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let vacant = MirrorCell {
            state: CellState::Vacant,
            last_epoch: 0,
        };
        Self {
            cells: vec![vacant; capacity].into_boxed_slice(),
            last_seq: None,
        }
    }

    /// The number of cells.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cells.len()
    }

    /// A cell's state; `None` for an id out of range.
    #[must_use]
    pub fn state(&self, cell: CellId) -> Option<CellState> {
        self.cells.get(cell.index()).map(|c| c.state)
    }

    /// The highest epoch initialized for a cell (0 when none).
    #[must_use]
    pub fn last_epoch(&self, cell: CellId) -> u32 {
        self.cells.get(cell.index()).map_or(0, |c| c.last_epoch)
    }

    /// The last applied batch sequence number.
    #[must_use]
    pub fn last_seq(&self) -> Option<u32> {
        self.last_seq
    }

    /// Applies `CellInit { cell, epoch, value }`.
    ///
    /// An epoch above `last_epoch` makes the cell `Live` with `value` (from
    /// `Vacant`, or superseding `Live`/`Retiring`). Any other epoch is a
    /// replay or a stale init: it writes nothing and is only acknowledged
    /// (init-once per epoch). An id out of range is not acknowledged, so
    /// the sender keeps re-sending and reports `host-transport`.
    pub fn apply_init(&mut self, cell: CellId, epoch: u32, value: f32) -> Option<HostMsg> {
        let slot = self.cells.get_mut(cell.index())?;
        if epoch > slot.last_epoch {
            slot.state = CellState::Live { epoch, value };
            slot.last_epoch = epoch;
        }
        Some(HostMsg::CellInitAck { cell, epoch })
    }

    /// Applies a `CellBatch { seq }` with its `(cell, epoch, value)` entries.
    ///
    /// A batch applies only when `seq` is greater than the last applied one,
    /// and each entry only on a `Live` cell with exactly its epoch (a stale
    /// update is inert). A re-sent batch with the last applied `seq` is
    /// acknowledged again without applying; an older `seq` is rejected
    /// with no acknowledgment.
    pub fn apply_batch(
        &mut self,
        seq: u32,
        entries: impl IntoIterator<Item = (CellId, u32, f32)>,
    ) -> Option<HostMsg> {
        match self.last_seq {
            Some(last) if seq == last => return Some(HostMsg::CellBatchAck { seq }),
            Some(last) if seq < last => return None,
            _ => {}
        }
        for (cell, epoch, value) in entries {
            if let Some(slot) = self.cells.get_mut(cell.index()) {
                if let CellState::Live { epoch: live, .. } = slot.state {
                    if live == epoch {
                        slot.state = CellState::Live { epoch, value };
                    }
                }
            }
        }
        self.last_seq = Some(seq);
        Some(HostMsg::CellBatchAck { seq })
    }

    /// Applies `CellRetire { cell, epoch }`: a `Live` cell with exactly that
    /// epoch becomes `Retiring`. Returns whether it did.
    pub fn retire(&mut self, cell: CellId, epoch: u32) -> bool {
        let Some(slot) = self.cells.get_mut(cell.index()) else {
            return false;
        };
        match slot.state {
            CellState::Live { epoch: live, .. } if live == epoch => {
                slot.state = CellState::Retiring { epoch };
                true
            }
            _ => false,
        }
    }

    /// The audio side has released every use of a `Retiring` cell: it
    /// becomes `Vacant` and the retire is acknowledged. Anything else is a
    /// no-op.
    pub fn retired(&mut self, cell: CellId) -> Option<HostMsg> {
        let slot = self.cells.get_mut(cell.index())?;
        match slot.state {
            CellState::Retiring { epoch } => {
                slot.state = CellState::Vacant;
                Some(HostMsg::CellRetired { cell, epoch })
            }
            _ => None,
        }
    }
}

impl CellRead for Mirror {
    fn get(&self, cell: CellId) -> f32 {
        match self.cells.get(cell.index()).map(|c| c.state) {
            Some(CellState::Live { value, .. }) => value,
            _ => 0.0,
        }
    }
}
