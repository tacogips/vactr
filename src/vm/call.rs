//! Call-boundary forcing (design 5.5) and native calls.
//!
//! At every call the VM resolves the callee to its CURRENT value and applies
//! ITS mask to the arguments, left to right, exactly once each: `Value`
//! forces a thunk (and derefs a `VarRef`), `Fn` and `Late` pass it, a
//! `Forward` link set is chased with `types::masks::chase` against the
//! current namespace right there, and `Undetermined` wraps a thunk in a
//! memoizing cell. Forcing pushes the thunk's frame and resumes the pending
//! call when it returns, so it costs no Rust recursion.

use std::rc::Rc;

use crate::compile::proto::{ArgKind, Closure};
use crate::ns::namespace::Namespace;
use crate::ns::stage::StagedEffect;
use crate::reader::span::Span;
use crate::types::masks::{chase, EffectiveEntry, ForcingMask, MaskEntry};
use crate::types::natives::NativeSig;
use crate::value::access::{get, index};
use crate::value::intern::{name_of_kw, KwId};
use crate::value::value::{NativeId, Value};
use crate::vm::fail::{FailCode, Failure, Origin};
use crate::vm::frame::{Pending, RetTo};
use crate::vm::vm::{internal, shallow, spread, EffectMode, Flow, Vm};

fn not_callable(v: &Value) -> Failure {
    Failure::new(
        FailCode::NotCallable,
        format!("{} is not callable", kind_name(v)),
    )
}

/// A short name for a value's kind, for messages.
#[must_use]
pub fn kind_name(v: &Value) -> &'static str {
    match v {
        Value::Nil => "nil",
        Value::Bool(_) => "a bool",
        Value::Int(_) | Value::Int64(_) => "an int",
        Value::Float(_) | Value::Float64(_) => "a float",
        Value::Ratio(_) => "a ratio",
        Value::Keyword(_) => "a keyword",
        Value::Str(_) => "a string",
        Value::List(_) => "a list",
        Value::Dict(_) => "a dict",
        Value::Struct(_) => "a struct",
        Value::Variant(_) => "a variant",
        Value::Fn(_) | Value::Native(_) => "a function",
        Value::Thunk(_) => "a block",
        Value::VarRef(_) => "a var",
        Value::Pattern(_) => "a pattern",
        Value::Signal(_) => "a signal",
        Value::Inst(_) => "an instrument",
        Value::Tex(_) => "a texture",
        Value::Range(_) => "a range",
        Value::Path(_) => "a path",
        Value::Url(_) => "a url",
        Value::Sound(_) => "a sound",
    }
}

fn effective(ns: &Namespace, entry: Option<&MaskEntry>) -> EffectiveEntry {
    match entry {
        Some(MaskEntry::Value) | None => EffectiveEntry::Value,
        Some(MaskEntry::Fn) => EffectiveEntry::Fn,
        Some(MaskEntry::Late) => EffectiveEntry::Late,
        Some(MaskEntry::Undetermined) => EffectiveEntry::Undetermined,
        Some(MaskEntry::Forward { links }) => chase(links, &|c| ns.callee_mask(c)).0,
    }
}

impl Vm {
    /// Starts a call: resolves the callee, computes the effective entry of
    /// every argument (chasing `Forward` links now, at this boundary), and
    /// forces the arguments left to right.
    pub(crate) fn begin_call(
        &mut self,
        ns: &Namespace,
        callee: Value,
        args: Vec<Value>,
        kw: Vec<(KwId, Value)>,
        ret: RetTo,
        span: Option<Span>,
    ) -> Result<Flow, Failure> {
        let mut callee = callee;
        while let Value::VarRef(slot) = &callee {
            callee = self.read_slot(&slot.clone())?;
        }
        if let Value::Thunk(c) = &callee {
            if args.is_empty() && kw.is_empty() {
                let c = Rc::clone(c);
                return self.force_value(ns, Value::Thunk(c), ret);
            }
            return Err(not_callable(&callee));
        }
        let (args, kw, entries) = match &callee {
            Value::Fn(c) => {
                let mask = &c.mask;
                let mut entries: Vec<EffectiveEntry> = (0..args.len())
                    .map(|k| effective(ns, mask.0.get(k)))
                    .collect();
                for (name, _) in &kw {
                    let pos = c.proto.arity.names.iter().position(|n| n == name);
                    entries.push(effective(ns, pos.and_then(|p| mask.0.get(p))));
                }
                (args, kw, entries)
            }
            Value::Native(id) => {
                let sig = self.native_sig(ns, *id)?;
                let (args, kw) = split_native_kw(&sig, args, kw);
                let mask = sig.forcing_mask();
                let mut entries: Vec<EffectiveEntry> = (0..args.len())
                    .map(|k| effective(ns, native_entry(&sig, &mask, k).as_ref()))
                    .collect();
                entries.extend(kw.iter().map(|_| EffectiveEntry::Value));
                (args, kw, entries)
            }
            _ => {
                let n = args.len() + kw.len();
                (args, kw, vec![EffectiveEntry::Value; n])
            }
        };
        self.pending.push(Pending {
            callee,
            args,
            kw,
            entries,
            idx: 0,
            ret,
            span,
        });
        self.advance(ns)
    }

