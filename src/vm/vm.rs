//! The interpreter loop (design 8.3, 8.4, 10.4).
//!
//! `Vm::run` executes a top-level proto. Calls between bytecode functions
//! push frames in the same loop; forcing a thunk at a call boundary pushes
//! the thunk's frame and resumes the pending call when it returns, so
//! neither costs Rust recursion. Only a native calling back into a closure
//! re-enters (`call_value`), bounded by `reentry_limit`.

use std::any::Any;
use std::rc::Rc;

use crate::compile::proto::{Closure, FnProto, ItemKind};
use crate::ns::insts::DspCx;
use crate::ns::namespace::{slot_id_mark, Namespace, SlotKind, VarSlotRef};
use crate::ns::stage::EffectBuffer;
use crate::reader::span::Span;
use crate::types::masks::ForcingMask;
use crate::value::access::{get, len, truthy};
use crate::value::dict::pairs;
use crate::value::eq::deep_eq;
use crate::value::intern::{name_of_sym, KwId};
use crate::value::key::Key;
use crate::value::value::{ListVal, StructVal, Value, VariantVal};
use crate::vm::fail::{FailCode, Failure, Origin};
use crate::vm::frame::{Frame, Pending, RetTo};
use crate::vm::ops::Op;

/// Default fuel per top-level evaluation (7.1.5).
pub const DEFAULT_FUEL: u64 = 1_000_000;
/// Default frame depth limit (7.1.5).
pub const DEFAULT_DEPTH_LIMIT: usize = 512;
/// Default nested Rust-level re-entry limit (7.1.5).
pub const DEFAULT_REENTRY_LIMIT: u32 = 64;
/// Default Rust stack budget in bytes for nested re-entries. A re-entry
/// past it fails `depth-exceeded` instead of overflowing the thread's stack
/// (wasm32 threads have 1 MiB, test threads 2 MiB).
pub const DEFAULT_STACK_BUDGET: usize = 768 * 1024;

/// Whether effects are allowed (10.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectMode {
    Normal,
    /// Global writes, slot binds, scheduling and host sends fail with
    /// `effect-in-query`; `print` is captured; frame-local state is legal.
    Query,
}

/// Observes eager reads of top-level slots (5.6 `Deref` instrumentation).
/// The reactive layer records edges here and may abort a read (a dirty
/// read of a scheduled or failed owner) by returning a failure.
pub trait ReadObserver {
    /// Called before the value of `slot` is consumed.
    ///
    /// # Errors
    /// A failure aborts the evaluation (for example `blocked`).
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure>;
}

/// What the loop does next.
pub(crate) enum Flow {
    Continue,
    Return(Value),
}

/// The bytecode VM.
pub struct Vm {
    pub(crate) stack: Vec<Value>,
    pub(crate) frames: Vec<Frame>,
    pub(crate) pending: Vec<Pending>,
    pub(crate) fuel: u64,
    pub fuel_limit: u64,
    pub depth_limit: usize,
    pub reentry_limit: u32,
    /// Bytes of Rust stack nested re-entries may use (7.1.5).
    pub stack_budget: usize,
    pub(crate) reentries: u32,
    pub(crate) effect_mode: EffectMode,
    pub(crate) effects: EffectBuffer,
    pub(crate) output: Vec<(Origin, Rc<str>)>,
    pub(crate) observer: Option<Box<dyn ReadObserver>>,
    pub(crate) stack_base: Option<usize>,
    host: Option<Box<dyn Any>>,
    song_work: Option<crate::pattern::eval::song_observation::SharedIndexWork>,
    song_frame_floor: usize,
    /// In Query mode, local cells with ids below this outlive the query.
    query_cells_from: u64,
    /// The instrument registry and the `inst`/`bus` body depth (12.8.6).
    pub dsp: DspCx,
}

impl Default for Vm {
    fn default() -> Self {
        Vm::new()
    }
}

impl std::fmt::Debug for Vm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vm")
            .field("frames", &self.frames.len())
            .field("fuel", &self.fuel)
            .field("effect_mode", &self.effect_mode)
            .finish_non_exhaustive()
    }
}

