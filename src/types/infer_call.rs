//! Call inference (design 7, 7.1.4): prelude natives with arity, named
//! arguments, the `scale`/`shape` overloads and the per-native rules;
//! `s`/`sound` with `kit:`, the sound-kit keyword check and SOUND FIRST;
//! user functions, index and key calls, struct constructors; and the
//! hygiene and forcing warnings that are decided at a call site.

#[path = "song_rules.rs"]
mod song_rules;

use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::types::check::Checker;
use crate::types::diag::DiagCode;
use crate::types::infer::ctl;
use crate::types::masks::{chase, static_mask, CalleeRef, EffectiveEntry, MaskEntry};
use crate::types::natives::{HostCap, NativeMask, NativeSig};
use crate::types::scope::BindExtra;
use crate::types::ty::Ty;

/// Natives whose operands must be numbers: a `?T` operand is
/// `optional-as-value` and an unnarrowed `any` is `any-not-narrowed`.
const NUMERIC: [&str; 20] = [
    "+", "-", "*", "/", "mod", "<", ">", "<=", ">=", "..", "min", "max", "abs", "neg", "sin",
    "cos", "round", "int", "int64", "float",
];

/// Step constructors whose arguments are steps (all of them for `alt`
/// and `choose`, the first for the rest): a keyword there is a sound.
pub(crate) const STEP_ALL: [&str; 2] = ["alt", "choose"];
pub(crate) const STEP_FIRST: [&str; 6] = ["maybe", "euclid", "hold", "repeat", "fast", "slow"];

/// Natives that consume a whole source (`unbounded-source` on `0..`).
const CONSUMERS: [&str; 8] = [
    "map",
    "filter",
    "reduce",
    "enumerate",
    "sort",
    "reverse",
    "len",
    "last",
];

/// The arguments of a call, split: positional, named pairs, and whether a
/// splat makes the positional count unknown.
pub(crate) struct Args<'n> {
    pub(crate) positional: Vec<&'n Node>,
    pub(crate) named: Vec<&'n Node>,
    pub(crate) splat: bool,
}

pub(crate) fn split_args(args: &[Node], pairs_named: bool) -> Args<'_> {
    let mut out = Args {
        positional: Vec::new(),
        named: Vec::new(),
        splat: false,
    };
    for a in args {
        match a.kind {
            NodeKind::Pair if pairs_named => out.named.push(a),
            NodeKind::Splat => out.splat = true,
            _ => out.positional.push(a),
        }
    }
    out
}

pub(crate) fn pair_key(p: &Node) -> Option<&str> {
    match &p.children.first()?.kind {
        NodeKind::Atom(Atom::Keyword(k)) => Some(k),
        _ => None,
    }
}

