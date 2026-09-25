//! `vactrol.toml` (design 14.5.7): a strict in-crate TOML subset.
//!
//! Accepted: the tables `[package]` and `[deps]`, `key = value` lines whose
//! key is bare (`[A-Za-z0-9_-]+`) or a basic string, values that are basic
//! strings or one-line string arrays, blank lines and `#` comments.
//! Anything else is a parse error with its line number.
//!
//! ```toml
//! [package]
//! path = "github.com/someone/vactrol-pads"
//! vactrol = "v0.1.0"
//! assets = ["samples"]
//!
//! [deps]
//! "github.com/someone/vactrol-drums" = "v1.2.0"
//! ```

use std::fmt;
use std::rc::Rc;

use crate::ns::pkg::PackageId;
use crate::pkg::semver::Version;

/// The `[package]` table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PackageMeta {
    /// Must equal the import path the package was fetched as.
    pub path: PackageId,
    /// The minimum language version.
    pub vactrol: Option<Version>,
    /// Sample directories, relative to the package root.
    pub assets: Vec<Rc<str>>,
}

/// A `vactrol.toml`. A set's root manifest may omit `[package]`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PkgManifest {
    pub package: Option<PackageMeta>,
    /// Requirements, sorted by path, one per path.
    pub deps: Vec<(PackageId, Version)>,
}

impl PkgManifest {
    /// Adds the requirement `id >= version`, keeping the higher of an
    /// existing requirement and `version`.
    pub fn add_or_raise(&mut self, id: PackageId, version: Version) {
        match self
            .deps
            .binary_search_by(|(p, _)| p.0.as_bytes().cmp(id.0.as_bytes()))
        {
            Ok(i) => {
                if version > self.deps[i].1 {
                    self.deps[i].1 = version;
                }
            }
            Err(i) => self.deps.insert(i, (id, version)),
        }
    }
}

/// A `vactrol.toml` or `vactrol.lock` parse error (lines count from 1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ManifestError {
    pub line: u32,
    pub message: String,
}

