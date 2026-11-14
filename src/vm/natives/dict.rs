//! Collection builders (lang-reference section 2, design 6.5.3): `put`
//! (append, assoc and merge), `join` (join many) and `dict`.

use crate::ns::namespace::Prelude;
use crate::value::dict::{dict_from_pairs, join as join_all, put as put_elems};
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::call::NativeCx;
use crate::vm::fail::Failure;
use crate::vm::natives::{arg, Seq};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("put", put);
    p.register("join", join);
    p.register("dict", dict);
}

/// `put coll x..`: on a list it appends; on a dict every element is a pair
/// and later pairs win.
fn put(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let Some((coll, elems)) = args.split_first() else {
        return Ok(Value::Nil);
    };
    let coll = cx.deep(coll)?;
    let mut out = Vec::with_capacity(elems.len());
    for e in elems {
        out.push(cx.deep(e)?);
    }
    put_elems(&coll, &out)
}

/// `join [c..]`: the first collection sets the result kind.
fn join(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let colls = Seq::of(&arg(args, 0), "`join`")?.collect(cx)?;
    let mut out = Vec::with_capacity(colls.len());
    for c in &colls {
        out.push(cx.deep(c)?);
    }
    join_all(&out)
}

/// `dict pairs`: a dict from a computed pair list.
fn dict(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let v = cx.deep(&arg(args, 0))?;
    let items = Seq::of(&v, "`dict`")?.collect(cx)?;
    Ok(Value::dict(dict_from_pairs(&items)?))
}
