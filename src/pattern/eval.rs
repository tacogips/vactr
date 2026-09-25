//! The query context: the VM handle, input cells and parameter evaluation
//! (design 7.1.3, 10.3).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::clock::tempo::Tempo;
use crate::ns::namespace::VarSlotRef;
use crate::pattern::pat::{PParam, Pat};
use crate::pattern::query::{q, Event, TimeSpan};
use crate::pattern::step::{steps, steps_of_list};
use crate::reader::span::Span;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure, Origin};

/// The VM handle used during query (the "VM handle" of 10.3). Every call
/// runs in Query effect mode (10.4); INTEGRATE implements it for `Vm`.
pub trait QueryVm {
    /// Calls a closure or native (`PParam::Fn`, `VParam::Fn`, transforms).
    ///
    /// # Errors
    /// Any failure of the callee.
    fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure>;

    /// Reads a late-bound var, fn or tweak slot (`PParam::Late`).
    ///
    /// # Errors
    /// A failed or blocked binding.
    fn deref(&mut self, r: &VarSlotRef) -> Result<Value, Failure>;

    /// Drains `print` output captured during the query (10.4).
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)>;

    /// The current session-level `sound-kit` (the prelude's when the session
    /// binds none).
    ///
    /// # Errors
    /// A failed binding.
    fn sound_kit(&mut self) -> Result<Value, Failure>;
}

id_newtype!(
    /// An analyzer cell (design 12.5).
    AnalyzerId(u32)
);

/// A host analysis signal (design 10.5).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HostSig {
    /// `fft n`: one band level, 0..1.
    Fft(u16),
    /// `amp`: overall amplitude, 0..1.
    Amp,
    /// The input envelope, 0..1.
    Env,
}

/// The `f32` input cells read by signals at query and frame time (7.1.3).
/// Only tests write them in this issue; TASK-007/008 hosts write them later.
/// An unset cell reads 0.
#[derive(Clone, Debug, Default)]
pub struct InputCells {
    cc: BTreeMap<(u8, u8), f32>,
    analyzers: BTreeMap<AnalyzerId, f32>,
    host: BTreeMap<HostSig, f32>,
    ctrl: BTreeMap<(KwId, KwId), f32>,
    hits: BTreeMap<KwId, f32>,
}

impl InputCells {
    /// Empty cells.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes a MIDI CC cell (0..1).
    pub fn set_cc(&mut self, channel: u8, controller: u8, v: f32) {
        self.cc.insert((channel, controller), v);
    }

    /// Reads a MIDI CC cell.
    #[must_use]
    pub fn cc(&self, channel: u8, controller: u8) -> f32 {
        self.cc.get(&(channel, controller)).copied().unwrap_or(0.0)
    }

    /// Writes an analyzer cell.
    pub fn set_analyzer(&mut self, id: AnalyzerId, v: f32) {
        self.analyzers.insert(id, v);
    }

    /// Reads an analyzer cell.
    #[must_use]
    pub fn analyzer(&self, id: AnalyzerId) -> f32 {
        self.analyzers.get(&id).copied().unwrap_or(0.0)
    }

    /// Writes a host analysis cell.
    pub fn set_host(&mut self, sig: HostSig, v: f32) {
        self.host.insert(sig, v);
    }

    /// Reads a host analysis cell.
    #[must_use]
    pub fn host(&self, sig: HostSig) -> f32 {
        self.host.get(&sig).copied().unwrap_or(0.0)
    }

    /// Writes a control telemetry cell.
    pub fn set_ctrl(&mut self, slot: KwId, control: KwId, v: f32) {
        self.ctrl.insert((slot, control), v);
    }

    /// Reads a control telemetry cell.
    #[must_use]
    pub fn ctrl(&self, slot: KwId, control: KwId) -> f32 {
        self.ctrl.get(&(slot, control)).copied().unwrap_or(0.0)
    }

    /// Writes a hit telemetry cell.
    pub fn set_hits(&mut self, slot: KwId, v: f32) {
        self.hits.insert(slot, v);
    }

    /// Reads a hit telemetry cell.
    #[must_use]
    pub fn hits(&self, slot: KwId) -> f32 {
        self.hits.get(&slot).copied().unwrap_or(0.0)
    }
}

/// What a query reads: the VM handle, the input cells, the session seed
/// and the tempo (for `time` and `beat`, and fault beats). The tempo is an
/// addition to the plan's field list.
pub struct QueryCtx<'a> {
    pub vm: &'a mut dyn QueryVm,
    pub cells: &'a InputCells,
    pub seed: u64,
    pub tempo: Tempo,
}

