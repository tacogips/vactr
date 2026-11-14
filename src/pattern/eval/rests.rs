//! Silent source intervals captured only by scheduler playback queries.
//! Kept outside the musical event stream so sampling/composition cannot turn
//! a rest into a control value, a sound, or a diagnostic.

use crate::pattern::pat::Pat;
use crate::pattern::query::{sect, Event, TimeSpan};
use crate::reader::span::{NodeId, SrcRef};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

#[derive(Default)]
pub(crate) struct RestCapture {
    pub enabled: bool,
    pub events: Vec<Event>,
    pub sample: Option<RestSample>,
}

#[derive(Clone, Copy)]
pub(crate) struct RestSample {
    child: *const Pat,
    at: Ratio64,
    whole: Option<TimeSpan>,
    part: TimeSpan,
}

/// Reused literal sources can have the same onset but different transformed
/// durations. Keep both so staging cannot shorten the longer highlight.
pub(crate) fn finish_occurrence(event: &mut Event) {
    event.occ.anchor_at(event.anchor());
    if let Some(whole) = event.whole {
        for value in [whole.end.num(), whole.end.den()] {
            let bytes = value.to_le_bytes();
            event.occ.push(
                NodeId::new(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
                u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            );
        }
    }
}

impl super::QState<'_, '_> {
    /// Visual-only predicate evaluation cannot consume musical work budget,
    /// publish print output, or create diagnostics for an otherwise silent rest.
    #[inline(never)]
    pub(crate) fn rest_predicate_work<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let budget = self.budget;
        let exhausted = self.exhausted;
        let faults = self.faults.len();
        let output = self.cx.vm.take_output();
        let capture = std::mem::replace(&mut self.rests.enabled, false);
        let result = f(self);
        let _ = self.cx.vm.take_output();
        self.cx.vm.put_output(output);
        self.rests.enabled = capture;
        self.faults.truncate(faults);
        self.budget = budget;
        self.exhausted = exhausted;
        result
    }

    #[inline(never)]
    pub(crate) fn sample_playback(&mut self, p: &Pat, t: Ratio64) -> Result<Vec<Event>, Failure> {
        let mark = self.faults.len();
        let sample = self.take_rest_sample(p, t);
        let rest_mark = self.rests.events.len();
        let capture = std::mem::replace(&mut self.rests.enabled, sample.is_some());
        let result = self.reenter(|st| Ok(crate::pattern::query::q(p, TimeSpan::point(t), st)));
        self.rests.enabled = capture;
        if let Some(sample) = sample {
            if result.is_ok() && self.faults.len() == mark {
                self.finish_rest_sample(rest_mark, sample);
            } else {
                self.rests.events.truncate(rest_mark);
            }
        }
        let events = result?;
        if self.faults.len() > mark {
            let first = self.faults.remove(mark);
            self.faults.truncate(mark);
            return Err(first);
        }
        Ok(events)
    }

    pub(crate) fn with_rest_sample<T>(
        &mut self,
        child: &Pat,
        at: Ratio64,
        whole: Option<TimeSpan>,
        part: TimeSpan,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let saved = self.rests.sample.replace(RestSample {
            child: std::ptr::from_ref(child),
            at,
            whole,
            part,
        });
        let result = f(self);
        self.rests.sample = saved;
        result
    }

    /// Consume only the matching structural sample. Parameter/VM samples
    /// inside it cannot borrow the caller's timing relation.
    pub(crate) fn take_rest_sample(&mut self, child: &Pat, at: Ratio64) -> Option<RestSample> {
        self.rests
            .sample
            .take()
            .filter(|sample| sample.child == std::ptr::from_ref(child) && sample.at == at)
    }

    pub(crate) fn finish_rest_sample(&mut self, mark: usize, sample: RestSample) {
        for event in &mut self.rests.events[mark..] {
            event.whole = sample.whole;
            event.part = sample.part;
        }
    }

    pub(crate) fn captures_timing(&self) -> bool {
        self.has_clock_observation() || self.rests.enabled
    }

    pub(crate) fn capture_rest(
        &mut self,
        src: Option<SrcRef>,
        whole: TimeSpan,
        piece: TimeSpan,
    ) -> Result<(), Failure> {
        if !self.rests.enabled {
            return Ok(());
        }
        let (Some(src), Some(part)) = (src, sect(whole, piece)) else {
            return Ok(());
        };
        // Bound visual work independently: a dense rest list must never consume
        // the musical query's remaining event budget or prevent sibling notes.
        if self.rests.events.len() >= 4096 {
            return Ok(());
        }
        let mut event = Event::new(Some(whole), part, Value::Nil, Some(src));
        // Separate markers from normal occurrence paths, with stable source
        // identity across overlapping scheduler windows.
        event.occ.push(NodeId::new(u32::MAX), src.span.file.get());
        event.occ.push(NodeId::new(src.span.start), src.span.end);
        self.rests.events.push(event);
        Ok(())
    }
}
