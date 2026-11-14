//! The query context: the VM handle, input cells and parameter evaluation
//! (design 7.1.3, 10.3).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::clock::tempo::Tempo;
use crate::ns::namespace::VarSlotRef;
use crate::pattern::occ::{ProducerKind, ProducerTrace};
use crate::pattern::pat::{PParam, Pat};
use crate::pattern::query::{q, Event, TimeSpan};
use crate::pattern::step::{steps, steps_of_list};
use crate::reader::span::Span;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::{Sound, Value};
use crate::vm::fail::{FailCode, Failure, Origin};

pub(crate) mod song_clock;
pub(crate) mod song_observation;
pub(crate) mod song_provenance;
pub(crate) mod song_replay;

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

    /// Replaces the captured output, used by filters to drop the output of
    /// the events they remove (10.4).
    fn put_output(&mut self, out: Vec<(Origin, Rc<str>)>);

    /// The current session-level `sound-kit` (the prelude's when the session
    /// binds none).
    ///
    /// # Errors
    /// A failed binding.
    fn sound_kit(&mut self) -> Result<Value, Failure>;

    /// The instrument registered as `k` (a prelude template or a session
    /// `inst`): where `s :k` resolves when the kit has no `k` (12.8.6).
    fn inst_sound(&mut self, _k: KwId) -> Option<Value> {
        None
    }

    /// Whether the resolved Audio route uses a sample resource (bank, path
    /// or buffer). Song realization uses the same fact as commit to interpret
    /// `n`; enum tags cannot distinguish registered synths from resource voices.
    ///
    /// # Errors
    /// Unavailable classification or invalid/non-audio source. The default
    /// deliberately does not guess, and leaves legacy query behavior intact.
    fn song_sample_backed(&mut self, _sound: &Sound) -> Result<bool, Failure> {
        Err(Failure::new(
            FailCode::HostUnavailable,
            "song audio resource classification is unavailable",
        ))
    }
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
    producer: Option<ProducerTrace>,
    song_limits: Option<crate::song::SongLimits>,
    observation: Option<song_observation::SharedIndexWork>,
    observation_owner: Option<song_observation::CanonicalOwnerFrame>,
    computational_query: bool,
    dependency_scope: bool,
    song_replay: Option<std::rc::Rc<song_replay::ReplayView>>,
    issued_query: Option<song_provenance::SharedIssuedTranscript>,
    song_clock: song_clock::CanonicalClockProjection,
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
            producer: None,
            song_limits: None,
            observation: None,
            observation_owner: None,
            computational_query: false,
            dependency_scope: false,
            song_replay: None,
            issued_query: None,
            song_clock: song_clock::CanonicalClockProjection::Unknown,
        }
    }

    /// Same evaluator bounds as a legacy query, with full producer tracing.
    pub fn new_traced(cx: &'c mut QueryCtx<'a>) -> Self {
        let mut state = Self::new(cx);
        state.producer = Some(ProducerTrace::default());
        state
    }

    /// Checked song-only context; nested queries reuse this state and counters.
    #[allow(dead_code)] // Canonical realizer is installed by SONG-04.
    pub fn new_song(
        cx: &'c mut QueryCtx<'a>,
        limits: crate::song::SongLimits,
    ) -> Result<Self, Failure> {
        limits.validate()?;
        let mut state = Self::new_traced(cx);
        state.song_limits = Some(limits);
        Ok(state)
    }

    /// Exact caller policy, never replaced by defaults in nested sampling.
    pub const fn song_limits(&self) -> Option<crate::song::SongLimits> {
        self.song_limits
    }

    /// Dynamic step leaves use their intrinsic onset only in song context.
    pub fn step_onset(&self, whole: TimeSpan, piece: TimeSpan) -> Ratio64 {
        if self.song_limits().is_some() {
            whole.begin
        } else {
            crate::pattern::query::sect(whole, piece).map_or(whole.begin, |s| s.begin)
        }
    }

    /// Whether full tracing is enabled, without cloning the active path.
    #[inline(always)]
    pub fn is_traced(&self) -> bool {
        self.producer.is_some()
    }

    /// Reads the current prefix length without cloning/allocating its path.
    /// Callers can charge their shared work budget before copying producer data.
    pub(crate) fn producer_len(&self) -> Option<usize> {
        self.producer.as_ref().map(|trace| trace.steps.len())
    }

    /// The source path at this emission site; untraced queries allocate none.
    pub fn producer(&self) -> Option<ProducerTrace> {
        self.producer.clone()
    }

    /// Executes one explicitly named source edge and restores its parent path,
    /// including when the operation returns a failure.
    #[inline(always)]
    pub fn with_producer<T>(
        &mut self,
        kind: ProducerKind,
        ordinal: u32,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.push_producer(kind, ordinal);
        let result = f(self);
        self.pop_producer();
        result
    }

    /// Enters a trace scope without introducing a recursive callback frame.
    #[inline(always)]
    pub fn push_producer(&mut self, kind: ProducerKind, ordinal: u32) {
        if let Some(trace) = self.producer.as_mut() {
            trace.push(kind, ordinal);
        }
    }

    /// Restores the parent of the paired trace scope.
    #[inline(always)]
    pub fn pop_producer(&mut self) {
        if let Some(trace) = self.producer.as_mut() {
            trace.steps.pop();
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
        if let Some(work) = &self.observation {
            work.borrow_mut().charge(n)?;
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
        let limit = self
            .song_limits
            .map_or(MAX_DEPTH, |limits| limits.max_depth);
        if self.depth >= limit {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "pattern nesting too deep",
            ));
        }
        self.depth += 1;
        if let Some(work) = &self.observation {
            let mut work = work.borrow_mut();
            work.depth = self.depth;
            work.peak_depth = work.peak_depth.max(self.depth);
        }
        Ok(())
    }

    /// Leaves one recursion level.
    pub fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if let Some(work) = &self.observation {
            work.borrow_mut().depth = self.depth;
        }
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
        let events = if self.has_clock_observation() {
            self.with_clock_sampling(p, t, |st| st.reenter(|st| Ok(q(p, TimeSpan::point(t), st))))?
        } else {
            self.reenter(|st| Ok(q(p, TimeSpan::point(t), st)))?
        };
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
            _ => {
                crate::pattern::build::reject_finite(&cur)?;
                return Ok(cur);
            }
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
    let value = match p {
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
    }?;
    crate::pattern::build::reject_finite(&value)?;
    Ok(value)
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