impl Vm {
    /// A VM with the 7.1.5 default limits.
    #[must_use]
    pub fn new() -> Vm {
        Vm {
            stack: Vec::new(),
            frames: Vec::new(),
            pending: Vec::new(),
            fuel: DEFAULT_FUEL,
            fuel_limit: DEFAULT_FUEL,
            depth_limit: DEFAULT_DEPTH_LIMIT,
            reentry_limit: DEFAULT_REENTRY_LIMIT,
            stack_budget: DEFAULT_STACK_BUDGET,
            reentries: 0,
            effect_mode: EffectMode::Normal,
            effects: EffectBuffer::default(),
            output: Vec::new(),
            observer: None,
            stack_base: None,
            host: None,
            song_work: None,
            song_frame_floor: 0,
            query_cells_from: 0,
            dsp: DspCx::default(),
        }
    }

    /// The current effect mode.
    #[must_use]
    pub fn effect_mode(&self) -> EffectMode {
        self.effect_mode
    }

    /// Runs `f` in `mode` and restores the previous mode afterwards, also
    /// when `f` fails (the 10.4 scope guard).
    ///
    /// Entering Query mode also marks which local `var` cells belong to the
    /// query: only cells created inside it may be updated (frame-local
    /// state); a cell captured from outside is state that outlives it.
    pub fn with_effect_mode<R>(&mut self, mode: EffectMode, f: impl FnOnce(&mut Vm) -> R) -> R {
        let saved = (self.effect_mode, self.query_cells_from);
        if mode == EffectMode::Query && saved.0 != EffectMode::Query {
            self.query_cells_from = slot_id_mark();
        }
        self.effect_mode = mode;
        let r = f(self);
        (self.effect_mode, self.query_cells_from) = saved;
        r
    }

    /// Remaining fuel.
    #[must_use]
    pub fn fuel(&self) -> u64 {
        self.fuel
    }

    /// Sets the remaining fuel (a `QueryVm` call gets a fresh budget).
    pub fn set_fuel(&mut self, fuel: u64) {
        self.fuel = fuel;
    }

    /// The staged effects of the forms run so far.
    #[must_use]
    pub fn effects(&self) -> &EffectBuffer {
        &self.effects
    }

    /// The staging buffer, for release or pass-level staging.
    pub fn effects_mut(&mut self) -> &mut EffectBuffer {
        &mut self.effects
    }

