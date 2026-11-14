//! Frames and pending calls (design 8.3, 5.5).

use std::rc::Rc;

use crate::compile::proto::Closure;
use crate::ns::namespace::{Namespace, SlotKind, VarSlotRef};
use crate::reader::span::Span;
use crate::types::masks::EffectiveEntry;
use crate::value::intern::{name_of_sym, KwId};
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};
use crate::vm::vm::{internal, EffectMode, Flow, Vm};

/// Where a returning frame's value goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RetTo {
    /// Onto the caller frame's operand stack.
    Stack,
    /// Onto the caller's stack, then forced again (`Force` of a thunk
    /// whose value is itself a thunk or a `VarRef`).
    Force,
    /// Into the argument being forced at the top pending call boundary.
    PendingArg,
    /// Out of the current `drive` (a top-level run or a native re-entry).
    Stop,
}

/// One activation. Locals live at `stack[base..base + locals]`; the
/// operand stack is above them.
#[derive(Debug)]
pub struct Frame {
    pub closure: Rc<Closure>,
    pub ip: usize,
    pub base: usize,
    pub ret: RetTo,
    /// The memoizing cell to fill when this thunk frame returns.
    pub memo: Option<Rc<Closure>>,
    /// The match flag the pattern ops set.
    pub flag: bool,
}

/// A call whose arguments are being forced at its boundary (5.5): each
/// argument gets the callee's CURRENT mask entry, left to right, once.
#[derive(Debug)]
pub struct Pending {
    pub callee: Value,
    pub args: Vec<Value>,
    pub kw: Vec<(KwId, Value)>,
    /// One effective entry per argument: positional ones, then keywords.
    pub entries: Vec<EffectiveEntry>,
    /// The next argument to process.
    pub idx: usize,
    pub ret: RetTo,
    pub span: Option<Span>,
}

impl Pending {
    /// The argument at `idx` (positional first, then keyword values).
    pub fn arg_mut(&mut self, idx: usize) -> Option<&mut Value> {
        let npos = self.args.len();
        if idx < npos {
            self.args.get_mut(idx)
        } else {
            self.kw.get_mut(idx - npos).map(|(_, v)| v)
        }
    }

    /// The number of arguments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.args.len() + self.kw.len()
    }

    /// True when there are no arguments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Frame lifecycle: push, return delivery, forcing and slot reads.
impl Vm {
    /// Pushes a frame for `closure` with its parameters already bound.
    pub(crate) fn push_frame(
        &mut self,
        closure: Rc<Closure>,
        params: Vec<Value>,
        ret: RetTo,
        memo: Option<Rc<Closure>>,
    ) -> Result<(), Failure> {
        if self.frames.len() >= self.depth_limit {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "recursion is too deep",
            ));
        }
        let base = self.stack.len();
        self.stack.extend(params);
        let want = base + usize::from(closure.proto.locals);
        if self.stack.len() < want {
            self.stack.resize(want, Value::Nil);
        }
        self.frames.push(Frame {
            closure,
            ip: 0,
            base,
            ret,
            memo,
            flag: false,
        });
        Ok(())
    }

    /// Reads a slot's value as an eager read (5.6): an unbound slot is
    /// `undefined-name`; a top-level slot is reported to the observer.
    pub(crate) fn read_slot(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        // A read costs fuel, so no chain of `VarRef`s can loop unbounded.
        self.tick()?;
        if !slot.is_bound() {
            return Err(Failure::new(
                FailCode::UndefinedName,
                format!("`{}` is not defined", name_of_sym(slot.name())),
            ));
        }
        if !slot.is_local() && slot.kind() != SlotKind::Prelude {
            if let Some(obs) = self.observer.as_mut() {
                obs.on_read(slot)?;
            }
        }
        Ok(slot.get())
    }

    /// Fails `effect-in-query` in Query mode.
    pub(crate) fn check_effect(&self, what: &str) -> Result<(), Failure> {
        match self.effect_mode {
            EffectMode::Normal => Ok(()),
            EffectMode::Query => Err(Failure::new(
                FailCode::EffectInQuery,
                format!("{what} is not allowed inside a query"),
            )),
        }
    }

    /// Delivers a value to where a finished frame or call returns.
    pub(crate) fn deliver(
        &mut self,
        ns: &Namespace,
        v: Value,
        ret: RetTo,
    ) -> Result<Flow, Failure> {
        match ret {
            RetTo::Stack => {
                self.stack.push(v);
                Ok(Flow::Continue)
            }
            RetTo::Force => self.force_value(ns, v, RetTo::Stack),
            RetTo::Stop => Ok(Flow::Return(v)),
            RetTo::PendingArg => {
                let p = self.pending.last_mut().ok_or_else(internal)?;
                let idx = p.idx;
                *p.arg_mut(idx).ok_or_else(internal)? = v;
                self.advance(ns)
            }
        }
    }

    /// Forces `v` (a `Force` demand point): derefs a `VarRef` and runs a
    /// thunk (once, for a memo cell), delivering the result to `ret`.
    pub(crate) fn force_value(
        &mut self,
        ns: &Namespace,
        v: Value,
        ret: RetTo,
    ) -> Result<Flow, Failure> {
        let mut v = v;
        loop {
            match v {
                Value::VarRef(slot) => v = self.read_slot(&slot)?,
                Value::Thunk(c) => {
                    if let Some(done) = c.memo_value() {
                        v = done;
                        continue;
                    }
                    let memo = c.memo.is_some().then(|| Rc::clone(&c));
                    let next = if ret == RetTo::Stack {
                        RetTo::Force
                    } else {
                        ret
                    };
                    self.push_frame(c, Vec::new(), next, memo)?;
                    return Ok(Flow::Continue);
                }
                other => return self.deliver(ns, other, ret),
            }
        }
    }
}
