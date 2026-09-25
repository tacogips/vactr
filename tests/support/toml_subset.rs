//! A test-only parser for the TOML subset of the spec fixture manifest
//! (design 6.5.6): `[[block]]`/`[[case]]` table arrays, `key = value` with
//! basic strings (escapes `\" \\ \n \t`), `'...'` and `'''...'''` literal
//! strings (a newline right after the opening `'''` is trimmed), integers,
//! booleans, one-line string arrays, and `#` comments. Anything else is an
//! error that names its line.

use std::collections::BTreeMap;

/// A field value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Int(i64),
    Bool(bool),
    Array(Vec<String>),
}

/// One `[[block]]` or `[[case]]` table.
#[derive(Clone, Debug, Default)]
pub struct Table {
    /// The 1-based line of the table header.
    pub line: usize,
    pub fields: BTreeMap<String, Value>,
}

impl Table {
    /// A string field.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.fields.get(key) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// A string field that must be present.
    pub fn req(&self, key: &str) -> &str {
        self.str(key)
            .unwrap_or_else(|| panic!("table at line {}: missing string `{key}`", self.line))
    }

    /// An integer field.
    pub fn int(&self, key: &str) -> Option<i64> {
        match self.fields.get(key) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        }
    }

    /// A boolean field.
    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.fields.get(key) {
            Some(Value::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// A string-array field (absent means empty).
    pub fn array(&self, key: &str) -> Vec<String> {
        match self.fields.get(key) {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        }
    }
}

/// The parsed manifest.
#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub blocks: Vec<Table>,
    pub cases: Vec<Table>,
}

fn fail<T>(line: usize, msg: &str) -> Result<T, String> {
    Err(format!("manifest line {line}: {msg}"))
}

/// Parses `text`. The error names the offending line.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut manifest = Manifest::default();
    let mut current: Option<(bool, Table)> = None;
    let mut k = 0;
    while k < lines.len() {
        let n = k + 1;
        let line = lines[k].trim_end_matches('\r');
        let trimmed = line.trim();
        k += 1;
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            let is_block = match trimmed {
                "[[block]]" => true,
                "[[case]]" => false,
                _ => return fail(n, "only [[block]] and [[case]] headers are supported"),
            };
            if let Some((b, t)) = current.take() {
                push(&mut manifest, b, t);
            }
            current = Some((
                is_block,
                Table {
                    line: n,
                    fields: BTreeMap::new(),
                },
            ));
            continue;
        }
        let Some((key, rest)) = line.split_once('=') else {
            return fail(n, "expected `key = value`");
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return fail(n, "unsupported key");
        }
        let rest = rest.trim_start();
        let (value, tail) = if let Some(after) = rest.strip_prefix("'''") {
            let (s, tail, used) = multiline(after, &lines[k..], n)?;
            k += used;
            (Value::Str(s), tail)
        } else {
            value(rest, n)?
        };
        let tail = tail.trim();
        if !tail.is_empty() && !tail.starts_with('#') {
            return fail(n, "unexpected text after the value");
        }
        let Some((_, table)) = current.as_mut() else {
            return fail(n, "a key outside a [[block]] or [[case]] table");
        };
        if table.fields.insert(key.to_string(), value).is_some() {
            return fail(n, "duplicate key");
        }
    }
    if let Some((b, t)) = current.take() {
        push(&mut manifest, b, t);
    }
    Ok(manifest)
}

fn push(manifest: &mut Manifest, is_block: bool, table: Table) {
    if is_block {
        manifest.blocks.push(table);
    } else {
        manifest.cases.push(table);
    }
}

/// A `'''` literal string starting right after the delimiter. Returns the
/// text, what follows the closing delimiter, and the extra lines consumed.
fn multiline<'a>(
    first: &'a str,
    more: &[&'a str],
    n: usize,
) -> Result<(String, &'a str, usize), String> {
    if let Some(end) = first.find("'''") {
        return Ok((first[..end].to_string(), &first[end + 3..], 0));
    }
    let mut text = String::new();
    if !first.trim_end_matches('\r').is_empty() {
        text.push_str(first.trim_end_matches('\r'));
        text.push('\n');
    }
    for (used, line) in more.iter().enumerate() {
        let line = line.trim_end_matches('\r');
        if let Some(end) = line.find("'''") {
            text.push_str(&line[..end]);
            return Ok((text, &line[end + 3..], used + 1));
        }
        text.push_str(line);
        text.push('\n');
    }
    fail(n, "unterminated ''' string")
}

/// A one-line value; returns it and the text after it.
fn value(s: &str, n: usize) -> Result<(Value, &str), String> {
    if let Some(rest) = s.strip_prefix('"') {
        let (text, tail) = basic(rest, n)?;
        return Ok((Value::Str(text), tail));
    }
    if let Some(rest) = s.strip_prefix('\'') {
        let Some(end) = rest.find('\'') else {
            return fail(n, "unterminated ' string");
        };
        return Ok((Value::Str(rest[..end].to_string()), &rest[end + 1..]));
    }
    if let Some(mut rest) = s.strip_prefix('[') {
        let mut items = Vec::new();
        loop {
            rest = rest.trim_start();
            if let Some(tail) = rest.strip_prefix(']') {
                return Ok((Value::Array(items), tail));
            }
            let Some(after) = rest.strip_prefix('"') else {
                return fail(n, "arrays hold basic strings only, on one line");
            };
            let (text, tail) = basic(after, n)?;
            items.push(text);
            rest = tail.trim_start();
            if let Some(tail) = rest.strip_prefix(',') {
                rest = tail;
            } else if !rest.starts_with(']') {
                return fail(n, "expected `,` or `]` in an array");
            }
        }
    }
    let word_end = s
        .find(|c: char| c.is_whitespace() || c == '#')
        .unwrap_or(s.len());
    let (word, tail) = s.split_at(word_end);
    match word {
        "true" => Ok((Value::Bool(true), tail)),
        "false" => Ok((Value::Bool(false), tail)),
        _ => match word.parse::<i64>() {
            Ok(v) => Ok((Value::Int(v), tail)),
            Err(_) => fail(n, "unsupported value"),
        },
    }
}

/// The rest of a basic string after its opening quote.
fn basic(s: &str, n: usize) -> Result<(String, &str), String> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((k, c)) = chars.next() {
        match c {
            '"' => return Ok((out, &s[k + 1..])),
            '\\' => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                _ => return fail(n, "unsupported escape"),
            },
            _ => out.push(c),
        }
    }
    fail(n, "unterminated string")
}