    /// Takes the `print` output captured in Query mode.
    pub fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        std::mem::take(&mut self.output)
    }

    /// Replaces the captured `print` output (10.4): a filter drops the
    /// output of the events it removes rather than forwarding it.
    pub fn put_output(&mut self, out: Vec<(Origin, Rc<str>)>) {
        self.output = out;
    }

    /// Installs (or removes) the eager-read observer.
    pub fn set_read_observer(&mut self, observer: Option<Box<dyn ReadObserver>>) {
        self.observer = observer;
    }

    /// Takes the eager-read observer back.
    pub fn take_read_observer(&mut self) -> Option<Box<dyn ReadObserver>> {
        self.observer.take()
    }

    /// Installs a host capability for natives that need one (the
    /// ME-REACTIVE `SourceLoader` behind `load`); returns the previous one.
    /// A native takes it with `take_host`, uses it, and puts it back.
    pub fn set_host(&mut self, host: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        std::mem::replace(&mut self.host, host)
    }

    /// Takes the host capability out (see `set_host`).
    pub fn take_host(&mut self) -> Option<Box<dyn Any>> {
        self.host.take()
    }

    /// Runs a top-level proto (8.3). A top-level evaluation starts with
    /// `fuel_limit`; a nested run (a native re-entering) keeps the current
    /// budget and counts as a re-entry. On failure the effects this run
    /// staged are dropped and the VM is unwound to where it started.
    ///
    /// # Errors
    /// Any runtime failure, with its origin.
    pub fn run(&mut self, proto: Rc<FnProto>, ns: &Namespace) -> Result<Value, Failure> {
        let nested = !self.frames.is_empty();
        if nested {
            self.enter()?;
        } else {
            if self.song_work.is_none() {
                self.fuel = self.fuel_limit;
            }
            self.stack_base = Some(stack_addr());
        }
        let mark = self.effects.len();
        let closure = Rc::new(Closure {
            proto,
            captures: Box::new([]),
            mask: ForcingMask::default(),
            memo: None,
        });
        let floors = self.floors();
        let r = self
            .push_frame(closure, Vec::new(), RetTo::Stop, None)
            .and_then(|()| self.drive(ns, floors, Flow::Continue));
        if nested {
            self.reentries = self.reentries.saturating_sub(1);
        }
        if r.is_err() {
            self.effects.truncate(mark);
        }
        r
    }

    pub(crate) fn enter(&mut self) -> Result<(), Failure> {
        let deep = self
            .stack_base
            .is_some_and(|base| base.abs_diff(stack_addr()) > self.stack_budget);
        if deep || self.reentries >= self.reentry_limit {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "too many nested re-entries",
            ));
        }
        self.reentries += 1;
        Ok(())
    }

    pub(crate) fn floors(&self) -> (usize, usize, usize) {
        (self.frames.len(), self.pending.len(), self.stack.len())
    }

    /// Runs the loop until the innermost `Stop` delivers, unwinding to
    /// `floors` on failure.
    pub(crate) fn drive(
        &mut self,
        ns: &Namespace,
        floors: (usize, usize, usize),
        first: Flow,
    ) -> Result<Value, Failure> {
        let mut flow = first;
        loop {
            match flow {
                Flow::Return(v) => return Ok(v),
                Flow::Continue => {}
            }
            flow = match self.step(ns) {
                Ok(f) => f,
                Err(mut e) => {
                    if e.origin.span.is_none() {
                        e.origin.span = self.current_span();
                    }
                    self.frames.truncate(floors.0);
                    self.pending.truncate(floors.1);
                    self.stack.truncate(floors.2);
                    return Err(e);
                }
            };
        }
    }

    /// The span of the op the top frame is executing.
    pub(crate) fn current_span(&self) -> Option<Span> {
        let f = self.frames.last()?;
        Some(f.closure.proto.span_at(f.ip.saturating_sub(1)))
    }

    fn pop(&mut self) -> Result<Value, Failure> {
        self.stack.pop().ok_or_else(internal)
    }

    fn pop_n(&mut self, n: usize) -> Result<Vec<Value>, Failure> {
        let at = self.stack.len().checked_sub(n).ok_or_else(internal)?;
        Ok(self.stack.split_off(at))
    }

    fn frame(&self) -> Result<&Frame, Failure> {
        self.frames.last().ok_or_else(internal)
    }

    pub(crate) fn song_work(
        &self,
    ) -> Option<crate::pattern::eval::song_observation::SharedIndexWork> {
        self.song_work.clone()
    }

    pub(crate) fn song_query_depth(&self) -> Result<u32, Failure> {
        if self.song_work.is_none() {
            return Ok(0);
        }
        let base = self
            .song_work
            .as_ref()
            .map_or(0, |work| work.borrow().depth);
        u32::try_from(self.frames.len().saturating_sub(self.song_frame_floor))
            .ok()
            .and_then(|frames| base.checked_add(frames))
            .ok_or_else(|| Failure::new(FailCode::DepthExceeded, "canonical VM depth overflow"))
    }

    pub(crate) fn with_song_query_frames<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let saved = self.song_frame_floor;
        self.song_frame_floor = self.frames.len();
        let result = f(self);
        self.song_frame_floor = saved;
        result
    }

    pub(crate) fn with_song_work<T>(
        &mut self,
        work: crate::pattern::eval::song_observation::SharedIndexWork,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if let Some(current) = &self.song_work {
            if !Rc::ptr_eq(current, &work) {
                return Err(Failure::new(FailCode::Type, "foreign canonical VM ledger"));
            }
            return f(self);
        }
        let (budget, depth) = {
            let ledger = work.borrow();
            let depth = ledger
                .limits
                .max_depth
                .checked_sub(ledger.depth)
                .filter(|d| *d > 0)
                .ok_or_else(|| {
                    Failure::new(
                        FailCode::DepthExceeded,
                        "canonical VM inherited depth exhausted",
                    )
                })?;
            (
                u64::from(ledger.remaining()).min(self.fuel_limit),
                depth as usize,
            )
        };
        let saved = (self.fuel, self.depth_limit);
        self.fuel = budget;
        self.depth_limit = self.depth_limit.min(depth);
        self.song_work = Some(work);
        let result = f(self);
        self.song_work = None;
        self.fuel = saved.0;
        self.depth_limit = saved.1;
        result
    }

    /// Consumes one unit of fuel.
    pub(crate) fn tick(&mut self) -> Result<(), Failure> {
        if self.fuel == 0 {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "the evaluation ran out of fuel (an unbounded source?)",
            ));
        }
        if let Some(work) = &self.song_work {
            if self.song_query_depth()? > work.borrow().limits.max_depth {
                return Err(Failure::new(
                    FailCode::DepthExceeded,
                    "canonical VM frame depth exhausted",
                ));
            }
            let mut ledger = work.borrow_mut();
            ledger.charge(1)?;
            ledger.vm_instructions = ledger.vm_instructions.checked_add(1).ok_or_else(|| {
                Failure::new(FailCode::Overflow, "canonical instruction total overflow")
            })?;
        }
        self.fuel -= 1;
        Ok(())
    }

    /// Executes one op of the top frame.
    ///
    /// The control ops (calls, forcing, return) are handled here and keep
    /// this frame small: it stays live across every native re-entry, so its
    /// size bounds how deep re-entry can go on a given stack.
    fn step(&mut self, ns: &Namespace) -> Result<Flow, Failure> {
        self.tick()?;
        let (op, base) = {
            let f = self.frames.last_mut().ok_or_else(internal)?;
            let op = *f.closure.proto.code.get(f.ip).ok_or_else(internal)?;
            f.ip += 1;
            (op, f.base)
        };
        match op {
            Op::Call(n) => {
                let args = self.pop_n(usize::from(n))?;
                let callee = self.pop()?;
                let span = self.current_span();
                self.begin_call(ns, callee, args, Vec::new(), RetTo::Stack, span)
            }
            Op::CallKw(s) => self.call_kw(ns, s),
            Op::Force => {
                let v = self.pop()?;
                self.force_value(ns, v, RetTo::Stack)
            }
            Op::Ret => {
                let v = self.pop()?;
                let f = self.frames.pop().ok_or_else(internal)?;
                self.stack.truncate(f.base);
                if let Some(cell) = &f.memo {
                    if let Some(m) = &cell.memo {
                        *m.borrow_mut() = Some(v.clone());
                    }
                }
                self.deliver(ns, v, f.ret)
            }
            _ => {
                self.simple(ns, op, base)?;
                Ok(Flow::Continue)
            }
        }
    }

    /// Executes a non-control op.
    #[inline(never)]
    fn simple(&mut self, ns: &Namespace, op: Op, base: usize) -> Result<(), Failure> {
        match op {
            Op::LoadConst(k) => {
                let v = self.konst(k)?;
                self.stack.push(v);
            }
            Op::LoadLocal(i) => {
                let v = self
                    .stack
                    .get(base + usize::from(i))
                    .cloned()
                    .ok_or_else(internal)?;
                self.stack.push(v);
            }
            Op::StoreLocal(i) => {
                let v = self.pop()?;
                *self
                    .stack
                    .get_mut(base + usize::from(i))
                    .ok_or_else(internal)? = v;
            }
            Op::LoadGlobal(g) => {
                let slot = self.global(g)?;
                let v = self.read_slot(&slot)?;
                self.stack.push(v);
            }
            Op::LoadGlobalRef(g) | Op::LoadTweak(g) => {
                let slot = self.global(g)?;
                if !slot.is_bound() {
                    return Err(Failure::new(
                        FailCode::UndefinedName,
                        format!("`{}` is not defined", name_of_sym(slot.name())),
                    ));
                }
                self.stack.push(Value::VarRef(slot));
            }
            Op::DefGlobal(d) => {
                self.check_effect("defining a global")?;
                let v = self.pop()?;
                let (slot, kind, gen) = {
                    let p = &self.frame()?.closure.proto;
                    let (g, kind) = *p.defs.get(d as usize).ok_or_else(internal)?;
                    let slot = p.globals.get(g as usize).cloned().ok_or_else(internal)?;
                    (slot, kind, p.form_gen)
                };
                ns.define_slot(&slot, kind, v.clone(), gen);
                self.stack.push(v);
            }
            Op::UpdGlobal(g) => {
                let slot = self.global(g)?;
                if slot.kind() == SlotKind::Var && slot.is_bound() {
                    self.check_effect("`upd` of a global")?;
                }
                let v = self.pop()?;
                ns.write_var(&slot, v.clone())?;
                self.stack.push(v);
            }
            Op::LoadCapture(i) => {
                let v = self.frame()?.closure.captures.get(usize::from(i)).cloned();
                self.stack.push(v.ok_or_else(internal)?);
            }
            Op::UpdCell => {
                let v = self.pop()?;
                let cell = self.pop()?;
                match cell {
                    Value::VarRef(slot) if slot.is_local() => {
                        if slot.id() < self.query_cells_from {
                            self.check_effect("`upd` of a var captured from outside the query")?;
                        }
                        slot.set(v.clone());
                    }
                    _ => {
                        return Err(Failure::new(
                            FailCode::UpdImmutable,
                            "only a var can be updated",
                        ))
                    }
                }
                self.stack.push(v);
            }
            Op::MakeCell => {
                let v = self.pop()?;
                let name = self.frame()?.closure.proto.name;
                let sym = name.unwrap_or_else(|| crate::value::intern::intern_sym("var"));
                self.stack
                    .push(Value::VarRef(VarSlotRef::local_cell(sym, v)));
            }
            Op::MakeList(s) => self.make_list(s)?,
            Op::MakeDict(n) => {
                let vals = self.pop_n(usize::from(n) * 2)?;
                let mut map = std::collections::BTreeMap::new();
                for kv in vals.chunks(2) {
                    let key = Key::from_value(&shallow(&kv[0]))?;
                    map.insert(key, kv[1].clone());
                }
                self.stack.push(Value::dict(map));
            }
            Op::MakeClosure(p) | Op::MakeThunk(p) => {
                let (sub, mask) = {
                    let proto = &self.frame()?.closure.proto;
                    let sub = proto
                        .protos
                        .get(usize::from(p))
                        .cloned()
                        .ok_or_else(internal)?;
                    let mask = proto.masks.get(usize::from(p)).cloned().unwrap_or_default();
                    (sub, mask)
                };
                let n = usize::from(sub.captures) + usize::from(sub.arity.keys);
                let captures = self.pop_n(n)?.into_boxed_slice();
                let c = Rc::new(Closure {
                    proto: sub,
                    captures,
                    mask,
                    memo: None,
                });
                self.stack.push(if matches!(op, Op::MakeThunk(_)) {
                    Value::Thunk(c)
                } else {
                    Value::Fn(c)
                });
            }
            Op::MakeShape(s) => {
                let shape = self
                    .frame()?
                    .closure
                    .proto
                    .shapes
                    .get(usize::from(s))
                    .cloned();
                let shape = shape.ok_or_else(internal)?;
                let vals = self.pop_n(shape.fields.len())?;
                let mut fields: Vec<(KwId, Value)> =
                    shape.fields.iter().copied().zip(vals).collect();
                fields.sort_by(|a, b| {
                    crate::value::intern::name_of_kw(a.0)
                        .cmp(&crate::value::intern::name_of_kw(b.0))
                });
                let fields = fields.into_boxed_slice();
                self.stack.push(if shape.is_struct {
                    Value::Struct(Rc::new(StructVal {
                        ty: shape.ty,
                        fields,
                    }))
                } else {
                    Value::Variant(Rc::new(VariantVal {
                        enum_ty: shape.ty,
                        tag: shape.tag,
                        fields,
                    }))
                });
            }
            Op::CallValue(n) => {
                let args = self.pop_n(usize::from(n))?;
                let callee = self.pop()?;
                let v = self.call_collection(ns, &callee, &args)?;
                self.stack.push(v);
            }
            Op::Deref => {
                let v = self.pop()?;
                let v = match v {
                    Value::VarRef(slot) => self.read_slot(&slot)?,
                    other => other,
                };
                self.stack.push(v);
            }
            Op::Jump(off) => self.jump(off)?,
            Op::JumpIfNoMatch(off) => {
                if !self.frame()?.flag {
                    self.jump(off)?;
                }
            }
            Op::TestLit(k) => {
                let lit = self.konst(k)?;
                let v = self.pop()?;
                let ok = deep_eq(&shallow(&v), &lit).unwrap_or(false);
                self.set_flag(ok)?;
            }
            Op::TestVariant(s) | Op::TestShape(s) => {
                let shape = self
                    .frame()?
                    .closure
                    .proto
                    .shapes
                    .get(usize::from(s))
                    .cloned();
                let shape = shape.ok_or_else(internal)?;
                let v = shallow(&self.pop()?);
                let ok = match &v {
                    Value::Variant(x) => {
                        !shape.is_struct && x.tag == shape.tag && x.enum_ty == shape.ty
                    }
                    Value::Struct(x) => shape.is_struct && x.ty == shape.ty,
                    _ => false,
                };
                self.set_flag(ok)?;
            }
            Op::TestLen(n) | Op::TestLenMin(n) => {
                let v = shallow(&self.pop()?);
                let size = match &v {
                    Value::List(_) | Value::Dict(_) => len(&v).ok(),
                    Value::Range(r) => match r.end {
                        Some(_) => len(&v).ok(),
                        None => Some(usize::MAX),
                    },
                    _ => None,
                };
                let ok = match (op, size) {
                    (Op::TestLen(_), Some(k)) => k == usize::from(n),
                    (_, Some(k)) => k >= usize::from(n),
                    (_, None) => false,
                };
                self.set_flag(ok)?;
            }
            Op::TestKey(k) => {
                let key = self.konst(k)?;
                let v = shallow(&self.pop()?);
                let ok = match (&v, &key) {
                    (Value::Dict(d), _) => Key::from_value(&key).is_ok_and(|k| d.contains_key(&k)),
                    (Value::Struct(s), Value::Keyword(kw)) => s.fields.iter().any(|(f, _)| f == kw),
                    (Value::Variant(s), Value::Keyword(kw)) => {
                        s.fields.iter().any(|(f, _)| f == kw)
                    }
                    _ => false,
                };
                self.set_flag(ok)?;
            }
            Op::TestTruthy => {
                let v = shallow(&self.pop()?);
                self.set_flag(truthy(&v))?;
            }
            Op::SplitRest(n) => {
                let v = shallow(&self.pop()?);
                let rest = split_rest(&v, usize::from(n))?;
                self.stack.push(rest);
            }
            Op::BindField(i) => {
                let v = shallow(&self.pop()?);
                let item = crate::value::access::index(&v, &Value::Int(i32::from(i)))?;
                self.stack.push(item);
            }
            Op::GetKey(k) => {
                let key = self.konst(k)?;
                let v = shallow(&self.pop()?);
                self.stack.push(get(&v, &key)?);
            }
            Op::Fail(c) => {
                let code = FailCode::ALL
                    .get(usize::from(c))
                    .copied()
                    .unwrap_or(FailCode::NoMatch);
                return Err(Failure::new(code, "no clause matched"));
            }
            Op::Pop => {
                self.pop()?;
            }
            Op::Dup => {
                let v = self.stack.last().cloned().ok_or_else(internal)?;
                self.stack.push(v);
            }
            Op::Call(_) | Op::CallKw(_) | Op::Force | Op::Ret => return Err(internal()),
        }
        Ok(())
    }

    fn konst(&self, k: u16) -> Result<Value, Failure> {
        self.frame()?
            .closure
            .proto
            .consts
            .get(usize::from(k))
            .cloned()
            .ok_or_else(internal)
    }

    fn global(&self, g: u32) -> Result<VarSlotRef, Failure> {
        self.frame()?
            .closure
            .proto
            .globals
            .get(g as usize)
            .cloned()
            .ok_or_else(internal)
    }

    fn set_flag(&mut self, ok: bool) -> Result<(), Failure> {
        self.frames.last_mut().ok_or_else(internal)?.flag = ok;
        Ok(())
    }

    fn jump(&mut self, off: i32) -> Result<(), Failure> {
        let f = self.frames.last_mut().ok_or_else(internal)?;
        let ip = i64::try_from(f.ip).map_err(|_| internal())? + i64::from(off);
        f.ip = usize::try_from(ip).map_err(|_| internal())?;
        Ok(())
    }

    fn make_list(&mut self, s: u16) -> Result<(), Failure> {
        let site = self
            .frame()?
            .closure
            .proto
            .list_sites
            .get(usize::from(s))
            .cloned();
        let site = site.ok_or_else(internal)?;
        let vals = self.pop_n(site.items.len())?;
        let mut items = Vec::with_capacity(vals.len());
        for (kind, v) in site.items.iter().zip(vals) {
            match kind {
                ItemKind::Item => items.push(v),
                ItemKind::Splat => {
                    for x in spread(self, &shallow(&v))? {
                        items.push(x);
                    }
                }
            }
        }
        self.stack.push(Value::List(Rc::new(ListVal {
            items: items.into_boxed_slice(),
            prov: site.prov,
        })));
        Ok(())
    }
}

