//! Checker types, schemes and the session environment (design 7, 7.1.3).
//!
//! `Scheme::parse` reads the type notation of lang-reference section 2
//! (`int`, `?T`, `[T]`, `[K: V]`, `fn A B -> R`, `pattern T`, `'a`), which the
//! native signature table uses to state its schemes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use crate::reader::span::Span;
use crate::types::masks::ForcingMask;

id_newtype!(
    /// A unification type variable.
    TyVar(u32)
);

id_newtype!(
    /// A declared struct or enum type.
    TypeId(u32)
);

id_newtype!(
    /// The kind variable of a numeric literal: it adapts to its context and
    /// defaults to int or float (design 7).
    NumKindVar(u32)
);

/// A set of keyword names. `Open` is "any keyword".
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum KeySet {
    Open,
    Of(BTreeSet<Rc<str>>),
}

impl KeySet {
    /// A closed set of the given names (without the leading `:`).
    pub fn of<I, S>(names: I) -> KeySet
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        KeySet::Of(names.into_iter().map(|n| Rc::from(n.as_ref())).collect())
    }

    /// True for `Open`.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self, KeySet::Open)
    }

    /// True when `name` is in the set; `Open` contains every name.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        match self {
            KeySet::Open => true,
            KeySet::Of(set) => set.contains(name),
        }
    }

    /// The union of two sets; `Open` absorbs.
    #[must_use]
    pub fn union(&self, other: &KeySet) -> KeySet {
        match (self, other) {
            (KeySet::Of(a), KeySet::Of(b)) => KeySet::Of(a.union(b).cloned().collect()),
            _ => KeySet::Open,
        }
    }
}

/// A checker type (design 7). `Tex` is the visual chain type the `scale`
/// overload needs (7.1.4); `UGen` a unit-generator node (12.8.6); `NumLit`
/// is a numeric literal's kind variable.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ty {
    Int,
    Int64,
    Float,
    Float64,
    Ratio,
    Bool,
    Str,
    KeywordOf(KeySet),
    Nil,
    Opt(Box<Ty>),
    List(Box<Ty>),
    Dict(Box<Ty>, Box<Ty>),
    Fn(Box<[Ty]>, Box<Ty>),
    Pattern(Box<Ty>),
    Signal,
    Path,
    Url,
    Sound,
    Tex,
    /// A unit-generator node built by an `inst` or `bus` body (12.8.6).
    UGen,
    Named(TypeId),
    Any,
    Var(TyVar),
    NumLit(NumKindVar),
}

impl Ty {
    /// `keyword`: any keyword.
    #[must_use]
    pub fn keyword() -> Ty {
        Ty::KeywordOf(KeySet::Open)
    }

    /// `fn params -> ret`.
    #[must_use]
    pub fn func(params: Vec<Ty>, ret: Ty) -> Ty {
        Ty::Fn(params.into_boxed_slice(), Box::new(ret))
    }

    /// True when the type is a function type.
    #[must_use]
    pub fn is_fn(&self) -> bool {
        matches!(self, Ty::Fn(..))
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => f.write_str("int"),
            Ty::Int64 => f.write_str("int64"),
            Ty::Float => f.write_str("float"),
            Ty::Float64 => f.write_str("float64"),
            Ty::Ratio => f.write_str("ratio"),
            Ty::Bool => f.write_str("bool"),
            Ty::Str => f.write_str("string"),
            Ty::KeywordOf(KeySet::Open) => f.write_str("keyword"),
            Ty::KeywordOf(KeySet::Of(set)) => {
                f.write_str("keyword{")?;
                for (k, name) in set.iter().enumerate() {
                    let sep = if k == 0 { "" } else { " " };
                    write!(f, "{sep}:{name}")?;
                }
                f.write_str("}")
            }
            Ty::Nil => f.write_str("nil"),
            Ty::Opt(t) => write!(f, "?{}", Paren(t)),
            Ty::List(t) => write!(f, "[{t}]"),
            Ty::Dict(k, v) => write!(f, "[{k}: {v}]"),
            Ty::Fn(params, ret) => {
                f.write_str("fn")?;
                for p in params.iter() {
                    write!(f, " {}", Paren(p))?;
                }
                write!(f, " -> {ret}")
            }
            Ty::Pattern(t) => write!(f, "pattern {}", Paren(t)),
            Ty::Signal => f.write_str("signal"),
            Ty::Path => f.write_str("path"),
            Ty::Url => f.write_str("url"),
            Ty::Sound => f.write_str("sound"),
            Ty::Tex => f.write_str("tex"),
            Ty::UGen => f.write_str("ugen"),
            Ty::Named(id) => write!(f, "type#{}", id.get()),
            Ty::Any => f.write_str("any"),
            Ty::Var(v) => write!(f, "'t{}", v.get()),
            Ty::NumLit(v) => write!(f, "num#{}", v.get()),
        }
    }
}

