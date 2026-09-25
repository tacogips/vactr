//! Kernel forms to bytecode (design 8.1, 5.5, 5.6).
//!
//! `compile` turns one expanded top-level form into a zero-parameter
//! `FnProto`. Names resolve at compile time per the 5.6 scope model:
//! locals innermost, then captures of enclosing functions, then the session,
//! opened imports and the prelude; the resolved slot is fixed in the proto.
//! A top-level `var` or `fn` loads as a `VarRef` (late), a `let` as a
//! snapshot. Every `fn` and lambda gets its forcing mask from
//! `types::masks::infer_masks`, with or without the diagnostics pass.
//! The only compile failure is `nesting-too-deep` (7.1.5).

use std::rc::Rc;

pub(crate) use crate::compile::matchc::{binder_name, literal};
use crate::compile::proto::{
    ArgKind, CallSite, CapSrc, Fb, FbKind, FnProto, ItemKind, ListSite, Local,
};
use crate::compile::sites::SiteCtx;
use crate::ns::namespace::{FormGen, Namespace, SlotKind, VarSlotRef};
use crate::ns::tweak::TweakSite;
use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::Span;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::types::masks::ForcingMask;
use crate::types::ty::CheckEnv;
use crate::value::intern::{intern_kw, intern_sym};
use crate::value::value::{ListProv, Value};
use crate::vm::fail::FailCode;
use crate::vm::ops::Op;
use crate::vm::vm::{stack_addr, DEFAULT_STACK_BUDGET};

/// Past this many nested nodes a form is `nesting-too-deep` (7.1.5).
pub const MAX_COMPILE_DEPTH: u32 = 1024;

/// Compilation state for one top-level form.
pub struct CompileCx<'a> {
    pub ns: &'a Namespace,
    pub form_gen: FormGen,
    pub doc_revision: u64,
    /// Classify and allocate tweak sites (Live mode, section 13).
    pub tweak_sites: bool,
    /// Bytes of Rust stack the recursive descent may use; past it the form
    /// is `nesting-too-deep` instead of overflowing (7.1.5).
    pub stack_budget: usize,
    /// `latent-forcing` warnings from mask inference (5.5).
    pub diags: Vec<Diagnostic>,
    /// The tweak sites of this form, in source order (also recorded in the
    /// namespace's tweak table).
    pub sites: Vec<TweakSite>,
    env: Option<CheckEnv>,
}

impl<'a> CompileCx<'a> {
    /// A context over `ns` for the form generation `form_gen`.
    #[must_use]
    pub fn new(ns: &'a Namespace, form_gen: FormGen) -> CompileCx<'a> {
        CompileCx {
            ns,
            form_gen,
            doc_revision: 0,
            tweak_sites: true,
            stack_budget: DEFAULT_STACK_BUDGET,
            diags: Vec::new(),
            sites: Vec::new(),
            env: None,
        }
    }

    /// The session as `infer_masks` sees it (built once per form).
    pub(crate) fn env(&mut self) -> &CheckEnv {
        let ns = self.ns;
        self.env.get_or_insert_with(|| ns.check_env())
    }
}

/// Compiles one expanded top-level form.
///
/// # Errors
/// `nesting-too-deep` past 1024 nested nodes; the form must not run.
pub fn compile(n: &Node, cx: &mut CompileCx<'_>) -> Result<Rc<FnProto>, Diagnostic> {
    let mut c = Compiler {
        cx,
        fbs: vec![Fb::new(None, n.span, FbKind::Top)],
        depth: 0,
        site: SiteCtx::None,
        self_def: None,
        stack_base: stack_addr(),
        overflow: false,
        dsp: 0,
        head: false,
    };
    c.top_form(n)?;
    if c.overflow {
        return Err(Diagnostic::error(
            DiagCode::NestingTooDeep,
            n.span,
            "the form is too large to compile",
        ));
    }
    c.emit(Op::Deref);
    c.emit(Op::Ret);
    let fb = c.fbs.pop().ok_or_else(|| too_deep(n))?;
    Ok(Rc::new(fb.finish(c.cx.form_gen)))
}

pub(crate) fn too_deep(n: &Node) -> Diagnostic {
    Diagnostic::error(
        DiagCode::NestingTooDeep,
        n.span,
        "the form nests too deeply to compile",
    )
}

/// How a name resolved.
#[derive(Clone, Debug)]
pub(crate) enum Res {
    Local(Local),
    Capture { idx: u16, is_var: bool },
    Global(VarSlotRef),
}

/// The compiler over a stack of function builders (innermost last).
pub(crate) struct Compiler<'c, 'a> {
    pub cx: &'c mut CompileCx<'a>,
    pub fbs: Vec<Fb>,
    pub depth: u32,
    pub site: SiteCtx,
    /// A top-level `fn` being defined: its name resolves to its own session
    /// slot inside the body, even when it shadows a prelude name.
    pub self_def: Option<(Rc<str>, VarSlotRef)>,
    /// The stack position at `compile` entry (the stack budget, 7.1.5).
    pub stack_base: usize,
    /// A proto table (constants, list sites, nested protos, shapes)
    /// outgrew its `u16` index: the form fails to compile.
    pub overflow: bool,
    /// Inside an `inst`, `bus` or `master` body (implicit control names).
    pub dsp: u32,
    /// The next atom is a call head (never an implicit control name).
    pub head: bool,
}

impl Compiler<'_, '_> {
    pub(crate) fn fb(&mut self) -> &mut Fb {
        let k = self.fbs.len() - 1;
        &mut self.fbs[k]
    }

