//! The checker's scope chain (design 5.6 "Scope model", 7.1.4):
//! `prelude -> session -> fn/block`.
//!
//! The session scope holds the top-level bindings of the checked document.
//! A child scope is opened by a `fn` (parameters and the body's top-level
//! statements share it), a lambda (parameters and body), a `{..}` or
//! indented block, and a `match` clause (pattern bindings and guard). The
//! prelude is the read-only native table; `CheckEnv::globals` is the
//! session state from before the checked document. This module classifies
//! each new binding into the 5.6 diagnostics; the checker reports them.

#[cfg(test)]
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::reader::span::{NodeId, Span};
use crate::types::check::{Checker, TypeDef};
use crate::types::diag::DiagCode;
use crate::types::masks::ForcingMask;
use crate::types::natives::NativeTable;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Scheme, Ty, TypeId};

/// What the checker knows about one binding beyond its type.
#[derive(Clone, Debug, Default)]
pub(crate) enum BindExtra {
    #[default]
    Plain,
    /// A `fn`: its keyword parameters (passed as pairs) and its forcing
    /// mask re-indexed by positional argument.
    Fn {
        keywords: Rc<[(Rc<str>, Ty)]>,
        mask: ForcingMask,
    },
    /// An enum variant of this enum.
    Variant { enum_id: TypeId },
    /// A struct constructor.
    Struct(TypeId),
}

/// One binding of a user scope.
#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub(crate) kind: BindKind,
    pub(crate) scheme: Scheme,
    pub(crate) span: Span,
    /// Annotated `any`: it must be narrowed before use (`any-not-narrowed`).
    pub(crate) annot_any: bool,
    pub(crate) extra: BindExtra,
    /// Known local callable effects; false does not certify unknown/imported code.
    pub(crate) query_effect: bool,
}

impl Binding {
    /// A binding of `kind` with a monomorphic type.
    pub(crate) fn mono(kind: BindKind, ty: Ty, span: Span) -> Binding {
        Binding {
            kind,
            scheme: Scheme::mono(ty),
            span,
            annot_any: false,
            extra: BindExtra::Plain,
            query_effect: false,
        }
    }
}

/// The kind of a scope, which decides the 5.6 diagnostics of its bindings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScopeKind {
    Fn,
    Lambda,
    Block,
    Clause,
}

#[derive(Debug)]
struct Scope {
    names: Vec<(Rc<str>, Binding)>,
    index: HashMap<Rc<str>, usize>,
}

impl Scope {
    fn empty() -> Self {
        Self {
            names: Vec::new(),
            index: HashMap::new(),
        }
    }
}