#[cfg(test)]
mod finite_query_tests {
    #[test]
    fn finite_song_values_event_handles_are_rejected_after_callable_resolution() {
        use super::*;
        use crate::pattern::build::pattern_of;
        use crate::pattern::query::query_traced;
        use crate::pattern::tests::stub_vm::StubVm;
        use crate::song::{capture_part, EventHandle, OccurrencePath, PlacementPath};
        use crate::value::intern::intern_kw;
        // Unit-only issuance exercises the otherwise unforgeable finite tag.
        let part = capture_part(Default::default(), Ratio64::ONE).unwrap();
        let handle = EventHandle::issue(
            part.revision(),
            intern_kw("track"),
            PlacementPath::default(),
            OccurrencePath {
                producer_ordinals: vec![],
                cycle: 0,
                onset: Ratio64::ZERO,
            },
            0,
        );
        let mut vm = StubVm::new();
        let callback = vm.define(991, move |_| {
            Ok(Value::EventHandle(Rc::new(handle.clone())))
        });
        let pattern = pattern_of(&callback, None).unwrap();
        let cells = InputCells::new();
        let result = query_traced(
            &pattern,
            TimeSpan::cycle(0).unwrap(),
            &mut QueryCtx::new(&mut vm, &cells, 42),
        );
        assert!(result.events.is_empty());
        assert_eq!(result.faults.len(), 1);
        assert_eq!(result.faults[0].code, FailCode::Type);
        assert!(result.faults[0].message.contains("finite song"));
    }
}

#[cfg(test)]
mod song_context_tests {
    use super::*;
    use crate::pattern::combinators::{control::control, time::slow};
    use crate::pattern::pat::PatNode;
    use crate::pattern::step::Step;
    use crate::pattern::tests::stub_vm::StubVm;
    use crate::song::SongLimits;
    use crate::value::intern::intern_kw;
    use crate::value::value::Sound;