    pub(crate) fn emit(&mut self, op: Op) -> usize {
        let fb = self.fb();
        fb.code.push(op);
        fb.code.len() - 1
    }

    /// Records `span` for the next op.
    pub(crate) fn mark(&mut self, span: Span) {
        let fb = self.fb();
        let ip = u32::try_from(fb.code.len()).unwrap_or(u32::MAX);
        match fb.spans.last_mut() {
            Some((at, s)) if *at == ip => *s = span,
            Some((_, s)) if *s == span => {}
            _ => fb.spans.push((ip, span)),
        }
    }

    /// Counts one level of nesting: past 1024 levels, or past the stack
    /// budget, the form is `nesting-too-deep` (7.1.5).
    pub(crate) fn enter(&mut self, n: &Node) -> Result<(), Diagnostic> {
        self.depth += 1;
        let used = self.stack_base.abs_diff(stack_addr());
        if self.depth > MAX_COMPILE_DEPTH || used > self.cx.stack_budget {
            return Err(too_deep(n));
        }
        Ok(())
    }

    pub(crate) fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub(crate) fn konst(&mut self, v: Value) -> u16 {
        let fb = self.fb();
        fb.consts.push(v);
        let k = fb.consts.len() - 1;
        self.idx16(k)
    }

    /// A table index as `u16`; past `u16::MAX` the form is marked too large.
    pub(crate) fn idx16(&mut self, k: usize) -> u16 {
        u16::try_from(k).unwrap_or_else(|_| {
            self.overflow = true;
            0
        })
    }

    pub(crate) fn load_const(&mut self, v: Value) {
        let k = self.konst(v);
        self.emit(Op::LoadConst(k));
    }

    pub(crate) fn global_index(&mut self, slot: &VarSlotRef) -> u32 {
        let fb = self.fb();
        if let Some(k) = fb.globals.iter().position(|g| g.same(slot)) {
            return u32::try_from(k).unwrap_or(u32::MAX);
        }
        fb.globals.push(slot.clone());
        u32::try_from(fb.globals.len() - 1).unwrap_or(u32::MAX)
    }

