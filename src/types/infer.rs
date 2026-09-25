//! Expression inference over kernel forms (design 7): literals, names,
//! lists and dicts, blocks and lambdas, the binding forms with
//! let-polymorphism, `fn` with thunk parameter typing from `infer_masks`,
//! `match` with its pattern bindings, `missing-variant` and U5
//! `bare-variant-binding`, and the definition heads `enum`, `struct`,
//! `inst` and `look` (7.1.4). Calls are in `infer_call.rs`.

use std::rc::Rc;

use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::types::check::{Checker, MAX_CHECK_DEPTH};
use crate::types::diag::DiagCode;
use crate::types::masks::{infer_masks, mixed_forcing, static_mask, ForcingMask, MaskEntry};
use crate::types::scope::{is_falsy_lhs, BindExtra, Binding, ScopeKind};
use crate::types::ty::{BindKind, GlobalInfo, Scheme, Ty};
use crate::types::unify::{Clash, NumFloor};

/// One parsed `fn` header parameter.
struct Param<'n> {
    node: &'n Node,
    name: Option<Rc<str>>,
    annot: Option<Ty>,
    default: Option<&'n Node>,
}

impl Checker<'_> {
    /// Infers `n` and records its type. Past the depth bound the subtree
    /// is `any` with `nesting-too-deep` (7.1.5).
    pub(crate) fn infer(&mut self, n: &Node, depth: u32) -> Ty {
        let t = if depth > MAX_CHECK_DEPTH {
            self.too_deep(n)
        } else {
            self.infer_node(n, depth + 1)
        };
        self.types.insert(n.id, t.clone());
        t
    }

    #[cold]
    #[inline(never)]
    fn too_deep(&mut self, n: &Node) -> Ty {
        if std::mem::replace(&mut self.too_deep_reported, true) {
            return Ty::Any;
        }
        self.emit(
            DiagCode::NestingTooDeep,
            n.span,
            "the form nests too deeply to check; typed as `any`",
        );
        Ty::Any
    }

    #[inline(never)]
    fn infer_node(&mut self, n: &Node, d: u32) -> Ty {
        match &n.kind {
            NodeKind::Atom(a) => self.atom(a, n),
            NodeKind::Call => self.call(n, d),
            NodeKind::List => self.list(n, d),
            NodeKind::Block => self.block(n, d, Some(ScopeKind::Block)),
            NodeKind::Arrow => self.lambda(n, d),
            NodeKind::Import(_) | NodeKind::Error => Ty::Any,
            _ => {
                for c in n.children.iter() {
                    self.infer(c, d);
                }
                match n.kind {
                    NodeKind::Pair => Ty::List(Box::new(Ty::Any)),
                    _ => Ty::Any,
                }
            }
        }
    }

    fn atom(&mut self, a: &Atom, n: &Node) -> Ty {
        match a {
            Atom::Int(v) => match i32::try_from(*v) {
                Ok(_) => self.u.fresh_num(NumFloor::Int),
                Err(_) => Ty::Int64,
            },
            Atom::Float { .. } => self.u.fresh_num(NumFloor::Float),
            Atom::Ratio(_) => Ty::Ratio,
            Atom::Str(_) => Ty::Str,
            Atom::Keyword(_) => Ty::keyword(),
            Atom::Sym(name) => self.resolve(name, n),
            Atom::Qualified { prefix, name } => match self.env.qualified(prefix, name) {
                Some(info) => match &info.scheme {
                    Some(s) => self.u.instantiate(s),
                    None => Ty::Any,
                },
                None => {
                    self.emit(
                        DiagCode::UndefinedName,
                        n.span,
                        format!("`{prefix}.{name}` is not defined by the imported package"),
                    );
                    Ty::Any
                }
            },
            Atom::Op(op) => self.native_value(op.as_str(), n),
            Atom::Builtin(name) => self.native_value(name, n),
            Atom::Path(_) => Ty::Path,
            Atom::Url(_) => Ty::Url,
            Atom::Nil => Ty::Nil,
            Atom::Bool(_) => Ty::Bool,
            Atom::Wildcard | Atom::ConsoleReg(_) => Ty::Any,
        }
    }

    /// A name in value position: user scopes, the session state, opened
    /// imports, then the prelude.
    pub(crate) fn resolve(&mut self, name: &str, n: &Node) -> Ty {
        if let Some(b) = self.scopes.lookup(name) {
            let (scheme, kind) = (b.scheme.clone(), b.kind);
            return match kind {
                BindKind::Inst => Ty::Sound,
                _ => self.u.instantiate(&scheme),
            };
        }
        if let Some(info) = self.env.global(name) {
            return match &info.scheme {
                Some(s) => {
                    let s = s.clone();
                    self.u.instantiate(&s)
                }
                None => Ty::Any,
            };
        }
        // `live` also holds the opens of the document's own imports.
        if let Some(prefix) = self.live.open_prefix(name) {
            return match self
                .live
                .qualified(prefix, name)
                .and_then(|i| i.scheme.clone())
            {
                Some(s) => self.u.instantiate(&s),
                None => Ty::Any,
            };
        }
        if self.table.get(name).is_some() {
            return self.native_value(name, n);
        }
        self.emit(
            DiagCode::UndefinedName,
            n.span,
            format!("`{name}` is not defined"),
        );
        Ty::Any
    }

    /// A prelude native used as a value: its (first) scheme, `any` for the
    /// overload group; `beyond-capability` when the host lacks what it
    /// needs.
    pub(crate) fn native_value(&mut self, name: &str, n: &Node) -> Ty {
        let Some((_, sig)) = self.table.get(name) else {
            return Ty::Any;
        };
        self.capability(sig.name, sig.needs, n);
        if sig.is_overloaded() {
            return Ty::Any;
        }
        match sig.schemes().first() {
            Some(s) => self.u.instantiate(s),
            None => Ty::Any,
        }
    }

    /// A list literal; a list whose items are all pairs is a dict (with
    /// `duplicate-key` for a repeated literal key).
    fn list(&mut self, n: &Node, d: u32) -> Ty {
        let items = &n.children;
        if items.is_empty() {
            return Ty::List(Box::new(self.u.fresh()));
        }
        if items
            .iter()
            .all(|i| matches!(i.kind, NodeKind::Pair) && i.children.len() == 2)
        {
            return self.dict(n, d);
        }
        for item in items.iter() {
            self.infer(item, d);
        }
        self.list_join(items)
    }

    /// The element type of an inferred list literal.
    #[inline(never)]
    fn list_join(&mut self, items: &[Node]) -> Ty {
        let mut elem: Option<Ty> = None;
        for item in items.iter() {
            let t = self.ty_of(item);
            let t = match item.kind {
                NodeKind::Splat => match self.u.shallow(&t) {
                    Ty::List(e) => *e,
                    _ => Ty::Any,
                },
                _ => t,
            };
            elem = Some(match elem {
                None => t,
                Some(prev) => self.u.join(&prev, &t),
            });
        }
        Ty::List(Box::new(elem.unwrap_or(Ty::Any)))
    }

    fn dict(&mut self, n: &Node, d: u32) -> Ty {
        for pair in n.children.iter() {
            self.infer(&pair.children[0], d);
            self.infer(&pair.children[1], d);
        }
        self.dict_join(n)
    }

    /// The key and value types of an inferred dict literal, and
    /// `duplicate-key`.
    #[inline(never)]
    fn dict_join(&mut self, n: &Node) -> Ty {
        let mut seen: Vec<(String, &Node)> = Vec::new();
        let (mut key, mut val): (Option<Ty>, Option<Ty>) = (None, None);
        for pair in n.children.iter() {
            let (k, v) = (&pair.children[0], &pair.children[1]);
            if let Some(text) = literal_key(k) {
                if seen.iter().any(|(s, _)| *s == text) {
                    self.emit(
                        DiagCode::DuplicateKey,
                        k.span,
                        format!("duplicate key {text} in a dict literal; the last pair wins"),
                    );
                } else {
                    seen.push((text, k));
                }
            }
            let (kt, vt) = (self.ty_of(k), self.ty_of(v));
            self.types.insert(pair.id, Ty::List(Box::new(Ty::Any)));
            key = Some(match key {
                None => kt,
                Some(p) => self.u.join(&p, &kt),
            });
            val = Some(match val {
                None => vt,
                Some(p) => self.u.join(&p, &vt),
            });
        }
        Ty::Dict(
            Box::new(key.unwrap_or(Ty::Any)),
            Box::new(val.unwrap_or(Ty::Any)),
        )
    }

    /// A block: its statements in order, in a new scope of `kind` (or in
    /// the current scope for a `fn` or lambda body); the value is the last
    /// statement's.
    pub(crate) fn block(&mut self, n: &Node, d: u32, kind: Option<ScopeKind>) -> Ty {
        let mark = self.scopes.depth();
        if let Some(kind) = kind {
            self.scopes.push(kind);
        }
        let mut last = Ty::Nil;
        for stmt in n.children.iter() {
            last = self.infer(stmt, d);
        }
        self.scopes.truncate(mark);
        last
    }

    /// A body: a block shares the enclosing scope, anything else is one
    /// expression.
    fn body(&mut self, n: &Node, d: u32) -> Ty {
        match n.kind {
            NodeKind::Block => {
                let t = self.block(n, d, None);
                self.types.insert(n.id, t.clone());
                t
            }
            _ => self.infer(n, d),
        }
    }

    /// `(-> (params..) body)`: each lhs item is a parameter pattern.
    fn lambda(&mut self, n: &Node, d: u32) -> Ty {
        let Some((body, lhs)) = n.children.split_last() else {
            return Ty::Any;
        };
        let mark = self.scopes.depth();
        self.scopes.push(ScopeKind::Lambda);
        let params: Vec<Ty> = lhs
            .iter()
            .map(|p| {
                let t = self.u.fresh();
                self.bind_pattern(p, &t, BindKind::Param);
                t
            })
            .collect();
        let ret = self.body(body, d);
        self.scopes.truncate(mark);
        Ty::func(params, ret)
    }

    /// A kernel-headed call, a definition head, or an application.
    fn call(&mut self, n: &Node, d: u32) -> Ty {
        let head = n.children.first().and_then(Node::sym_name);
        match head {
            Some("let" | "var") if n.children.len() == 3 => self.binding_form(n, d),
            Some("upd") if n.children.len() == 3 => self.upd_form(n, d),
            Some("fn") if is_fn_form(n) => self.fn_form(n, d),
            Some("match") if n.children.len() == 3 => self.match_form(n, d),
            Some("enum" | "struct" | "inst" | "look") if self.def_head_free(head) => {
                self.definition(n, d)
            }
            _ => self.apply(n, d),
        }
    }

    /// True when a definition head word is not shadowed by a user binding.
    fn def_head_free(&self, head: Option<&str>) -> bool {
        head.is_some_and(|h| self.scopes.lookup(h).is_none() && self.env.global(h).is_none())
    }

    /// `(let TARGET EXPR)` / `(var TARGET EXPR)`.
    fn binding_form(&mut self, n: &Node, d: u32) -> Ty {
        let is_var = n.children[0].sym_name() == Some("var");
        let kind = if is_var { BindKind::Var } else { BindKind::Let };
        self.u.enter();
        let t = self.infer(&n.children[2], d);
        self.u.exit();
        self.bind_value(n, kind, t)
    }

    /// Binds the target of an inferred `let`/`var`.
    #[inline(never)]
    fn bind_value(&mut self, n: &Node, kind: BindKind, t: Ty) -> Ty {
        let t = if self.u.too_large(&t) { Ty::Any } else { t };
        let is_var = kind == BindKind::Var;
        let (target, value) = (&n.children[1], &n.children[2]);
        match &target.kind {
            NodeKind::Atom(Atom::Sym(name)) => {
                let generic = !is_var && is_syntactic_value(value);
                if !generic {
                    self.u.default_nums(&t);
                }
                let scheme = if generic {
                    self.u.generalize(&t)
                } else {
                    Scheme::mono(t.clone())
                };
                let b = Binding {
                    kind,
                    scheme,
                    span: target.span,
                    annot_any: false,
                    extra: BindExtra::Plain,
                };
                self.declare(name, target, b);
            }
            NodeKind::Pair => {
                if let Some((name, annot)) = self.annotated(target) {
                    if self.u.unify(&annot, &t).is_err() {
                        self.emit(
                            DiagCode::AnnotationMismatch,
                            value.span,
                            format!(
                                "`{name}` is annotated `{}` but the value is `{}`",
                                annot,
                                self.u.zonk(&t, true)
                            ),
                        );
                    }
                    let mut b = Binding::mono(kind, annot.clone(), target.span);
                    // Only a written `any` must be narrowed; an unknown type
                    // name reads as `any` without that obligation.
                    b.annot_any = target.children.get(1).and_then(Node::sym_name) == Some("any");
                    self.declare(&name, target, b);
                }
            }
            _ => {
                self.u.default_nums(&t);
                self.bind_pattern(target, &t, kind);
            }
        }
        t
    }

    /// `(upd NAME EXPR)`: only a `var` may be updated (5.6).
    fn upd_form(&mut self, n: &Node, d: u32) -> Ty {
        let (target, value) = (&n.children[1], &n.children[2]);
        let t = self.infer(value, d);
        let Some(name) = target.sym_name() else {
            return t;
        };
        match crate::types::scope::upd_problem(&self.scopes, &self.live, name) {
            Some((code, msg)) => self.emit(code, target.span, msg),
            None => {
                let declared = match self.scopes.lookup(name) {
                    Some(b) => Some(b.scheme.ty.clone()),
                    None => self
                        .env
                        .global(name)
                        .and_then(|g| g.scheme.as_ref().map(|s| s.ty.clone())),
                };
                if let Some(declared) = declared {
                    self.check_arg(&declared, &t, value, "`upd` value");
                }
            }
        }
        t
    }

    /// Unifies an argument into its expected type, reporting a clash.
    pub(crate) fn check_arg(&mut self, expected: &Ty, actual: &Ty, at: &Node, what: &str) {
        match self.u.unify(expected, actual) {
            Ok(()) => {}
            Err(Clash::Optional) => self.emit(
                DiagCode::OptionalAsValue,
                at.span,
                format!(
                    "{what} may be nil; supply a default with `?` before using it as `{}`",
                    self.u.zonk(expected, true)
                ),
            ),
            Err(Clash::Mismatch) => self.emit(
                DiagCode::TypeMismatch,
                at.span,
                format!(
                    "{what}: expected `{}`, found `{}`",
                    self.u.zonk(expected, true),
                    self.u.zonk(actual, true)
                ),
            ),
        }
    }

    /// Parses a `fn` header (`name`, `name: type`, `= default`,
    /// `-> RET`); a `fn` type annotation is flat after reading.
    fn header<'n>(&self, items: &'n [Node]) -> (Vec<Param<'n>>, Option<Ty>) {
        let mut out: Vec<Param<'n>> = Vec::new();
        let mut ret = None;
        let mut k = 0;
        while let Some(item) = items.get(k) {
            k += 1;
            match &item.kind {
                NodeKind::Atom(Atom::Op(Op::Arrow)) => {
                    ret = items.get(k).and_then(|t| self.annot_ty(t));
                    break;
                }
                NodeKind::Atom(Atom::Op(Op::Eq)) => {
                    if let (Some(last), Some(def)) = (out.last_mut(), items.get(k)) {
                        last.default = Some(def);
                    }
                    k += 1;
                }
                NodeKind::Atom(Atom::Sym(name)) => out.push(Param {
                    node: item,
                    name: Some(name.clone()),
                    annot: None,
                    default: None,
                }),
                NodeKind::Atom(Atom::Wildcard) | NodeKind::List => out.push(Param {
                    node: item,
                    name: None,
                    annot: None,
                    default: None,
                }),
                NodeKind::Pair => {
                    let name = match item.children.first().map(|c| &c.kind) {
                        Some(NodeKind::Atom(Atom::Keyword(n))) => Some(n.clone()),
                        _ => None,
                    };
                    let ty_node = item.children.get(1);
                    let annot = match ty_node.and_then(Node::sym_name) {
                        Some("fn") => {
                            let start = k;
                            while let Some(t) = items.get(k) {
                                k += 1;
                                if matches!(t.kind, NodeKind::Atom(Atom::Op(Op::Arrow))) {
                                    break;
                                }
                            }
                            let params: Vec<Ty> = items[start..k.saturating_sub(1).max(start)]
                                .iter()
                                .map(|t| self.annot_ty(t).unwrap_or(Ty::Any))
                                .collect();
                            let r = items
                                .get(k)
                                .and_then(|t| self.annot_ty(t))
                                .unwrap_or(Ty::Any);
                            k += 1;
                            Some(Ty::func(params, r))
                        }
                        Some("pattern") => {
                            let inner = items.get(k).and_then(|t| self.annot_ty(t));
                            k += 1;
                            Some(Ty::Pattern(Box::new(inner.unwrap_or(Ty::Any))))
                        }
                        _ => match ty_node.map(|t| &t.kind) {
                            Some(NodeKind::Atom(Atom::Op(Op::Question))) => {
                                let inner = items.get(k).and_then(|t| self.annot_ty(t));
                                k += 1;
                                Some(Ty::Opt(Box::new(inner.unwrap_or(Ty::Any))))
                            }
                            _ => ty_node.and_then(|t| self.annot_ty(t)),
                        },
                    };
                    out.push(Param {
                        node: item,
                        name,
                        annot,
                        default: None,
                    });
                }
                _ => {}
            }
        }
        (out, ret)
    }

    /// `(fn NAME HEADER.. Block)`: parameters and the body's statements
    /// share the fn scope; generalized at the end (let-polymorphism).
    fn fn_form(&mut self, n: &Node, d: u32) -> Ty {
        let ch = &n.children;
        let name_node = &ch[1];
        let name: Rc<str> = Rc::from(name_node.sym_name().unwrap_or("_"));
        let at_session = self.scopes.at_session();
        self.u.enter();
        let self_ty = self.u.fresh();
        self.declare(
            &name,
            name_node,
            Binding::mono(BindKind::Fn, self_ty.clone(), name_node.span),
        );
        let (params, ret_annot) = self.header(&ch[2..ch.len() - 1]);
        let mark = self.scopes.depth();
        self.scopes.push(ScopeKind::Fn);
        let mut positional = Vec::new();
        // The `infer_masks` entry of each positional parameter: the mask has
        // one entry per header parameter (keyword ones included) and none
        // for `_`.
        let mut mask_of = Vec::new();
        let mut mask_k = 0usize;
        let mut keywords = Vec::new();
        for p in &params {
            let entry = match p.node.kind {
                NodeKind::Atom(Atom::Wildcard) => None,
                _ => {
                    mask_k += 1;
                    Some(mask_k - 1)
                }
            };
            let t = p.annot.clone().unwrap_or_else(|| self.u.fresh());
            match &p.name {
                Some(pname) => {
                    let mut b = Binding::mono(BindKind::Param, t.clone(), p.node.span);
                    b.annot_any = p.annot == Some(Ty::Any);
                    self.declare(pname, p.node, b);
                    self.types.insert(p.node.id, t.clone());
                }
                None => self.bind_pattern(p.node, &t, BindKind::Param),
            }
            match (p.default, &p.name) {
                (Some(def), Some(pname)) => {
                    let dt = self.infer(def, d);
                    if self.u.unify(&t, &dt).is_err() {
                        let code = if p.annot.is_some() {
                            DiagCode::AnnotationMismatch
                        } else {
                            DiagCode::TypeMismatch
                        };
                        self.emit(
                            code,
                            def.span,
                            format!(
                                "the default of `{pname}` does not fit its type `{}`",
                                self.u.zonk(&t, true)
                            ),
                        );
                    }
                    keywords.push((pname.clone(), t));
                }
                _ => {
                    positional.push(t);
                    mask_of.push(entry);
                }
            }
        }
        let body = &ch[ch.len() - 1];
        let body_ty = self.body(body, d);
        if let Some(r) = &ret_annot {
            if self.u.unify(r, &body_ty).is_err() {
                self.emit(
                    DiagCode::AnnotationMismatch,
                    body.span,
                    format!(
                        "`{name}` is annotated to return `{r}` but returns `{}`",
                        self.u.zonk(&body_ty, true)
                    ),
                );
            }
        }
        self.scopes.truncate(mark);
        // Thunk parameter typing (5.5): a parameter the body calls as a
        // statement receives the closure.
        let mask = infer_masks(n, &self.live);
        let pos_mask = ForcingMask(
            mask_of
                .iter()
                .map(|e| match e.and_then(|k| mask.0.get(k)) {
                    Some(entry) => entry.clone(),
                    None => MaskEntry::Value,
                })
                .collect(),
        );
        for (t, entry) in positional.iter().zip(pos_mask.0.iter()) {
            if *entry == MaskEntry::Fn && matches!(self.u.shallow(t), Ty::Var(_)) {
                let r = self.u.fresh();
                let _ = self.u.unify(t, &Ty::func(Vec::new(), r));
            }
        }
        let live = &self.live;
        let mixed = mixed_forcing(&mask, &|c| static_mask(live, c));
        for pos in mixed {
            self.emit(
                DiagCode::MixedForcing,
                name_node.span,
                format!("parameter {} of `{name}` is forwarded both as a value and as a closure; one call site forces it", pos + 1),
            );
        }
        let fn_ty = Ty::func(positional, body_ty);
        let _ = self.u.unify(&self_ty, &fn_ty);
        self.u.exit();
        self.u.default_nums(&fn_ty);
        let scheme = self.u.generalize(&fn_ty);
        // Call sites index the document fn's mask by positional argument.
        let extra = BindExtra::Fn {
            keywords: keywords.into(),
            mask: pos_mask,
        };
        self.scopes.update(&name, scheme.clone(), extra);
        if at_session {
            self.live.globals.insert(
                name.clone(),
                GlobalInfo {
                    kind: BindKind::Fn,
                    scheme: Some(scheme),
                    mask: Some(mask),
                    span: Some(name_node.span),
                },
            );
        }
        fn_ty
    }

    /// `(match SUBJ {clauses})`.
    fn match_form(&mut self, n: &Node, d: u32) -> Ty {
        let s_ty = self.infer(&n.children[1], d);
        self.bare_variant_binding(n);
        let mut result: Option<Ty> = None;
        let mut after_falsy = false;
        for clause in n.children[2].children.iter() {
            let t = self.clause(clause, &s_ty, &mut after_falsy, d);
            result = Some(match result {
                None => t,
                Some(prev) => self.u.join(&prev, &t),
            });
        }
        self.missing_variant(n, &s_ty);
        result.unwrap_or(Ty::Nil)
    }

    /// One clause in its own scope: pattern bindings, guard, body. After a
    /// `false | nil` clause a `?T` subject binds as `T`.
    fn clause(&mut self, clause: &Node, s_ty: &Ty, after_falsy: &mut bool, d: u32) -> Ty {
        let Some((body, lhs)) = clause.children.split_last() else {
            return Ty::Nil;
        };
        let mark = self.scopes.depth();
        self.scopes.push(ScopeKind::Clause);
        let guard = self.open_clause(lhs, s_ty, after_falsy);
        for g in &lhs[guard..] {
            self.infer(g, d);
        }
        let t = match body.kind {
            NodeKind::Block => self.block(body, d, Some(ScopeKind::Block)),
            _ => self.infer(body, d),
        };
        self.types.insert(body.id, t.clone());
        self.scopes.truncate(mark);
        t
    }

    /// Binds a clause's pattern and returns where its guard starts.
    #[inline(never)]
    fn open_clause(&mut self, lhs: &[Node], s_ty: &Ty, after_falsy: &mut bool) -> usize {
        let split = lhs.iter().position(|i| i.sym_name() == Some("if"));
        let (pattern, guard) = match split {
            Some(k) => (&lhs[..k], k + 1),
            None => (lhs, lhs.len()),
        };
        let bound_ty = match self.u.shallow(s_ty) {
            Ty::Opt(inner) if *after_falsy => *inner,
            other => other,
        };
        self.clause_pattern(pattern, &bound_ty);
        *after_falsy |= is_falsy_lhs(pattern);
        guard
    }
}

