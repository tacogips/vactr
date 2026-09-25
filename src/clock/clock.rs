//! The piecewise-linear cycle clock and MIDI clock slaving (design 11.1, 11.7).

use crate::clock::tempo::Tempo;
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};

/// MIDI clock resolution: 24 pulses per quarter note.
pub const PULSES_PER_BEAT: i64 = 24;

/// How many anchors a clock keeps; older positions extrapolate from the
/// oldest kept anchor.
const MAX_ANCHORS: usize = 64;

/// Where the clock takes its tempo from (design 11.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ClockSource {
    Internal,
    /// Slaved to incoming MIDI clock and transport (`use-clock :midi`).
    MidiClock,
    /// Ableton Link: represented, planned only. Selecting it is the
    /// checker's `clock-source-unavailable` diagnostic in v1.
    Link,
}

impl ClockSource {
    /// The source named by a `use-clock` keyword (`internal`, `midi`, `link`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "internal" => Some(Self::Internal),
            "midi" => Some(Self::MidiClock),
            "link" => Some(Self::Link),
            _ => None,
        }
    }

    /// False for `Link`, which v1 does not implement.
    #[must_use]
    pub const fn is_available(self) -> bool {
        !matches!(self, Self::Link)
    }
}

/// One linear piece: at host time `host` the clock was at `pos` cycles and
/// ran at `cps` from there on.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Anchor {
    host: f64,
    pos: Ratio64,
    cps: Ratio64,
}

/// Host seconds <-> `Ratio64` cycles. Each tempo change stores a new anchor
/// at an exact logical position, so past positions never move.
#[derive(Clone, PartialEq, Debug)]
pub struct Clock {
    source: ClockSource,
    tempo: Tempo,
    anchors: Vec<Anchor>,
    pos: Ratio64,
}

fn clock_external() -> Failure {
    Failure::new(FailCode::Type, "clock is external")
}

impl Clock {
    /// A clock at cycle 0 at host time `epoch_host`.
    ///
    /// # Errors
    /// Ratio overflow from the tempo's cps.
    pub fn new(source: ClockSource, tempo: Tempo, epoch_host: f64) -> Result<Self, Failure> {
        let cps = tempo.cps()?;
        Ok(Self {
            source,
            tempo,
            anchors: vec![Anchor {
                host: epoch_host,
                pos: Ratio64::ZERO,
                cps,
            }],
            pos: Ratio64::ZERO,
        })
    }

    /// The clock source.
    #[must_use]
    pub const fn source(&self) -> ClockSource {
        self.source
    }

    /// The current tempo.
    #[must_use]
    pub const fn tempo(&self) -> Tempo {
        self.tempo
    }

    /// The logical position in cycles.
    #[must_use]
    pub const fn pos(&self) -> Ratio64 {
        self.pos
    }

    /// Host seconds at cycle 0 of the oldest kept anchor's line.
    #[must_use]
    pub fn epoch_host(&self) -> f64 {
        self.anchors
            .first()
            .map_or(0.0, |a| a.host - a.pos.to_f64() / a.cps.to_f64())
    }

    /// The number of kept anchors.
    #[must_use]
    pub fn anchor_count(&self) -> usize {
        self.anchors.len()
    }

    /// Advances the logical position (the scheduler's cursor).
    pub fn set_pos(&mut self, pos: Ratio64) {
        self.pos = pos;
    }

    /// Changes the tempo from the exact position `at` on (`use-bpm`,
    /// `use-cycle`). Positions before `at` keep their host times.
    ///
    /// # Errors
    /// `Type` ("clock is external") when the clock is slaved; ratio overflow.
    pub fn set_tempo(&mut self, tempo: Tempo, at: Ratio64) -> Result<(), Failure> {
        if self.source != ClockSource::Internal {
            return Err(clock_external());
        }
        let host = self.to_host(at);
        self.push_anchor(host, at, tempo)
    }

    /// Re-anchors the line at (`host`, `pos`) with `tempo` (MIDI slaving).
    ///
    /// # Errors
    /// Ratio overflow from the tempo's cps.
    pub fn anchor_at(&mut self, host: f64, pos: Ratio64, tempo: Tempo) -> Result<(), Failure> {
        self.push_anchor(host, pos, tempo)
    }

    /// Drops every anchor and restarts the line at (`host`, `pos`).
    ///
    /// # Errors
    /// Ratio overflow from the tempo's cps.
    pub fn reset(&mut self, host: f64, pos: Ratio64) -> Result<(), Failure> {
        let cps = self.tempo.cps()?;
        self.anchors.clear();
        self.anchors.push(Anchor { host, pos, cps });
        self.pos = pos;
        Ok(())
    }

    fn push_anchor(&mut self, host: f64, pos: Ratio64, tempo: Tempo) -> Result<(), Failure> {
        let cps = tempo.cps()?;
        // A rewind drops the anchors after the new position.
        self.anchors.retain(|a| a.pos < pos);
        self.anchors.push(Anchor { host, pos, cps });
        if self.anchors.len() > MAX_ANCHORS {
            let extra = self.anchors.len() - MAX_ANCHORS;
            self.anchors.drain(..extra);
        }
        self.tempo = tempo;
        Ok(())
    }

