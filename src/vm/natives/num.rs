//! Arithmetic, comparison and numeric natives (lang-reference sections 2
//! and 5; design 6.5.3).
//!
//! `+ - *` and the comparisons widen along the 6.5.3 lattice. Integer
//! overflow and a zero divisor are failures. `/` of two exact operands is
//! exact: an integral quotient of two ints is an int, any other exact
//! quotient a ratio.

use std::cmp::Ordering;

use crate::ns::namespace::Prelude;
use crate::value::eq::deep_eq;
use crate::value::intern::KwId;
use crate::value::key::Key;
use crate::value::num::{join, widen, NumKind};
use crate::value::ratio::Ratio64;
use crate::value::value::{RangeVal, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::{dsp, int_of, type_err};
use crate::vm::vm::int_value;

type Kw<'a> = &'a [(KwId, Value)];

pub(super) fn register(p: &mut Prelude) {
    p.register("+", add);
    p.register("-", sub);
    p.register("*", mul);
    p.register("/", div);
    p.register("=", eq);
    p.register("<", lt);
    p.register(">", gt);
    p.register("<=", le);
    p.register(">=", ge);
    p.register("..", range);
    p.register("mod", modulo);
    p.register("neg", neg);
    p.register("abs", abs);
    p.register("min", min);
    p.register("max", max);
    p.register("sin", sin);
    p.register("cos", cos);
    p.register("int", to_int);
    p.register("int64", to_int64);
    p.register("float", to_float);
    p.register("round", round);
}

#[derive(Clone, Copy)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
}

impl Arith {
    const fn name(self) -> &'static str {
        match self {
            Arith::Add => "+",
            Arith::Sub => "-",
            Arith::Mul => "*",
            Arith::Div => "/",
        }
    }
}

fn overflow() -> Failure {
    Failure::new(FailCode::Overflow, "integer overflow")
}

fn div_zero() -> Failure {
    Failure::new(FailCode::DivisionByZero, "division by zero")
}

fn kind(op: &str, v: &Value) -> Result<NumKind, Failure> {
    NumKind::of(v).ok_or_else(|| type_err(format!("`{op}` expects numbers, got {}", kind_name(v))))
}

fn exact_quotient(n: i64, d: i64, as_int: bool) -> Result<Value, Failure> {
    if d == 0 {
        return Err(div_zero());
    }
    let r = Ratio64::new(n, d)?;
    Ok(match (as_int, r.is_integral()) {
        (true, true) => int_value(r.num()),
        _ => Value::Ratio(r),
    })
}

/// One binary step of `op` over the widening lattice.
fn arith2(op: Arith, a: &Value, b: &Value) -> Result<Value, Failure> {
    let k = join(kind(op.name(), a)?, kind(op.name(), b)?);
    let (a, b) = (widen(a, k)?, widen(b, k)?);
    Ok(match (a, b) {
        (Value::Int(x), Value::Int(y)) => match op {
            Arith::Add => Value::Int(x.checked_add(y).ok_or_else(overflow)?),
            Arith::Sub => Value::Int(x.checked_sub(y).ok_or_else(overflow)?),
            Arith::Mul => Value::Int(x.checked_mul(y).ok_or_else(overflow)?),
            Arith::Div => exact_quotient(i64::from(x), i64::from(y), true)?,
        },
        (Value::Int64(x), Value::Int64(y)) => match op {
            Arith::Add => Value::Int64(x.checked_add(y).ok_or_else(overflow)?),
            Arith::Sub => Value::Int64(x.checked_sub(y).ok_or_else(overflow)?),
            Arith::Mul => Value::Int64(x.checked_mul(y).ok_or_else(overflow)?),
            Arith::Div => match exact_quotient(x, y, true)? {
                Value::Int(i) => Value::Int64(i64::from(i)),
                other => other,
            },
        },
        (Value::Float(x), Value::Float(y)) => match op {
            Arith::Add => Value::Float(x + y),
            Arith::Sub => Value::Float(x - y),
            Arith::Mul => Value::Float(x * y),
            Arith::Div if y == 0.0 => return Err(div_zero()),
            Arith::Div => Value::Float(x / y),
        },
        (Value::Float64(x), Value::Float64(y)) => match op {
            Arith::Add => Value::Float64(x + y),
            Arith::Sub => Value::Float64(x - y),
            Arith::Mul => Value::Float64(x * y),
            Arith::Div if y == 0.0 => return Err(div_zero()),
            Arith::Div => Value::Float64(x / y),
        },
        (Value::Ratio(x), Value::Ratio(y)) => Value::Ratio(match op {
            Arith::Add => x.checked_add(y)?,
            Arith::Sub => x.checked_sub(y)?,
            Arith::Mul => x.checked_mul(y)?,
            Arith::Div if y.num() == 0 => return Err(div_zero()),
            Arith::Div => x.checked_div(y)?,
        }),
        _ => return Err(type_err(format!("`{}` expects numbers", op.name()))),
    })
}