    /// Continues the top pending call: forces the next arguments, then
    /// performs the call.
    pub(crate) fn advance(&mut self, ns: &Namespace) -> Result<Flow, Failure> {
        loop {
            let p = self.pending.last_mut().ok_or_else(internal)?;
            if p.idx >= p.len() {
                break;
            }
            let idx = p.idx;
            let entry = p.entries.get(idx).copied().unwrap_or(EffectiveEntry::Value);
            let arg = p.arg_mut(idx).ok_or_else(internal)?;
            match entry {
                EffectiveEntry::Value => match arg.clone() {
                    Value::VarRef(slot) => {
                        let v = self.read_slot(&slot)?;
                        let p = self.pending.last_mut().ok_or_else(internal)?;
                        *p.arg_mut(idx).ok_or_else(internal)? = v;
                    }
                    Value::Thunk(c) => {
                        if let Some(done) = c.memo_value() {
                            *arg = done;
                            continue;
                        }
                        let memo = c.memo.is_some().then(|| Rc::clone(&c));
                        self.push_frame(c, Vec::new(), RetTo::PendingArg, memo)?;
                        return Ok(Flow::Continue);
                    }
                    _ => p.idx += 1,
                },
                EffectiveEntry::Fn | EffectiveEntry::Late => p.idx += 1,
                EffectiveEntry::Undetermined => {
                    if let Value::Thunk(c) = arg {
                        if c.memo.is_none() {
                            *arg = Value::Thunk(Rc::new(c.memoized()));
                        }
                    }
                    p.idx += 1;
                }
            }
        }
        let p = self.pending.pop().ok_or_else(internal)?;
        self.perform(ns, p)
    }

    /// Performs a call whose arguments are ready.
    fn perform(&mut self, ns: &Namespace, p: Pending) -> Result<Flow, Failure> {
        match &p.callee {
            Value::Fn(c) => {
                let params = bind_params(c, p.args, p.kw)?;
                self.push_frame(Rc::clone(c), params, p.ret, None)?;
                Ok(Flow::Continue)
            }
            Value::Native(id) => {
                let v = self.call_native(ns, *id, &p.args, &p.kw, p.span)?;
                self.deliver(ns, v, p.ret)
            }
            other => {
                if !p.kw.is_empty() {
                    return Err(not_callable(other));
                }
                let v = self.call_collection(ns, other, &p.args)?;
                self.deliver(ns, v, p.ret)
            }
        }
    }

    fn native_sig(&self, ns: &Namespace, id: NativeId) -> Result<NativeSig, Failure> {
        ns.prelude()
            .native(id)
            .map(|e| e.sig)
            .ok_or_else(|| Failure::new(FailCode::UndefinedName, "the native is not available"))
    }

    fn call_native(
        &mut self,
        ns: &Namespace,
        id: NativeId,
        args: &[Value],
        kw: &[(KwId, Value)],
        span: Option<Span>,
    ) -> Result<Value, Failure> {
        let entry = *ns
            .prelude()
            .native(id)
            .ok_or_else(|| Failure::new(FailCode::UndefinedName, "the native is not available"))?;
        let sig = entry.sig;
        let n = args.len();
        if n < usize::from(sig.min_args) || sig.max_args.is_some_and(|m| n > usize::from(m)) {
            return Err(Failure::new(
                FailCode::Arity,
                format!("`{}` got {n} arguments", sig.name),
            ));
        }
        let mut cx = NativeCx {
            vm: self,
            ns,
            id,
            span,
        };
        (entry.f)(&mut cx, args, kw)
    }