/// An internal VM invariant broke. Reported as a failure, never a panic.
pub(crate) fn internal() -> Failure {
    Failure::new(FailCode::Type, "internal VM error")
}

/// Derefs a `VarRef` without recording a read (pattern tests and
/// accessors on already-demanded structure).
pub(crate) fn shallow(v: &Value) -> Value {
    let mut v = v.clone();
    for _ in 0..8 {
        match v {
            Value::VarRef(slot) => v = slot.get(),
            other => return other,
        }
    }
    v
}

/// The items a splat contributes: list items, dict pairs, range elements
/// (fuel-metered); `nil` contributes nothing.
pub(crate) fn spread(vm: &mut Vm, v: &Value) -> Result<Vec<Value>, Failure> {
    match v {
        Value::Nil => Ok(Vec::new()),
        Value::List(l) => Ok(l.items.to_vec()),
        Value::Dict(d) => Ok(pairs(d).collect()),
        Value::Range(r) => {
            let mut out = Vec::new();
            let mut k = r.start;
            while r.end.is_none_or(|e| k < e) {
                vm.tick()?;
                out.push(int_value(k));
                k = k
                    .checked_add(1)
                    .ok_or_else(|| Failure::new(FailCode::Overflow, "range overflow"))?;
            }
            Ok(out)
        }
        _ => Err(Failure::new(FailCode::Type, "`&` expects a list or dict")),
    }
}