fn fold(op: Arith, args: &[Value]) -> Result<Value, Failure> {
    let Some((first, rest)) = args.split_first() else {
        return Err(Failure::new(
            FailCode::Arity,
            format!("`{}` needs an argument", op.name()),
        ));
    };
    if rest.is_empty() {
        kind(op.name(), first)?;
        return match op {
            Arith::Add | Arith::Mul => Ok(first.clone()),
            Arith::Sub => arith2(Arith::Sub, &Value::Int(0), first),
            Arith::Div => arith2(Arith::Div, &Value::Int(1), first),
        };
    }
    let mut acc = first.clone();
    for x in rest {
        acc = arith2(op, &acc, x)?;
    }
    Ok(acc)
}

fn add(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    // A unit-generator operand builds a node (12.8.6).
    if let Some(r) = dsp::arith('+', args) {
        return r;
    }
    fold(Arith::Add, args)
}

fn sub(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    // A unit-generator operand builds a node (12.8.6).
    if let Some(r) = dsp::arith('-', args) {
        return r;
    }
    fold(Arith::Sub, args)
}

fn mul(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    // A unit-generator operand builds a node (12.8.6).
    if let Some(r) = dsp::arith('*', args) {
        return r;
    }
    fold(Arith::Mul, args)
}

fn div(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    fold(Arith::Div, args)
}

/// Orders two numbers exactly, or two strings or two keywords.
fn order(op: &str, a: &Value, b: &Value) -> Result<Ordering, Failure> {
    let same = matches!(
        (a, b),
        (Value::Str(_), Value::Str(_)) | (Value::Keyword(_), Value::Keyword(_))
    ) || (NumKind::of(a).is_some() && NumKind::of(b).is_some());
    if !same {
        return Err(type_err(format!(
            "`{op}` cannot compare {} and {}",
            kind_name(a),
            kind_name(b)
        )));
    }
    Ok(Key::from_value(a)?.cmp(&Key::from_value(b)?))
}

fn compare(op: &str, args: &[Value], want: fn(Ordering) -> bool) -> Result<Value, Failure> {
    let [a, b] = args else {
        return Err(Failure::new(
            FailCode::Arity,
            format!("`{op}` takes two arguments"),
        ));
    };
    Ok(Value::Bool(want(order(op, a, b)?)))
}

fn eq(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    let a = cx.deep(&super::arg(args, 0))?;
    let b = cx.deep(&super::arg(args, 1))?;
    Ok(Value::Bool(deep_eq(&a, &b)?))
}

fn lt(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    compare("<", args, |o| o == Ordering::Less)
}

fn gt(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    compare(">", args, |o| o == Ordering::Greater)
}

fn le(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    compare("<=", args, |o| o != Ordering::Greater)
}

fn ge(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    compare(">=", args, |o| o != Ordering::Less)
}

/// `0..8` (exclusive end) and the lazy `0..`.
fn range(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    let start = int_of(&super::arg(args, 0), "`..`")?;
    let end = match args.get(1) {
        Some(e) => Some(int_of(e, "`..`")?),
        None => None,
    };
    Ok(Value::Range(RangeVal { start, end }))
}

fn modulo(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    let (a, b) = (super::arg(args, 0), super::arg(args, 1));
    let k = join(kind("mod", &a)?, kind("mod", &b)?);
    Ok(match (widen(&a, k)?, widen(&b, k)?) {
        (Value::Int(x), Value::Int(y)) => {
            if y == 0 {
                return Err(div_zero());
            }
            Value::Int(x.checked_rem_euclid(y).ok_or_else(overflow)?)
        }
        (Value::Int64(x), Value::Int64(y)) => {
            if y == 0 {
                return Err(div_zero());
            }
            Value::Int64(x.checked_rem_euclid(y).ok_or_else(overflow)?)
        }
        (Value::Float(x), Value::Float(y)) if y != 0.0 => Value::Float(x.rem_euclid(y)),
        (Value::Float64(x), Value::Float64(y)) if y != 0.0 => Value::Float64(x.rem_euclid(y)),
        (Value::Ratio(x), Value::Ratio(y)) if y.num() != 0 => {
            let q = x.checked_div(y)?.floor();
            Value::Ratio(x.checked_sub(y.checked_mul(Ratio64::from_int(q))?)?)
        }
        _ => return Err(div_zero()),
    })
}

