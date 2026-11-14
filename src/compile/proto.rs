//! Function prototypes and closures (design 5.5, 8.1).

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::compile::compiler::binder_name;
use crate::dsp::controls::ScalarType;
use crate::ns::namespace::{FormGen, SlotKind, VarSlotRef};
use crate::reader::node::{Atom, Node, NodeKind, Op as NodeOp};
use crate::reader::span::Span;
use crate::types::masks::ForcingMask;
use crate::value::intern::{intern_sym, KwId, SymId};
use crate::value::value::{ListProv, Value};
use crate::vm::ops::Op;

/// The parameters of a function (8.1): positional parameters first, then
/// keyword parameters. A keyword parameter's default lives in the closure's
/// captures (`default_base + k`), because a default is an expression
/// evaluated where the `fn` is defined.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Arity {
    /// Positional parameters.
    pub fixed: u8,
    /// Every parameter name in header order (positional ones may also be
    /// passed by name, like struct fields).
    pub names: Box<[KwId]>,
    /// Scalar annotations in the same order as `names`.
    pub scalar_types: Box<[ScalarType]>,
    /// Keyword parameters, after the positional ones.
    pub keys: u8,
}

impl Arity {
    /// A function with `fixed` unnamed positional parameters.
    #[must_use]
    pub fn fixed(fixed: u8) -> Arity {
        Arity {
            fixed,
            names: Box::new([]),
            scalar_types: Box::new([]),
            keys: 0,
        }
    }

    /// The number of parameters.
    #[must_use]
    pub fn params(&self) -> usize {
        usize::from(self.fixed) + usize::from(self.keys)
    }
}

/// What one argument of a call site is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArgKind {
    /// A positional argument (one stack value).
    Pos,
    /// A `name: value` pair (two stack values: key, value).
    Pair,
    /// `& v`: a list spreads positionally, a dict spreads as pairs.
    Splat,
}

/// The argument layout of a `CallKw` site.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CallSite {
    pub args: Box<[ArgKind]>,
}

/// One item of a list literal with a splat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Item,
    Splat,
}

/// A list literal: its layout and provenance (5.4).
#[derive(Clone, Debug)]
pub struct ListSite {
    pub items: Box<[ItemKind]>,
    pub prov: Option<Rc<ListProv>>,
}

/// An enum variant or struct: its type, tag and fields in declared order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Shape {
    /// The enum name, or the struct name.
    pub ty: SymId,
    /// The variant tag (equal to `ty` for a struct).
    pub tag: SymId,
    /// Field names in declared order.
    pub fields: Box<[KwId]>,
    /// True for a struct (`StructVal`), false for a variant.
    pub is_struct: bool,
}

/// A compiled function body (8.1). Top-level forms compile to a
/// zero-parameter proto.
#[derive(Debug)]
pub struct FnProto {
    pub arity: Arity,
    pub code: Vec<Op>,
    pub consts: Vec<Value>,
    /// Frame locals, parameters and captures included.
    pub locals: u16,
    /// `(ip, span)`, ascending by `ip`; a failure at `ip` takes the span of
    /// the last entry at or before it.
    pub spans: Vec<(u32, Span)>,
    pub name: Option<SymId>,
    /// Global slots fixed at compile time (5.6 scope model).
    pub globals: Vec<VarSlotRef>,
    /// `DefGlobal` targets: an index into `globals` and the slot kind.
    pub defs: Vec<(u32, SlotKind)>,
    /// The generation of the top-level form this proto belongs to.
    pub form_gen: FormGen,
    /// Nested function bodies (`MakeClosure`, `MakeThunk`).
    pub protos: Vec<Rc<FnProto>>,
    /// Masks of the nested functions, parallel to `protos` (empty for thunks).
    pub masks: Vec<ForcingMask>,
    pub call_sites: Vec<CallSite>,
    pub list_sites: Vec<ListSite>,
    pub shapes: Vec<Shape>,
    /// Captured values (`LoadCapture`); a closure's keyword-parameter
    /// defaults follow them in `Closure::captures`.
    pub captures: u16,
    /// The span of the whole form or function.
    pub span: Span,
    /// Set for a variant or struct constructor: calls build the shape.
    pub ctor: Option<u16>,
}

