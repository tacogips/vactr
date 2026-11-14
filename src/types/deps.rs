//! Advisory static dependency edges of a top-level form (design 5.6):
//! the free top-level names it reads, split into EAGER reads (forced
//! during the form's own evaluation: arithmetic, data, a value argument)
//! and LATE reads (captured deferred: a name at a `Late` or `Fn` position
//! of a domain constructor, a lambda or `fn` body). Runtime recording
//! stays authoritative; the reactive graph may use these edges for
//! display and pre-planning only.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::reader::node::{Atom, Node, NodeKind};
use crate::types::masks::{
    chase, header_params, static_mask, CalleeRef, EffectiveEntry, MaskEntry,
};
use crate::types::natives::{NativeMask, NativeTable};
use crate::types::ty::CheckEnv;

/// The free top-level names of one form.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FormDeps {
    pub eager: BTreeSet<Rc<str>>,
    pub late: BTreeSet<Rc<str>>,
}

/// The recursion bound over the expanded tree (7.1.5).
const MAX_DEPS_DEPTH: u32 = 1024;

/// The eager and late free top-level names of an expanded form. A name
/// read both ways is only eager. Never panics.
#[must_use]
pub fn deps(form: &Node, env: &CheckEnv) -> FormDeps {
    let mut w = Walker {
        env,
        table: NativeTable::global(),
        locals: Vec::new(),
        out: FormDeps::default(),
    };
    w.visit(form, false, 0);
    let FormDeps { eager, mut late } = w.out;
    late.retain(|n| !eager.contains(n));
    FormDeps { eager, late }
}

struct Walker<'a> {
    env: &'a CheckEnv,
    table: &'static NativeTable,
    locals: Vec<Rc<str>>,
    out: FormDeps,
}

