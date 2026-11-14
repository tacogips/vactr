//! Forcing masks: per-parameter forcing, their local inference, and the
//! call-boundary `Forward` chase (design 5.5, 7.1.3).
//!
//! This is the only mask logic in the crate. The compiler calls
//! `infer_masks` for every `fn` and lambda (with or without the diagnostics
//! pass), the VM calls `chase` at the call boundary, and the checker calls
//! `mixed_forcing` for the `mixed-forcing` warning.

use std::rc::Rc;

use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::types::natives::{NativeMask, NativeSig, NativeTable};
use crate::types::ty::{CheckEnv, Ty};
use crate::value::value::NativeId;

/// A callee a `Forward` link points at.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CalleeRef {
    /// A native that cannot be rebound: an operator or an expander
    /// `Builtin` head.
    Native(NativeId),
    /// An unqualified name. It resolves against the CURRENT bindings: the
    /// session, then opened imports, then the prelude, so redefining it
    /// (even a prelude name such as `at`) changes the chase result.
    Global(Rc<str>),
    /// `prefix.name` of an imported package.
    Qualified { prefix: Rc<str>, name: Rc<str> },
}

/// How one parameter receives a thunk argument (design 5.5).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MaskEntry {
    /// Force the thunk before binding.
    Value,
    /// Pass the closure; the callee calls it.
    Fn,
    /// Capture the argument deferred (domain `PParam`/`VParam`).
    Late,
    /// Forwarded to these (callee, parameter position) destinations; chased
    /// to one effective entry at the wrapper's own call boundary.
    Forward { links: Box<[(CalleeRef, u16)]> },
    /// Not determinable: memoize at most once, `latent-forcing`.
    Undetermined,
}

/// One entry per parameter, in header order.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct ForcingMask(pub Box<[MaskEntry]>);

impl ForcingMask {
    /// The entry for argument `pos`. Past the end the last entry applies
    /// (the rest convention of variadic natives); `None` for an empty mask.
    #[must_use]
    pub fn entry(&self, pos: usize) -> Option<&MaskEntry> {
        self.0.get(pos).or_else(|| self.0.last())
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The entry a call boundary applies after chasing `Forward` links.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectiveEntry {
    Value,
    Fn,
    Late,
    Undetermined,
}

/// The longest `Forward` chain the chase follows.
const MAX_CHASE_DEPTH: usize = 64;
/// The most links one chase resolves (bounds diamond-shaped fan-out).
const CHASE_BUDGET: u32 = 4096;

/// Chases a `Forward` link set against the current bindings (design 5.5).
///
/// Each link resolves to its destination's current entry, recursively for
/// `Forward` destinations. The combination is: any `Value` gives `Value`;
/// otherwise any `Undetermined` (including an unresolvable link, a chase
/// cycle, or a chase past the depth or budget bound) gives `Undetermined`;
/// otherwise `Fn` if any destination is `Fn`, else `Late`. An empty set is
/// `Undetermined`. The flag is true when one link of THIS set resolves to
/// `Value` and another to `Fn` or `Late` (the `mixed-forcing` case).
#[must_use]
pub fn chase(
    links: &[(CalleeRef, u16)],
    lookup: &dyn Fn(&CalleeRef) -> Option<ForcingMask>,
) -> (EffectiveEntry, bool) {
    let mut ch = Chaser {
        lookup,
        path: Vec::new(),
        budget: CHASE_BUDGET,
    };
    ch.combine(links)
}

struct Chaser<'a> {
    lookup: &'a dyn Fn(&CalleeRef) -> Option<ForcingMask>,
    path: Vec<(CalleeRef, u16)>,
    budget: u32,
}

impl Chaser<'_> {
    fn combine(&mut self, links: &[(CalleeRef, u16)]) -> (EffectiveEntry, bool) {
        let (mut value, mut func, mut late, mut undet) = (false, false, false, false);
        for link in links {
            match self.resolve(link) {
                EffectiveEntry::Value => value = true,
                EffectiveEntry::Fn => func = true,
                EffectiveEntry::Late => late = true,
                EffectiveEntry::Undetermined => undet = true,
            }
        }
        let eff = if value {
            EffectiveEntry::Value
        } else if undet || links.is_empty() {
            EffectiveEntry::Undetermined
        } else if func {
            EffectiveEntry::Fn
        } else {
            EffectiveEntry::Late
        };
        (eff, value && (func || late))
    }