    fn anchor_for(&self, pos: Ratio64) -> Option<&Anchor> {
        self.anchors
            .iter()
            .rev()
            .find(|a| a.pos <= pos)
            .or_else(|| self.anchors.first())
    }

    /// The host time of a logical position (host boundary only).
    #[must_use]
    pub fn to_host(&self, pos: Ratio64) -> f64 {
        match self.anchor_for(pos) {
            Some(a) => match pos.checked_sub(a.pos) {
                Ok(d) => a.host + d.to_f64() / a.cps.to_f64(),
                Err(_) => a.host,
            },
            None => 0.0,
        }
    }

    /// The approximate cycle position of a host time.
    #[must_use]
    pub fn to_cycles(&self, host: f64) -> f64 {
        let a = self
            .anchors
            .iter()
            .rev()
            .find(|a| a.host <= host)
            .or_else(|| self.anchors.first());
        match a {
            Some(a) => a.pos.to_f64() + (host - a.host) * a.cps.to_f64(),
            None => 0.0,
        }
    }
}

/// MIDI clock slaving (design 11.7). Each pulse advances the logical
/// position by exactly `1 / (24 * beats_per_cycle)` cycles. Only the host
/// anchor and the tempo follow the smoothed pulse period, so jitter never
/// moves a logical position.
#[derive(Clone, PartialEq, Debug)]
pub struct MidiClockSync {
    alpha: f64,
    period: Option<f64>,
    last_pulse: Option<f64>,
    expected: Option<f64>,
    pulses: i64,
    start_pos: Ratio64,
    running: bool,
}

impl MidiClockSync {
    /// A stopped sync with smoothing factor `alpha` in `(0, 1]` (other
    /// values fall back to 0.1).
    #[must_use]
    pub fn new(alpha: f64) -> Self {
        let alpha = if alpha > 0.0 && alpha <= 1.0 {
            alpha
        } else {
            0.1
        };
        Self {
            alpha,
            period: None,
            last_pulse: None,
            expected: None,
            pulses: 0,
            start_pos: Ratio64::ZERO,
            running: false,
        }
    }

    /// True between `Start`/`Continue` and `Stop`.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// The smoothed tempo, once two pulses arrived.
    #[must_use]
    pub fn smoothed_bpm(&self) -> Option<f64> {
        self.period.map(|p| 60.0 / (PULSES_PER_BEAT as f64 * p))
    }

    /// MIDI `Start`: the position restarts at cycle 0.
    ///
    /// # Errors
    /// Ratio overflow from the tempo's cps.
    pub fn start(&mut self, clock: &mut Clock, host: f64) -> Result<(), Failure> {
        self.pulses = 0;
        self.start_pos = Ratio64::ZERO;
        self.last_pulse = None;
        self.expected = None;
        self.running = true;
        clock.reset(host, Ratio64::ZERO)
    }

    /// MIDI `Stop`: the position freezes.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// MIDI `Continue`: resumes from the frozen position.
    pub fn resume(&mut self, clock: &Clock) {
        self.start_pos = clock.pos();
        self.pulses = 0;
        self.last_pulse = None;
        self.expected = None;
        self.running = true;
    }

    /// A clock pulse at host time `host`. Returns the new logical position.
    ///
    /// # Errors
    /// Ratio overflow (after about 2^62 pulses).
    pub fn pulse(&mut self, clock: &mut Clock, host: f64) -> Result<Ratio64, Failure> {
        if !self.running || !host.is_finite() {
            return Ok(clock.pos());
        }
        self.pulses = self
            .pulses
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "pulse count overflow"))?;
        let tempo = clock.tempo();
        let per_cycle = tempo
            .beats_per_cycle
            .checked_mul(Ratio64::from_int(PULSES_PER_BEAT))?;
        let pos = self
            .start_pos
            .checked_add(Ratio64::from_int(self.pulses).checked_div(per_cycle)?)?;
        if let Some(last) = self.last_pulse {
            let dt = host - last;
            if dt > 0.0 && dt.is_finite() {
                self.period = Some(match self.period {
                    None => dt,
                    Some(p) => p + self.alpha * (dt - p),
                });
            }
        }
        self.last_pulse = Some(host);
        let anchor_host = match (self.expected, self.period) {
            (Some(prev), Some(p)) => {
                let predicted = prev + p;
                predicted + self.alpha * (host - predicted)
            }
            _ => host,
        };
        self.expected = Some(anchor_host);
        let tempo = self
            .smoothed_bpm()
            .and_then(bpm_ratio)
            .and_then(|bpm| Tempo::new(bpm, tempo.beats_per_cycle).ok())
            .unwrap_or(tempo);
        clock.anchor_at(anchor_host, pos, tempo)?;
        clock.set_pos(pos);
        Ok(pos)
    }
}

/// A float bpm rounded to 1/1000 as a ratio.
fn bpm_ratio(bpm: f64) -> Option<Ratio64> {
    let scaled = (bpm * 1000.0).round();
    if !scaled.is_finite() || scaled <= 0.0 || scaled > 1.0e15 {
        return None;
    }
    Ratio64::new(scaled as i64, 1000).ok()
}