impl ManifestError {
    pub(crate) fn at(line: usize, message: impl Into<String>) -> ManifestError {
        ManifestError {
            line: u32::try_from(line).unwrap_or(u32::MAX),
            message: message.into(),
        }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ManifestError {}

/// True for a lowercase `github.com/<owner>/<name>` path whose segments
/// are `[a-z0-9._-]`, not `.` or `..`.
#[must_use]
pub fn valid_package_path(path: &str) -> bool {
    let segs: Vec<&str> = path.split('/').collect();
    segs.len() == 3
        && segs[0] == "github.com"
        && segs[1..].iter().all(|s| {
            !s.is_empty()
                && *s != "."
                && *s != ".."
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        })
}

/// A package id from a manifest or lock path.
///
/// # Errors
/// The message when `path` is not a valid package path.
pub(crate) fn package_id(path: &str) -> Result<PackageId, String> {
    if valid_package_path(path) {
        Ok(PackageId::new(path))
    } else {
        Err(format!(
            "`{}` is not a lowercase `github.com/<owner>/<name>` path",
            path.escape_debug()
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    None,
    Package,
    Deps,
}

enum Val {
    Str(String),
    Arr(Vec<String>),
}

/// A cursor over one line.
struct Line<'a> {
    b: &'a [u8],
    at: usize,
}

impl Line<'_> {
    fn skip_ws(&mut self) {
        while self.at < self.b.len() && matches!(self.b[self.at], b' ' | b'\t') {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.b.get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// The rest is blank or a comment.
    fn end(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.at == self.b.len() || self.b[self.at] == b'#' {
            Ok(())
        } else {
            Err("unexpected text after the value".into())
        }
    }

    fn bare_key(&mut self) -> Option<String> {
        let start = self.at;
        while self.at < self.b.len()
            && (self.b[self.at].is_ascii_alphanumeric() || matches!(self.b[self.at], b'_' | b'-'))
        {
            self.at += 1;
        }
        (self.at > start).then(|| String::from_utf8_lossy(&self.b[start..self.at]).into_owned())
    }

    fn hex_char(&mut self, n: usize) -> Result<char, String> {
        let digits = self.b.get(self.at..self.at + n).ok_or("truncated escape")?;
        let text = std::str::from_utf8(digits).map_err(|_| "bad escape")?;
        let v = u32::from_str_radix(text, 16).map_err(|_| "bad escape")?;
        self.at += n;
        char::from_u32(v).ok_or_else(|| "bad unicode escape".into())
    }

    /// A basic string; the cursor is on the opening quote.
    fn string(&mut self) -> Result<String, String> {
        if !self.eat(b'"') {
            return Err("expected a basic string".into());
        }
        let mut out = Vec::new();
        loop {
            let Some(&c) = self.b.get(self.at) else {
                return Err("unterminated string".into());
            };
            self.at += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = *self.b.get(self.at).ok_or("unterminated string")?;
                    self.at += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => self.hex_char(4)?,
                        b'U' => self.hex_char(8)?,
                        _ => return Err("unknown escape".into()),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c if (c < 0x20 && c != b'\t') || c == 0x7f => {
                    return Err("control character in a string".into())
                }
                c => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| "a string is not UTF-8".into())
    }

    fn value(&mut self) -> Result<Val, String> {
        self.skip_ws();
        if self.b.get(self.at) == Some(&b'"') {
            return self.string().map(Val::Str);
        }
        if !self.eat(b'[') {
            return Err("a value must be a basic string or a string array".into());
        }
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.eat(b']') {
                return Ok(Val::Arr(items));
            }
            items.push(self.string()?);
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            self.skip_ws();
            if !self.eat(b']') {
                return Err("expected `,` or `]` in an array".into());
            }
            return Ok(Val::Arr(items));
        }
    }
}

#[derive(Default)]
struct PackageDraft {
    path: Option<PackageId>,
    vactrol: Option<Version>,
    assets: Option<Vec<Rc<str>>>,
}

fn version(text: &str) -> Result<Version, String> {
    Version::parse_tag(text).ok_or_else(|| {
        format!(
            "`{}` is not a `vMAJOR.MINOR.PATCH` version",
            text.escape_debug()
        )
    })
}

/// Parses a `vactrol.toml`.
///
/// # Errors
/// The first error, with its line number.
pub fn parse(text: &str) -> Result<PkgManifest, ManifestError> {
    let mut table = Table::None;
    let mut seen_tables = (false, false);
    let mut draft: Option<PackageDraft> = None;
    let mut package_line = 0;
    let mut deps: Vec<(PackageId, Version)> = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        let n = i + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let err = |m: String| ManifestError::at(n, m);
        let mut l = Line {
            b: raw.as_bytes(),
            at: 0,
        };
        l.skip_ws();
        if l.at == l.b.len() || l.b[l.at] == b'#' {
            continue;
        }
        if l.eat(b'[') {
            l.skip_ws();
            let name = l
                .bare_key()
                .ok_or_else(|| err("expected a table name".into()))?;
            l.skip_ws();
            if !l.eat(b']') {
                return Err(err("expected `]`".into()));
            }
            l.end().map_err(err)?;
            let (t, seen) = match name.as_str() {
                "package" => (Table::Package, &mut seen_tables.0),
                "deps" => (Table::Deps, &mut seen_tables.1),
                other => return Err(err(format!("unknown table `[{other}]`"))),
            };
            if *seen {
                return Err(err(format!("the table `[{name}]` appears twice")));
            }
            *seen = true;
            table = t;
            if t == Table::Package {
                draft = Some(PackageDraft::default());
                package_line = n;
            }
            continue;
        }
        let key = if l.b[l.at] == b'"' {
            l.string().map_err(err)?
        } else {
            l.bare_key().ok_or_else(|| err("expected a key".into()))?
        };
        l.skip_ws();
        if !l.eat(b'=') {
            return Err(err("expected `=` after the key".into()));
        }
        let val = l.value().map_err(err)?;
        l.end().map_err(err)?;
        match table {
            Table::None => return Err(err(format!("`{key}` is outside a table"))),
            Table::Package => {
                let d = draft.get_or_insert_with(PackageDraft::default);
                let dup = |set: bool| {
                    if set {
                        Err(err(format!("`{key}` appears twice")))
                    } else {
                        Ok(())
                    }
                };
                match (key.as_str(), val) {
                    ("path", Val::Str(s)) => {
                        dup(d.path.is_some())?;
                        d.path = Some(package_id(&s).map_err(err)?);
                    }
                    ("vactrol", Val::Str(s)) => {
                        dup(d.vactrol.is_some())?;
                        d.vactrol = Some(version(&s).map_err(err)?);
                    }
                    ("assets", Val::Arr(items)) => {
                        dup(d.assets.is_some())?;
                        d.assets = Some(items.iter().map(|s| Rc::from(s.as_str())).collect());
                    }
                    ("path" | "vactrol", _) => {
                        return Err(err(format!("`{key}` must be a string")))
                    }
                    ("assets", _) => {
                        return Err(err("`assets` must be an array of strings".into()))
                    }
                    (other, _) => return Err(err(format!("unknown key `{other}` in `[package]`"))),
                }
            }
            Table::Deps => {
                let id = package_id(&key).map_err(err)?;
                let Val::Str(v) = val else {
                    return Err(err("a dependency version must be a string".into()));
                };
                let v = version(&v).map_err(err)?;
                if deps.iter().any(|(p, _)| *p == id) {
                    return Err(err(format!("`{key}` appears twice")));
                }
                deps.push((id, v));
            }
        }
    }
    let package = match draft {
        None => None,
        Some(d) => Some(PackageMeta {
            path: d
                .path
                .ok_or_else(|| ManifestError::at(package_line, "`[package]` has no `path`"))?,
            vactrol: d.vactrol,
            assets: d.assets.unwrap_or_default(),
        }),
    };
    deps.sort_by(|a, b| a.0 .0.as_bytes().cmp(b.0 .0.as_bytes()));
    Ok(PkgManifest { package, deps })
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Renders a manifest deterministically (deps sorted by path).
#[must_use]
pub fn render(m: &PkgManifest) -> String {
    let mut out = String::new();
    if let Some(p) = &m.package {
        out.push_str("[package]\n");
        out.push_str(&format!("path = {}\n", quote(&p.path.0)));
        if let Some(v) = &p.vactrol {
            out.push_str(&format!("vactrol = {}\n", quote(&v.to_string())));
        }
        if !p.assets.is_empty() {
            let items: Vec<String> = p.assets.iter().map(|a| quote(a)).collect();
            out.push_str(&format!("assets = [{}]\n", items.join(", ")));
        }
        out.push('\n');
    }
    out.push_str("[deps]\n");
    let mut deps: Vec<&(PackageId, Version)> = m.deps.iter().collect();
    deps.sort_by(|a, b| a.0 .0.as_bytes().cmp(b.0 .0.as_bytes()));
    for (id, v) in deps {
        out.push_str(&format!("{} = {}\n", quote(&id.0), quote(&v.to_string())));
    }
    out
}
