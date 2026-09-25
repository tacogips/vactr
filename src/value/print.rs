//! The print format (design 6.5.3): values print in literal syntax.

use std::fmt::{self, Display, Write as _};

use crate::value::intern::{name_of_kw, name_of_sym};
use crate::value::key::Key;
use crate::value::value::{Sound, Value};

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_value(f, self, true)
    }
}

/// Floats use the shortest round-trip form for their width, with `.0`
/// appended when there is no `.` or exponent. Non-finite values print as
/// `inf`, `-inf` and `NaN`.
fn write_float(f: &mut fmt::Formatter<'_>, x: impl Display, finite: bool) -> fmt::Result {
    let text = x.to_string();
    f.write_str(&text)?;
    if finite && !text.contains(['.', 'e', 'E']) {
        f.write_str(".0")?;
    }
    Ok(())
}

fn write_quoted(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    f.write_char('"')?;
    for c in s.chars() {
        match c {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\t' => f.write_str("\\t")?,
            '{' => f.write_str("\\{")?,
            '}' => f.write_str("\\}")?,
            _ => f.write_char(c)?,
        }
    }
    f.write_char('"')
}

fn write_seq<'a>(
    f: &mut fmt::Formatter<'_>,
    items: impl Iterator<Item = &'a Value>,
) -> fmt::Result {
    for (i, item) in items.enumerate() {
        if i > 0 {
            f.write_char(' ')?;
        }
        write_value(f, item, false)?;
    }
    Ok(())
}

fn write_value(f: &mut fmt::Formatter<'_>, v: &Value, top: bool) -> fmt::Result {
    match v {
        Value::Nil => f.write_str("nil"),
        Value::Bool(b) => write!(f, "{b}"),
        Value::Int(i) => write!(f, "{i}"),
        Value::Int64(i) => write!(f, "{i}"),
        Value::Float(x) => write_float(f, x, x.is_finite()),
        Value::Float64(x) => write_float(f, x, x.is_finite()),
        Value::Ratio(r) => write!(f, "{r}"),
        Value::Keyword(k) => write!(f, ":{}", name_of_kw(*k)),
        Value::Str(s) if top => f.write_str(s),
        Value::Str(s) => write_quoted(f, s),
        Value::List(l) => {
            f.write_char('[')?;
            write_seq(f, l.items.iter())?;
            f.write_char(']')
        }
        Value::Dict(d) => {
            f.write_char('[')?;
            for (i, (k, val)) in d.iter().enumerate() {
                if i > 0 {
                    f.write_char(' ')?;
                }
                if let Key::Kw(kw) = k {
                    write!(f, "{}: ", name_of_kw(*kw))?;
                    write_value(f, val, false)?;
                } else {
                    f.write_char('[')?;
                    write_value(f, &k.to_value(), false)?;
                    f.write_char(' ')?;
                    write_value(f, val, false)?;
                    f.write_char(']')?;
                }
            }
            f.write_char(']')
        }
        Value::Struct(s) => {
            f.write_str(&name_of_sym(s.ty))?;
            for (k, val) in s.fields.iter() {
                write!(f, " {}: ", name_of_kw(*k))?;
                write_value(f, val, false)?;
            }
            Ok(())
        }
        Value::Variant(s) => {
            f.write_str(&name_of_sym(s.tag))?;
            for (_, val) in s.fields.iter() {
                f.write_char(' ')?;
                write_value(f, val, false)?;
            }
            Ok(())
        }
        Value::Range(r) => match r.end {
            Some(end) => write!(f, "{}..{end}", r.start),
            None => write!(f, "{}..", r.start),
        },
        Value::Path(p) => f.write_str(&p.text),
        Value::Url(u) => f.write_str(u),
        Value::Sound(s) => match &**s {
            Sound::Builtin(k) => write!(f, "(sound :{})", name_of_kw(*k)),
            Sound::Sample(p) => write!(f, "(sound {})", p.text),
            Sound::MidiOut(ch) => write!(f, "(sound midi {ch})"),
        },
        Value::Fn(_) => f.write_str("<fn>"),
        Value::Native(_) => f.write_str("<native>"),
        Value::Thunk(_) => f.write_str("<thunk>"),
        Value::VarRef(_) => f.write_str("<var>"),
        Value::Pattern(_) => f.write_str("<pattern>"),
        Value::Signal(_) => f.write_str("<signal>"),
        Value::Inst(_) => f.write_str("<inst>"),
        Value::Tex(_) => f.write_str("<tex>"),
    }
}
