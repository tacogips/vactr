//! Console and strings: `print` (an effect: staged in Normal mode, captured
//! in Query mode, 10.4) and `concat`, the string-interpolation head (6.5.5).

use std::fmt::Write as _;

use crate::ns::namespace::Prelude;
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::call::NativeCx;
use crate::vm::fail::Failure;

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("print", print);
    p.register("concat", concat);
}

fn text(cx: &mut NativeCx<'_>, args: &[Value], sep: &str) -> Result<String, Failure> {
    let mut out = String::new();
    for (k, v) in args.iter().enumerate() {
        if k > 0 {
            out.push_str(sep);
        }
        let v = cx.deep(v)?;
        let _ = write!(out, "{v}");
    }
    Ok(out)
}

/// `print a ..`: prints the values separated by spaces and returns its
/// first argument (`nil` when there is none).
fn print(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let line = text(cx, args, " ")?;
    cx.print(&line);
    Ok(args.first().cloned().unwrap_or(Value::Nil))
}

/// `concat a ..`: the printed forms joined with nothing (strings raw).
fn concat(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    Ok(Value::str(&text(cx, args, "")?))
}