fn neg(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    arith2(Arith::Sub, &Value::Int(0), &super::arg(args, 0))
}

fn abs(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    let x = super::arg(args, 0);
    let negative = order("abs", &x, &Value::Int(0))? == Ordering::Less;
    if negative {
        arith2(Arith::Sub, &Value::Int(0), &x)
    } else {
        Ok(x)
    }
}

fn extreme(
    cx: &mut NativeCx<'_>,
    op: &str,
    args: &[Value],
    keep: Ordering,
) -> Result<Value, Failure> {
    let mut items: Vec<Value> = match args {
        [Value::List(l)] => l.items.to_vec(),
        _ => args.to_vec(),
    };
    for x in &mut items {
        *x = cx.deep(x)?;
    }
    let mut it = items.into_iter();
    let Some(mut best) = it.next() else {
        return Ok(Value::Nil);
    };
    for x in it {
        if order(op, &x, &best)? == keep {
            best = x;
        }
    }
    Ok(best)
}

fn min(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    extreme(cx, "min", args, Ordering::Less)
}

fn max(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    extreme(cx, "max", args, Ordering::Greater)
}

fn float_of(op: &str, v: &Value) -> Result<f64, Failure> {
    Ok(match v {
        Value::Int(i) => f64::from(*i),
        Value::Int64(i) => *i as f64,
        Value::Float(x) => f64::from(*x),
        Value::Float64(x) => *x,
        Value::Ratio(r) => r.to_f64(),
        other => {
            return Err(type_err(format!(
                "`{op}` expects a number, got {}",
                kind_name(other)
            )))
        }
    })
}

fn unary_float(op: &str, v: &Value, f: fn(f64) -> f64) -> Result<Value, Failure> {
    let x = float_of(op, v)?;
    Ok(match v {
        Value::Float64(_) | Value::Int64(_) | Value::Ratio(_) => Value::Float64(f(x)),
        _ => Value::Float(f(x) as f32),
    })
}

fn sin(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    unary_float("sin", &super::arg(args, 0), f64::sin)
}

fn cos(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    unary_float("cos", &super::arg(args, 0), f64::cos)
}

/// Truncates toward zero to an `i64`, failing past its range or for NaN.
fn trunc_i64(op: &str, v: &Value, round: bool) -> Result<i64, Failure> {
    match v {
        Value::Int(i) => Ok(i64::from(*i)),
        Value::Int64(i) => Ok(*i),
        Value::Ratio(r) if round => {
            // Half away from zero, in i128 so no step overflows.
            let (n, d) = (i128::from(r.num()), i128::from(r.den()));
            let (q, rem) = (n / d, n % d);
            let q = if 2 * rem.abs() >= d {
                q + n.signum()
            } else {
                q
            };
            i64::try_from(q).map_err(|_| overflow())
        }
        Value::Ratio(r) => Ok(r.num() / r.den()),
        _ => {
            let x = float_of(op, v)?;
            let y = if round { x.round() } else { x.trunc() };
            if (-9.223_372_036_854_775e18..9.223_372_036_854_775e18).contains(&y) {
                Ok(y as i64)
            } else {
                Err(overflow())
            }
        }
    }
}

fn to_int(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    let n = trunc_i64("int", &super::arg(args, 0), false)?;
    Ok(Value::Int(i32::try_from(n).map_err(|_| overflow())?))
}

fn to_int64(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    Ok(Value::Int64(trunc_i64(
        "int64",
        &super::arg(args, 0),
        false,
    )?))
}

fn to_float(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    Ok(Value::Float(float_of("float", &super::arg(args, 0))? as f32))
}

fn round(_: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> Result<Value, Failure> {
    let n = trunc_i64("round", &super::arg(args, 0), true)?;
    Ok(Value::Int(i32::try_from(n).map_err(|_| overflow())?))
}
