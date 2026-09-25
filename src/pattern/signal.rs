//! Continuous signals (design 10.5). A signal is queried at a point.

use std::f64::consts::TAU;
use std::rc::Rc;

use crate::pattern::eval::{AnalyzerId, HostSig, QueryCtx};
use crate::pattern::rng::{mix64, to_unit, Hasher};
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// A continuous signal. Waveforms run 0..1 over one cycle.
#[derive(Clone, Debug)]
pub enum Sig {
    Sine,
    Saw,
    Tri,
    Square,
    Rand,
    Perlin,
    IRand(i64),
    /// Seconds (cycles over cps).
    Time,
    /// Beats (cycles times beats per cycle).
    Beat,
    /// The position inside the cycle, 0..1.
    Phase,
    /// The cycle number.
    Cycle,
    /// Host analysis (`fft n`, `amp`).
    Host(HostSig),
    /// A MIDI CC cell, 0..1 (11.7).
    Cc {
        controller: u8,
        channel: u8,
    },
    /// An analyzer cell (12.5).
    Analyzer(AnalyzerId),
    /// A per-slot control telemetry cell (slot, control).
    Ctrl(KwId, KwId),
    /// A per-slot hit telemetry cell.
    Hits(KwId),
    /// `lag s seconds`. The smoothing is stateful and belongs to the frame
    /// evaluator (TASK-007/010); a point query reads the inner signal.
    Lag(Rc<Sig>, f64),
    /// `map-range s lo hi`.
    MapRange(Rc<Sig>, f64, f64),
}

/// Exact sine of a cycle phase at the quarter points; `f64::sin` elsewhere.
fn sine_at(phase: Ratio64) -> f64 {
    let quarter = |n: i64, d: i64| Ratio64::new(n, d).ok() == Some(phase);
    if phase == Ratio64::ZERO || quarter(1, 2) {
        0.5
    } else if quarter(1, 4) {
        1.0
    } else if quarter(3, 4) {
        0.0
    } else {
        (TAU * phase.to_f64()).sin() * 0.5 + 0.5
    }
}

/// A pure random value in `[0, 1)` for a time position and seed.
fn time_rand(seed: u64, t: Ratio64) -> f64 {
    to_unit(mix64(Hasher::new(0x0073_6967).word(seed).ratio(t).finish()))
}

fn smoother_step(x: f64) -> f64 {
    6.0 * x.powi(5) - 15.0 * x.powi(4) + 10.0 * x.powi(3)
}

impl Sig {
    /// The value at cycle position `t`: a `Float64`, or an `Int` for
    /// `irand` and `cycle`.
    ///
    /// # Errors
    /// Ratio overflow.
    pub fn value_at(&self, t: Ratio64, cx: &QueryCtx<'_>) -> Result<Value, Failure> {
        let phase = t.frac();
        Ok(match self {
            Sig::Sine => Value::Float64(sine_at(phase)),
            Sig::Saw | Sig::Phase => Value::Float64(phase.to_f64()),
            Sig::Tri => {
                let half = Ratio64::new(1, 2)?;
                let v = if phase < half {
                    phase.checked_mul(Ratio64::from_int(2))?
                } else {
                    Ratio64::from_int(2).checked_sub(phase.checked_mul(Ratio64::from_int(2))?)?
                };
                Value::Float64(v.to_f64())
            }
            Sig::Square => {
                let half = Ratio64::new(1, 2)?;
                Value::Float64(if phase < half { 0.0 } else { 1.0 })
            }
            Sig::Rand => Value::Float64(time_rand(cx.seed, t)),
            Sig::IRand(n) => {
                let n = (*n).max(0);
                let v = (time_rand(cx.seed, t) * n as f64).floor() as i64;
                int_value(v.min(n.saturating_sub(1)).max(0))
            }
            Sig::Perlin => {
                let a = t.floor();
                let x = phase.to_f64();
                let ra = time_rand(cx.seed, Ratio64::from_int(a));
                let rb = time_rand(cx.seed, Ratio64::from_int(a.saturating_add(1)));
                Value::Float64(ra + smoother_step(x) * (rb - ra))
            }
            Sig::Time => {
                let cps = cx.tempo.cps()?;
                Value::Float64(t.checked_div(cps)?.to_f64())
            }
            Sig::Beat => Value::Float64(cx.tempo.beats_at(t)?.to_f64()),
            Sig::Cycle => int_value(t.floor()),
            Sig::Host(h) => Value::Float64(f64::from(cx.cells.host(*h))),
            Sig::Cc {
                controller,
                channel,
            } => Value::Float64(f64::from(cx.cells.cc(*channel, *controller))),
            Sig::Analyzer(id) => Value::Float64(f64::from(cx.cells.analyzer(*id))),
            Sig::Ctrl(slot, ctl) => Value::Float64(f64::from(cx.cells.ctrl(*slot, *ctl))),
            Sig::Hits(slot) => Value::Float64(f64::from(cx.cells.hits(*slot))),
            Sig::Lag(inner, _) => inner.value_at(t, cx)?,
            Sig::MapRange(inner, lo, hi) => {
                let v = inner.value_at(t, cx)?;
                let Some(x) = crate::pattern::eval::num_f64(&v) else {
                    return Err(Failure::new(FailCode::Type, "map-range expects a number"));
                };
                Value::Float64(lo + x * (hi - lo))
            }
        })
    }
}

fn int_value(n: i64) -> Value {
    match i32::try_from(n) {
        Ok(i) => Value::Int(i),
        Err(_) => Value::Int64(n),
    }
}
