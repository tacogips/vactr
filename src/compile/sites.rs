//! Tweak-site classification (design section 13 "Site scope" and "Site
//! tiers") for the Decided site set:
//!
//! - numeric literals inside patterns and controls in top-level forms: a
//!   literal (or list item) at a `Late` parameter of a domain native is a
//!   `Direct` site; a literal that flows through computation inside such an
//!   argument (`gain {* 0.5 2}`) is `Reeval`;
//! - top-level `let`/`var` numbers (`var` is `Direct`: the write is an
//!   `upd`; `let` is `Reeval`: dependents rebuild);
//! - `inst` header parameter defaults (`Direct`).
//!
//! Function-body literals are out of scope. A pattern-literal or inst-default
//! site compiles to `LoadTweak` of an anonymous `Tweak` slot; a binding site
//! is the binding's own slot. The tier comes from the form's structure; the
//! reactive layer downgrades non-replayable owners to `Manual`.

use crate::compile::compiler::{binder_name, Compiler, Pos, Res};
use crate::ns::namespace::{Resolved, SlotKind, VarSlotRef};
use crate::ns::tweak::{SiteOrigin, SiteTier, TweakSite};
use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::Span;
use crate::types::diag::Diagnostic;
use crate::types::natives::{NativeMask, NativeSig};
use crate::value::intern::intern_sym;
use crate::value::num::NumKind;
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::ops::Op;

/// The site context of the expression being compiled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SiteCtx {
    None,
    /// At a `Late` parameter of a domain native (or an item of a list there).
    Direct,
    /// Inside computation within such an argument.
    Reeval,
    /// An `inst` header default.
    InstDefault,
}

impl SiteCtx {
    /// The context of a sub-expression that is not a list item.
    pub(crate) fn nested(self) -> SiteCtx {
        match self {
            SiteCtx::Direct | SiteCtx::Reeval => SiteCtx::Reeval,
            SiteCtx::InstDefault | SiteCtx::None => SiteCtx::None,
        }
    }

    /// The context of a list item.
    pub(crate) fn item(self) -> SiteCtx {
        match self {
            SiteCtx::InstDefault => SiteCtx::None,
            other => other,
        }
    }
}

impl Compiler<'_, '_> {
    /// The context of positional argument `pos` of a call whose callee is
    /// the native `sig` (when the compiler can see it).
    pub(crate) fn arg_site(&self, outer: SiteCtx, sig: Option<&NativeSig>, pos: usize) -> SiteCtx {
        if !self.sites_allowed() {
            return SiteCtx::None;
        }
        match sig.and_then(|s| s.entry_at(pos)) {
            Some(NativeMask::Late) if outer == SiteCtx::Reeval => SiteCtx::Reeval,
            Some(NativeMask::Late) => SiteCtx::Direct,
            _ => outer.nested(),
        }
    }

    fn add_site(
        &mut self,
        span: Span,
        slot: VarSlotRef,
        initial: Value,
        tier: SiteTier,
        origin: SiteOrigin,
    ) {
        let Some(ty) = NumKind::of(&initial) else {
            return;
        };
        let ns = self.cx.ns;
        let id = ns.tweaks().borrow_mut().alloc();
        let site = TweakSite {
            id,
            span,
            slot,
            initial,
            ty,
            tier,
            form_gen: self.cx.form_gen,
            origin,
            index: u32::try_from(self.cx.sites.len()).unwrap_or(u32::MAX),
        };
        ns.tweaks().borrow_mut().insert(site.clone());
        self.cx.sites.push(site);
    }

    /// A numeric literal: `LoadTweak` at a site, `LoadConst` elsewhere.
    pub(crate) fn number(&mut self, n: &Node, v: Value) {
        let tier = match self.site {
            _ if !self.sites_allowed() => None,
            SiteCtx::Direct => Some((SiteTier::Direct, SiteOrigin::PatternLiteral)),
            SiteCtx::Reeval => Some((SiteTier::Reeval, SiteOrigin::PatternLiteral)),
            SiteCtx::InstDefault => Some((SiteTier::Direct, SiteOrigin::InstDefault)),
            SiteCtx::None => None,
        };
        match tier {
            Some((tier, origin)) => {
                let slot = VarSlotRef::new(intern_sym("tweak"), SlotKind::Tweak, v.clone());
                self.add_site(n.span, slot.clone(), v, tier, origin);
                let g = self.global_index(&slot);
                self.emit(Op::LoadTweak(g));
            }
            None => self.load_const(v),
        }
    }