    fn resolve(&mut self, link: &(CalleeRef, u16)) -> EffectiveEntry {
        if self.budget == 0 || self.path.len() >= MAX_CHASE_DEPTH || self.path.contains(link) {
            return EffectiveEntry::Undetermined;
        }
        self.budget -= 1;
        let Some(mask) = (self.lookup)(&link.0) else {
            return EffectiveEntry::Undetermined;
        };
        match mask.entry(usize::from(link.1)) {
            Some(MaskEntry::Value) => EffectiveEntry::Value,
            Some(MaskEntry::Fn) => EffectiveEntry::Fn,
            Some(MaskEntry::Late) => EffectiveEntry::Late,
            Some(MaskEntry::Forward { links }) => {
                self.path.push(link.clone());
                let (eff, _) = self.combine(links);
                self.path.pop();
                eff
            }
            Some(MaskEntry::Undetermined) | None => EffectiveEntry::Undetermined,
        }
    }
}

/// Parameter positions whose `Forward` links currently disagree (one
/// `Value`, another `Fn`/`Late`): the `mixed-forcing` set check.
#[must_use]
pub fn mixed_forcing(
    mask: &ForcingMask,
    lookup: &dyn Fn(&CalleeRef) -> Option<ForcingMask>,
) -> Vec<u16> {
    mask.0
        .iter()
        .enumerate()
        .filter_map(|(k, entry)| match entry {
            MaskEntry::Forward { links } if chase(links, lookup).1 => u16::try_from(k).ok(),
            _ => None,
        })
        .collect()
}

/// The mask a callee has as far as the checker knows: a session binding's
/// recorded mask, else the prelude native's. For the checker's static
/// `mixed-forcing` and `latent-forcing` checks; the VM chases against its
/// live namespace instead.
#[must_use]
pub fn static_mask(env: &CheckEnv, callee: &CalleeRef) -> Option<ForcingMask> {
    let table = NativeTable::global();
    match callee {
        CalleeRef::Native(id) => table.sig(*id).map(NativeSig::forcing_mask),
        CalleeRef::Global(name) => match env.global(name) {
            Some(info) => info.mask.clone(),
            None => table.get(name).map(|(_, sig)| sig.forcing_mask()),
        },
        CalleeRef::Qualified { prefix, name } => env.qualified(prefix, name)?.mask.clone(),
    }
}

/// One parameter of a `fn` header or lambda.
#[derive(Clone, PartialEq, Debug)]
pub struct HeaderParam<'a> {
    /// The binder node: a `Sym`, a `name: type` pair or a list pattern.
    pub node: &'a Node,
    /// The bound name; `None` for a list pattern or `_`.
    pub name: Option<Rc<str>>,
    /// Annotated with a function type (`g: fn int -> int`).
    pub fn_annotated: bool,
    /// Has a `= default`, so it is a keyword parameter.
    pub keyword: bool,
}

/// The parameters of a kernel `(fn NAME HEADER.. Block)` or a lambda
/// `(-> (params..) body)`, in header order; empty for any other node. The
/// mask of `infer_masks` has one entry per parameter in this order.
///
/// A header is flat after reading (`g: fn int -> int` reads as
/// `[:g fn] int -> int`), so type words that follow an annotation are
/// skipped: after `name: pattern` one type item, after `name: fn` every item
/// up to and including the `->` and the return type. An unowned `->` starts
/// the fn's return type and ends the header.
#[must_use]
pub fn header_params(f: &Node) -> Vec<HeaderParam<'_>> {
    match f.kind {
        NodeKind::Arrow => lambda_params(f),
        NodeKind::Call if is_fn_form(f) => fn_params(&f.children[2..f.children.len() - 1]),
        _ => Vec::new(),
    }
}