impl<'a> QueryCtx<'a> {
    /// A context at the default tempo.
    pub fn new(vm: &'a mut dyn QueryVm, cells: &'a InputCells, seed: u64) -> Self {
        Self {
            vm,
            cells,
            seed,
            tempo: Tempo::default(),
        }
    }
}

/// Recursion bound for one query (7.1.5): late refs can make a pattern
/// sample itself.
const MAX_DEPTH: u32 = 256;
/// Nested re-entries (VM calls and point samples) inside one query: the
/// 7.1.5 bound on Rust-level re-entry.
const MAX_REENTRY: u32 = 64;
/// Work bound for one query: cycle pieces plus events.
const QUERY_BUDGET: u64 = 1_000_000;

/// Per-query state: the context, the recorded faults and the bounds.
pub(crate) struct QState<'c, 'a> {
    pub cx: &'c mut QueryCtx<'a>,
    pub faults: Vec<Failure>,
    depth: u32,
    reentry: u32,
    budget: u64,
    exhausted: bool,
}

impl<'c, 'a> QState<'c, 'a> {
    pub fn new(cx: &'c mut QueryCtx<'a>) -> Self {
        Self {
            cx,
            faults: Vec::new(),
            depth: 0,
            reentry: 0,
            budget: QUERY_BUDGET,
            exhausted: false,
        }
    }

    /// Records a fault with its origin: the node span when the failure has
    /// none, and the beat of `at`.
    pub fn fault(&mut self, mut f: Failure, span: Option<Span>, at: Option<Ratio64>) {
        if f.code == FailCode::FuelExhausted
            && self
                .faults
                .iter()
                .any(|g| g.code == FailCode::FuelExhausted)
        {
            return;
        }
        if f.origin.span.is_none() {
            f.origin.span = span;
        }
        if f.origin.beat.is_none() {
            f.origin.beat = at.and_then(|t| self.cx.tempo.beats_at(t).ok());
        }
        self.faults.push(f);
    }

    /// Spends query work.
    ///
    /// # Errors
    /// `FuelExhausted` once the budget is gone.
    pub fn spend(&mut self, n: u64, _span: Option<Span>) -> Result<(), Failure> {
        if self.exhausted || self.budget < n {
            self.exhausted = true;
            self.budget = 0;
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "query produced too many events",
            ));
        }
        self.budget -= n;
        Ok(())
    }

    /// True once the budget ran out.
    pub const fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// Enters one recursion level.
    pub fn enter(&mut self) -> Result<(), Failure> {
        if self.depth >= MAX_DEPTH {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "pattern nesting too deep",
            ));
        }
        self.depth += 1;
        Ok(())
    }

    /// Leaves one recursion level.
    pub fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Runs `f` as one nested re-entry.
    ///
    /// # Errors
    /// `DepthExceeded` past the bound, or `f`'s failure.
    pub fn reenter<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if self.reentry >= MAX_REENTRY {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "too many nested re-entries in one query",
            ));
        }
        self.reentry += 1;
        let r = f(self);
        self.reentry -= 1;
        r
    }

    /// Splits `span` at cycle boundaries. A point span is one piece.
    pub fn cycles(&mut self, span: TimeSpan, node: Option<Span>) -> Vec<TimeSpan> {
        match span.cycle_pieces(self) {
            Ok(v) => v,
            Err(f) => {
                self.fault(f, node, Some(span.begin));
                Vec::new()
            }
        }
    }

    /// Queries `p` at the point `t`, turning any fault raised inside into
    /// this call's error so the caller records it once.
    pub fn sample(&mut self, p: &Pat, t: Ratio64) -> Result<Vec<Event>, Failure> {
        let mark = self.faults.len();
        let events = self.reenter(|st| Ok(q(p, TimeSpan::point(t), st)))?;
        if self.faults.len() > mark {
            let first = self.faults.remove(mark);
            self.faults.truncate(mark);
            return Err(first);
        }
        Ok(events)
    }

    /// The value of `p` at `t`: its first event's value, `nil` for a rest.
    pub fn sample_value(&mut self, p: &Pat, t: Ratio64) -> Result<Value, Failure> {
        Ok(self
            .sample(p, t)?
            .into_iter()
            .next()
            .map_or(Value::Nil, |e| e.value))
    }
}