impl Checker<'_> {
    /// The native a call head names, when it resolves to the prelude: an
    /// operator, an expander `Builtin`, or a name no user scope, session
    /// binding or open import rebinds.
    pub(crate) fn prelude_head(&self, head: &Node) -> Option<&'static NativeSig> {
        let name: &str = match &head.kind {
            NodeKind::Atom(Atom::Op(op)) => op.as_str(),
            NodeKind::Atom(Atom::Builtin(name)) => name,
            NodeKind::Atom(Atom::Sym(name)) => {
                if self.scopes.lookup(name).is_some()
                    || self.env.global(name).is_some()
                    || self.live.open_prefix(name).is_some()
                {
                    return None;
                }
                name
            }
            _ => return None,
        };
        self.table.get(name).map(|(_, sig)| sig)
    }

    /// True when `name` is only an `inst` header parameter of the
    /// document, so it is a control name at a call head (M3).
    fn inst_control(&self, name: &str) -> bool {
        self.inst_controls.contains(name)
            && self.scopes.lookup(name).is_none()
            && self.env.global(name).is_none()
            && self.live.open_prefix(name).is_none()
            && self.table.get(name).is_none()
    }

    /// `beyond-capability` when the host lacks what a native needs.
    pub(crate) fn capability(&mut self, name: &str, needs: &[HostCap], at: &Node) {
        if !self.manifest.has_caps(needs) {
            self.emit(
                DiagCode::BeyondCapability,
                at.span,
                format!("`{name}` needs a host capability ({needs:?}) this host does not have"),
            );
        }
    }

    /// An application `(head args..)`.
    pub(crate) fn apply(&mut self, n: &Node, d: u32) -> Ty {
        let Some((head, args)) = n.children.split_first() else {
            return Ty::Nil;
        };
        if let Some(sig) = self.prelude_head(head) {
            if sig.kind == crate::types::natives::NativeKind::Function {
                self.types.insert(head.id, Ty::Any);
                return match sig.name {
                    "s" | "sound" => song_rules::lazy_sound_call(self, n, sig, args, d)
                        .unwrap_or_else(|| self.sound_call(n, sig, args, d)),
                    _ => self.native_call(n, sig, args, d),
                };
            }
        }
        if head.sym_name().is_some_and(|h| self.inst_control(h)) {
            // An `inst` header parameter used as a pattern step is a
            // control name (M3): `s :pluck > cutoff {..}`.
            let control = Ty::func(vec![ctl(), Ty::Any], ctl());
            self.types.insert(head.id, control.clone());
            return self.value_call(n, head, &control, args, d);
        }
        let callee = self.infer(head, d);
        self.value_call(n, head, &callee, args, d)
    }

    /// Infers every argument (a pair through its value) in order. Call
    /// recursion goes through here only, and every check runs after it
    /// returns, so a deep pipe keeps small stack frames (7.1.5).
    #[inline(never)]
    fn infer_each(&mut self, args: &[Node], d: u32) {
        for a in args {
            self.infer(a, d);
        }
    }

    /// The recorded type of an inferred node.
    pub(crate) fn ty_of(&self, n: &Node) -> Ty {
        self.types.get(&n.id).cloned().unwrap_or(Ty::Any)
    }

    /// Checks an inferred argument against its expected type: a `?T`
    /// operand of a numeric native, an unnarrowed `any`, a `{..}` block
    /// where `fn -> R` is expected (the thunk itself, 5.5), then
    /// unification.
    fn check_one(&mut self, a: &Node, expected: Option<&Ty>, numeric: bool, what: &str) {
        let t = self.ty_of(a);
        if numeric && matches!(self.u.shallow(&t), Ty::Opt(_)) {
            self.emit(
                DiagCode::OptionalAsValue,
                a.span,
                format!("{what} may be nil; supply a default with `?` first"),
            );
            return;
        }
        let Some(exp) = expected else {
            return;
        };
        let exp_open = matches!(self.u.shallow(exp), Ty::Any | Ty::Var(_));
        if let Some(name) = a.sym_name() {
            let any = self.scopes.lookup(name).is_some_and(|b| b.annot_any);
            if any && (numeric || !exp_open) {
                self.emit(
                    DiagCode::AnyNotNarrowed,
                    a.span,
                    format!("`{name}` is `any`; narrow it with a `match` or `if` pattern before this use"),
                );
                return;
            }
        }
        if matches!(a.kind, NodeKind::Block) {
            if let Ty::Fn(ps, r) = self.u.shallow(exp) {
                if ps.is_empty() {
                    self.check_arg(&r, &t, a, what);
                    return;
                }
            }
        }
        self.check_arg(exp, &t, a, what);
    }

    /// A call to a prelude function.
    fn native_call(&mut self, n: &Node, sig: &'static NativeSig, args: &[Node], d: u32) -> Ty {
        self.infer_each(args, d);
        self.native_check(n, sig, args)
    }

    /// The checks of a prelude call, after its arguments are inferred.
    #[inline(never)]
    fn native_check(&mut self, n: &Node, sig: &'static NativeSig, args: &[Node]) -> Ty {
        let head = &n.children[0];
        self.capability(sig.name, sig.needs, head);
        let a = split_args(args, !sig.keywords.is_empty());
        for p in &a.named {
            let key = pair_key(p).unwrap_or("");
            if !sig.keywords.contains(&key) {
                self.emit(
                    DiagCode::TypeMismatch,
                    p.span,
                    format!("`{}` has no named argument `{key}:`", sig.name),
                );
            }
        }
        let count = a.positional.len();
        let arity_ok = count >= usize::from(sig.min_args)
            && sig.max_args.map_or(true, |m| count <= usize::from(m));
        if !a.splat && !arity_ok {
            let want = match sig.max_args {
                Some(m) if m == sig.min_args => format!("{m}"),
                Some(m) => format!("{}..{m}", sig.min_args),
                None => format!("{} or more", sig.min_args),
            };
            self.emit(
                DiagCode::TypeMismatch,
                n.span,
                format!("`{}` takes {want} arguments, found {count}", sig.name),
            );
        }
        if let Some(t) = self.dsp_call(sig, &a) {
            return t;
        }
        let numeric = NUMERIC.contains(&sig.name);
        // The overload group resolves on the first argument's type (M1); a
        // call without one takes the first scheme (bare `render`).
        let scheme = if sig.is_overloaded() {
            match a.positional.first().map(|f| self.ty_of(f)) {
                Some(t) => self.overload(sig, &t),
                None => sig.schemes().into_iter().next(),
            }
        } else {
            sig.schemes().into_iter().next()
        };
        let Some(scheme) = scheme else {
            return Ty::Any;
        };
        let (params, ret) = match self.u.instantiate(&scheme) {
            Ty::Fn(ps, r) => (ps.into_vec(), *r),
            other => (Vec::new(), other),
        };
        let variadic = sig.max_args.is_none();
        // After an arity error the positions do not line up, and `or`/`and`
        // take operands of any type (they return one of them).
        let per_arg = (arity_ok || a.splat) && !matches!(sig.name, "or" | "and");
        for (k, p) in a.positional.iter().enumerate().filter(|_| per_arg) {
            if sig.name == "transform-instrument" && k == 3 {
                // Song source callbacks admit two concrete pattern views locally.
                continue;
            }
            let expected = match params.get(k) {
                Some(t) => Some(t.clone()),
                None if variadic => params.last().cloned(),
                None => None,
            };
            let what = format!("argument {} of `{}`", k + 1, sig.name);
            self.check_one(p, expected.as_ref(), numeric, &what);
        }
        let arg_tys: Vec<Ty> = a.positional.iter().map(|p| self.ty_of(p)).collect();
        self.native_rules(n, sig, &a, &params);
        song_rules::check(self, n, sig, &a);
        match sig.name {
            "or" | "and" => self.truthy_result(&arg_tys),
            "/" if arg_tys.iter().all(|t| self.int_like(t)) => Ty::Ratio,
            "+" | "-" | "*" | "/" | "mod" | "min" | "max" => self.arith_result(&arg_tys, ret),
            _ => ret,
        }
    }

    /// A ugen operand of `+ - *` or a ugen subject of a name shared with a
    /// control, signal or visual selects the DSP node (12.8.6, B2); `osc`
    /// on a string is an OSC sound.
    fn dsp_call(&self, sig: &NativeSig, a: &Args<'_>) -> Option<Ty> {
        let ty = |p: &Node| self.u.shallow(&self.ty_of(p));
        let first = a.positional.first().map(|p| ty(p));
        match sig.name {
            "osc" if first == Some(Ty::Str) => Some(Ty::Sound),
            "+" | "-" | "*" if a.positional.iter().any(|p| ty(p) == Ty::UGen) => Some(Ty::UGen),
            "gain" | "pan" | "lpf" | "hpf" | "room" | "delay" | "saturate" | "range"
                if first == Some(Ty::UGen) =>
            {
                Some(Ty::UGen)
            }
            _ => None,
        }
    }

    /// The widest operand type (6.5.3 widening; a float in the expression
    /// makes the result float). All-literal operands keep the scheme's
    /// result, so the literal still adapts to its context.
    fn arith_result(&self, tys: &[Ty], ret: Ty) -> Ty {
        let rank = |t: &Ty| match t {
            Ty::Int => 1,
            Ty::Int64 => 2,
            Ty::Ratio => 3,
            Ty::Float => 4,
            Ty::Float64 => 5,
            Ty::Signal => 6,
            _ => 0,
        };
        let shallow: Vec<Ty> = tys.iter().map(|t| self.u.shallow(t)).collect();
        if shallow.iter().all(|t| matches!(t, Ty::NumLit(_))) {
            return ret;
        }
        let widest = tys
            .iter()
            .map(|t| self.u.zonk(t, true))
            .max_by_key(&rank)
            .unwrap_or(Ty::Any);
        if rank(&widest) == 0 {
            ret
        } else {
            widest
        }
    }

    fn int_like(&self, t: &Ty) -> bool {
        match self.u.shallow(t) {
            Ty::Int | Ty::Int64 => true,
            Ty::NumLit(_) => matches!(self.u.zonk(t, true), Ty::Int),
            _ => false,
        }
    }

    /// `or`/`and`: every operand but the last loses its `?`.
    fn truthy_result(&mut self, tys: &[Ty]) -> Ty {
        let Some((last, init)) = tys.split_last() else {
            return Ty::Nil;
        };
        let mut acc = last.clone();
        for t in init {
            let t = match self.u.shallow(t) {
                Ty::Opt(inner) => *inner,
                Ty::Nil => continue,
                other => other,
            };
            acc = self.u.join(&t, &acc);
        }
        acc
    }

    /// Picks the scheme of the overload group from the subject type (M1): a
    /// pattern (or list), texture, number, keyword, sound or ugen subject
    /// selects the scheme whose first parameter takes it (`scale`/`shape`,
    /// and the 14.5.9 `scope`/`spectrum`/`render`). `None` when the subject
    /// type is not known (the VM dispatches on the tag).
    fn overload(&mut self, sig: &NativeSig, subject: &Ty) -> Option<crate::types::ty::Scheme> {
        let schemes = sig.schemes();
        let subject = self.u.shallow(subject);
        let takes = |first: Option<&Ty>| {
            matches!(
                (&subject, first),
                (Ty::Pattern(_) | Ty::List(_), Some(Ty::Pattern(_)))
                    | (Ty::Tex, Some(Ty::Tex))
                    | (Ty::Sound, Some(Ty::Sound))
                    | (Ty::UGen, Some(Ty::UGen))
                    | (Ty::KeywordOf(_), Some(Ty::KeywordOf(_)))
                    | (
                        Ty::Int | Ty::Int64 | Ty::Float | Ty::Float64 | Ty::Ratio | Ty::NumLit(_),
                        Some(Ty::Float | Ty::Any),
                    )
            )
        };
        schemes.into_iter().find(|s| match &s.ty {
            Ty::Fn(ps, _) => takes(ps.first()),
            _ => false,
        })
    }

    /// The per-native rules of design 7 and 7.1.4.
    fn native_rules(&mut self, n: &Node, sig: &'static NativeSig, a: &Args<'_>, params: &[Ty]) {
        let pos = &a.positional;
        match sig.name {
            "/" => {
                for p in pos.iter().skip(1) {
                    if is_literal_zero(p) {
                        self.emit(
                            DiagCode::LiteralDivisionByZero,
                            p.span,
                            "division by a literal zero",
                        );
                    }
                }
            }
            "midi" => {
                if let Some(NodeKind::Atom(Atom::Int(ch))) = pos.first().map(|p| &p.kind) {
                    if !(1..=16).contains(ch) {
                        self.emit(
                            DiagCode::TypeMismatch,
                            pos[0].span,
                            format!("MIDI channel {ch} is outside 1..16"),
                        );
                    }
                }
            }
            "use-clock" => {
                if let Some(NodeKind::Atom(Atom::Keyword(k))) = pos.first().map(|p| &p.kind) {
                    match &**k {
                        "internal" | "midi" => {}
                        "link" => self.emit(
                            DiagCode::ClockSourceUnavailable,
                            pos[0].span,
                            "the `:link` clock source is planned and not available",
                        ),
                        other => self.emit(
                            DiagCode::UnknownKeyword,
                            pos[0].span,
                            format!("`:{other}` is not a clock source (`:internal`, `:midi`)"),
                        ),
                    }
                }
            }
            "slice" | "splice" => {
                if let Some(points) = pos.get(1) {
                    self.slice_points(points);
                }
            }
            "chord" => {
                let target = if pos.len() >= 2 {
                    pos.get(1)
                } else {
                    pos.first()
                };
                if let Some(t) = target {
                    self.chord_arg(t, 0);
                }
            }
            name if CONSUMERS.contains(&name) => {
                if let Some(src) = pos.first() {
                    if is_open_range(src) {
                        self.emit(
                            DiagCode::UnboundedSource,
                            n.span,
                            format!("`{name}` over the open range `{}` never ends; bound it with `take`", "0.."),
                        );
                    }
                }
            }
            _ => {}
        }
        // Effects inside the deferred arguments of a pattern constructor
        // run at query time (10.4): `effect-in-pattern`.
        let returns_pattern = sig
            .schemes()
            .iter()
            .any(|s| matches!(&s.ty, Ty::Fn(_, r) if matches!(**r, Ty::Pattern(_))));
        if returns_pattern {
            for (k, p) in pos.iter().enumerate() {
                let time_source = sig.entry_at(k) == Some(NativeMask::Value)
                    && params
                        .get(k)
                        .is_some_and(|ty| matches!(self.u.shallow(ty), Ty::Pattern(_)))
                    && matches!(self.u.shallow(&self.ty_of(p)),Ty::Fn(ref ps,_) if ps.len()==1);
                if time_source && self.contains_query_effect(p, 0) {
                    // A known effect invalidates this newly coerced query function;
                    // the ordinary deferred-argument diagnostic remains a warning.
                    if self.mute == 0 {
                        self.diags.push(crate::types::diag::Diagnostic::error(
                            DiagCode::EffectInPattern,
                            p.span,
                            format!(
                                "an effectful time function inside a `{}` argument runs at every query",
                                sig.name
                            ),
                        ));
                    }
                } else if matches!(sig.entry_at(k), Some(NativeMask::Late | NativeMask::Fn))
                    && self.contains_effect(p, 0)
                {
                    self.emit(
                        DiagCode::EffectInPattern,
                        p.span,
                        format!(
                            "an effectful call inside a `{}` argument runs at every query",
                            sig.name
                        ),
                    );
                }
            }
        }
    }

    /// `bad-slice-points`: literal manual points strictly ascending in [0, 1].
    fn slice_points(&mut self, points: &Node) {
        if !matches!(points.kind, NodeKind::List) || points.children.is_empty() {
            return;
        }
        let vals: Option<Vec<f64>> = points.children.iter().map(literal_number).collect();
        let Some(vals) = vals else {
            return;
        };
        let in_range = vals.iter().all(|v| (0.0..=1.0).contains(v));
        let ascending = vals.windows(2).all(|w| w[0] < w[1]);
        if !in_range || !ascending {
            self.emit(
                DiagCode::BadSlicePoints,
                points.span,
                "manual slice points must be strictly ascending within 0..1",
            );
        }
    }

    /// Chord qualities of a literal chord `[root quality]`, through lists,
    /// blocks and step constructors.
    fn chord_arg(&mut self, n: &Node, depth: u32) {
        if depth > 64 {
            return;
        }
        match &n.kind {
            NodeKind::List => {
                let kw = |c: &Node| matches!(c.kind, NodeKind::Atom(Atom::Keyword(_)));
                if n.children.len() == 2 && n.children.iter().all(kw) {
                    if let NodeKind::Atom(Atom::Keyword(q)) = &n.children[1].kind {
                        if !crate::types::chords::is_chord_quality(q) {
                            self.emit(
                                DiagCode::UnknownKeyword,
                                n.children[1].span,
                                format!("`:{q}` is not a chord quality (for example `:maj :m :dom7 :maj9 :sus4`)"),
                            );
                        }
                    }
                } else {
                    for c in n.children.iter() {
                        self.chord_arg(c, depth + 1);
                    }
                }
            }
            NodeKind::Block => {
                if let Some(last) = n.children.last() {
                    self.chord_arg(last, depth + 1);
                }
            }
            NodeKind::Call => {
                let step = n
                    .children
                    .first()
                    .and_then(|h| self.prelude_head(h))
                    .is_some_and(|s| STEP_ALL.contains(&s.name) || STEP_FIRST.contains(&s.name));
                if step {
                    for c in n.children.iter().skip(1) {
                        self.chord_arg(c, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }

    /// True when the subtree calls an effectful prelude native.
    pub(super) fn contains_effect(&self, n: &Node, depth: u32) -> bool {
        if depth > 256 {
            return false;
        }
        if let NodeKind::Call = n.kind {
            if let Some(sig) = n.children.first().and_then(|head| self.prelude_head(head)) {
                if sig.effectful {
                    return true;
                }
            }
        }
        n.children
            .iter()
            .any(|child| self.contains_effect(child, depth + 1))
    }
    /// Known local query effects, retaining lambda facts from their lexical scope.
    pub(super) fn contains_query_effect(&self, n: &Node, depth: u32) -> bool {
        if depth > 256 {
            return false;
        }
        if let Some(effect) = self.scopes.node_query_effect(n.id) {
            return effect;
        }
        if self.prelude_head(n).is_some_and(|sig| sig.effectful) {
            return true;
        }
        if n.sym_name()
            .and_then(|name| self.scopes.lookup(name))
            .is_some_and(|b| b.query_effect)
        {
            return true;
        }
        if let NodeKind::Call = n.kind {
            if n.children
                .first()
                .and_then(Node::sym_name)
                .and_then(|name| self.scopes.lookup(name))
                .is_some_and(|b| b.query_effect)
            {
                return true;
            }
            if let Some(sig) = n.children.first().and_then(|h| self.prelude_head(h)) {
                if sig.effectful {
                    return true;
                }
            }
        }
        n.children
            .iter()
            .any(|c| self.contains_query_effect(c, depth + 1))
    }

    /// A call whose head is a value: a user or local function, a list or
    /// dict (index and key lookup), a struct constructor or value.
    fn value_call(&mut self, n: &Node, head: &Node, callee: &Ty, args: &[Node], d: u32) -> Ty {
        self.infer_each(args, d);
        self.value_check(n, head, callee, args)
    }

    /// The checks of a value call, after its arguments are inferred.
    #[inline(never)]
    fn value_check(&mut self, n: &Node, head: &Node, callee: &Ty, args: &[Node]) -> Ty {
        let binding = head.sym_name().and_then(|s| self.scopes.lookup(s)).cloned();
        if let Some(BindExtra::Struct(id)) = binding.as_ref().map(|b| &b.extra) {
            let id = *id;
            self.struct_ctor(id, args);
            return Ty::Named(id);
        }
        match self.u.shallow(callee) {
            Ty::Fn(params, ret) => {
                let persisted = if binding.is_none() {
                    match &head.kind {
                        NodeKind::Atom(Atom::Qualified { prefix, name }) => self
                            .env
                            .qualified_callables
                            .get(prefix)
                            .and_then(|m| m.get(name))
                            .cloned(),
                        _ => head.sym_name().and_then(|name| {
                            if self.env.global(name).is_some() {
                                self.env.global_callables.get(name).cloned()
                            } else {
                                self.live
                                    .open_prefix(name)
                                    .and_then(|prefix| {
                                        self.env
                                            .qualified_callables
                                            .get(prefix)
                                            .and_then(|m| m.get(name))
                                    })
                                    .cloned()
                            }
                        }),
                    }
                } else {
                    None
                };
                let mut persisted_keywords = None;
                if let Some(schema) = persisted {
                    if let Ty::Fn(all, result) = self.u.instantiate(&schema.signature) {
                        if all.len() == schema.positional + schema.keywords.len() {
                            let view =
                                Ty::func(all[..schema.positional].to_vec(), (*result).clone());
                            let _ = self.u.try_unify(&view, callee);
                            persisted_keywords = Some(std::rc::Rc::from(
                                schema
                                    .keywords
                                    .into_iter()
                                    .zip(all[schema.positional..].iter().cloned())
                                    .collect::<Vec<_>>(),
                            ));
                        }
                    }
                }
                let keywords = match binding.as_ref().map(|b| &b.extra) {
                    Some(BindExtra::Fn { keywords, .. }) => Some(keywords.clone()),
                    _ => persisted_keywords,
                };
                let a = split_args(args, keywords.is_some());
                for p in &a.named {
                    let key = pair_key(p).unwrap_or("");
                    match keywords
                        .as_ref()
                        .and_then(|ks| ks.iter().find(|(k, _)| &**k == key))
                    {
                        Some((_, t)) => {
                            if let Some(v) = p.children.get(1) {
                                let (t, vt) = (t.clone(), self.ty_of(v));
                                self.check_arg(&t, &vt, v, &format!("`{key}:`"));
                            }
                        }
                        None => self.emit(
                            DiagCode::TypeMismatch,
                            p.span,
                            format!("this function has no keyword parameter `{key}:`"),
                        ),
                    }
                }
                if !a.splat && a.positional.len() != params.len() {
                    self.emit(
                        DiagCode::TypeMismatch,
                        n.span,
                        format!(
                            "this function takes {} arguments, found {}",
                            params.len(),
                            a.positional.len()
                        ),
                    );
                }
                let mask = self.callee_mask(head);
                for (k, p) in a.positional.iter().enumerate() {
                    let what = format!("argument {}", k + 1);
                    self.check_one(p, params.get(k), false, &what);
                    if matches!(p.kind, NodeKind::Block) {
                        self.latent_forcing(p, mask.as_ref(), k);
                    }
                }
                *ret
            }
            Ty::Var(_) => {
                let a = split_args(args, true);
                let params: Vec<Ty> = a.positional.iter().map(|p| self.ty_of(p)).collect();
                let ret = self.u.fresh();
                if !a.splat {
                    let _ = self.u.unify(callee, &Ty::func(params, ret.clone()));
                }
                ret
            }
            Ty::List(e) => Ty::Opt(e),
            Ty::Dict(_, v) => Ty::Opt(v),
            Ty::UGen => {
                let a = split_args(args, true);
                let valid = if a.named.is_empty() && !a.splat && a.positional.len() == 1 {
                    let selector = self.ty_of(a.positional[0]);
                    matches!(self.u.shallow(&selector), Ty::KeywordOf(_))
                        || matches!(self.u.shallow(&selector), Ty::Var(_) | Ty::Any)
                        || self.int_like(&selector)
                } else {
                    false
                };
                if !valid {
                    self.emit(
                        DiagCode::TypeMismatch,
                        n.span,
                        "a ugen is called with one output name or index",
                    );
                }
                Ty::UGen
            }
            Ty::Named(id) => self.field_access(id, args),
            Ty::Nil => Ty::Nil,
            Ty::Any | Ty::Opt(_) => Ty::Any,
            other => {
                self.emit(
                    DiagCode::TypeMismatch,
                    head.span,
                    format!("a `{}` value is not callable", self.u.zonk(&other, true)),
                );
                Ty::Any
            }
        }
    }

    /// `v :amp` on a struct: the field's type; an unknown literal field
    /// is `unknown-keyword` (a struct is closed).
    fn field_access(&mut self, id: crate::types::ty::TypeId, args: &[Node]) -> Ty {
        let Some(crate::types::check::TypeDef::Struct { name, fields }) = self.def(id).cloned()
        else {
            return Ty::Any;
        };
        if let [key] = args {
            if let NodeKind::Atom(Atom::Keyword(k)) = &key.kind {
                return match fields.iter().find(|f| f.name == *k) {
                    Some(f) => f.ty.clone(),
                    None => {
                        self.emit(
                            DiagCode::UnknownKeyword,
                            key.span,
                            format!("`:{k}` is not a field of `{name}`"),
                        );
                        Ty::Any
                    }
                };
            }
        }
        Ty::Any
    }

    /// A struct constructor call: named fields must exist and fit.
    fn struct_ctor(&mut self, id: crate::types::ty::TypeId, args: &[Node]) {
        let fields = match self.def(id) {
            Some(crate::types::check::TypeDef::Struct { fields, .. }) => fields.clone(),
            _ => Vec::new(),
        };
        for a in args.iter().filter(|a| matches!(a.kind, NodeKind::Pair)) {
            let key = pair_key(a).unwrap_or("");
            match fields.iter().find(|f| &*f.name == key) {
                Some(f) => {
                    if let Some(v) = a.children.get(1) {
                        let vt = self.ty_of(v);
                        self.check_arg(&f.ty, &vt, v, &format!("field `{key}`"));
                    }
                }
                None => self.emit(
                    DiagCode::TypeMismatch,
                    a.span,
                    format!("`{key}` is not a field of this struct"),
                ),
            }
        }
    }

    /// The statically known mask of a callee: a document `fn` or a session
    /// binding.
    fn callee_mask(&self, head: &Node) -> Option<crate::types::masks::ForcingMask> {
        let name = head.sym_name()?;
        match self.scopes.lookup(name) {
            Some(b) => match &b.extra {
                BindExtra::Fn { mask, .. } => Some(mask.clone()),
                _ => None,
            },
            None => static_mask(&self.live, &CalleeRef::Global(name.into())),
        }
    }

    /// `latent-forcing`: a thunk passed where the callee's mask cannot be
    /// determined (5.5), so the VM memoizes it at most once.
    fn latent_forcing(
        &mut self,
        p: &Node,
        mask: Option<&crate::types::masks::ForcingMask>,
        k: usize,
    ) {
        let Some(mask) = mask else {
            return;
        };
        let undetermined = match mask.entry(k) {
            Some(MaskEntry::Undetermined) => true,
            Some(MaskEntry::Forward { links }) => {
                let live = &self.live;
                chase(links, &|c| static_mask(live, c)).0 == EffectiveEntry::Undetermined
            }
            _ => false,
        };
        if undetermined {
            self.emit(
                DiagCode::LatentForcing,
                p.span,
                format!("the callee's forcing of argument {} is not determined; the block runs at most once", k + 1),
            );
        }
    }
}

/// `0`, `0.0` or `0/1` as a literal.
fn is_literal_zero(n: &Node) -> bool {
    match &n.kind {
        NodeKind::Atom(Atom::Int(0)) => true,
        NodeKind::Atom(Atom::Float { value, .. }) => *value == 0.0,
        NodeKind::Atom(Atom::Ratio(r)) => r.num() == 0,
        _ => false,
    }
}

/// A numeric literal's value.
fn literal_number(n: &Node) -> Option<f64> {
    match &n.kind {
        #[allow(clippy::cast_precision_loss)]
        NodeKind::Atom(Atom::Int(v)) => Some(*v as f64),
        NodeKind::Atom(Atom::Float { value, .. }) => Some(*value),
        #[allow(clippy::cast_precision_loss)]
        NodeKind::Atom(Atom::Ratio(r)) => Some(r.num() as f64 / r.den() as f64),
        _ => None,
    }
}

/// `(.. X)`: the open range `X..`.
fn is_open_range(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Call)
        && n.children.len() == 2
        && matches!(n.children[0].kind, NodeKind::Atom(Atom::Op(Op::Range)))
}