fn is_fn_form(f: &Node) -> bool {
    f.children.len() >= 3
        && f.children[0].sym_name() == Some("fn")
        && f.children[1].sym_name().is_some()
        && matches!(f.children[f.children.len() - 1].kind, NodeKind::Block)
}

fn lambda_params(f: &Node) -> Vec<HeaderParam<'_>> {
    let lhs = match f.children.split_last() {
        Some((_, lhs)) => lhs,
        None => &[],
    };
    lhs.iter()
        .map(|node| HeaderParam {
            node,
            name: node.sym_name().map(Rc::from),
            fn_annotated: false,
            keyword: false,
        })
        .collect()
}

fn fn_params(items: &[Node]) -> Vec<HeaderParam<'_>> {
    let mut out: Vec<HeaderParam<'_>> = Vec::new();
    let mut k = 0;
    while let Some(item) = items.get(k) {
        k += 1;
        match &item.kind {
            NodeKind::Atom(Atom::Op(Op::Arrow)) => break,
            NodeKind::Atom(Atom::Op(Op::Eq)) => {
                if let Some(last) = out.last_mut() {
                    last.keyword = true;
                }
                k += 1;
            }
            NodeKind::Atom(Atom::Sym(name)) => out.push(HeaderParam {
                node: item,
                name: Some(name.clone()),
                fn_annotated: false,
                keyword: false,
            }),
            NodeKind::List => out.push(HeaderParam {
                node: item,
                name: None,
                fn_annotated: false,
                keyword: false,
            }),
            NodeKind::Pair => {
                let name = match item.children.first().map(|n| &n.kind) {
                    Some(NodeKind::Atom(Atom::Keyword(name))) => Some(name.clone()),
                    _ => None,
                };
                let ty = item.children.get(1).and_then(Node::sym_name);
                match ty {
                    Some("pattern") => k += 1,
                    Some("fn") => {
                        while let Some(t) = items.get(k) {
                            k += 1;
                            if matches!(t.kind, NodeKind::Atom(Atom::Op(Op::Arrow))) {
                                k += 1;
                                break;
                            }
                        }
                    }
                    _ => {}
                }
                out.push(HeaderParam {
                    node: item,
                    name,
                    fn_annotated: ty == Some("fn"),
                    keyword: false,
                });
            }
            _ => {}
        }
    }
    out
}

/// Past this many nested nodes `infer_masks` stops descending (7.1.5).
const MAX_INFER_DEPTH: u32 = 1024;