impl FnProto {
    /// The span of the op at `ip`.
    #[must_use]
    pub fn span_at(&self, ip: usize) -> Span {
        let ip = u32::try_from(ip).unwrap_or(u32::MAX);
        let k = self.spans.partition_point(|(at, _)| *at <= ip);
        match k.checked_sub(1).and_then(|k| self.spans.get(k)) {
            Some((_, span)) => *span,
            None => self.span,
        }
    }
}

/// A function value or a thunk (5.5). A thunk is a zero-parameter closure.
///
/// `memo` is set only for the memoizing cell an `Undetermined` parameter
/// wraps its argument in: the body runs at most once, at first demand.
pub struct Closure {
    pub proto: Rc<FnProto>,
    pub captures: Box<[Value]>,
    pub mask: ForcingMask,
    pub memo: Option<RefCell<Option<Value>>>,
}

impl Closure {
    /// A memoizing cell over the same body and captures.
    #[must_use]
    pub fn memoized(&self) -> Closure {
        Closure {
            proto: Rc::clone(&self.proto),
            captures: self.captures.clone(),
            mask: self.mask.clone(),
            memo: Some(RefCell::new(None)),
        }
    }

    /// The memoized value, once forced.
    #[must_use]
    pub fn memo_value(&self) -> Option<Value> {
        self.memo.as_ref().and_then(|m| m.borrow().clone())
    }
}

impl fmt::Debug for Closure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closure")
            .field("name", &self.proto.name)
            .field("arity", &self.proto.arity)
            .field("mask", &self.mask)
            .field("memo", &self.memo.is_some())
            .finish_non_exhaustive()
    }
}

// ---- Builders (used by the compiler) ----

/// What kind of function body a builder compiles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FbKind {
    /// The top-level form.
    Top,
    /// A `{}` block passed as an argument.
    Thunk,
    /// A `fn`, lambda, constructor or `inst` body.
    Func,
}

/// A local binding.
#[derive(Clone, Debug)]
pub(crate) struct Local {
    pub name: Rc<str>,
    pub slot: u16,
    pub is_var: bool,
    pub is_param: bool,
}

/// Where a captured value comes from in the enclosing function.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CapSrc {
    Local(u16),
    Capture(u16),
}

/// A function body under construction.
pub(crate) struct Fb {
    pub kind: FbKind,
    pub code: Vec<Op>,
    pub consts: Vec<Value>,
    pub spans: Vec<(u32, Span)>,
    pub globals: Vec<VarSlotRef>,
    pub defs: Vec<(u32, SlotKind)>,
    pub protos: Vec<Rc<FnProto>>,
    pub masks: Vec<ForcingMask>,
    pub call_sites: Vec<CallSite>,
    pub list_sites: Vec<ListSite>,
    pub shapes: Vec<Shape>,
    pub scopes: Vec<Vec<Local>>,
    pub nlocals: u16,
    /// Captured names: (name, is_var, source in the enclosing function).
    pub captures: Vec<(Rc<str>, bool, CapSrc)>,
    pub arity: Arity,
    pub name: Option<Rc<str>>,
    pub span: Span,
    pub ctor: Option<u16>,
}

impl Fb {
    pub(crate) fn new(name: Option<Rc<str>>, span: Span, kind: FbKind) -> Fb {
        Fb {
            kind,
            code: Vec::new(),
            consts: Vec::new(),
            spans: Vec::new(),
            globals: Vec::new(),
            defs: Vec::new(),
            protos: Vec::new(),
            masks: Vec::new(),
            call_sites: Vec::new(),
            list_sites: Vec::new(),
            shapes: Vec::new(),
            scopes: vec![Vec::new()],
            nlocals: 0,
            captures: Vec::new(),
            arity: Arity::default(),
            name,
            span,
            ctor: None,
        }
    }