/// Prints a type, in parentheses when it has spaces.
struct Paren<'a>(&'a Ty);

impl fmt::Display for Paren<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Ty::Fn(..) | Ty::Pattern(_) => write!(f, "({})", self.0),
            t => write!(f, "{t}"),
        }
    }
}

/// A type scheme: `ty` generalized over `vars`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scheme {
    pub vars: Box<[TyVar]>,
    pub ty: Ty,
}

impl Scheme {
    /// A scheme with no quantified variables.
    #[must_use]
    pub fn mono(ty: Ty) -> Scheme {
        Scheme {
            vars: Box::new([]),
            ty,
        }
    }

    /// Parses the type notation. Every `'name` becomes a quantified
    /// variable, numbered from 0 in order of first appearance. `ctl` is
    /// short for `pattern [keyword: any]`, the control-map pattern. Returns
    /// `None` on any syntax error; never panics.
    #[must_use]
    pub fn parse(text: &str) -> Option<Scheme> {
        let toks = tokens(text);
        let mut p = Parser {
            toks: &toks,
            pos: 0,
            vars: Vec::new(),
            depth: 0,
        };
        let ty = p.ty()?;
        if p.pos != toks.len() {
            return None;
        }
        let vars = (0..p.vars.len())
            .map(|k| TyVar::new(u32::try_from(k).unwrap_or(u32::MAX)))
            .collect();
        Some(Scheme { vars, ty })
    }
}

/// Splits the notation into words and the punctuation `[ ] ( ) ? :`.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    for (k, c) in text.char_indices() {
        let punct = matches!(c, '[' | ']' | '(' | ')' | '?' | ':');
        if c.is_whitespace() || punct {
            if let Some(s) = start.take() {
                out.push(&text[s..k]);
            }
            if punct {
                out.push(&text[k..k + 1]);
            }
        } else if start.is_none() {
            start = Some(k);
        }
    }
    if let Some(s) = start {
        out.push(&text[s..]);
    }
    out
}

/// Nesting guard for the notation parser.
const MAX_PARSE_DEPTH: u32 = 64;

