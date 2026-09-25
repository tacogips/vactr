//! The runtime value (design 5.1, 6.5.1).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::compile::proto::Closure;
use crate::dsp::graph::InstId;
use crate::ns::namespace::{FormGen, VarSlotRef};
use crate::pattern::pat::Pat;
use crate::pattern::signal::Sig;
use crate::reader::span::Span;
use crate::tex::texnode::TexNode;
use crate::value::intern::{intern_kw, KwId, SymId};
use crate::value::key::Key;
use crate::value::ratio::Ratio64;

/// A runtime value. It holds `Rc`, so it is `!Send`: a `Value` never
/// crosses to the audio or render thread (design 5.1).
#[derive(Clone, Debug)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i32),
    Int64(i64),
    Float(f32),
    Float64(f64),
    /// Exact int64/int64.
    Ratio(Ratio64),
    Keyword(KwId),
    Str(Rc<str>),
    /// Immutable; pairs are 2-lists (5.4).
    List(Rc<ListVal>),
    /// A sorted map with key-ordered iteration (5.4).
    Dict(Rc<BTreeMap<Key, Value>>),
    /// A declared dict: type id plus key-sorted fields.
    Struct(Rc<StructVal>),
    /// An enum tag plus fields.
    Variant(Rc<VariantVal>),
    Fn(Rc<Closure>),
    Native(NativeId),
    /// A zero-argument block, forced on demand.
    Thunk(Rc<Closure>),
    /// A late-bound global var, fn or tweak (5.6).
    VarRef(VarSlotRef),
    Pattern(Rc<Pat>),
    Signal(Rc<Sig>),
    /// An instrument reference.
    Inst(InstId),
    /// A visual chain.
    Tex(Rc<TexNode>),
    /// `0..8` (eager, list-like) or `0..` (lazy).
    Range(RangeVal),
}

impl Value {
    /// A list without provenance.
    #[must_use]
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(ListVal {
            items: items.into_boxed_slice(),
            prov: None,
        }))
    }

    /// A string.
    #[must_use]
    pub fn str(text: &str) -> Value {
        Value::Str(Rc::from(text))
    }

    /// A keyword, interned by name (without the leading `:`).
    #[must_use]
    pub fn kw(name: &str) -> Value {
        Value::Keyword(intern_kw(name))
    }

    /// A dict over an existing map.
    #[must_use]
    pub fn dict(map: BTreeMap<Key, Value>) -> Value {
        Value::Dict(Rc::new(map))
    }
}

/// An immutable list, with element provenance when it came from a literal.
#[derive(Clone, Debug)]
pub struct ListVal {
    pub items: Box<[Value]>,
    pub prov: Option<Rc<ListProv>>,
}

/// Literal element spans plus origin identity (5.4).
#[derive(Clone, Debug)]
pub struct ListProv {
    pub form_gen: FormGen,
    pub doc_revision: u64,
    pub elems: Box<[Span]>,
}

/// A struct value; fields are sorted by key name.
#[derive(Clone, Debug)]
pub struct StructVal {
    pub ty: SymId,
    pub fields: Box<[(KwId, Value)]>,
}

/// An enum variant value; fields are sorted by key name.
#[derive(Clone, Debug)]
pub struct VariantVal {
    pub enum_ty: SymId,
    pub tag: SymId,
    pub fields: Box<[(KwId, Value)]>,
}

id_newtype!(
    /// A native function.
    NativeId(u32)
);

/// A range with an exclusive end; `end: None` is the lazy open range.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RangeVal {
    pub start: i64,
    pub end: Option<i64>,
}