    /// A list or range called with an index, a dict, struct or variant
    /// called with a key.
    pub(crate) fn call_collection(
        &mut self,
        _ns: &Namespace,
        callee: &Value,
        args: &[Value],
    ) -> Result<Value, Failure> {
        let callee = shallow(callee);
        let [arg] = args else {
            return Err(match callee {
                Value::List(_)
                | Value::Dict(_)
                | Value::Range(_)
                | Value::Struct(_)
                | Value::Variant(_) => {
                    Failure::new(FailCode::Arity, "a collection takes one index or key")
                }
                _ => not_callable(&callee),
            });
        };
        let arg = shallow(arg);
        match callee {
            Value::List(_) | Value::Range(_) => index(&callee, &arg),
            Value::Dict(_) | Value::Struct(_) | Value::Variant(_) | Value::Nil => {
                get(&callee, &arg)
            }
            _ => Err(not_callable(&callee)),
        }
    }

    /// `CallKw`: pops the layout of a call site with pairs and splats.
    pub(crate) fn call_kw(&mut self, ns: &Namespace, s: u16) -> Result<Flow, Failure> {
        let site = {
            let f = self.frames.last().ok_or_else(internal)?;
            f.closure
                .proto
                .call_sites
                .get(usize::from(s))
                .cloned()
                .ok_or_else(internal)?
        };
        let width: usize = site
            .args
            .iter()
            .map(|a| if *a == ArgKind::Pair { 2 } else { 1 })
            .sum();
        let at = self
            .stack
            .len()
            .checked_sub(width + 1)
            .ok_or_else(internal)?;
        let mut vals = self.stack.split_off(at).into_iter();
        let callee = vals.next().ok_or_else(internal)?;
        let mut args = Vec::new();
        let mut kw = Vec::new();
        for kind in site.args.iter() {
            match kind {
                ArgKind::Pos => args.push(vals.next().ok_or_else(internal)?),
                ArgKind::Pair => {
                    let key = shallow(&vals.next().ok_or_else(internal)?);
                    let v = vals.next().ok_or_else(internal)?;
                    push_pair(&mut args, &mut kw, key, v);
                }
                ArgKind::Splat => {
                    let v = shallow(&vals.next().ok_or_else(internal)?);
                    let is_dict = matches!(v, Value::Dict(_));
                    for item in spread(self, &v)? {
                        match (&item, is_dict) {
                            (Value::List(l), true) if l.items.len() == 2 => {
                                push_pair(
                                    &mut args,
                                    &mut kw,
                                    l.items[0].clone(),
                                    l.items[1].clone(),
                                );
                            }
                            _ => args.push(item),
                        }
                    }
                }
            }
        }
        let span = self.current_span();
        self.begin_call(ns, callee, args, kw, RetTo::Stack, span)
    }

    /// Calls `f` from Rust (a native calling back, or a `QueryVm` call).
    /// Counts as one re-entry (7.1.5).
    ///
    /// # Errors
    /// Any failure of the call; `depth-exceeded` past the re-entry limit.
    pub fn call_value(
        &mut self,
        ns: &Namespace,
        f: &Value,
        args: Vec<Value>,
        kw: Vec<(KwId, Value)>,
    ) -> Result<Value, Failure> {
        if self.frames.is_empty() && self.reentries == 0 {
            self.stack_base = Some(crate::vm::vm::stack_addr());
        }
        self.enter()?;
        let floors = self.floors();
        let span = self.current_span();
        let r = match self.begin_call(ns, f.clone(), args, kw, RetTo::Stop, span) {
            Ok(flow) => self.drive(ns, floors, flow),
            Err(e) => {
                self.frames.truncate(floors.0);
                self.pending.truncate(floors.1);
                self.stack.truncate(floors.2);
                Err(e)
            }
        };
        self.reentries = self.reentries.saturating_sub(1);
        r
    }

    /// Forces a value from Rust: derefs a `VarRef` and runs a thunk.
    ///
    /// # Errors
    /// Any failure of the thunk.
    pub fn force(&mut self, ns: &Namespace, v: Value) -> Result<Value, Failure> {
        let mut v = v;
        while let Value::VarRef(slot) = &v {
            v = self.read_slot(&slot.clone())?;
        }
        match v {
            Value::Thunk(_) => self.call_value(ns, &v, Vec::new(), Vec::new()),
            other => Ok(other),
        }
    }
}

fn push_pair(args: &mut Vec<Value>, kw: &mut Vec<(KwId, Value)>, key: Value, v: Value) {
    match key {
        Value::Keyword(k) => kw.push((k, v)),
        other => args.push(Value::list(vec![other, v])),
    }
}

/// For a native: pairs whose key is one of its named parameters stay
/// keyword arguments; any other pair is a positional 2-list (`put d amp:
/// 0.7` passes the pair as an element).
fn split_native_kw(
    sig: &NativeSig,
    mut args: Vec<Value>,
    kw: Vec<(KwId, Value)>,
) -> (Vec<Value>, Vec<(KwId, Value)>) {
    let mut named = Vec::new();
    for (k, v) in kw {
        if sig.keywords.contains(&&*name_of_kw(k)) {
            named.push((k, v));
        } else {
            args.push(Value::list(vec![Value::Keyword(k), v]));
        }
    }
    (args, named)
}