    /// A fresh local in the innermost scope.
    pub(crate) fn new_local(
        &mut self,
        name: &str,
        is_var: bool,
        is_param: bool,
        at: &Node,
    ) -> Result<u16, Diagnostic> {
        let fb = self.fb();
        let slot = fb.nlocals;
        fb.nlocals = fb.nlocals.checked_add(1).ok_or_else(|| too_deep(at))?;
        if !name.is_empty() && name != "_" {
            let local = Local {
                name: Rc::from(name),
                slot,
                is_var,
                is_param,
            };
            if let Some(scope) = fb.scopes.last_mut() {
                scope.push(local);
            }
        }
        Ok(slot)
    }

    /// A fresh unnamed temporary.
    pub(crate) fn temp(&mut self, at: &Node) -> Result<u16, Diagnostic> {
        self.new_local("", false, false, at)
    }

    pub(crate) fn open_scope(&mut self) {
        self.fb().scopes.push(Vec::new());
    }

    pub(crate) fn close_scope(&mut self) {
        self.fb().scopes.pop();
    }

    /// Resolves `name` in builder `k` and the builders around it.
    fn resolve_in(&mut self, k: usize, name: &str) -> Option<Res> {
        if let Some(l) = self.fbs[k].find(name) {
            return Some(Res::Local(l.clone()));
        }
        if let Some(i) = self.fbs[k]
            .captures
            .iter()
            .position(|(n, _, _)| &**n == name)
        {
            let is_var = self.fbs[k].captures[i].1;
            return Some(Res::Capture {
                idx: u16::try_from(i).ok()?,
                is_var,
            });
        }
        if k == 0 {
            return None;
        }
        let (src, is_var) = match self.resolve_in(k - 1, name)? {
            Res::Local(l) => (CapSrc::Local(l.slot), l.is_var),
            Res::Capture { idx, is_var } => (CapSrc::Capture(idx), is_var),
            Res::Global(g) => return Some(Res::Global(g)),
        };
        let caps = &mut self.fbs[k].captures;
        caps.push((Rc::from(name), is_var, src));
        Some(Res::Capture {
            idx: u16::try_from(caps.len() - 1).ok()?,
            is_var,
        })
    }

    /// Resolves a name: locals and captures, the definition being compiled,
    /// then the namespace (reserving a session slot for a forward reference).
    pub(crate) fn resolve(&mut self, name: &str) -> Res {
        let k = self.fbs.len() - 1;
        if let Some(r) = self.resolve_in(k, name) {
            return r;
        }
        if let Some((n, slot)) = &self.self_def {
            if &**n == name {
                return Res::Global(slot.clone());
            }
        }
        let sym = intern_sym(name);
        match self.cx.ns.lookup(sym) {
            Some(r) => Res::Global(r.slot().clone()),
            None => Res::Global(self.cx.ns.reserve(sym)),
        }
    }

    /// Loads a global slot: a `let` or prelude slot as a snapshot, anything
    /// else (var, fn, tweak, or a not-yet-defined name) as a `VarRef`.
    pub(crate) fn load_global(&mut self, slot: &VarSlotRef) {
        let g = self.global_index(slot);
        let late = !slot.is_bound()
            || matches!(slot.kind(), SlotKind::Var | SlotKind::Fn | SlotKind::Tweak);
        self.emit(if late {
            Op::LoadGlobalRef(g)
        } else {
            Op::LoadGlobal(g)
        });
    }

    /// Loads a resolved name.
    pub(crate) fn load_res(&mut self, r: &Res) {
        match r {
            Res::Local(l) => {
                self.emit(Op::LoadLocal(l.slot));
                if l.is_var {
                    self.emit(Op::Deref);
                }
            }
            Res::Capture { idx, is_var } => {
                self.emit(Op::LoadCapture(*idx));
                if *is_var {
                    self.emit(Op::Deref);
                }
            }
            Res::Global(slot) => self.load_global(slot),
        }
    }

    /// Emits a forward jump to be patched; returns its index.
    pub(crate) fn jump_slot(&mut self, op: Op) -> usize {
        self.emit(op)
    }

    /// Points the jump at `at` to the next op.
    pub(crate) fn patch(&mut self, at: usize) {
        let fb = self.fb();
        let off = i32::try_from(fb.code.len() - at - 1).unwrap_or(i32::MAX);
        fb.code[at] = match fb.code[at] {
            Op::JumpIfNoMatch(_) => Op::JumpIfNoMatch(off),
            _ => Op::Jump(off),
        };
    }

