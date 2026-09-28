//! `import` forms and the document-local alias environment (5.7, 6.5.4).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::reader::lexer::is_identifier;
use crate::reader::span::{FileId, Span};

/// `import PATH [as ALIAS] [open]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportDecl {
    pub path: Rc<str>,
    pub alias: Option<Rc<str>>,
    pub open: bool,
    /// `ALIAS`, or the last path segment without a `vactr-` prefix.
    pub prefix: Rc<str>,
    pub span: Span,
}

/// Maps a bound qualifier prefix to its package path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AliasEnv {
    pub prefixes: BTreeMap<Rc<str>, Rc<str>>,
}

impl AliasEnv {
    /// An environment with no bound prefix (a fresh session).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `prefix` to the package at `path`; a later bind replaces it.
    pub fn bind(&mut self, prefix: Rc<str>, path: Rc<str>) {
        self.prefixes.insert(prefix, path);
    }

    /// True when `prefix` is bound.
    #[must_use]
    pub fn is_bound(&self, prefix: &str) -> bool {
        self.prefixes.contains_key(prefix)
    }
}

/// True when `path` is lowercase `[a-z0-9._-]` segments joined by `/`, with at
/// least one `/`, and no segment empty, `.` or `..`.
fn valid_path(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    segments.len() >= 2
        && segments.iter().all(|seg| {
            !seg.is_empty()
                && *seg != "."
                && *seg != ".."
                && seg
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
        })
}

/// Parses the code text of one `import` line (`text` starts with `import`).
///
/// # Errors
/// A message for `bad-import` when the form breaks the 6.5.4 rules.
pub(crate) fn parse_import(text: &str, span: Span) -> Result<ImportDecl, &'static str> {
    let mut words = text.split_ascii_whitespace();
    if words.next() != Some("import") {
        return Err("not an import form");
    }
    let path = words.next().ok_or("`import` needs a repository path")?;
    if !valid_path(path) {
        return Err("an import path is lowercase segments joined by `/`");
    }
    let mut alias = None;
    let mut open = false;
    let mut next = words.next();
    if next == Some("as") {
        let name = words.next().ok_or("`as` needs an alias")?;
        if !is_identifier(name) {
            return Err("an import alias must be an identifier");
        }
        alias = Some(Rc::from(name));
        next = words.next();
    }
    if next == Some("open") {
        open = true;
        next = words.next();
    }
    if next.is_some() {
        return Err("unexpected text after the import form");
    }
    let prefix: Rc<str> = match &alias {
        Some(a) => Rc::clone(a),
        None => {
            let last = path.rsplit('/').next().unwrap_or(path);
            let name = last.strip_prefix("vactr-").unwrap_or(last);
            if !is_identifier(name) {
                return Err("the import prefix is not an identifier; add `as ALIAS`");
            }
            Rc::from(name)
        }
    };
    Ok(ImportDecl {
        path: Rc::from(path),
        alias,
        open,
        prefix,
        span,
    })
}

/// Phase 1 of the two-phase frontend (5.7): finds the top-level `import`
/// forms by layout alone, without reading the document. Malformed imports are
/// skipped, because `read` reports them.
///
/// The spans are byte offsets into `src` and carry `FileId::CONSOLE`, since
/// the prescan has no file of its own; callers use the path and prefix.
#[must_use]
pub fn prescan_imports(src: &str) -> Vec<ImportDecl> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for raw in src.split('\n') {
        let line_start = offset;
        offset += raw.len() + 1;
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let code = line.split('#').next().unwrap_or("").trim_end();
        let is_import = code
            .strip_prefix("import")
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']));
        if !is_import {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(line_start),
            u32::try_from(line_start + code.len()),
        ) else {
            break;
        };
        if let Ok(decl) = parse_import(code, Span::new(FileId::CONSOLE, start, end)) {
            out.push(decl);
        }
    }
    out
}