/// Infers the forcing mask of an expanded `fn` or lambda node (design 5.5).
///
/// Per parameter: a function-type annotation, a call-position use, or a use
/// as a statement line of its own (`body` in `maybe-do`) is `Fn`; any other
/// directly determined use (an argument of a `Value`-masked native, a named
/// argument, a list item, a binding value, a returned value) is `Value`;
/// an argument of an `Fn`/`Late` native position or of a session or imported
/// callee is a `Forward` link; an argument of a callee unknown at compile
/// time (a parameter, a local, an undefined name, an `any`-typed global, or
/// any position after a splat) is `Undetermined`. `Fn` wins over `Value`,
/// `Value` over `Undetermined`, and `Undetermined` over links. An unused
/// parameter and a list-pattern parameter are `Value`. Past 1024 nested
/// nodes the walk stops and every parameter not already `Fn` or `Value` is
/// `Undetermined`. Any other node gives an empty mask. Never panics.
#[must_use]
pub fn infer_masks(f: &Node, env: &CheckEnv) -> ForcingMask {
    let params = header_params(f);
    let (self_name, body) = match f.kind {
        NodeKind::Call if is_fn_form(f) => (f.children[1].sym_name(), f.children.last()),
        NodeKind::Arrow => (None, f.children.last()),
        _ => return ForcingMask::default(),
    };
    let mut cx = Infer {
        env,
        table: NativeTable::global(),
        uses: vec![Uses::default(); params.len()],
        scope: Vec::new(),
        truncated: false,
        self_name,
    };
    for (k, p) in params.iter().enumerate() {
        match &p.name {
            Some(name) if &**name != "_" => cx.scope.push((name.clone(), Some(k))),
            _ => cx.bind_pattern(p.node),
        }
    }
    if let Some(body) = body {
        cx.visit(body, 1);
    }
    let entries = params
        .iter()
        .zip(cx.uses)
        .map(|(p, u)| {
            if p.fn_annotated || u.fn_use {
                MaskEntry::Fn
            } else if u.value || p.name.is_none() {
                MaskEntry::Value
            } else if u.undetermined || cx.truncated {
                MaskEntry::Undetermined
            } else if u.links.is_empty() {
                MaskEntry::Value
            } else {
                MaskEntry::Forward {
                    links: u.links.into_boxed_slice(),
                }
            }
        })
        .collect();
    ForcingMask(entries)
}

#[derive(Clone, Default)]
struct Uses {
    fn_use: bool,
    value: bool,
    undetermined: bool,
    links: Vec<(CalleeRef, u16)>,
}

/// What a call head resolved to.
enum Callee {
    /// A prelude native: its signature and the link to record.
    Native(&'static NativeSig, CalleeRef),
    /// A session or imported binding.
    Link(CalleeRef),
    /// A list or dict literal called with an index or key.
    Index,
    Unknown,
}

struct Infer<'a> {
    env: &'a CheckEnv,
    table: &'static NativeTable,
    uses: Vec<Uses>,
    /// Innermost last: a name and its parameter index (`None` for a local).
    scope: Vec<(Rc<str>, Option<usize>)>,
    truncated: bool,
    self_name: Option<&'a str>,
}