    pub(crate) fn fail(&mut self, code: FailCode) {
        let c = FailCode::ALL.iter().position(|x| *x == code).unwrap_or(0);
        self.emit(Op::Fail(u16::try_from(c).unwrap_or(0)));
    }

    /// Whether literals here may be tweak sites: the top-level form or a
    /// thunk inside it, never a function body (section 13).
    pub(crate) fn sites_allowed(&self) -> bool {
        self.cx.tweak_sites && self.fbs.iter().all(|f| f.kind != FbKind::Func)
    }

    /// A top-level form: definitions bind session slots.
    fn top_form(&mut self, n: &Node) -> Result<(), Diagnostic> {
        if let NodeKind::Call = n.kind {
            let head = n.children.first().and_then(Node::sym_name);
            match head {
                Some("let" | "var") if n.children.len() == 3 => return self.def_binding(n, true),
                Some("fn") => return self.def_fn(n, true, FnDef::Fn),
                Some("enum") => return self.def_enum(n, true),
                Some("struct" | "inst" | "look") if self.is_def_head(head) => {
                    return match head {
                        Some("struct") => self.def_struct(n, true),
                        Some("inst") => self.def_fn(n, true, FnDef::Inst),
                        _ => self.def_fn(n, true, FnDef::Fn),
                    };
                }
                _ => {}
            }
        }
        if self.bare_native_call(n) {
            return Ok(());
        }
        self.expr(n, Pos::Value)
    }

    /// A bare name on a line of its own that names a zero-parameter native
    /// (`hush`) is a call, as written in design-music section 1.
    fn bare_native_call(&mut self, s: &Node) -> bool {
        let Some(name) = s.sym_name() else {
            return false;
        };
        let Res::Global(slot) = self.resolve(name) else {
            return false;
        };
        let zero = match slot.is_bound().then(|| slot.get()) {
            Some(Value::Native(id)) => self
                .cx
                .ns
                .prelude()
                .native(id)
                .is_some_and(|e| e.sig.max_args == Some(0)),
            _ => false,
        };
        if zero {
            self.mark(s.span);
            self.load_global(&slot);
            self.emit(Op::Call(0));
        }
        zero
    }

    /// `struct`/`inst`/`look` are definition heads unless the name is bound.
    fn is_def_head(&mut self, head: Option<&str>) -> bool {
        match head {
            Some(h) => match self.resolve_in(self.fbs.len() - 1, h) {
                Some(_) => false,
                None => self
                    .cx
                    .ns
                    .lookup(intern_sym(h))
                    .is_none_or(|r| !r.slot().is_bound()),
            },
            None => false,
        }
    }

    /// Compiles an expression.
    pub(crate) fn expr(&mut self, n: &Node, pos: Pos) -> Result<(), Diagnostic> {
        self.enter(n)?;
        self.mark(n.span);
        let r = self.expr_inner(n, pos);
        self.leave();
        r
    }

    fn expr_inner(&mut self, n: &Node, pos: Pos) -> Result<(), Diagnostic> {
        match &n.kind {
            NodeKind::Atom(a) => self.atom(n, a),
            NodeKind::Call => self.call_dsp(n),
            NodeKind::List => self.list(n),
            NodeKind::Block if pos == Pos::Arg => self.thunk(n),
            NodeKind::Block => self.block(n),
            NodeKind::Pair => {
                let site = self.site;
                self.site = site.nested();
                for c in n.children.iter() {
                    self.expr(c, Pos::Value)?;
                }
                self.site = site;
                self.list_site(n, &[ItemKind::Item, ItemKind::Item], None);
                Ok(())
            }
            NodeKind::Arrow => self.lambda(n),
            NodeKind::Import(_) => {
                // Package loading is TASK-009's; an import evaluates to nil.
                self.load_const(Value::Nil);
                Ok(())
            }
            NodeKind::Splat
            | NodeKind::Neg
            | NodeKind::Fallback
            | NodeKind::Interp
            | NodeKind::IfChain
            | NodeKind::Error => {
                self.fail(FailCode::Type);
                Ok(())
            }
        }
    }