struct Parser<'a> {
    toks: &'a [&'a str],
    pos: usize,
    vars: Vec<&'a str>,
    depth: u32,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.toks.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<&'a str> {
        let t = self.peek()?;
        self.pos += 1;
        Some(t)
    }

    fn eat(&mut self, tok: &str) -> Option<()> {
        (self.next()? == tok).then_some(())
    }

    /// A full type: `fn P.. -> R` or an atom-level type.
    fn ty(&mut self) -> Option<Ty> {
        self.depth += 1;
        if self.depth > MAX_PARSE_DEPTH {
            return None;
        }
        let out = if self.peek() == Some("fn") {
            self.pos += 1;
            let mut params = Vec::new();
            while self.peek() != Some("->") {
                params.push(self.atom()?);
            }
            self.pos += 1;
            let ret = self.ty()?;
            Some(Ty::func(params, ret))
        } else {
            self.atom()
        };
        self.depth -= 1;
        out
    }

    /// An atom-level type: a name, `?T`, `[T]`, `[K: V]`, `pattern T`,
    /// `(T)` or `'a`.
    fn atom(&mut self) -> Option<Ty> {
        self.depth += 1;
        if self.depth > MAX_PARSE_DEPTH {
            return None;
        }
        let tok = self.next()?;
        let out = match tok {
            "?" => Some(Ty::Opt(Box::new(self.atom()?))),
            "(" => {
                let t = self.ty()?;
                self.eat(")")?;
                Some(t)
            }
            "[" => {
                let t = self.ty()?;
                if self.peek() == Some(":") {
                    self.pos += 1;
                    let v = self.ty()?;
                    self.eat("]")?;
                    Some(Ty::Dict(Box::new(t), Box::new(v)))
                } else {
                    self.eat("]")?;
                    Some(Ty::List(Box::new(t)))
                }
            }
            "pattern" => Some(Ty::Pattern(Box::new(self.atom()?))),
            "ctl" => Some(Ty::Pattern(Box::new(Ty::Dict(
                Box::new(Ty::keyword()),
                Box::new(Ty::Any),
            )))),
            _ => self.name(tok),
        };
        self.depth -= 1;
        out
    }

    fn name(&mut self, tok: &'a str) -> Option<Ty> {
        Some(match tok {
            "int" => Ty::Int,
            "int64" => Ty::Int64,
            "float" => Ty::Float,
            "float64" => Ty::Float64,
            "ratio" => Ty::Ratio,
            "bool" => Ty::Bool,
            "string" => Ty::Str,
            "keyword" => Ty::keyword(),
            "nil" => Ty::Nil,
            "signal" => Ty::Signal,
            "path" => Ty::Path,
            "url" => Ty::Url,
            "sound" => Ty::Sound,
            "tex" => Ty::Tex,
            "ugen" => Ty::UGen,
            "any" => Ty::Any,
            _ => {
                let name = tok.strip_prefix('\'').filter(|n| !n.is_empty())?;
                let k = match self.vars.iter().position(|v| *v == name) {
                    Some(k) => k,
                    None => {
                        self.vars.push(name);
                        self.vars.len() - 1
                    }
                };
                Ty::Var(TyVar::new(u32::try_from(k).ok()?))
            }
        })
    }
}

/// What introduced a binding.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BindKind {
    Let,
    Var,
    Fn,
    Inst,
    Struct,
    Enum,
    Param,
    PatternBinding,
}

/// What the checker knows about a session binding.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GlobalInfo {
    pub kind: BindKind,
    pub scheme: Option<Scheme>,
    pub mask: Option<ForcingMask>,
    pub span: Option<Span>,
}

/// The session state a program is checked against. `globals` holds the
/// bindings made before the checked program: a top-level binding of one of
/// them is Live redefinition, not `rebinding` (7.1.4). `qualified` maps an
/// import prefix to its package's names; `opens` lists opened imports with
/// the names they expose, oldest first.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CheckEnv {
    pub globals: BTreeMap<Rc<str>, GlobalInfo>,
    pub qualified: BTreeMap<Rc<str>, BTreeMap<Rc<str>, GlobalInfo>>,
    pub opens: Vec<(Rc<str>, BTreeSet<Rc<str>>)>,
}

impl CheckEnv {
    /// No session bindings and no imports.
    #[must_use]
    pub fn empty() -> CheckEnv {
        CheckEnv::default()
    }

    /// The session binding of `name`.
    #[must_use]
    pub fn global(&self, name: &str) -> Option<&GlobalInfo> {
        self.globals.get(name)
    }

    /// `prefix.name` of an imported package.
    #[must_use]
    pub fn qualified(&self, prefix: &str, name: &str) -> Option<&GlobalInfo> {
        self.qualified.get(prefix)?.get(name)
    }

    /// The prefix of the most recent opened import that exposes `name`.
    #[must_use]
    pub fn open_prefix(&self, name: &str) -> Option<&Rc<str>> {
        self.opens
            .iter()
            .rev()
            .find(|(_, names)| names.contains(name))
            .map(|(prefix, _)| prefix)
    }
}