/// An integer as `Int` when it fits, else `Int64`.
pub(crate) fn int_value(n: i64) -> Value {
    match i32::try_from(n) {
        Ok(i) => Value::Int(i),
        Err(_) => Value::Int64(n),
    }
}

fn split_rest(v: &Value, n: usize) -> Result<Value, Failure> {
    match v {
        Value::List(l) => Ok(Value::list(l.items.iter().skip(n).cloned().collect())),
        Value::Dict(d) => Ok(Value::list(pairs(d).skip(n).collect())),
        Value::Range(r) => {
            let k = i64::try_from(n).map_err(|_| internal())?;
            let start = r
                .start
                .checked_add(k)
                .ok_or_else(|| Failure::new(FailCode::Overflow, "range overflow"))?;
            Ok(Value::Range(crate::value::value::RangeVal {
                start: r.end.map_or(start, |e| start.min(e)),
                end: r.end,
            }))
        }
        _ => Err(Failure::new(
            FailCode::Type,
            "`&` in a pattern expects a list",
        )),
    }
}

/// The address of a local in a fresh frame: an estimate of the current
/// stack position (the stack budget, 7.1.5).
#[inline(never)]
pub(crate) fn stack_addr() -> usize {
    let probe = 0u8;
    std::hint::black_box(std::ptr::addr_of!(probe)) as usize
}