    fn dynamic_pattern(vm: &mut StubVm) -> Pat {
        let callable = vm.define(911, |args| {
            let Value::Ratio(at) = args[0] else {
                panic!("time argument")
            };
            let name = if at == Ratio64::ZERO {
                "piano"
            } else {
                "pluck"
            };
            let subject = Rc::new(Pat::new(
                PatNode::Pure(Step::bare(Value::Sound(Rc::new(Sound::Builtin(
                    intern_kw(name),
                ))))),
                None,
                false,
            ));
            let note = Rc::new(Pat::new(
                PatNode::Pure(Step::bare(Value::Float64(60.25 + 12.0 * at.to_f64()))),
                None,
                false,
            ));
            Ok(Value::Pattern(Rc::new(control(
                intern_kw("note"),
                note,
                subject,
                None,
            ))))
        });
        slow(
            Rc::new(steps(vec![Step::bare(callable)].into_boxed_slice(), None)),
            PParam::Const(Value::Int(4)),
            None,
        )
    }
    fn run(pattern: &Pat, begin: i64, song: bool, traced: bool, vm: &mut StubVm) -> Event {
        let cells = InputCells::new();
        let mut cx = QueryCtx::new(vm, &cells, 77);
        let mut state = if song {
            QState::new_song(&mut cx, SongLimits::default()).unwrap()
        } else if traced {
            QState::new_traced(&mut cx)
        } else {
            QState::new(&mut cx)
        };
        let events = q(pattern, TimeSpan::cycle(begin).unwrap(), &mut state);
        assert!(state.faults.is_empty(), "{:?}", state.faults);
        assert_eq!(events.len(), 1);
        events.into_iter().next().unwrap()
    }
    #[test]
    fn song_continuations_sample_intrinsic_whole_onset_and_keep_payload() {
        let mut vm = StubVm::new();
        let pattern = dynamic_pattern(&mut vm);
        let first = run(&pattern, 0, true, true, &mut vm);
        for begin in [1, 2, 3] {
            let continuation = run(&pattern, begin, true, true, &mut vm);
            assert!(crate::value::eq::deep_eq(&first.value, &continuation.value).unwrap());
            assert!(crate::value::eq::deep_eq(
                &first.controls[&intern_kw("note")],
                &continuation.controls[&intern_kw("note")]
            )
            .unwrap());
            assert_eq!(first.whole, continuation.whole);
            assert_eq!(first.producer, continuation.producer);
        }
        assert!(matches!(
            first.controls[&intern_kw("note")],
            Value::Float64(60.25)
        ));
    }
    #[test]
    fn legacy_traced_and_untraced_dynamic_sampling_remains_clipped() {
        for traced in [false, true] {
            let mut vm = StubVm::new();
            let pattern = dynamic_pattern(&mut vm);
            let first = run(&pattern, 0, false, traced, &mut vm);
            let continuation = run(&pattern, 1, false, traced, &mut vm);
            assert!(!crate::value::eq::deep_eq(&first.value, &continuation.value).unwrap());
            assert!(matches!(
                continuation.controls[&intern_kw("note")],
                Value::Float64(63.25)
            ));
        }
    }
    #[test]
    fn nested_sampling_inherits_exact_limits_and_shared_work_depth_counters() {
        let mut vm = StubVm::new();
        let cells = InputCells::new();
        let mut cx = QueryCtx::new(&mut vm, &cells, 7);
        let limits = SongLimits {
            max_depth: 2,
            max_nodes: 99,
            max_tracks: 3,
            max_cached_events: 5,
            max_frames: 1234,
        };
        let mut state = QState::new_song(&mut cx, limits).unwrap();
        state.enter().unwrap();
        state.spend(QUERY_BUDGET - 1, None).unwrap();
        state
            .reenter(|nested| {
                assert_eq!(nested.song_limits(), Some(limits));
                nested.enter()?;
                assert_eq!(nested.enter().unwrap_err().code, FailCode::DepthExceeded);
                nested.leave();
                nested.spend(1, None)
            })
            .unwrap();
        assert_eq!(state.depth, 1);
        assert_eq!(state.reentry, 0);
        assert_eq!(
            state.spend(1, None).unwrap_err().code,
            FailCode::FuelExhausted
        );
        assert_eq!(state.song_limits(), Some(limits));
    }
    #[test]
    fn real_nested_point_sampling_uses_existing_budget_and_limits() {
        let mut vm = StubVm::new();
        let cells = InputCells::new();
        let mut cx = QueryCtx::new(&mut vm, &cells, 1);
        let limits = SongLimits {
            max_depth: 2,
            max_cached_events: 3,
            ..SongLimits::default()
        };
        let mut state = QState::new_song(&mut cx, limits).unwrap();
        let pattern = Pat::new(PatNode::Pure(Step::bare(Value::Int(7))), None, false);
        state.spend(QUERY_BUDGET - 1, None).unwrap();
        state.enter().unwrap();
        assert_eq!(state.sample(&pattern, Ratio64::ZERO).unwrap().len(), 1);
        assert_eq!(state.song_limits(), Some(limits));
        assert_eq!(state.depth, 1);
        assert_eq!(state.reentry, 0);
        assert_eq!(state.budget, 0);
        assert_eq!(
            state.sample(&pattern, Ratio64::ZERO).unwrap_err().code,
            FailCode::FuelExhausted
        );
        assert_eq!(state.depth, 1);
        assert_eq!(state.reentry, 0);
    }
    #[test]
    fn song_context_rejects_invalid_limits_without_default_substitution() {
        let mut vm = StubVm::new();
        let cells = InputCells::new();
        let mut cx = QueryCtx::new(&mut vm, &cells, 1);
        let limits = SongLimits {
            max_depth: 300,
            ..SongLimits::default()
        };
        assert!(matches!(
            QState::new_song(&mut cx, limits),
            Err(Failure {
                code: FailCode::Type,
                ..
            })
        ));
    }
    #[test]
    fn song_query_depth_200_succeeds_and_300_fails_explicitly() {
        for depth in [200, 300] {
            let mut value = Value::Int(1);
            for _ in 0..depth {
                value = Value::list(vec![value]);
            }
            let pattern = steps(vec![Step::bare(value)].into_boxed_slice(), None);
            let mut vm = StubVm::new();
            let cells = InputCells::new();
            let mut cx = QueryCtx::new(&mut vm, &cells, 1);
            let mut state = QState::new_song(&mut cx, SongLimits::default()).unwrap();
            let events = q(&pattern, TimeSpan::cycle(0).unwrap(), &mut state);
            if depth == 200 {
                assert_eq!(events.len(), 1);
                assert!(state.faults.is_empty());
            } else {
                assert!(events.is_empty());
                assert_eq!(state.faults.len(), 1);
                assert_eq!(state.faults[0].code, FailCode::DepthExceeded);
            }
        }
    }
}