/// The control-map pattern `ctl`.
pub(crate) fn ctl() -> Ty {
    Ty::Pattern(Box::new(Ty::Dict(
        Box::new(Ty::keyword()),
        Box::new(Ty::Any),
    )))
}

/// `(fn NAME HEADER* Block)`.
fn is_fn_form(n: &Node) -> bool {
    let ch = &n.children;
    ch.len() >= 3
        && ch[1].sym_name().is_some()
        && ch.last().is_some_and(|b| matches!(b.kind, NodeKind::Block))
}

/// A syntactic value (the value restriction of let-polymorphism): a
/// lambda, a literal, or a list of values.
fn is_syntactic_value(n: &Node) -> bool {
    match n.kind {
        NodeKind::Arrow => true,
        NodeKind::Atom(Atom::Sym(_)) => false,
        NodeKind::Atom(_) => true,
        NodeKind::List | NodeKind::Pair => n.children.iter().all(is_syntactic_value),
        _ => false,
    }
}

/// A literal dict key as comparable text (`duplicate-key`).
fn literal_key(k: &Node) -> Option<String> {
    match &k.kind {
        NodeKind::Atom(Atom::Keyword(s)) => Some(format!(":{s}")),
        NodeKind::Atom(Atom::Str(s)) => Some(format!("{s:?}")),
        NodeKind::Atom(Atom::Int(v)) => Some(v.to_string()),
        NodeKind::Atom(Atom::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}