    pub(crate) fn finish(self, form_gen: FormGen) -> FnProto {
        FnProto {
            arity: self.arity,
            code: self.code,
            consts: self.consts,
            locals: self.nlocals,
            spans: self.spans,
            name: self.name.as_deref().map(intern_sym),
            globals: self.globals,
            defs: self.defs,
            form_gen,
            protos: self.protos,
            masks: self.masks,
            call_sites: self.call_sites,
            list_sites: self.list_sites,
            shapes: self.shapes,
            captures: u16::try_from(self.captures.len()).unwrap_or(u16::MAX),
            span: self.span,
            ctor: self.ctor,
        }
    }

    pub(crate) fn find(&self, name: &str) -> Option<&Local> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|l| &*l.name == name)
    }
}

/// One parameter or field: its binder node, name and default.
pub(crate) struct Param<'n> {
    pub node: &'n Node,
    pub name: Option<Rc<str>>,
    pub default: Option<&'n Node>,
}

/// Parameters of a `(HEAD NAME HEADER.. Block)` definition (`fn`, `inst`,
/// `look`), in header order, with the item after `=` as the default.
///
/// The walk mirrors `types::masks::header_params`, so for a `fn` the
/// parameters line up one to one with the entries of `infer_masks`: after
/// `name: pattern` one type item is skipped, after `name: fn` every item up
/// to and including the `->` and the return type, and an unowned `->` ends
/// the header.
pub(crate) fn fn_params(f: &Node) -> Vec<Param<'_>> {
    if f.children.len() < 3 {
        return Vec::new();
    }
    let items = &f.children[2..f.children.len() - 1];
    let mut out: Vec<Param<'_>> = Vec::new();
    let mut k = 0;
    while let Some(item) = items.get(k) {
        k += 1;
        match &item.kind {
            NodeKind::Atom(Atom::Op(NodeOp::Arrow)) => break,
            NodeKind::Atom(Atom::Op(NodeOp::Eq)) => {
                if let Some(last) = out.last_mut() {
                    last.default = items.get(k);
                }
                k += 1;
            }
            NodeKind::Atom(Atom::Sym(_)) | NodeKind::List => out.push(Param {
                node: item,
                name: binder_name(item).filter(|n| &**n != "_"),
                default: None,
            }),
            NodeKind::Pair => {
                match item.children.get(1).and_then(Node::sym_name) {
                    Some("pattern") => k += 1,
                    Some("fn") => {
                        while let Some(t) = items.get(k) {
                            k += 1;
                            if matches!(t.kind, NodeKind::Atom(Atom::Op(NodeOp::Arrow))) {
                                k += 1;
                                break;
                            }
                        }
                    }
                    _ => {}
                }
                out.push(Param {
                    node: item,
                    name: binder_name(item),
                    default: None,
                });
            }
            _ => {}
        }
    }
    out
}

/// Fields of a variant line `(circle r: float = 1)`.
pub(crate) fn field_params(items: &[Node]) -> Vec<Param<'_>> {
    let mut out: Vec<Param<'_>> = Vec::new();
    let mut k = 0;
    while let Some(item) = items.get(k) {
        k += 1;
        match &item.kind {
            NodeKind::Atom(Atom::Op(NodeOp::Eq)) => {
                if let Some(last) = out.last_mut() {
                    last.default = items.get(k);
                }
                k += 1;
            }
            NodeKind::Atom(Atom::Sym(_)) | NodeKind::Pair => out.push(Param {
                node: item,
                name: binder_name(item),
                default: None,
            }),
            _ => {}
        }
    }
    out
}

/// Fields of a `struct` block: `note`, `note: int`, `amp 1.0`, `amp: float 1.0`.
pub(crate) fn struct_fields(block: &Node) -> Vec<Param<'_>> {
    block
        .children
        .iter()
        .filter_map(|line| match &line.kind {
            NodeKind::Call => {
                let head = line.children.first()?;
                Some(Param {
                    node: head,
                    name: binder_name(head),
                    default: line.children.get(1),
                })
            }
            _ => Some(Param {
                node: line,
                name: binder_name(line),
                default: None,
            }),
        })
        .filter(|p| p.name.is_some())
        .collect()
}