impl Infer<'_> {
    /// The parameter a name resolves to, when the innermost binding of the
    /// name is a parameter; `Some(None)` for a local.
    fn local(&self, name: &str) -> Option<Option<usize>> {
        self.scope
            .iter()
            .rev()
            .find(|(n, _)| &**n == name)
            .map(|(_, p)| *p)
    }

    fn param_of(&self, n: &Node) -> Option<usize> {
        self.local(n.sym_name()?).flatten()
    }

    /// Binds every name of a `match`/lambda pattern as a local. In a dict
    /// pattern `[amp: a]` the pair value is the binder.
    fn bind_pattern(&mut self, n: &Node) {
        match &n.kind {
            NodeKind::Atom(Atom::Sym(name)) => self.scope.push((name.clone(), None)),
            NodeKind::Pair => {
                if let Some(v) = n.children.get(1) {
                    self.bind_pattern(v);
                }
            }
            NodeKind::List | NodeKind::Splat | NodeKind::Call => {
                for c in n.children.iter() {
                    self.bind_pattern(c);
                }
            }
            _ => {}
        }
    }

    /// Binds a `let`/`var` target or a header parameter: `x`, `x: type`
    /// (a pair whose key is the name) or a list pattern.
    fn bind_target(&mut self, n: &Node) {
        match (&n.kind, n.children.first().map(|k| &k.kind)) {
            (NodeKind::Pair, Some(NodeKind::Atom(Atom::Keyword(name)))) => {
                self.scope.push((name.clone(), None));
            }
            _ => self.bind_pattern(n),
        }
    }

    fn visit(&mut self, n: &Node, depth: u32) {
        if depth > MAX_INFER_DEPTH {
            self.truncated = true;
            return;
        }
        match &n.kind {
            NodeKind::Atom(Atom::Sym(_)) => {
                if let Some(i) = self.param_of(n) {
                    self.uses[i].value = true;
                }
            }
            NodeKind::Atom(_) | NodeKind::Import(_) | NodeKind::Error => {}
            NodeKind::Call => self.call(n, depth),
            NodeKind::Block => self.block(n, depth),
            NodeKind::Arrow => {
                let mark = self.scope.len();
                for p in lambda_params(n) {
                    self.bind_pattern(p.node);
                }
                if let Some(body) = n.children.last() {
                    self.visit(body, depth + 1);
                }
                self.scope.truncate(mark);
            }
            _ => {
                for c in n.children.iter() {
                    self.visit(c, depth + 1);
                }
            }
        }
    }

    /// Statements in order; a bare parameter on a line of its own is a call
    /// (lang-reference section 1, `maybe-do`). A `let`/`var`/`fn` binds its
    /// name for the statements after it.
    fn block(&mut self, n: &Node, depth: u32) {
        let mark = self.scope.len();
        for stmt in n.children.iter() {
            match self.param_of(stmt) {
                Some(i) => self.uses[i].fn_use = true,
                None => self.visit(stmt, depth + 1),
            }
            if matches!(stmt.kind, NodeKind::Call) {
                match stmt.children.first().and_then(Node::sym_name) {
                    Some("let" | "var") => {
                        if let Some(target) = stmt.children.get(1) {
                            self.bind_target(target);
                        }
                    }
                    Some("fn") => {
                        if let Some(name) = stmt.children.get(1).and_then(Node::sym_name) {
                            self.scope.push((Rc::from(name), None));
                        }
                    }
                    _ => {}
                }
            }
        }
        self.scope.truncate(mark);
    }

    fn call(&mut self, n: &Node, depth: u32) {
        let ch = &n.children;
        let Some(head) = ch.first() else {
            return;
        };
        match head.sym_name() {
            Some("let" | "var" | "upd") => {
                if let Some(value) = ch.get(2) {
                    self.visit(value, depth + 1);
                }
                return;
            }
            Some("fn") if is_fn_form(n) => {
                let mark = self.scope.len();
                if let Some(name) = ch[1].sym_name() {
                    self.scope.push((Rc::from(name), None));
                }
                for p in header_params(n) {
                    self.bind_target(p.node);
                }
                if let Some(body) = ch.last() {
                    self.visit(body, depth + 1);
                }
                self.scope.truncate(mark);
                return;
            }
            Some("match") => {
                self.match_form(n, depth);
                return;
            }
            Some("enum") => return,
            _ => {}
        }
        let callee = self.callee(head, depth);
        let mut pos: usize = 0;
        let mut after_splat = false;
        for arg in ch[1..].iter() {
            match arg.kind {
                NodeKind::Pair => {
                    for c in arg.children.iter().skip(1) {
                        self.visit(c, depth + 1);
                    }
                }
                NodeKind::Splat => {
                    self.visit(arg, depth + 1);
                    after_splat = true;
                }
                _ => {
                    match self.param_of(arg) {
                        Some(i) if after_splat => self.uses[i].undetermined = true,
                        Some(i) => self.arg_use(i, &callee, pos),
                        None => self.visit(arg, depth + 1),
                    }
                    pos = pos.saturating_add(1);
                }
            }
        }
    }

    fn match_form(&mut self, n: &Node, depth: u32) {
        let ch = &n.children;
        if let Some(subject) = ch.get(1) {
            self.visit(subject, depth + 1);
        }
        let Some(clauses) = ch.get(2) else {
            return;
        };
        for clause in clauses.children.iter() {
            if !matches!(clause.kind, NodeKind::Arrow) {
                self.visit(clause, depth + 1);
                continue;
            }
            let Some((body, lhs)) = clause.children.split_last() else {
                continue;
            };
            let guard = lhs.iter().position(|i| i.sym_name() == Some("if"));
            let (pattern, guard) = match guard {
                Some(k) => (&lhs[..k], &lhs[k + 1..]),
                None => (lhs, &[][..]),
            };
            let mark = self.scope.len();
            for p in pattern {
                self.bind_pattern(p);
            }
            for g in guard {
                self.visit(g, depth + 1);
            }
            self.visit(body, depth + 1);
            self.scope.truncate(mark);
        }
    }

    fn callee(&mut self, head: &Node, depth: u32) -> Callee {
        match &head.kind {
            NodeKind::Atom(Atom::Sym(name)) => match self.local(name) {
                Some(Some(i)) => {
                    self.uses[i].fn_use = true;
                    Callee::Unknown
                }
                Some(None) => Callee::Unknown,
                None => self.global_callee(name),
            },
            NodeKind::Atom(Atom::Qualified { prefix, name }) => {
                match self.env.qualified(prefix, name) {
                    Some(info) if !is_any(info.scheme.as_ref().map(|s| &s.ty)) => {
                        Callee::Link(CalleeRef::Qualified {
                            prefix: prefix.clone(),
                            name: name.clone(),
                        })
                    }
                    _ => Callee::Unknown,
                }
            }
            NodeKind::Atom(Atom::Op(op)) => self.native(op.as_str(), false),
            NodeKind::Atom(Atom::Builtin(name)) => self.native(name, false),
            NodeKind::List => {
                self.visit(head, depth + 1);
                Callee::Index
            }
            _ => {
                self.visit(head, depth + 1);
                Callee::Unknown
            }
        }
    }

    fn global_callee(&self, name: &Rc<str>) -> Callee {
        if let Some(info) = self.env.global(name) {
            return if is_any(info.scheme.as_ref().map(|s| &s.ty)) {
                Callee::Unknown
            } else {
                Callee::Link(CalleeRef::Global(name.clone()))
            };
        }
        if let Some(prefix) = self.env.open_prefix(name) {
            return Callee::Link(CalleeRef::Qualified {
                prefix: prefix.clone(),
                name: name.clone(),
            });
        }
        if self.table.get(name).is_some() {
            return self.native(name, true);
        }
        if self.self_name == Some(&**name) {
            return Callee::Link(CalleeRef::Global(name.clone()));
        }
        Callee::Unknown
    }

    /// A prelude native. A named (`Sym`) head links by name so a session
    /// redefinition is seen; an operator or `Builtin` head links by id.
    fn native(&self, name: &str, by_name: bool) -> Callee {
        match self.table.get(name) {
            Some((id, sig)) => {
                let link = if by_name {
                    CalleeRef::Global(Rc::from(name))
                } else {
                    CalleeRef::Native(id)
                };
                Callee::Native(sig, link)
            }
            None => Callee::Unknown,
        }
    }

    fn arg_use(&mut self, i: usize, callee: &Callee, pos: usize) {
        let link = |r: &CalleeRef| u16::try_from(pos).ok().map(|p| (r.clone(), p));
        let target = match callee {
            Callee::Unknown => None,
            Callee::Index => {
                self.uses[i].value = true;
                return;
            }
            Callee::Native(sig, r) => match sig.entry_at(pos) {
                Some(NativeMask::Fn | NativeMask::Late) => link(r),
                Some(NativeMask::Value) | None => {
                    self.uses[i].value = true;
                    return;
                }
            },
            Callee::Link(r) => link(r),
        };
        let u = &mut self.uses[i];
        match target {
            Some(l) => {
                if !u.links.contains(&l) {
                    u.links.push(l);
                }
            }
            None => u.undetermined = true,
        }
    }
}

fn is_any(ty: Option<&Ty>) -> bool {
    matches!(ty, Some(Ty::Any))
}
