//! Unification over `Ty` (design 7): type variables with levels for
//! let-polymorphism, numeric-literal kind variables, the occurs check, and
//! the one subsumption rule `[sound]` accepted where `sound` is expected
//! (7.1.4 sample bank).
//!
//! `unify(expected, actual)` is directed only where the language is: a `T`
//! is accepted where `?T` is expected, a list, a single step or a fn of time
//! is accepted where a pattern is expected, a keyword names a sound, and
//! any two numeric types join (the 6.5.3 widening lattice; lossy steps are
//! accepted, as `* 60 1.5` is). Every change is trailed, so `snapshot` and
//! `rollback` make a trial unification free of side effects.

use std::collections::BTreeSet;

use crate::types::ty::{NumKindVar, Scheme, Ty, TyVar};

/// Why a unification failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clash {
    /// The types do not fit (`type-mismatch`).
    Mismatch,
    /// A `?T` was used where a `T` is required (`optional-as-value`).
    Optional,
}

/// The least type a numeric literal can take: an int literal fits every
/// numeric type, a ratio literal ratio and the floats, a decimal literal
/// float, float64 or (as an exact decimal) ratio.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum NumFloor {
    Int,
    Ratio,
    Float,
}

impl NumFloor {
    /// The type an unconstrained literal defaults to.
    #[must_use]
    pub fn default_ty(self) -> Ty {
        match self {
            NumFloor::Int => Ty::Int,
            NumFloor::Ratio => Ty::Ratio,
            NumFloor::Float => Ty::Float,
        }
    }
}

#[derive(Clone, Debug)]
struct VarSlot {
    bound: Option<Ty>,
    level: u32,
}

#[derive(Clone, Debug)]
enum NumState {
    Unbound(NumFloor),
    Bound(Ty),
}

#[derive(Clone, Debug)]
enum Undo {
    Var(usize),
    Level(usize, u32),
    Num(usize, NumState),
}

/// The substitution of one `check` run.
#[derive(Debug, Default)]
pub struct Unifier {
    vars: Vec<VarSlot>,
    nums: Vec<NumState>,
    trail: Vec<Undo>,
    level: u32,
}

/// A trail position to roll back to.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot(usize);

/// The type depth past which unification and resolution give up (an
/// adversarial type cannot overflow the stack).
const MAX_TY_DEPTH: u32 = 256;

/// The largest binding type kept, in resolved nodes. HM types can grow
/// exponentially in the program size (`let x2 f -> f x1 x1`, ...); a
/// binding past this is `any`, so checking stays fast on hostile input.
pub const MAX_TY_SIZE: usize = 2048;

/// True for the numeric lattice members (a signal is a numeric stream, so
/// `* 20 {sin time}` checks).
fn is_numeric(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Int | Ty::Int64 | Ty::Float | Ty::Float64 | Ty::Ratio | Ty::Signal
    )
}

fn var_index(v: TyVar) -> usize {
    usize::try_from(v.get()).unwrap_or(usize::MAX)
}

fn num_index(v: NumKindVar) -> usize {
    usize::try_from(v.get()).unwrap_or(usize::MAX)
}

impl Unifier {
    /// An empty substitution at level 0.
    #[must_use]
    pub fn new() -> Unifier {
        Unifier::default()
    }

    /// Enters a `let`/`fn` right-hand side: new variables are generalizable.
    pub fn enter(&mut self) {
        self.level += 1;
    }

    /// Leaves a right-hand side.
    pub fn exit(&mut self) {
        self.level = self.level.saturating_sub(1);
    }

    /// A fresh type variable at the current level.
    pub fn fresh(&mut self) -> Ty {
        let k = u32::try_from(self.vars.len()).unwrap_or(u32::MAX);
        self.vars.push(VarSlot {
            bound: None,
            level: self.level,
        });
        Ty::Var(TyVar::new(k))
    }

    /// A fresh numeric-literal kind variable.
    pub fn fresh_num(&mut self, floor: NumFloor) -> Ty {
        let k = u32::try_from(self.nums.len()).unwrap_or(u32::MAX);
        self.nums.push(NumState::Unbound(floor));
        Ty::NumLit(NumKindVar::new(k))
    }