fn native_entry(sig: &NativeSig, mask: &ForcingMask, k: usize) -> Option<MaskEntry> {
    match mask.0.get(k) {
        Some(e) => Some(e.clone()),
        None if sig.max_args.is_none() => mask.0.last().cloned(),
        None => None,
    }
}

/// Binds arguments to a closure's parameters: positional first, then named
/// (a positional parameter may also be passed by name); a missing keyword
/// parameter takes its default from the captures.
fn bind_params(
    c: &Closure,
    args: Vec<Value>,
    kw: Vec<(KwId, Value)>,
) -> Result<Vec<Value>, Failure> {
    let arity = &c.proto.arity;
    let fixed = usize::from(arity.fixed);
    let total = arity.params();
    let name = |c: &Closure| {
        c.proto.name.map_or_else(
            || "the function".to_string(),
            |n| format!("`{}`", crate::value::intern::name_of_sym(n)),
        )
    };
    if args.len() > fixed {
        return Err(Failure::new(
            FailCode::Arity,
            format!("{} takes {fixed} arguments, got {}", name(c), args.len()),
        ));
    }
    let mut params: Vec<Option<Value>> = args.into_iter().map(Some).collect();
    params.resize(total, None);
    for (k, v) in kw {
        let Some(pos) = arity.names.iter().position(|n| *n == k) else {
            return Err(Failure::new(
                FailCode::Arity,
                format!("{} has no parameter `{}`", name(c), name_of_kw(k)),
            ));
        };
        match params.get_mut(pos) {
            Some(slot @ None) => *slot = Some(v),
            _ => {
                return Err(Failure::new(
                    FailCode::Arity,
                    format!("`{}` is passed twice", name_of_kw(k)),
                ))
            }
        }
    }
    let defaults = &c.captures[usize::from(c.proto.captures).min(c.captures.len())..];
    let mut out = Vec::with_capacity(total);
    for (k, p) in params.into_iter().enumerate() {
        match p {
            Some(v) => out.push(v),
            None if k >= fixed => out.push(defaults.get(k - fixed).cloned().unwrap_or(Value::Nil)),
            None => {
                return Err(Failure::new(
                    FailCode::Arity,
                    format!("{} takes {fixed} arguments", name(c)),
                ))
            }
        }
    }
    Ok(out)
}

/// What a native sees of the VM.
pub struct NativeCx<'a> {
    pub vm: &'a mut Vm,
    pub ns: &'a Namespace,
    pub id: NativeId,
    pub span: Option<Span>,
}

impl NativeCx<'_> {
    /// Calls a function value (a re-entry).
    ///
    /// # Errors
    /// Any failure of the call.
    pub fn call(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, Failure> {
        self.vm.call_value(self.ns, f, args, Vec::new())
    }

    /// Forces a value: derefs a `VarRef`, runs a thunk.
    ///
    /// # Errors
    /// Any failure of the thunk.
    pub fn force(&mut self, v: &Value) -> Result<Value, Failure> {
        self.vm.force(self.ns, v.clone())
    }

    /// One unit of fuel for native iteration (7.1.5).
    ///
    /// # Errors
    /// `fuel-exhausted`.
    pub fn tick(&mut self) -> Result<(), Failure> {
        self.vm.tick()
    }

    /// The current effect mode.
    #[must_use]
    pub fn effect_mode(&self) -> EffectMode {
        self.vm.effect_mode
    }

    /// Stages a host-visible effect.
    ///
    /// # Errors
    /// `effect-in-query` in Query mode.
    pub fn stage(&mut self, effect: StagedEffect) -> Result<(), Failure> {
        let name = self
            .ns
            .prelude()
            .native(self.id)
            .map_or("an effect", |e| e.sig.name);
        self.vm.check_effect(&format!("`{name}`"))?;
        self.vm.effects.push(effect);
        Ok(())
    }

    /// Console output: staged in Normal mode, captured in Query mode.
    pub fn print(&mut self, text: &str) {
        match self.vm.effect_mode {
            EffectMode::Normal => self.vm.effects.push(StagedEffect::Console(Rc::from(text))),
            EffectMode::Query => self.vm.output.push((
                Origin {
                    span: self.span,
                    slot: None,
                    beat: None,
                },
                Rc::from(text),
            )),
        }
    }
}