    fn atom(&mut self, n: &Node, a: &Atom) -> Result<(), Diagnostic> {
        match a {
            Atom::Int(_) | Atom::Float { .. } | Atom::Ratio(_) => {
                let v = literal(a).unwrap_or(Value::Nil);
                self.number(n, v);
            }
            Atom::Sym(name) => {
                let r = self.resolve(name);
                if !self.implicit_control(name, &r) {
                    self.load_res(&r);
                }
            }
            Atom::Qualified { prefix, name } => {
                match self
                    .cx
                    .ns
                    .lookup_qualified(intern_sym(prefix), intern_sym(name))
                {
                    Some(slot) => self.load_global(&slot),
                    None => self.fail(FailCode::UndefinedName),
                }
            }
            Atom::Op(op) => self.native_const(op.as_str()),
            Atom::Builtin(name) => self.native_const(name),
            Atom::Path(text) => {
                let rel = text.starts_with("./") || text.starts_with("../");
                self.load_const(Value::path(text, rel.then_some(n.span.file)));
            }
            Atom::Url(text) => self.load_const(Value::url(text)),
            Atom::Wildcard | Atom::ConsoleReg(_) => self.fail(FailCode::UndefinedName),
            _ => self.load_const(literal(a).unwrap_or(Value::Nil)),
        }
        Ok(())
    }

    /// A prelude native by name (operators and expander `Builtin` heads
    /// cannot be rebound).
    fn native_const(&mut self, name: &str) {
        match self.cx.ns.prelude().slot(intern_sym(name)) {
            Some(slot) => self.load_const(slot.get()),
            None => self.fail(FailCode::UndefinedName),
        }
    }

    /// A list literal: a dict when every item is a pair.
    fn list(&mut self, n: &Node) -> Result<(), Diagnostic> {
        let ch = &n.children;
        let site = self.site;
        let is_dict = !ch.is_empty() && ch.iter().all(|c| matches!(c.kind, NodeKind::Pair));
        if is_dict {
            self.site = site.nested();
            for pair in ch.iter() {
                for c in pair.children.iter().take(2) {
                    self.expr(c, Pos::Value)?;
                }
                if pair.children.len() < 2 {
                    self.load_const(Value::Nil);
                }
            }
            self.site = site;
            self.emit(Op::MakeDict(
                u16::try_from(ch.len()).map_err(|_| too_deep(n))?,
            ));
            return Ok(());
        }
        let mut kinds = Vec::with_capacity(ch.len());
        for c in ch.iter() {
            if let NodeKind::Splat = c.kind {
                self.site = site.nested();
                match c.children.first() {
                    Some(inner) => self.expr(inner, Pos::Value)?,
                    None => self.load_const(Value::Nil),
                }
                kinds.push(ItemKind::Splat);
            } else {
                self.site = site.item();
                self.expr(c, Pos::Value)?;
                kinds.push(ItemKind::Item);
            }
        }
        self.site = site;
        let prov = (!kinds.contains(&ItemKind::Splat)).then(|| {
            Rc::new(ListProv {
                form_gen: self.cx.form_gen,
                doc_revision: self.cx.doc_revision,
                elems: ch.iter().map(|c| c.span).collect(),
            })
        });
        self.list_site(n, &kinds, prov);
        Ok(())
    }

    fn list_site(&mut self, _n: &Node, kinds: &[ItemKind], prov: Option<Rc<ListProv>>) {
        let fb = self.fb();
        fb.list_sites.push(ListSite {
            items: kinds.into(),
            prov,
        });
        let k = fb.list_sites.len() - 1;
        let s = self.idx16(k);
        self.emit(Op::MakeList(s));
    }

