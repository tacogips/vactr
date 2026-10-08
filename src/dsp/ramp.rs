use crate::dsp::cells::{CellId, CellRead};

/// Maximum simultaneous audio-side parameter ramps.
pub const MAX_CELL_RAMPS: usize = 32;
const NONE: u8 = u8::MAX;

#[derive(Clone, Copy, Debug)]
struct RampSlot {
    cell: CellId,
    epoch: u32,
    seq: u32,
    from: f32,
    target: f32,
    start: u64,
    frames: u32,
    release: bool,
    live: bool,
}

impl Default for RampSlot {
    fn default() -> Self {
        Self {
            cell: CellId::new(0),
            epoch: 0,
            seq: 0,
            from: 0.0,
            target: 0.0,
            start: 0,
            frames: 0,
            release: false,
            live: false,
        }
    }
}

/// Fixed-capacity audio-clock cell ramps.
#[derive(Debug)]
pub struct CellRamps {
    slots: [RampSlot; MAX_CELL_RAMPS],
    by_cell: Box<[u8]>,
    dropped: u32,
}

impl Default for CellRamps {
    fn default() -> Self {
        Self {
            slots: [RampSlot::default(); MAX_CELL_RAMPS],
            by_cell: Box::new([]),
            dropped: 0,
        }
    }
}

impl CellRamps {
    /// Allocates the cell index once, during engine construction.
    #[must_use]
    pub fn new(cell_capacity: usize) -> Self {
        Self {
            by_cell: vec![NONE; cell_capacity].into_boxed_slice(),
            ..Self::default()
        }
    }

    /// Number of distinct ramp slots that could not be admitted.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }

    /// `(epoch, sequence)` associated with a live cell ramp.
    #[must_use]
    pub fn metadata(&self, cell: CellId) -> Option<(u32, u32)> {
        let slot = self.slots.get(self.slot_for(cell)?)?;
        Some((slot.epoch, slot.seq))
    }

    fn slot_for(&self, cell: CellId) -> Option<usize> {
        self.by_cell
            .get(cell.index())
            .copied()
            .filter(|i| *i != NONE)
            .map(usize::from)
    }

    fn clear_slot(&mut self, index: usize) {
        let slot = &mut self.slots[index];
        if let Some(mapped) = self.by_cell.get_mut(slot.cell.index()) {
            *mapped = NONE;
        }
        slot.live = false;
    }

    /// Applies a target and returns whether the ramp was accepted.
    // This argument list is the pinned wire-to-ramp interface in LP-ENGINE.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        cell: CellId,
        epoch: u32,
        seq: u32,
        target: f32,
        frames: u32,
        release: bool,
        frame: u64,
        current: f32,
    ) -> bool {
        let Some(map) = self.by_cell.get(cell.index()) else {
            return false;
        };
        let index = if *map != NONE {
            usize::from(*map)
        } else if let Some(index) = self.slots.iter().position(|slot| !slot.live) {
            index
        } else {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        };
        if release && frames == 0 {
            if self.slots[index].live {
                self.clear_slot(index);
            }
            return true;
        }
        self.slots[index] = RampSlot {
            cell,
            epoch,
            seq,
            from: current,
            target,
            start: frame,
            frames,
            release,
            live: true,
        };
        self.by_cell[cell.index()] = u8::try_from(index).unwrap_or(NONE);
        true
    }

    /// Reaps completed release ramps at block start.
    pub fn reap(&mut self, frame: u64) {
        for i in 0..self.slots.len() {
            let slot = self.slots[i];
            if slot.live
                && slot.release
                && frame >= slot.start.saturating_add(u64::from(slot.frames))
            {
                self.clear_slot(i);
            }
        }
    }

    /// Drops a cell's override, typically on `CellRetire`.
    pub fn drop_cell(&mut self, cell: CellId) {
        if let Some(index) = self.slot_for(cell) {
            self.clear_slot(index);
        }
    }

    fn value_at(&self, cell: CellId, frame: u64) -> Option<f32> {
        let slot = self.slots[self.slot_for(cell)?];
        if !slot.live {
            return None;
        }
        if slot.frames == 0 {
            return Some(slot.target);
        }
        let elapsed = frame.saturating_sub(slot.start).min(u64::from(slot.frames));
        let fraction = elapsed as f32 / slot.frames as f32;
        Some(slot.from + (slot.target - slot.from) * fraction)
    }
}

/// A `CellRead` view that overlays the current audio-clock ramp values.
pub struct RampedCells<'a, C: CellRead + ?Sized> {
    ramps: &'a CellRamps,
    inner: &'a C,
    frame: u64,
}

impl<'a, C: CellRead + ?Sized> RampedCells<'a, C> {
    /// Creates a read view for one block's starting frame.
    #[must_use]
    pub const fn new(ramps: &'a CellRamps, inner: &'a C, frame: u64) -> Self {
        Self {
            ramps,
            inner,
            frame,
        }
    }
}

impl<C: CellRead + ?Sized> CellRead for RampedCells<'_, C> {
    fn get(&self, cell: CellId) -> f32 {
        self.ramps
            .value_at(cell, self.frame)
            .unwrap_or_else(|| self.inner.get(cell))
    }
}