    /// The current trail position.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot(self.trail.len())
    }

    /// Undoes every binding made after `snap`.
    pub fn rollback(&mut self, snap: Snapshot) {
        while self.trail.len() > snap.0 {
            match self.trail.pop() {
                Some(Undo::Var(k)) => {
                    if let Some(slot) = self.vars.get_mut(k) {
                        slot.bound = None;
                    }
                }
                Some(Undo::Level(k, level)) => {
                    if let Some(slot) = self.vars.get_mut(k) {
                        slot.level = level;
                    }
                }
                Some(Undo::Num(k, old)) => {
                    if let Some(slot) = self.nums.get_mut(k) {
                        *slot = old;
                    }
                }
                None => break,
            }
        }
    }

    /// Follows variable and literal bindings at the top of `t`.
    #[must_use]
    pub fn shallow(&self, t: &Ty) -> Ty {
        let mut cur = t.clone();
        for _ in 0..MAX_TY_DEPTH {
            match &cur {
                Ty::Var(v) => match self.vars.get(var_index(*v)).and_then(|s| s.bound.clone()) {
                    Some(next) => cur = next,
                    None => return cur,
                },
                Ty::NumLit(n) => match self.nums.get(num_index(*n)) {
                    Some(NumState::Bound(next)) => cur = next.clone(),
                    _ => return cur,
                },
                _ => return cur,
            }
        }
        Ty::Any
    }

    /// The floor of an unbound literal variable.
    fn floor(&self, n: NumKindVar) -> NumFloor {
        match self.nums.get(num_index(n)) {
            Some(NumState::Unbound(f)) => *f,
            _ => NumFloor::Int,
        }
    }

    /// Resolves every variable in `t`. Unbound literals default to their
    /// floor type when `default_nums` is set.
    #[must_use]
    pub fn zonk(&self, t: &Ty, default_nums: bool) -> Ty {
        self.zonk_at(t, default_nums, 0)
    }

    fn zonk_at(&self, t: &Ty, dn: bool, depth: u32) -> Ty {
        if depth > MAX_TY_DEPTH {
            return Ty::Any;
        }
        let d = depth + 1;
        match self.shallow(t) {
            Ty::NumLit(n) if dn => self.floor(n).default_ty(),
            Ty::Opt(a) => Ty::Opt(Box::new(self.zonk_at(&a, dn, d))),
            Ty::List(a) => Ty::List(Box::new(self.zonk_at(&a, dn, d))),
            Ty::Pattern(a) => Ty::Pattern(Box::new(self.zonk_at(&a, dn, d))),
            Ty::Dict(k, v) => Ty::Dict(
                Box::new(self.zonk_at(&k, dn, d)),
                Box::new(self.zonk_at(&v, dn, d)),
            ),
            Ty::Fn(ps, r) => Ty::Fn(
                ps.iter().map(|p| self.zonk_at(p, dn, d)).collect(),
                Box::new(self.zonk_at(&r, dn, d)),
            ),
            other => other,
        }
    }

    /// Binds unbound literal variables in `t` to their default type (done
    /// when a binding is generalized, so hover shows `int`, not a kind).
    pub fn default_nums(&mut self, t: &Ty) {
        let mut lits = Vec::new();
        self.collect(t, &mut |x| {
            if let Ty::NumLit(n) = x {
                lits.push(*n);
            }
        });
        for n in lits {
            if let Some(NumState::Unbound(f)) = self.nums.get(num_index(n)).cloned() {
                self.bind_num(n, f.default_ty());
            }
        }
    }

    /// Visits the resolved leaves (`Var`, `NumLit`) of `t`.
    fn collect(&self, t: &Ty, f: &mut dyn FnMut(&Ty)) {
        let mut stack = vec![(t.clone(), 0u32)];
        while let Some((cur, depth)) = stack.pop() {
            if depth > MAX_TY_DEPTH {
                continue;
            }
            match self.shallow(&cur) {
                leaf @ (Ty::Var(_) | Ty::NumLit(_)) => f(&leaf),
                Ty::Opt(a) | Ty::List(a) | Ty::Pattern(a) => stack.push((*a, depth + 1)),
                Ty::Dict(k, v) => {
                    stack.push((*k, depth + 1));
                    stack.push((*v, depth + 1));
                }
                Ty::Fn(ps, r) => {
                    stack.extend(ps.iter().map(|p| (p.clone(), depth + 1)));
                    stack.push((*r, depth + 1));
                }
                _ => {}
            }
        }
    }

    /// True when `t` has more than `MAX_TY_SIZE` resolved nodes.
    #[must_use]
    pub fn too_large(&self, t: &Ty) -> bool {
        let mut stack = vec![t.clone()];
        let mut count = 0usize;
        while let Some(cur) = stack.pop() {
            count += 1;
            if count > MAX_TY_SIZE {
                return true;
            }
            match self.shallow(&cur) {
                Ty::Opt(a) | Ty::List(a) | Ty::Pattern(a) => stack.push(*a),
                Ty::Dict(k, v) => {
                    stack.push(*k);
                    stack.push(*v);
                }
                Ty::Fn(ps, r) => {
                    stack.extend(ps.iter().cloned());
                    stack.push(*r);
                }
                _ => {}
            }
        }
        false
    }

    /// Generalizes the variables of `t` created inside the current level.
    /// A type past `MAX_TY_SIZE` generalizes to `any`.
    #[must_use]
    pub fn generalize(&self, t: &Ty) -> Scheme {
        if self.too_large(t) {
            return Scheme::mono(Ty::Any);
        }
        let mut vars = BTreeSet::new();
        self.collect(t, &mut |x| {
            if let Ty::Var(v) = x {
                let deeper = self
                    .vars
                    .get(var_index(*v))
                    .is_some_and(|s| s.level > self.level);
                if deeper {
                    vars.insert(*v);
                }
            }
        });
        Scheme {
            vars: vars.into_iter().collect(),
            ty: self.zonk(t, false),
        }
    }

    /// A fresh instance of `s`.
    pub fn instantiate(&mut self, s: &Scheme) -> Ty {
        if s.vars.is_empty() {
            return s.ty.clone();
        }
        let map: Vec<(TyVar, Ty)> = s.vars.iter().map(|v| (*v, self.fresh())).collect();
        subst(&s.ty, &map, 0)
    }

    fn bind_var(&mut self, v: TyVar, t: Ty) {
        let k = var_index(v);
        let level = match self.vars.get(k) {
            Some(slot) => slot.level,
            None => return,
        };
        let mut lower = Vec::new();
        self.collect(&t, &mut |x| {
            if let Ty::Var(w) = x {
                lower.push(*w);
            }
        });
        for w in lower {
            let j = var_index(w);
            if let Some(slot) = self.vars.get_mut(j) {
                if slot.level > level {
                    self.trail.push(Undo::Level(j, slot.level));
                    slot.level = level;
                }
            }
        }
        if let Some(slot) = self.vars.get_mut(k) {
            slot.bound = Some(t);
            self.trail.push(Undo::Var(k));
        }
    }

    fn bind_num(&mut self, n: NumKindVar, t: Ty) {
        let k = num_index(n);
        if let Some(slot) = self.nums.get_mut(k) {
            let old = std::mem::replace(slot, NumState::Bound(t));
            self.trail.push(Undo::Num(k, old));
        }
    }

    fn occurs(&self, v: TyVar, t: &Ty) -> bool {
        let mut found = false;
        self.collect(t, &mut |x| {
            if *x == Ty::Var(v) {
                found = true;
            }
        });
        found
    }

    /// Unifies `actual` into `expected`. On failure the bindings made by
    /// the failed attempt stay; callers that need a clean state take a
    /// snapshot first.
    pub fn unify(&mut self, expected: &Ty, actual: &Ty) -> Result<(), Clash> {
        self.unify_at(expected, actual, 0)
    }

    /// Unifies inside a snapshot and rolls back on failure.
    pub fn try_unify(&mut self, expected: &Ty, actual: &Ty) -> Result<(), Clash> {
        let snap = self.snapshot();
        let out = self.unify(expected, actual);
        if out.is_err() {
            self.rollback(snap);
        }
        out
    }

    /// The common type of two branches or list items: `nil` with `T` is
    /// `?T`; otherwise whichever direction unifies; otherwise `any` (a
    /// heterogeneous list is legal, so this never reports).
    pub fn join(&mut self, a: &Ty, b: &Ty) -> Ty {
        let (sa, sb) = (self.shallow(a), self.shallow(b));
        match (&sa, &sb) {
            (Ty::Nil, Ty::Nil) => return Ty::Nil,
            (Ty::Nil, Ty::Opt(_)) => return sb,
            (Ty::Opt(_), Ty::Nil) => return sa,
            (Ty::Nil, _) => return Ty::Opt(Box::new(sb)),
            (_, Ty::Nil) => return Ty::Opt(Box::new(sa)),
            _ => {}
        }
        if self.try_unify(&sa, &sb).is_ok() {
            return sa;
        }
        if self.try_unify(&sb, &sa).is_ok() {
            return sb;
        }
        Ty::Any
    }

    fn unify_at(&mut self, expected: &Ty, actual: &Ty, depth: u32) -> Result<(), Clash> {
        if depth > MAX_TY_DEPTH {
            return Ok(());
        }
        let d = depth + 1;
        let (e, a) = (self.shallow(expected), self.shallow(actual));
        match (&e, &a) {
            (Ty::Any, _) | (_, Ty::Any) => Ok(()),
            (Ty::Var(x), Ty::Var(y)) if x == y => Ok(()),
            (Ty::Var(x), t) | (t, Ty::Var(x)) => {
                if self.occurs(*x, t) {
                    Err(Clash::Mismatch)
                } else {
                    self.bind_var(*x, t.clone());
                    Ok(())
                }
            }
            (Ty::NumLit(x), Ty::NumLit(y)) => {
                if x != y {
                    let floor = self.floor(*x).max(self.floor(*y));
                    self.bind_num(*y, Ty::NumLit(*x));
                    let k = num_index(*x);
                    if let Some(slot) = self.nums.get_mut(k) {
                        let old = std::mem::replace(slot, NumState::Unbound(floor));
                        self.trail.push(Undo::Num(k, old));
                    }
                }
                Ok(())
            }
            (Ty::NumLit(x), t) | (t, Ty::NumLit(x)) if is_numeric(t) => {
                // The literal keeps its own kind where the lattice widens
                // the other way (`* 1/3 0.5` is float, 6.5.3).
                let bound = match (self.floor(*x), t) {
                    (NumFloor::Float, Ty::Ratio | Ty::Int | Ty::Int64) => Ty::Float,
                    (NumFloor::Ratio, Ty::Int | Ty::Int64) => Ty::Ratio,
                    _ => t.clone(),
                };
                self.bind_num(*x, bound);
                Ok(())
            }
            // A ugen input takes a number, a ugen or a signal (12.8.6).
            (Ty::UGen, t) if is_numeric(t) || matches!(t, Ty::NumLit(_) | Ty::UGen) => Ok(()),
            (Ty::Pattern(x), _) => self.unify_step(x, &a, d),
            (Ty::Opt(x), Ty::Opt(y)) => self.unify_at(x, y, d),
            (Ty::Opt(_), Ty::Nil) | (Ty::Nil, Ty::Opt(_)) => Ok(()),
            (Ty::Opt(x), _) => self.unify_at(x, &a, d),
            (_, Ty::Opt(_)) => Err(Clash::Optional),
            _ if is_numeric(&e) && is_numeric(&a) => Ok(()),
            (Ty::List(x), Ty::List(y)) => self.unify_at(x, y, d),
            (Ty::Dict(k1, v1), Ty::Dict(k2, v2)) => {
                self.unify_at(k1, k2, d)?;
                self.unify_at(v1, v2, d)
            }
            // A dict is a list of pairs, and `[]` is both; collection
            // accessors pun nil (`first nil`, `len nil`).
            (Ty::List(_), Ty::Dict(..)) | (Ty::Dict(..), Ty::List(_)) => Ok(()),
            (Ty::List(_) | Ty::Dict(..), Ty::Nil) => Ok(()),
            (Ty::Fn(ps, r), Ty::Fn(qs, s)) => {
                if ps.len() != qs.len() {
                    return Err(Clash::Mismatch);
                }
                for (p, q) in ps.iter().zip(qs.iter()) {
                    self.unify_at(q, p, d)?;
                }
                self.unify_at(r, s, d)
            }
            // A keyword names a sound; a list of sounds is a sample bank.
            (Ty::Sound, Ty::KeywordOf(_) | Ty::Sound) => Ok(()),
            (Ty::Sound, Ty::List(x)) => self.unify_at(&Ty::Sound, x, d),
            (Ty::KeywordOf(_), Ty::KeywordOf(_)) => Ok(()),
            _ if e == a => Ok(()),
            _ => Err(Clash::Mismatch),
        }
    }

    /// One step of a pattern: a list is one cycle of steps (nested lists
    /// subdivide), `nil` is a rest, a pattern or a fn of time is itself a
    /// step source, a signal is a continuous pattern.
    fn unify_step(&mut self, elem: &Ty, actual: &Ty, depth: u32) -> Result<(), Clash> {
        if depth > MAX_TY_DEPTH {
            return Ok(());
        }
        let d = depth + 1;
        if finite_result(&self.zonk(actual, false), d) {
            return Err(Clash::Mismatch);
        }
        match self.shallow(actual) {
            Ty::List(x) | Ty::Pattern(x) | Ty::Opt(x) => self.unify_step(elem, &x, d),
            Ty::Fn(ps, r) if ps.is_empty() => self.unify_step(elem, &r, d),
            Ty::Fn(ps, r) if ps.len() == 1 => {
                if !matches!(
                    self.shallow(&ps[0]),
                    Ty::Var(_)
                        | Ty::Ratio
                        | Ty::Int
                        | Ty::Int64
                        | Ty::Float
                        | Ty::Float64
                        | Ty::NumLit(_)
                ) {
                    return Err(Clash::Mismatch);
                }
                // A declared Any return cannot acquire sound authority from coercion.
                let mut result = self.shallow(&r);
                let mut result_depth = d;
                loop {
                    if result_depth > MAX_TY_DEPTH {
                        return Err(Clash::Mismatch);
                    }
                    match result {
                        Ty::Any => return Err(Clash::Mismatch),
                        Ty::Pattern(inner) | Ty::List(inner) | Ty::Opt(inner) => {
                            result = self.shallow(&inner);
                            result_depth += 1;
                        }
                        _ => break,
                    }
                }
                // Check both sides together: a shared time/result variable stays shared.
                let expected = Ty::func(vec![Ty::Ratio], Ty::Pattern(Box::new(elem.clone())));
                self.unify_at(&expected, &Ty::Fn(ps, r), d)
            }
            Ty::Part | Ty::Song | Ty::EventHandle => Err(Clash::Mismatch),
            Ty::Nil | Ty::Signal => Ok(()),
            other => self.unify_at(elem, &other, d),
        }
    }
}

