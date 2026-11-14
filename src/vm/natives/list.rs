//! List natives (lang-reference sections 3 and 5): fuel-metered iteration
//! over lists, dicts (as pairs in key order) and ranges (lazy for `0..`).

use crate::ns::namespace::Prelude;
use crate::value::access::{first, len, truthy};
use crate::value::intern::KwId;
use crate::value::key::Key;
use crate::value::value::Value;
use crate::vm::call::NativeCx;
use crate::vm::fail::Failure;
use crate::vm::natives::{arg, int_of, list_of, type_err, Seq};
use crate::vm::vm::int_value;

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("len", len_n);
    p.register("first", first_n);
    p.register("last", last);
    p.register("tail", tail);
    p.register("reverse", reverse);
    p.register("sort", sort);
    p.register("map", map);
    p.register("filter", filter);
    p.register("reduce", reduce);
    p.register("find", find);
    p.register("any", any);
    p.register("all", all);
    p.register("take", take);
    p.register("take-while", take_while);
    p.register("drop", drop);
    p.register("enumerate", enumerate);
    p.register("repeat", repeat);
}

fn items(cx: &mut NativeCx<'_>, v: &Value, what: &str) -> Result<Vec<Value>, Failure> {
    Seq::of(v, what)?.collect(cx)
}

fn count(v: &Value, what: &str) -> Result<usize, Failure> {
    let n = int_of(v, what)?;
    Ok(usize::try_from(n.max(0)).unwrap_or(usize::MAX))
}

fn len_n(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    match arg(args, 0) {
        Value::Range(r) if r.end.is_none() => Err(type_err("`len` of an unbounded range")),
        v => Ok(int_value(i64::try_from(len(&v)?).unwrap_or(i64::MAX))),
    }
}

fn first_n(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    first(&arg(args, 0))
}

fn last(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    Ok(items(cx, &arg(args, 0), "`last`")?
        .pop()
        .unwrap_or(Value::Nil))
}

fn tail(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let v = arg(args, 0);
    if let Value::Range(r) = &v {
        if r.end.is_none() {
            let start = r.start.saturating_add(1);
            return Ok(Value::Range(crate::value::value::RangeVal {
                start,
                end: None,
            }));
        }
    }
    let mut all = items(cx, &v, "`tail`")?;
    if !all.is_empty() {
        all.remove(0);
    }
    Ok(list_of(all))
}

fn reverse(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let mut all = items(cx, &arg(args, 0), "`reverse`")?;
    all.reverse();
    Ok(list_of(all))
}

fn sort(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let v = cx.deep(&arg(args, 0))?;
    let all = items(cx, &v, "`sort`")?;
    let mut keyed = Vec::with_capacity(all.len());
    for x in all {
        keyed.push((Key::from_value(&x)?, x));
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(list_of(keyed.into_iter().map(|(_, x)| x).collect()))
}

fn map(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`map`")?;
    let mut out = Vec::new();
    while let Some(x) = seq.next(cx)? {
        out.push(cx.call(&f, vec![x])?);
    }
    Ok(list_of(out))
}

fn filter(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`filter`")?;
    let mut out = Vec::new();
    while let Some(x) = seq.next(cx)? {
        if truthy(&cx.call(&f, vec![x.clone()])?) {
            out.push(x);
        }
    }
    Ok(list_of(out))
}

fn reduce(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 2);
    let mut acc = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`reduce`")?;
    while let Some(x) = seq.next(cx)? {
        acc = cx.call(&f, vec![acc, x])?;
    }
    Ok(acc)
}

fn find(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`find`")?;
    while let Some(x) = seq.next(cx)? {
        if truthy(&cx.call(&f, vec![x.clone()])?) {
            return Ok(x);
        }
    }
    Ok(Value::Nil)
}

fn any(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`any`")?;
    while let Some(x) = seq.next(cx)? {
        if truthy(&cx.call(&f, vec![x])?) {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

fn all(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`all`")?;
    while let Some(x) = seq.next(cx)? {
        if !truthy(&cx.call(&f, vec![x])?) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn take(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let n = count(&arg(args, 1), "`take`")?;
    let mut seq = Seq::of(&arg(args, 0), "`take`")?;
    let mut out = Vec::new();
    while out.len() < n {
        match seq.next(cx)? {
            Some(x) => out.push(x),
            None => break,
        }
    }
    Ok(list_of(out))
}

fn take_while(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let f = arg(args, 1);
    let mut seq = Seq::of(&arg(args, 0), "`take-while`")?;
    let mut out = Vec::new();
    while let Some(x) = seq.next(cx)? {
        if !truthy(&cx.call(&f, vec![x.clone()])?) {
            break;
        }
        out.push(x);
    }
    Ok(list_of(out))
}

fn drop(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let n = count(&arg(args, 1), "`drop`")?;
    let v = arg(args, 0);
    if let Value::Range(r) = &v {
        if r.end.is_none() {
            let k = i64::try_from(n).unwrap_or(i64::MAX);
            let start = r.start.saturating_add(k);
            return Ok(Value::Range(crate::value::value::RangeVal {
                start,
                end: None,
            }));
        }
    }
    let all = items(cx, &v, "`drop`")?;
    Ok(list_of(all.into_iter().skip(n).collect()))
}

fn enumerate(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let all = items(cx, &arg(args, 0), "`enumerate`")?;
    Ok(list_of(
        all.into_iter()
            .enumerate()
            .map(|(k, x)| list_of(vec![int_value(i64::try_from(k).unwrap_or(i64::MAX)), x]))
            .collect(),
    ))
}

/// `repeat value count` (subject first).
fn repeat(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let v = arg(args, 0);
    let n = count(&arg(args, 1), "`repeat`")?;
    let mut out = Vec::new();
    for _ in 0..n {
        cx.tick()?;
        out.push(v.clone());
    }
    Ok(list_of(out))
}