    /// A `{}` block in value position runs where it stands.
    pub(crate) fn block(&mut self, n: &Node) -> Result<(), Diagnostic> {
        self.open_scope();
        let r = self.statements(&n.children);
        self.close_scope();
        r
    }

    /// Statements in order; the last one is the value (`nil` if none).
    pub(crate) fn statements(&mut self, stmts: &[Node]) -> Result<(), Diagnostic> {
        if stmts.is_empty() {
            self.load_const(Value::Nil);
            return Ok(());
        }
        for (k, s) in stmts.iter().enumerate() {
            self.statement(s)?;
            if k + 1 < stmts.len() {
                self.emit(Op::Pop);
            }
        }
        Ok(())
    }

    /// One statement. A bare parameter on a line of its own is called
    /// (`maybe-do`, lang-reference section 3); `let`/`var`/`fn` bind a
    /// local for the statements after it.
    fn statement(&mut self, s: &Node) -> Result<(), Diagnostic> {
        if let Some(name) = s.sym_name() {
            if let Res::Local(l) = self.resolve(name) {
                if l.is_param {
                    self.mark(s.span);
                    self.load_res(&Res::Local(l));
                    self.emit(Op::Call(0));
                    return Ok(());
                }
            }
        }
        if self.bare_native_call(s) {
            return Ok(());
        }
        if let NodeKind::Call = s.kind {
            match s.children.first().and_then(Node::sym_name) {
                Some("let" | "var") if s.children.len() == 3 => return self.def_binding(s, false),
                Some("fn") => return self.def_fn(s, false, FnDef::Fn),
                Some("enum") => return self.def_enum(s, false),
                _ => {}
            }
        }
        self.expr(s, Pos::Value)
    }

    /// A thunk: a zero-parameter closure over the block.
    fn thunk(&mut self, n: &Node) -> Result<(), Diagnostic> {
        self.fbs.push(Fb::new(None, n.span, FbKind::Thunk));
        let body = self.block(n);
        let r = body.map(|()| {
            self.emit(Op::Ret);
        });
        let fb = self.fbs.pop().ok_or_else(|| too_deep(n))?;
        r?;
        self.load_captures(&fb);
        self.close_closure_loaded(fb, ForcingMask::default(), true);
        Ok(())
    }

    /// Loads the values a nested builder captures, in capture order.
    pub(crate) fn load_captures(&mut self, fb: &Fb) {
        for (_, _, src) in &fb.captures {
            match src {
                CapSrc::Local(slot) => self.emit(Op::LoadLocal(*slot)),
                CapSrc::Capture(idx) => self.emit(Op::LoadCapture(*idx)),
            };
        }
    }

    /// Finishes a nested builder whose captures (and keyword defaults) are
    /// already on the stack: `MakeClosure` or `MakeThunk`.
    pub(crate) fn close_closure_loaded(&mut self, fb: Fb, mask: ForcingMask, thunk: bool) {
        let proto = Rc::new(fb.finish(self.cx.form_gen));
        let parent = self.fb();
        parent.protos.push(proto);
        parent.masks.push(mask);
        let k = parent.protos.len() - 1;
        let p = self.idx16(k);
        self.emit(if thunk {
            Op::MakeThunk(p)
        } else {
            Op::MakeClosure(p)
        });
    }