/// Replaces quantified variables.
fn subst(t: &Ty, map: &[(TyVar, Ty)], depth: u32) -> Ty {
    if depth > MAX_TY_DEPTH {
        return Ty::Any;
    }
    let d = depth + 1;
    match t {
        Ty::Var(v) => map
            .iter()
            .find(|(w, _)| w == v)
            .map_or_else(|| t.clone(), |(_, n)| n.clone()),
        Ty::Opt(a) => Ty::Opt(Box::new(subst(a, map, d))),
        Ty::List(a) => Ty::List(Box::new(subst(a, map, d))),
        Ty::Pattern(a) => Ty::Pattern(Box::new(subst(a, map, d))),
        Ty::Dict(k, v) => Ty::Dict(Box::new(subst(k, map, d)), Box::new(subst(v, map, d))),
        Ty::Fn(ps, r) => Ty::Fn(
            ps.iter().map(|p| subst(p, map, d)).collect(),
            Box::new(subst(r, map, d)),
        ),
        other => other.clone(),
    }
}

/// A known finite return cannot serve as a lazy pattern source. Inspect only
/// source-shaped wrappers; ordinary function/collection value passing is unchanged.
fn finite_result(ty: &Ty, depth: u32) -> bool {
    if depth > MAX_TY_DEPTH {
        return false;
    }
    match ty {
        Ty::Part | Ty::Song | Ty::EventHandle => true,
        Ty::List(inner) | Ty::Opt(inner) | Ty::Pattern(inner) | Ty::Fn(_, inner) => {
            finite_result(inner, depth + 1)
        }
        Ty::Dict(_, inner) => finite_result(inner, depth + 1),
        _ => false,
    }
}
