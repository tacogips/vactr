//! Audio-cell ramp transport (design 5.7).

use crate::dsp::cells::CellId;
use crate::host::wire::CtlMsg;
use crate::sched::cells::ControlCells;

/// Maximum simultaneous cell ramps owned by one runtime.
pub const MAX_CELL_RAMPS: usize = 32;

/// Capacity rejection for a new cell ramp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Capacity;

#[derive(Clone, Copy, Debug)]
struct SentRamp {
    cell: CellId,
    epoch: u32,
    seq: u32,
    target: f32,
    end_frame: u64,
    waited: u32,
    release: bool,
    drop: bool,
    acked: bool,
}

/// Bounded reliable sender for engine cell ramps.
#[derive(Debug, Default)]
pub(crate) struct RampSender {
    entries: Vec<SentRamp>,
    seq: u32,
}

impl RampSender {
    /// Whether admitting this cell would remain within the fixed ramp bound.
    pub(crate) fn can_post(&self, cell: CellId, epoch: u32) -> bool {
        self.entries
            .iter()
            .any(|r| r.cell == cell && r.epoch == epoch)
            || self.entries.len() < MAX_CELL_RAMPS
    }

    /// Whether every distinct requested incarnation fits as one admission.
    pub(crate) fn can_post_all<I>(&self, cells: I) -> bool
    where
        I: IntoIterator<Item = (CellId, u32)>,
    {
        let mut new_cells = Vec::with_capacity(MAX_CELL_RAMPS);
        for (cell, epoch) in cells {
            if self
                .entries
                .iter()
                .any(|r| r.cell == cell && r.epoch == epoch)
                || new_cells.contains(&(cell, epoch))
            {
                continue;
            }
            if !self.can_post(cell, epoch) {
                return false;
            }
            new_cells.push((cell, epoch));
        }
        true
    }

    /// Starts or replaces a ramp for an acknowledged cell incarnation.
    pub(crate) fn post(
        &mut self,
        cell: CellId,
        epoch: u32,
        target: f32,
        frames: u32,
        release: bool,
        now_frame: u64,
    ) -> Result<CtlMsg, Capacity> {
        let existing = self
            .entries
            .iter()
            .position(|r| r.cell == cell && r.epoch == epoch);
        if existing.is_none() && self.entries.len() >= MAX_CELL_RAMPS {
            return Err(Capacity);
        }
        self.seq = self.seq.wrapping_add(1);
        let seq = self.seq;
        let ramp = SentRamp {
            cell,
            epoch,
            seq,
            target,
            end_frame: now_frame.saturating_add(u64::from(frames)),
            waited: 0,
            release,
            drop: frames == 0 && release,
            acked: false,
        };
        if let Some(index) = existing {
            self.entries[index] = ramp;
        } else {
            self.entries.push(ramp);
        }
        Ok(CtlMsg::CellRamp {
            cell,
            epoch,
            seq,
            target,
            frames,
            release,
        })
    }

    /// Marks a matching cell ramp acknowledged.
    pub(crate) fn on_ack(&mut self, cell: CellId, seq: u32) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|r| r.cell == cell && r.seq == seq)
        {
            if self.entries[index].drop {
                self.entries.remove(index);
            } else {
                self.entries[index].acked = true;
            }
        }
    }

    /// Posts remaining-duration copies for unacknowledged ramps and retires completed releases.
    pub(crate) fn tick(
        &mut self,
        now_frame: u64,
        resend_ticks: u32,
        cells: &ControlCells,
    ) -> Vec<CtlMsg> {
        let mut out = Vec::new();
        let interval = resend_ticks.max(1);
        self.entries.retain_mut(|ramp| {
            if !cells.has_cell(ramp.cell, ramp.epoch) {
                return false;
            }
            if ramp.drop && ramp.acked {
                return false;
            }
            if ramp.release && now_frame >= ramp.end_frame {
                return false;
            }
            if !ramp.acked {
                ramp.waited = ramp.waited.saturating_add(1);
                if ramp.waited >= interval {
                    ramp.waited = 0;
                    let frames =
                        u32::try_from(ramp.end_frame.saturating_sub(now_frame)).unwrap_or(u32::MAX);
                    out.push(CtlMsg::CellRamp {
                        cell: ramp.cell,
                        epoch: ramp.epoch,
                        seq: ramp.seq,
                        target: ramp.target,
                        frames,
                        release: ramp.release,
                    });
                }
            }
            true
        });
        out
    }

    /// Drops sender state for one cell incarnation.
    pub(crate) fn forget(&mut self, cell: CellId, epoch: u32) {
        self.entries
            .retain(|ramp| ramp.cell != cell || ramp.epoch != epoch);
    }

    /// Releases a cell immediately; vanished incarnations free their slot without a post.
    pub(crate) fn drop_cell(
        &mut self,
        cell: CellId,
        epoch: u32,
        target: f32,
        now_frame: u64,
        cells: &ControlCells,
    ) -> Option<CtlMsg> {
        if !cells.has_cell(cell, epoch) {
            self.forget(cell, epoch);
            return None;
        }
        self.post(cell, epoch, target, 0, true, now_frame).ok()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}
