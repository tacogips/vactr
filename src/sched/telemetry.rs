//! Playing-state telemetry (design 11.6).
//!
//! Every realized event is published as a `PlayingEvent` into a bounded
//! queue the session layer drains (editor highlighting), and into a
//! per-slot rolling window that backs the `hits` and `ctrl` event signals
//! through `InputCells`. Provenance is the event's own `SrcRef`; only an
//! event without one falls back to the binding form's.

use std::collections::{BTreeMap, VecDeque};

use crate::pattern::eval::{num_f64, InputCells};
use crate::reader::span::SrcRef;
use crate::sched::slots::SlotKind;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

/// The telemetry queue bound; the oldest entries are dropped past it.
pub const QUEUE_CAP: usize = 1024;

/// How long the rolling window looks back, in host seconds.
pub const WINDOW_SECONDS: f64 = 1.0;

/// One realized event (design 11.6).
#[derive(Clone, PartialEq, Debug)]
pub struct PlayingEvent {
    pub slot: KwId,
    pub beat: Ratio64,
    pub time: f64,
    pub src: Option<SrcRef>,
    pub dur: f64,
    pub kind: SlotKind,
    /// Committed with less than the configured commit lead (a rebind too
    /// close to its boundary, 11.3 "Insufficient lead").
    pub reduced_lead: bool,
    /// Announcement id, present on an early preview and its confirmation.
    pub id: Option<u64>,
}

/// Recent event starts of one slot: `(time, numeric controls)`.
type Window = VecDeque<(f64, Vec<(KwId, f32)>)>;

/// The bounded queue and the per-slot windows.
#[derive(Clone, Debug, Default)]
pub struct Telemetry {
    queue: VecDeque<PlayingEvent>,
    dropped: u64,
    windows: BTreeMap<KwId, Window>,
}

impl Telemetry {
    /// Publishes one event with its numeric controls.
    pub fn publish(&mut self, ev: PlayingEvent, controls: &[(KwId, Value)]) {
        let numeric: Vec<(KwId, f32)> = controls
            .iter()
            .filter_map(|(k, v)| {
                #[allow(clippy::cast_possible_truncation)]
                num_f64(v).map(|x| (*k, x as f32))
            })
            .collect();
        self.windows
            .entry(ev.slot)
            .or_default()
            .push_back((ev.time, numeric));
        if self.queue.len() >= QUEUE_CAP {
            self.queue.pop_front();
            self.dropped += 1;
        }
        self.queue.push_back(ev);
    }

    /// Drains the queue (the session forwards it to editors).
    pub fn drain(&mut self) -> Vec<PlayingEvent> {
        self.queue.drain(..).collect()
    }

    /// Entries dropped because the queue was full.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Trims every window to the last `WINDOW_SECONDS` before `now` and
    /// writes `hits` (events per second started in the window) and the
    /// latest `ctrl` value of each control into the input cells.
    pub fn refresh(&mut self, now: f64, cells: &mut InputCells) {
        for (slot, w) in &mut self.windows {
            while w.front().is_some_and(|(t, _)| *t < now - WINDOW_SECONDS) {
                w.pop_front();
            }
            let started = w.iter().filter(|(t, _)| *t <= now).count();
            #[allow(clippy::cast_precision_loss)]
            cells.set_hits(*slot, started as f32 / WINDOW_SECONDS as f32);
            for (t, ctls) in w.iter() {
                if *t <= now {
                    for (k, v) in ctls {
                        cells.set_ctrl(*slot, *k, *v);
                    }
                }
            }
        }
    }
}