impl Walker<'_> {
    fn name(&mut self, name: &Rc<str>, late: bool) {
        if self.locals.contains(name) {
            return;
        }
        // A prelude name is not a slot unless the session rebinds it.
        if self.env.global(name).is_none() && self.table.get(name).is_some() {
            return;
        }
        if late {
            self.out.late.insert(name.clone());
        } else {
            self.out.eager.insert(name.clone());
        }
    }

    /// Binds the names of a target, header parameter or pattern.
    fn bind(&mut self, n: &Node) {
        self.bind_at(n, true, 0);
    }

    /// At the top a pair is the annotation `name: type` (the key binds);
    /// inside a list it is a dict pattern `[amp: a]` (the value binds).
    fn bind_at(&mut self, n: &Node, top: bool, depth: u32) {
        if depth > MAX_DEPS_DEPTH {
            return;
        }
        match &n.kind {
            NodeKind::Atom(Atom::Sym(name)) => self.locals.push(name.clone()),
            NodeKind::Pair => match n.children.first().map(|k| &k.kind) {
                Some(NodeKind::Atom(Atom::Keyword(name))) if top => {
                    self.locals.push(name.clone());
                }
                _ => {
                    if let Some(v) = n.children.get(1) {
                        self.bind_at(v, false, depth + 1);
                    }
                }
            },
            _ => {
                for c in n.children.iter() {
                    self.bind_at(c, false, depth + 1);
                }
            }
        }
    }

    fn visit(&mut self, n: &Node, late: bool, depth: u32) {
        if depth > MAX_DEPS_DEPTH {
            return;
        }
        let d = depth + 1;
        match &n.kind {
            NodeKind::Atom(Atom::Sym(name)) => self.name(name, late),
            NodeKind::Atom(_) | NodeKind::Import(_) | NodeKind::Error => {}
            NodeKind::Call => self.call(n, late, d),
            NodeKind::Block => {
                let mark = self.locals.len();
                for c in n.children.iter() {
                    self.visit(c, late, d);
                }
                self.locals.truncate(mark);
            }
            NodeKind::Arrow => {
                let mark = self.locals.len();
                if let Some((body, lhs)) = n.children.split_last() {
                    for p in lhs {
                        self.bind(p);
                    }
                    self.visit(body, true, d);
                }
                self.locals.truncate(mark);
            }
            _ => {
                for c in n.children.iter() {
                    self.visit(c, late, d);
                }
            }
        }
    }

    fn call(&mut self, n: &Node, late: bool, d: u32) {
        let ch = &n.children;
        let Some(head) = ch.first() else {
            return;
        };
        match head.sym_name() {
            Some("let" | "var") if ch.len() == 3 => {
                self.visit(&ch[2], late, d);
                self.bind(&ch[1]);
                return;
            }
            Some("upd") if ch.len() == 3 => {
                self.visit(&ch[2], late, d);
                return;
            }
            Some("fn") if ch.len() >= 3 => {
                if let Some(name) = ch[1].sym_name() {
                    self.locals.push(Rc::from(name));
                }
                let mark = self.locals.len();
                for p in header_params(n) {
                    self.bind(p.node);
                }
                if let Some(body) = ch.last() {
                    self.visit(body, true, d);
                }
                self.locals.truncate(mark);
                return;
            }
            Some("match") if ch.len() == 3 => {
                self.visit(&ch[1], late, d);
                for clause in ch[2].children.iter() {
                    let mark = self.locals.len();
                    if let Some((body, lhs)) = clause.children.split_last() {
                        let guard = lhs.iter().position(|i| i.sym_name() == Some("if"));
                        let (pat, g) = match guard {
                            Some(k) => (&lhs[..k], &lhs[k + 1..]),
                            None => (lhs, &[][..]),
                        };
                        for p in pat {
                            self.bind(p);
                        }
                        for x in g {
                            self.visit(x, late, d);
                        }
                        self.visit(body, late, d);
                    }
                    self.locals.truncate(mark);
                }
                return;
            }
            Some("enum" | "struct") => return,
            Some("inst" | "look") => {
                for c in ch.iter().skip(2) {
                    self.visit(c, true, d);
                }
                return;
            }
            _ => {}
        }
        self.visit(head, late, d);
        let mut pos = 0usize;
        for arg in ch.iter().skip(1) {
            match arg.kind {
                NodeKind::Pair | NodeKind::Splat => self.visit(arg, late, d),
                _ => {
                    let deferred = self.deferred(head, pos);
                    self.visit(arg, late || deferred, d);
                    pos = pos.saturating_add(1);
                }
            }
        }
    }

    /// True when the callee captures argument `pos` deferred: a native
    /// `Late`/`Fn` position, or a session callee whose chased entry is not
    /// `Value`.
    fn deferred(&self, head: &Node, pos: usize) -> bool {
        let native = match &head.kind {
            NodeKind::Atom(Atom::Op(op)) => self.table.get(op.as_str()),
            NodeKind::Atom(Atom::Builtin(name)) => self.table.get(name),
            NodeKind::Atom(Atom::Sym(name)) if self.env.global(name).is_none() => {
                if self.locals.contains(name) {
                    return false;
                }
                self.table.get(name)
            }
            NodeKind::Atom(Atom::Sym(name)) => {
                let mask = static_mask(self.env, &CalleeRef::Global(name.clone()));
                return match mask.as_ref().and_then(|m| m.entry(pos)) {
                    Some(MaskEntry::Fn | MaskEntry::Late) => true,
                    Some(MaskEntry::Forward { links }) => {
                        let env = self.env;
                        matches!(
                            chase(links, &|c| static_mask(env, c)).0,
                            EffectiveEntry::Fn | EffectiveEntry::Late
                        )
                    }
                    _ => false,
                };
            }
            _ => None,
        };
        native.is_some_and(|(_, sig)| {
            matches!(sig.entry_at(pos), Some(NativeMask::Late | NativeMask::Fn))
        })
    }
}