    /// The value of a top-level `let`/`var` named `name`: a numeric literal
    /// is a binding site on the binding's own slot.
    pub(crate) fn binding_value(
        &mut self,
        value: &Node,
        name: &str,
        is_var: bool,
    ) -> Result<(), Diagnostic> {
        let lit = match &value.kind {
            NodeKind::Atom(a @ (Atom::Int(_) | Atom::Float { .. } | Atom::Ratio(_))) => {
                crate::compile::compiler::literal(a)
            }
            _ => None,
        };
        match lit {
            Some(v) if self.sites_allowed() => {
                let slot = self.def_target(name);
                let tier = if is_var {
                    SiteTier::Direct
                } else {
                    SiteTier::Reeval
                };
                self.add_site(value.span, slot, v.clone(), tier, SiteOrigin::Binding);
                self.mark(value.span);
                self.load_const(v);
                Ok(())
            }
            _ => self.expr(value, Pos::Value),
        }
    }
}

/// Binding definitions: `let`/`var` (whose top-level numbers are binding
/// sites) and `upd`.
impl Compiler<'_, '_> {
    /// `upd NAME EXPR`.
    pub(crate) fn upd(&mut self, n: &Node) -> Result<(), Diagnostic> {
        let name = n.children[1].sym_name().unwrap_or("");
        let r = self.resolve(name);
        match &r {
            Res::Local(l) if l.is_var => self.emit(Op::LoadLocal(l.slot)),
            Res::Capture { idx, is_var: true } => self.emit(Op::LoadCapture(*idx)),
            _ => 0,
        };
        self.expr(&n.children[2], Pos::Value)?;
        self.emit(Op::Force);
        self.mark(n.span);
        match r {
            Res::Local(l) if l.is_var => {
                self.emit(Op::UpdCell);
            }
            Res::Capture { is_var: true, .. } => {
                self.emit(Op::UpdCell);
            }
            Res::Global(slot) => {
                let g = self.global_index(&slot);
                self.emit(Op::UpdGlobal(g));
            }
            _ => self.fail(FailCode::UpdImmutable),
        }
        Ok(())
    }

    /// `let`/`var`: a session slot at the top level, a local elsewhere.
    pub(crate) fn def_binding(&mut self, n: &Node, top: bool) -> Result<(), Diagnostic> {
        let is_var = n.children[0].sym_name() == Some("var");
        let target = &n.children[1];
        let value = &n.children[2];
        let name = binder_name(target);
        match (&name, top) {
            (Some(name), true) => self.binding_value(value, name, is_var)?,
            _ => self.expr(value, Pos::Value)?,
        }
        self.emit(Op::Force);
        match (name, top) {
            (Some(name), true) => {
                let slot = self.cx.ns.reserve(intern_sym(&name));
                self.def_global(&slot, if is_var { SlotKind::Var } else { SlotKind::Let });
            }
            (Some(name), false) => {
                if is_var {
                    self.emit(Op::MakeCell);
                }
                self.emit(Op::Dup);
                let slot = self.new_local(&name, is_var, false, target)?;
                self.emit(Op::StoreLocal(slot));
                if is_var {
                    self.emit(Op::Deref);
                }
            }
            (None, _) => self.destructure(target, top, is_var)?,
        }
        Ok(())
    }

    /// Emits `DefGlobal` for `slot` with `kind` (the value is on the stack).
    pub(crate) fn def_global(&mut self, slot: &VarSlotRef, kind: SlotKind) {
        let g = self.global_index(slot);
        let fb = self.fb();
        fb.defs.push((g, kind));
        let d = u32::try_from(fb.defs.len() - 1).unwrap_or(u32::MAX);
        self.emit(Op::DefGlobal(d));
    }

    /// Resolves the session slot a definition targets.
    pub(crate) fn def_target(&mut self, name: &str) -> VarSlotRef {
        let sym = intern_sym(name);
        match self.cx.ns.lookup(sym) {
            Some(Resolved::Session(s)) => s,
            _ => self.cx.ns.reserve(sym),
        }
    }
}