    /// A call: kernel heads, definition heads, then an ordinary call.
    pub(crate) fn call(&mut self, n: &Node) -> Result<(), Diagnostic> {
        let ch = &n.children;
        let Some(head) = ch.first() else {
            self.load_const(Value::Nil);
            return Ok(());
        };
        match head.sym_name() {
            Some("match") => return self.match_form(n),
            Some("fn") => return self.def_fn(n, false, FnDef::Fn),
            Some("let" | "var") if ch.len() == 3 => return self.def_binding(n, false),
            Some("upd") if ch.len() == 3 => return self.upd(n),
            Some("enum") => return self.def_enum(n, false),
            _ => {}
        }
        let args = &ch[1..];
        // A free, never-bound name that is an `InstParam` control (12.8.6
        // M3, B2): `cutoff` after `inst .. cutoff: .. :` or a prelude
        // template's own header (`analog`'s `cutoff`) compiles as a call to
        // the `"inst control"` native with the name appended as a keyword,
        // not as a call to the (undefined) name itself.
        let inst_ctl = self.inst_control_head(head).and_then(|name| {
            self.inst_control_native()
                .map(|(slot, sig)| (name, slot, sig))
        });
        let callee_sig = match &inst_ctl {
            Some((_, _, sig)) => Some(*sig),
            None => self.static_native(head),
        };
        let site = self.site;
        self.site = site.nested();
        if let NodeKind::List = head.kind {
            self.expr(head, Pos::Value)?;
            for a in args {
                self.expr(a, Pos::Value)?;
            }
            self.site = site;
            self.mark(n.span);
            self.emit(Op::CallValue(
                u8::try_from(args.len()).map_err(|_| too_deep(n))?,
            ));
            return Ok(());
        }
        self.head = true;
        let r = match &inst_ctl {
            Some((_, slot, _)) => {
                self.mark(head.span);
                self.load_global(slot);
                Ok(())
            }
            None => self.expr(head, Pos::Value),
        };
        self.head = false;
        r?;
        let mut kinds = Vec::with_capacity(args.len() + 1);
        let mut pos: usize = 0;
        for a in args {
            match a.kind {
                NodeKind::Pair => {
                    self.site = site.nested();
                    for c in a.children.iter().take(2) {
                        self.expr(c, Pos::Value)?;
                    }
                    if a.children.len() < 2 {
                        self.load_const(Value::Nil);
                    }
                    kinds.push(ArgKind::Pair);
                }
                NodeKind::Splat => {
                    self.site = site.nested();
                    match a.children.first() {
                        Some(inner) => self.expr(inner, Pos::Value)?,
                        None => self.load_const(Value::Nil),
                    }
                    kinds.push(ArgKind::Splat);
                }
                _ => {
                    self.site = self.arg_site(site, callee_sig.as_ref(), pos);
                    self.expr(a, Pos::Arg)?;
                    kinds.push(ArgKind::Pos);
                    pos += 1;
                }
            }
        }
        if let Some((name, _, _)) = &inst_ctl {
            self.load_const(Value::Keyword(intern_kw(name)));
            kinds.push(ArgKind::Pos);
        }
        self.site = site;
        self.mark(n.span);
        let simple = kinds.iter().all(|k| *k == ArgKind::Pos);
        match (simple, u8::try_from(kinds.len())) {
            (true, Ok(k)) => {
                self.emit(Op::Call(k));
            }
            _ => {
                let fb = self.fb();
                fb.call_sites.push(CallSite { args: kinds.into() });
                let s = u16::try_from(fb.call_sites.len() - 1).map_err(|_| too_deep(n))?;
                self.emit(Op::CallKw(s));
            }
        }
        Ok(())
    }

    /// The native a call head names, when the compiler can see it.
    fn static_native(&mut self, head: &Node) -> Option<crate::types::natives::NativeSig> {
        let slot = match &head.kind {
            NodeKind::Atom(Atom::Sym(name)) => match self.resolve(name) {
                Res::Global(slot) => slot,
                _ => return None,
            },
            NodeKind::Atom(Atom::Op(op)) => self.cx.ns.prelude().slot(intern_sym(op.as_str()))?,
            NodeKind::Atom(Atom::Builtin(name)) => self.cx.ns.prelude().slot(intern_sym(name))?,
            _ => return None,
        };
        if !slot.is_bound() {
            return None;
        }
        match slot.get() {
            Value::Native(id) => self.cx.ns.prelude().native(id).map(|e| e.sig),
            _ => None,
        }
    }
}

/// Where an expression sits: a positional call argument turns a `{}` block
/// into a thunk; everywhere else it runs where it stands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pos {
    Value,
    Arg,
}

/// Which definition head `def_fn` compiles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FnDef {
    Fn,
    Inst,
}
