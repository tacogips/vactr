//! Value natives (lang-reference sections 2 and 5): predicates and
//! truthiness (`or` is also the `?` fallback, 6.5.5).

use crate::ns::namespace::Prelude;
use crate::value::access::truthy;
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::call::NativeCx;
use crate::vm::fail::Failure;
use crate::vm::natives::arg;

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("is-nil", is_nil);
    p.register("is-list", is_list);
    p.register("not", not);
    p.register("or", or);
    p.register("and", and);
}

fn is_nil(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    Ok(Value::Bool(matches!(arg(args, 0), Value::Nil)))
}

/// Lists, dicts (a dict is a list of pairs) and ranges are lists.
fn is_list(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    Ok(Value::Bool(matches!(
        arg(args, 0),
        Value::List(_) | Value::Dict(_) | Value::Range(_)
    )))
}

fn not(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    Ok(Value::Bool(!truthy(&arg(args, 0))))
}

/// The first truthy argument, else the last one.
fn or(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    for v in args {
        if truthy(v) {
            return Ok(v.clone());
        }
    }
    Ok(args.last().cloned().unwrap_or(Value::Nil))
}

/// The first falsy argument, else the last one.
fn and(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    for v in args {
        if !truthy(v) {
            return Ok(v.clone());
        }
    }
    Ok(args.last().cloned().unwrap_or(Value::Nil))
}