fn type_err(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

/// Resolves a late ref or a function of time to its value at `t`
/// (a closure gets `t` as a ratio; a thunk gets no argument). Patterns and
/// signals are returned as they are.
pub(crate) fn resolve_dynamic(
    v: &Value,
    t: Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<Value, Failure> {
    let mut cur = v.clone();
    for _ in 0..8 {
        cur = match &cur {
            Value::VarRef(r) => st.reenter(|st| st.cx.vm.deref(r))?,
            Value::Thunk(_) => st.reenter(|st| st.cx.vm.call(&cur, &[]))?,
            Value::Fn(_) | Value::Native(_) => {
                st.reenter(|st| st.cx.vm.call(&cur, &[Value::Ratio(t)]))?
            }
            _ => return Ok(cur),
        };
    }
    Err(Failure::new(
        FailCode::DepthExceeded,
        "late reference chain too long",
    ))
}

/// The value of a (possibly dynamic) value at `t`: patterns are sampled,
/// signals read.
pub(crate) fn value_at(v: &Value, t: Ratio64, st: &mut QState<'_, '_>) -> Result<Value, Failure> {
    let v = resolve_dynamic(v, t, st)?;
    match v {
        Value::Pattern(p) => st.sample_value(&p, t),
        Value::Signal(s) => s.value_at(t, st.cx),
        other => Ok(other),
    }
}

/// Evaluates a parameter at `t` (10.1: per query, on the evaluator thread).
pub(crate) fn eval_param(
    p: &PParam,
    t: Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<Value, Failure> {
    match p {
        PParam::Const(v) => value_at(v, t, st),
        PParam::Late(r) => {
            let v = st.reenter(|st| st.cx.vm.deref(r))?;
            value_at(&v, t, st)
        }
        PParam::Fn(f) => {
            let v = resolve_dynamic(f, t, st)?;
            value_at(&v, t, st)
        }
        PParam::Pat(p) => st.sample_value(p, t),
    }
}

/// A numeric parameter as an exact ratio.
pub(crate) fn eval_param_ratio(
    p: &PParam,
    t: Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<Ratio64, Failure> {
    let v = eval_param(p, t, st)?;
    num_ratio(&v).ok_or_else(|| type_err("expected a number"))
}

/// An integer parameter.
pub(crate) fn eval_param_int(
    p: &PParam,
    t: Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<i64, Failure> {
    let v = eval_param(p, t, st)?;
    int_of(&v).ok_or_else(|| type_err("expected an integer"))
}

/// A numeric parameter as a float (probabilities).
pub(crate) fn eval_param_f64(
    p: &PParam,
    t: Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<f64, Failure> {
    let v = eval_param(p, t, st)?;
    num_f64(&v).ok_or_else(|| type_err("expected a number"))
}

/// Calls a pattern transform (`every 4 {p -> fast p 2}`); it must return a
/// pattern (a list is coerced to steps).
pub(crate) fn apply_transform(
    f: &Value,
    p: &Rc<Pat>,
    st: &mut QState<'_, '_>,
) -> Result<Rc<Pat>, Failure> {
    match st.reenter(|st| st.cx.vm.call(f, &[Value::Pattern(Rc::clone(p))]))? {
        Value::Pattern(r) => Ok(r),
        Value::List(l) => Ok(Rc::new(steps(steps_of_list(&l), None))),
        _ => Err(type_err("a pattern transform must return a pattern")),
    }
}

/// Any number as `f64`.
#[must_use]
pub fn num_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(f64::from(*i)),
        Value::Int64(i) => Some(*i as f64),
        Value::Float(x) => Some(f64::from(*x)),
        Value::Float64(x) => Some(*x),
        Value::Ratio(r) => Some(r.to_f64()),
        _ => None,
    }
}

/// Any number as an exact ratio. A float converts through its shortest
/// decimal text, so `0.31` is `31/100`.
#[must_use]
pub fn num_ratio(v: &Value) -> Option<Ratio64> {
    match v {
        Value::Int(i) => Some(Ratio64::from_int(i64::from(*i))),
        Value::Int64(i) => Some(Ratio64::from_int(*i)),
        Value::Ratio(r) => Some(*r),
        Value::Float(x) if x.is_finite() => Ratio64::from_decimal(&x.to_string()),
        Value::Float64(x) if x.is_finite() => Ratio64::from_decimal(&x.to_string()),
        _ => None,
    }
}

/// An integral number as `i64`.
#[must_use]
pub fn int_of(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) => Some(i64::from(*i)),
        Value::Int64(i) => Some(*i),
        Value::Ratio(r) if r.is_integral() => Some(r.num()),
        Value::Float(_) | Value::Float64(_) => {
            let x = num_f64(v)?;
            if x.fract() == 0.0 && x.abs() < 9.0e15 {
                Some(x as i64)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// An exact number as a value: an `Int` when integral and in range, else a
/// `Ratio`.
#[must_use]
pub fn exact_value(r: Ratio64) -> Value {
    if r.is_integral() {
        if let Ok(i) = i32::try_from(r.num()) {
            return Value::Int(i);
        }
    }
    Value::Ratio(r)
}