#[cfg(test)]
mod song_sound_depth_tests {
    use super::*;
    use crate::pattern::combinators::sound::sound;
    use crate::pattern::pat::PatNode;
    use crate::pattern::step::Step;
    use crate::pattern::tests::stub_vm::StubVm;
    use crate::song::SongLimits;
    use crate::value::intern::intern_kw;
    use crate::value::value::Sound;
    #[test]
    fn callable_structured_sound_200_succeeds_300_fails_without_stack_changes() {
        for depth in [200, 300] {
            let mut child = Pat::new(
                PatNode::Pure(Step::bare(Value::Sound(Rc::new(Sound::Builtin(
                    intern_kw("piano"),
                ))))),
                None,
                true,
            );
            for _ in 0..depth {
                child = sound(PParam::Pat(Rc::new(child)), None, None);
            }
            let mut vm = StubVm::new();
            let callable = vm.define(912, move |_| Ok(Value::Pattern(Rc::new(child.clone()))));
            let pattern = sound(PParam::Fn(callable), None, None);
            let cells = InputCells::new();
            let mut cx = QueryCtx::new(&mut vm, &cells, 1);
            let mut state = QState::new_song(&mut cx, SongLimits::default()).unwrap();
            let events = q(&pattern, TimeSpan::cycle(0).unwrap(), &mut state);
            if depth == 200 {
                assert_eq!(events.len(), 1);
                assert!(state.faults.is_empty());
            } else {
                assert!(events.is_empty());
                assert_eq!(state.faults.len(), 1);
                assert_eq!(state.faults[0].code, FailCode::DepthExceeded);
            }
        }
    }
}

#[cfg(test)]
mod producer_length_tests {
    use super::*;
    use crate::pattern::tests::stub_vm::StubVm;
    use crate::song::SongLimits;

    #[test]
    fn prefix_length_is_read_only_for_legacy_traced_and_song_scopes() {
        for kind in 0..3 {
            let mut vm = StubVm::new();
            let cells = InputCells::new();
            let mut cx = QueryCtx::new(&mut vm, &cells, 3);
            let mut state = match kind {
                0 => QState::new(&mut cx),
                1 => QState::new_traced(&mut cx),
                _ => QState::new_song(&mut cx, SongLimits::default()).unwrap(),
            };
            let expected = |length| if kind == 0 { None } else { Some(length) };
            let budget = state.budget;
            assert_eq!(state.producer_len(), expected(0));
            for ordinal in 0..8 {
                state.push_producer(ProducerKind::Child, ordinal);
                assert_eq!(state.producer_len(), expected(ordinal as usize + 1));
            }
            let before = state.producer();
            assert_eq!(state.producer_len(), expected(8));
            assert_eq!(state.producer(), before);
            state.with_producer(ProducerKind::DynamicExpansion, 0, |nested| {
                assert_eq!(nested.producer_len(), expected(9));
            });
            assert_eq!(state.producer_len(), expected(8));
            assert_eq!(state.producer(), before);
            for remaining in (0..8).rev() {
                state.pop_producer();
                assert_eq!(state.producer_len(), expected(remaining));
            }
            assert_eq!(state.budget, budget);
            assert_eq!(state.depth, 0);
            assert_eq!(state.reentry, 0);
            assert!(state.faults.is_empty());
        }
    }
}
