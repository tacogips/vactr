//! The worklet's cell table with read probes (design 11.3, 12.8.11).
//!
//! `ProbeMirror` is the browser `Mirror` the engine reads, plus counters the
//! dev harness uses to observe the real-worklet cell transport: reads of up
//! to `PROBES` probed cells (with the value of the last read and every read
//! of a cell that is not `Live`, i.e. an uninitialized read), and a ring of
//! the last `READ_LOG` probed reads with the engine time of their block.
//! Everything is fixed-size; a read only updates `Cell`s, so the probes
//! allocate nothing on the audio path.

use std::cell::Cell;

use crate::dsp::cells::{CellId, CellRead, CellState, Mirror};
use crate::dsp::ring::CellStore;
use crate::host::wire::{BatchView, HostMsg};

/// Probed cells.
pub const PROBES: usize = 4;
/// Probed reads remembered.
pub const READ_LOG: usize = 16;

/// Per-probe counters.
#[derive(Clone, Copy, Debug, Default)]
pub struct Probe {
    pub cell: Option<CellId>,
    pub reads: u32,
    pub uninit: u32,
    pub last: f32,
}

/// One probed read: `(block time, cell, value, live)`.
pub type ReadEntry = (f64, u32, f32, bool);

/// The worklet mirror with probes.
pub struct ProbeMirror {
    pub mirror: Mirror,
    probes: [Cell<Probe>; PROBES],
    log: [Cell<ReadEntry>; READ_LOG],
    logged: Cell<u64>,
    /// The engine time of the block being rendered.
    pub now: f64,
}

impl ProbeMirror {
    /// A mirror of `capacity` cells.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            mirror: Mirror::new(capacity),
            probes: Default::default(),
            log: Default::default(),
            logged: Cell::new(0),
            now: 0.0,
        }
    }

    /// Probes `cell` in probe slot `k` (counters reset).
    pub fn set_probe(&mut self, k: usize, cell: Option<CellId>) {
        if let Some(p) = self.probes.get(k) {
            p.set(Probe {
                cell,
                ..Probe::default()
            });
        }
    }

    /// Probe slot `k`.
    #[must_use]
    pub fn probe(&self, k: usize) -> Probe {
        self.probes.get(k).map(Cell::get).unwrap_or_default()
    }

    /// Read-log entry `i` (the ring slot `i % READ_LOG`).
    #[must_use]
    pub fn log(&self, i: usize) -> ReadEntry {
        self.log[i % READ_LOG].get()
    }

    /// Probed reads logged so far.
    #[must_use]
    pub fn logged(&self) -> u64 {
        self.logged.get()
    }

    /// The state of a cell as the harness reads it: `(kind, epoch, value)`
    /// with kind 0 vacant, 1 live, 2 retiring, -1 out of range.
    #[must_use]
    pub fn describe(&self, cell: CellId) -> (i32, u32, f32) {
        match self.mirror.state(cell) {
            Some(CellState::Vacant) => (0, 0, 0.0),
            Some(CellState::Live { epoch, value }) => (1, epoch, value),
            Some(CellState::Retiring { epoch }) => (2, epoch, 0.0),
            None => (-1, 0, 0.0),
        }
    }
}

impl CellRead for ProbeMirror {
    fn get(&self, cell: CellId) -> f32 {
        let value = self.mirror.get(cell);
        for p in &self.probes {
            let mut pr = p.get();
            if pr.cell != Some(cell) {
                continue;
            }
            let live = matches!(self.mirror.state(cell), Some(CellState::Live { .. }));
            pr.reads += 1;
            pr.last = value;
            if !live {
                pr.uninit += 1;
            }
            p.set(pr);
            let n = self.logged.get();
            self.log[(n % READ_LOG as u64) as usize].set((self.now, cell.get(), value, live));
            self.logged.set(n + 1);
        }
        value
    }
}

impl CellStore for ProbeMirror {
    fn init(&mut self, cell: CellId, epoch: u32, value: f32) -> Option<HostMsg> {
        self.mirror.init(cell, epoch, value)
    }
    fn batch(&mut self, seq: u32, view: &BatchView<'_>) -> Option<HostMsg> {
        self.mirror.batch(seq, view)
    }
    fn retire(&mut self, cell: CellId, epoch: u32) -> bool {
        CellStore::retire(&mut self.mirror, cell, epoch)
    }
    fn retired(&mut self, cell: CellId) -> bool {
        CellStore::retired(&mut self.mirror, cell)
    }
}