#[cfg(test)]
thread_local! {
    static NAME_PROBES: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn name_probes() -> u64 {
    NAME_PROBES.with(Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_name_probes() {
    NAME_PROBES.with(|probes| probes.set(0));
}

#[cfg(test)]
#[inline]
fn note_probe() {
    NAME_PROBES.with(|probes| probes.set(probes.get() + 1));
}

#[cfg(not(test))]
#[inline(always)]
fn note_probe() {}

/// The user scopes, session first. Never empty.
#[derive(Debug)]
pub(crate) struct Scopes {
    stack: Vec<Scope>,
    query_effects: HashMap<NodeId, bool>,
}

/// The scope diagnostic a new binding gets (7.1.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum DeclNote {
    /// No diagnostic: a fresh name, or Live redefinition of a session name.
    None,
    /// The same scope already binds the name (error); the earlier
    /// binding's span.
    Rebinding(Span),
    /// An enclosing user scope binds the name (warning).
    Shadowing,
    /// The name resolves in the prelude (hint).
    ShadowsPrelude,
}

impl DeclNote {
    /// The diagnostic code and message for `name`, if any.
    pub(crate) fn diag(&self, name: &str) -> Option<(DiagCode, String)> {
        match self {
            DeclNote::None => None,
            DeclNote::Rebinding(first) => Some((
                DiagCode::Rebinding,
                format!(
                    "`{name}` is already bound in this scope (at byte {}); a name is bound once per scope",
                    first.start
                ),
            )),
            DeclNote::Shadowing => Some((
                DiagCode::Shadowing,
                format!("`{name}` shadows a binding of an enclosing scope"),
            )),
            DeclNote::ShadowsPrelude => Some((
                DiagCode::ShadowsPrelude,
                format!("`{name}` shadows the prelude `{name}`"),
            )),
        }
    }
}

impl Scopes {
    /// Only the session scope.
    pub(crate) fn new() -> Scopes {
        Scopes {
            stack: vec![Scope::empty()],
            query_effects: HashMap::new(),
        }
    }

    pub(crate) fn record_query_effect(&mut self, node: NodeId, effect: bool) {
        self.query_effects.insert(node, effect);
    }
    pub(crate) fn node_query_effect(&self, node: NodeId) -> Option<bool> {
        self.query_effects.get(&node).copied()
    }

    /// Opens a child scope. Every kind classifies bindings the same way;
    /// the kind documents which 5.6 boundary the caller opened.
    pub(crate) fn push(&mut self, _kind: ScopeKind) {
        self.stack.push(Scope::empty());
    }

    /// The number of open scopes (the session counts).
    pub(crate) fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Closes scopes until `depth` remain (restores after a walk).
    pub(crate) fn truncate(&mut self, depth: usize) {
        self.stack.truncate(depth.max(1));
    }

    /// True when the innermost scope is the session scope.
    pub(crate) fn at_session(&self) -> bool {
        self.stack.len() == 1
    }

    /// Classifies a new binding of `name` in the innermost scope (7.1.4).
    /// `env.globals` is the session state before the document: binding one
    /// of them again at the session level is Live redefinition.
    pub(crate) fn classify(&self, name: &str, env: &CheckEnv) -> DeclNote {
        let Some((inner, outer)) = self.stack.split_last() else {
            return DeclNote::None;
        };
        note_probe();
        if let Some(slot) = inner.index.get(name) {
            return DeclNote::Rebinding(inner.names[*slot].1.span);
        }
        for scope in outer {
            note_probe();
            if scope.index.contains_key(name) {
                return DeclNote::Shadowing;
            }
        }
        if env.global(name).is_some() {
            return if self.at_session() {
                DeclNote::None
            } else {
                DeclNote::Shadowing
            };
        }
        if NativeTable::global().get(name).is_some() {
            return DeclNote::ShadowsPrelude;
        }
        DeclNote::None
    }

    /// Binds `name` in the innermost scope. A rebinding replaces the
    /// earlier binding, so later uses see the latest type.
    pub(crate) fn bind(&mut self, name: Rc<str>, b: Binding) {
        if let Some(scope) = self.stack.last_mut() {
            note_probe();
            match scope.index.get(&*name).copied() {
                Some(slot) => scope.names[slot].1 = b,
                None => {
                    let slot = scope.names.len();
                    scope.names.push((Rc::clone(&name), b));
                    note_probe();
                    scope.index.insert(name, slot);
                }
            }
        }
    }

    /// The session-scope binding of `name` (a local binding does not count).
    pub(crate) fn session_lookup(&self, name: &str) -> Option<&Binding> {
        let scope = self.stack.first()?;
        note_probe();
        scope.index.get(name).map(|slot| &scope.names[*slot].1)
    }

    /// The innermost user binding of `name`.
    pub(crate) fn lookup(&self, name: &str) -> Option<&Binding> {
        self.stack.iter().rev().find_map(|scope| {
            note_probe();
            scope.index.get(name).map(|slot| &scope.names[*slot].1)
        })
    }

    /// Replaces the scheme and extra of the innermost binding of `name`
    /// (a `fn` is bound before its body is checked, for recursion).
    pub(crate) fn update(
        &mut self,
        name: &str,
        scheme: Scheme,
        extra: BindExtra,
        query_effect: bool,
    ) {
        for scope in self.stack.iter_mut().rev() {
            note_probe();
            if let Some(slot) = scope.index.get(name).copied() {
                let b = &mut scope.names[slot].1;
                b.scheme = scheme;
                b.extra = extra;
                b.query_effect = query_effect;
                return;
            }
        }
    }
}

/// Why an `upd` target is not allowed, or `None` when it is a `var`.
pub(crate) fn upd_problem(
    scopes: &Scopes,
    env: &CheckEnv,
    name: &str,
) -> Option<(DiagCode, String)> {
    let kind = match scopes.lookup(name) {
        Some(b) => Some(b.kind),
        None => env.global(name).map(|g| g.kind),
    };
    match kind {
        Some(BindKind::Var) => None,
        Some(k) => Some((
            DiagCode::UpdImmutable,
            format!("`{name}` is a {} binding; only a `var` may be updated", kind_word(k)),
        )),
        None if NativeTable::global().get(name).is_some() => Some((
            DiagCode::UpdImmutable,
            format!("`{name}` is a prelude name; the prelude is read-only (bind a session `let` instead)"),
        )),
        None if env.open_prefix(name).is_some() => Some((
            DiagCode::UpdImmutable,
            format!("`{name}` belongs to an imported package; only a `var` may be updated"),
        )),
        None => Some((DiagCode::UndefinedName, format!("`{name}` is not defined"))),
    }
}

fn kind_word(k: BindKind) -> &'static str {
    match k {
        BindKind::Let => "`let`",
        BindKind::Var => "`var`",
        BindKind::Fn => "`fn`",
        BindKind::Inst => "`inst`",
        BindKind::Struct => "`struct`",
        BindKind::Enum => "enum",
        BindKind::Param => "parameter",
        BindKind::PatternBinding => "pattern",
    }
}

/// The names a binding pattern binds, with their nodes, in source order:
/// a name, `_` (nothing), a list pattern with nested lists and `& rest`, a
/// dict pattern `[amp: a]` (the pair value binds). `variant` says whether
/// a bare name is an enum variant (a variant pattern, which binds nothing).
pub(crate) fn pattern_names<'n>(
    n: &'n Node,
    variant: &dyn Fn(&str) -> bool,
    out: &mut Vec<(Rc<str>, &'n Node)>,
    depth: u32,
) {
    if depth > 256 {
        return;
    }
    match &n.kind {
        NodeKind::Atom(Atom::Sym(name)) if !variant(name) => out.push((name.clone(), n)),
        NodeKind::Pair => {
            if let Some(v) = n.children.get(1) {
                pattern_names(v, variant, out, depth + 1);
            }
        }
        NodeKind::List | NodeKind::Splat => {
            for c in n.children.iter() {
                pattern_names(c, variant, out, depth + 1);
            }
        }
        NodeKind::Call => {
            // A variant pattern `circle r`: the head is the variant.
            for c in n.children.iter().skip(1) {
                pattern_names(c, variant, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// Splits a clause lhs at top-level `|` into its alternatives.
pub(crate) fn alternatives(lhs: &[Node]) -> Vec<&[Node]> {
    lhs.split(|n| matches!(n.kind, NodeKind::Atom(Atom::Op(Op::Bar))))
        .collect()
}

/// True for the falsy alternative `false | nil` that `if` desugars to.
pub(crate) fn is_falsy_lhs(lhs: &[Node]) -> bool {
    matches!(
        lhs,
        [f, bar, nil]
            if matches!(f.kind, NodeKind::Atom(Atom::Bool(false)))
                && matches!(bar.kind, NodeKind::Atom(Atom::Op(Op::Bar)))
                && matches!(nil.kind, NodeKind::Atom(Atom::Nil))
    )
}

/// Binding, pattern and `match` rules of the checker (5.6, 7.1.4).
impl Checker<'_> {
    /// Declares `name` in the innermost scope with its 5.6 diagnostic;
    /// a session binding is also recorded for the forcing checks.
    pub(crate) fn declare(&mut self, name: &Rc<str>, at: &Node, b: Binding) {
        if &**name == "_" {
            return;
        }
        let note = self.scopes.classify(name, self.env);
        if let Some((code, msg)) = note.diag(name) {
            self.emit(code, at.span, msg);
        }
        if self.scopes.at_session() {
            let mask = match &b.extra {
                BindExtra::Fn { mask, .. } => Some(mask.clone()),
                _ => None,
            };
            self.live.globals.insert(
                name.clone(),
                GlobalInfo {
                    kind: b.kind,
                    scheme: Some(b.scheme.clone()),
                    mask,
                    span: Some(at.span),
                },
            );
        }
        self.scopes.bind(name.clone(), b);
    }

    /// Binds every name of a binding pattern: a name gets `ty`, a list
    /// pattern gives its items the element type, a dict pattern gives its
    /// pair values the value type, `& rest` the list type.
    pub(crate) fn bind_pattern(&mut self, p: &Node, ty: &Ty, kind: BindKind) {
        self.bind_pattern_at(p, ty, kind, true, 0);
    }

    /// `top` is true only for the whole target or header parameter, where
    /// a pair is the annotation `name: type`; inside a list a pair is a
    /// dict pattern whose value binds.
    fn bind_pattern_at(&mut self, p: &Node, ty: &Ty, kind: BindKind, top: bool, depth: u32) {
        if depth > 256 {
            return;
        }
        match &p.kind {
            NodeKind::Atom(Atom::Sym(name)) if !self.is_variant_name(name) => {
                let b = Binding::mono(kind, ty.clone(), p.span);
                self.declare(name, p, b);
                self.types.insert(p.id, ty.clone());
            }
            NodeKind::Pair => match p.children.first().map(|k| &k.kind) {
                // `x: int` in a header or `let` target.
                Some(NodeKind::Atom(Atom::Keyword(name)))
                    if top && kind != BindKind::PatternBinding =>
                {
                    let name = name.clone();
                    let annot = p.children.get(1).and_then(|t| self.annot_ty(t));
                    let t = annot.clone().unwrap_or_else(|| ty.clone());
                    let mut b = Binding::mono(kind, t, p.span);
                    b.annot_any = annot == Some(Ty::Any);
                    self.declare(&name, p, b);
                }
                _ => {
                    if let Some(v) = p.children.get(1) {
                        let vt = match self.u.shallow(ty) {
                            Ty::Dict(_, v) => *v,
                            _ => Ty::Any,
                        };
                        self.bind_pattern_at(v, &vt, kind, false, depth + 1);
                    }
                }
            },
            NodeKind::List => {
                let dict = p.children.iter().all(|c| matches!(c.kind, NodeKind::Pair));
                let elem = match self.u.shallow(ty) {
                    Ty::List(e) => *e,
                    Ty::Dict(k, v) if !dict => Ty::List(Box::new(self.u.join(&k, &v))),
                    _ => Ty::Any,
                };
                for c in p.children.iter() {
                    match c.kind {
                        NodeKind::Splat => {
                            if let Some(inner) = c.children.first() {
                                let rest = Ty::List(Box::new(elem.clone()));
                                self.bind_pattern_at(inner, &rest, kind, false, depth + 1);
                            }
                        }
                        NodeKind::Pair => self.bind_pattern_at(c, ty, kind, false, depth + 1),
                        _ => self.bind_pattern_at(c, &elem, kind, false, depth + 1),
                    }
                }
            }
            _ => {}
        }
    }

    /// Binds a clause pattern. The names of the first alternative are
    /// bound; a variant head types the subject as its enum.
    pub(crate) fn clause_pattern(&mut self, pattern: &[Node], s_ty: &Ty) {
        let alts = alternatives(pattern);
        let Some(first) = alts.first() else {
            return;
        };
        match first {
            [single] => {
                if let Some((enum_id, _)) = single.sym_name().and_then(|s| self.variant(s)) {
                    let _ = self.u.try_unify(&Ty::Named(enum_id), s_ty);
                } else {
                    self.bind_pattern(single, s_ty, BindKind::PatternBinding);
                }
            }
            [head, fields @ ..] => {
                let known = head.sym_name().and_then(|s| {
                    self.variant(s)
                        .map(|(e, i)| (e, Some(i)))
                        .or_else(|| self.type_names.get(s).map(|id| (*id, None)))
                });
                match known {
                    Some((id, _)) => {
                        let _ = self.u.try_unify(&Ty::Named(id), s_ty);
                    }
                    None if head.sym_name().is_some_and(|s| !self.is_variant_name(s)) => {
                        let name = head.sym_name().unwrap_or("");
                        self.emit(
                            DiagCode::UndefinedName,
                            head.span,
                            format!("`{name}` is not an enum variant or struct"),
                        );
                    }
                    None => {}
                }
                for f in fields {
                    self.bind_pattern(f, &Ty::Any, BindKind::PatternBinding);
                }
            }
            [] => {}
        }
    }

    /// U5 (7.1.4): `match S {false | nil -> E; V -> T}` with `V` a
    /// field-less enum variant binds nothing and misroutes truthy values.
    pub(crate) fn bare_variant_binding(&mut self, n: &Node) {
        let clauses = &n.children[2].children;
        if clauses.len() != 2 {
            return;
        }
        let lhs = |c: &Node| -> Vec<Node> {
            c.children
                .split_last()
                .map(|(_, l)| l.to_vec())
                .unwrap_or_default()
        };
        let (first, second) = (lhs(&clauses[0]), lhs(&clauses[1]));
        if !is_falsy_lhs(&first) {
            return;
        }
        let [only] = second.as_slice() else {
            return;
        };
        let Some(name) = only.sym_name() else {
            return;
        };
        let fieldless = match self.variant(name) {
            Some((e, i)) => self.variant_fields(e, i).map_or(true, <[_]>::is_empty),
            None => {
                self.scopes.lookup(name).is_none()
                    && self.env.global(name).is_some_and(|g| {
                        g.kind == BindKind::Enum && !g.scheme.as_ref().is_some_and(|s| s.ty.is_fn())
                    })
            }
        };
        if fieldless {
            self.emit(
                DiagCode::BareVariantBinding,
                only.span,
                format!(
                    "`{name}` is a field-less variant, so this pattern binds nothing and any truthy value takes it; \
                     write `match S` with `{name} -> ..` and `_ -> ..`, or `if {{= S {name}}}`"
                ),
            );
        }
    }

    /// `missing-variant`: a `match` on an enum with no catch-all clause
    /// that leaves a variant uncovered.
    pub(crate) fn missing_variant(&mut self, n: &Node, s_ty: &Ty) {
        // An `if`-shaped match (first clause `false | nil`) is not a dispatch.
        let first = n.children[2].children.first();
        if first
            .and_then(|c| c.children.split_last())
            .is_some_and(|(_, l)| is_falsy_lhs(l))
        {
            return;
        }
        let mut enum_id = match self.u.shallow(s_ty) {
            Ty::Named(id) => Some(id),
            _ => None,
        };
        let mut covered = Vec::new();
        for clause in n.children[2].children.iter() {
            let Some((_, lhs)) = clause.children.split_last() else {
                continue;
            };
            let guarded = lhs.iter().any(|i| i.sym_name() == Some("if"));
            let pattern = match lhs.iter().position(|i| i.sym_name() == Some("if")) {
                Some(k) => &lhs[..k],
                None => lhs,
            };
            for alt in alternatives(pattern) {
                let head = alt.first();
                let variant = head.and_then(Node::sym_name).and_then(|s| self.variant(s));
                match (variant, alt) {
                    (Some((e, i)), _) => {
                        enum_id.get_or_insert(e);
                        if !guarded {
                            covered.push((e, i));
                        }
                    }
                    (None, [single]) if !guarded && is_catch_all(single) => return,
                    _ => {}
                }
            }
        }
        let Some(e) = enum_id else {
            return;
        };
        let Some(TypeDef::Enum { name, variants }) = self.def(e) else {
            return;
        };
        let missing: Vec<String> = variants
            .iter()
            .enumerate()
            .filter(|(i, _)| !covered.contains(&(e, *i)))
            .map(|(_, (v, _))| v.to_string())
            .collect();
        if !missing.is_empty() {
            let msg = format!(
                "`match` on `{name}` does not cover {}; add the clauses or `_ -> ..`",
                missing.join(", ")
            );
            self.emit(DiagCode::MissingVariant, n.span, msg);
        }
    }
}

/// A clause pattern that matches anything: `_` or a bare binding name.
fn is_catch_all(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Atom(Atom::Wildcard | Atom::Sym(_)))
}
