//! Name and atom resolution (design 5.6, 5.7, 14.5.10).
//!
//! Identifiers resolve innermost first: locals, captures of enclosing
//! functions, the definition being compiled, then the namespace (session,
//! opened imports, prelude), reserving a session slot for a forward
//! reference. Qualified names go through their import. A console register
//! `_n` reads the namespace's console slot `n`, and only in the console.

use std::rc::Rc;

use crate::compile::compiler::{literal, Compiler};
use crate::compile::proto::{CapSrc, Local};
use crate::ns::namespace::{SlotKind, VarSlotRef};
use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::FileId;
use crate::types::diag::Diagnostic;
use crate::value::intern::intern_sym;
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::ops::Op;

/// How a name resolved.
#[derive(Clone, Debug)]
pub(crate) enum Res {
    Local(Local),
    Capture { idx: u16, is_var: bool },
    Global(VarSlotRef),
}

impl Compiler<'_, '_> {
    /// Resolves `name` in builder `k` and the builders around it.
    pub(crate) fn resolve_in(&mut self, k: usize, name: &str) -> Option<Res> {
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

    pub(crate) fn atom(&mut self, n: &Node, a: &Atom) -> Result<(), Diagnostic> {
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
            Atom::ConsoleReg(reg) => self.console_reg(n, *reg),
            Atom::Wildcard => self.fail(FailCode::UndefinedName),
            _ => self.load_const(literal(a).unwrap_or(Value::Nil)),
        }
        Ok(())
    }

    /// `_n` reads the console slot `n` (14.5.10). An unset register is an
    /// unbound slot, so the read fails `undefined-name` at run time; in a
    /// file (not the console) the register does not exist.
    fn console_reg(&mut self, n: &Node, reg: u32) {
        if n.span.file != FileId::CONSOLE {
            self.fail(FailCode::UndefinedName);
            return;
        }
        let slot = self.cx.ns.console_slot(reg);
        self.load_global(&slot);
    }

    /// A prelude native by name (operators and expander `Builtin` heads
    /// cannot be rebound).
    fn native_const(&mut self, name: &str) {
        match self.cx.ns.prelude().slot(intern_sym(name)) {
            Some(slot) => self.load_const(slot.get()),
            None => self.fail(FailCode::UndefinedName),
        }
    }

    /// The native a call head names, when the compiler can see it.
    pub(crate) fn static_native(
        &mut self,
        head: &Node,
    ) -> Option<crate::types::natives::NativeSig> {
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
