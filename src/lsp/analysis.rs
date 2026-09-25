//! The ONE analysis thread of `vactrol lsp` (design 14.5.11).
//!
//! Every `!Send` value (interned names, `Rc` nodes, types, the lock and
//! the package cache) lives in an `Analyzer` owned by one `std::thread`.
//! The tower-lsp handlers send it `AnalysisReq`s over an `std::sync::mpsc`
//! channel, each with a `tokio::sync::oneshot` reply, so nothing `!Send`
//! crosses an `.await`.
//!
//! Analysis never executes user code, never touches the network and never
//! writes files: it runs `session::eval::analyze` (reader, expander,
//! checker, packages) and `directives::build_table` (directive lint), and
//! reads the package cache only when it already exists.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use tokio::sync::oneshot;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic as LspDiag, Hover, HoverContents, MarkupContent,
    MarkupKind, Position, TextEdit, Url,
};

use crate::directives::build_table;
use crate::expand::{expand, ExpandCx};
use crate::lsp::convert::{offset, range, to_lsp, wire_to_lsp};
use crate::ns::pkg::PackageId;
use crate::pkg::cache::CacheBackend;
use crate::pkg::load::locked_sources;
use crate::pkg::lock::LockFile;
use crate::pkg::native::fs_cache::STAGING_DIR;
use crate::pkg::native::{pkg_cache_root, FsCache};
use crate::reader::node::{Node, NodeKind};
use crate::reader::span::FileId;
use crate::reader::{read, AliasEnv};
use crate::session::eval::{analyze, Analysis, PackageView};
use crate::session::protocol::{DiagBody, WireDiag};
use crate::types::natives::NativeTable;
use crate::types::ty::KeySet;

/// The lock file name in the workspace root.
pub const LOCK_FILE: &str = "vactrol.lock";

/// One document's diagnostics, ready for `textDocument/publishDiagnostics`.
#[derive(Clone, Debug, PartialEq)]
pub struct Published {
    pub uri: Url,
    pub version: Option<i32>,
    pub diagnostics: Vec<LspDiag>,
}

/// A request to the analysis thread. Every variant but `Configure` carries
/// its reply channel.
#[derive(Debug)]
pub enum AnalysisReq {
    /// Reload the package view from the workspace root.
    Configure { root: Option<PathBuf> },
    Open {
        uri: Url,
        version: i32,
        text: String,
        reply: oneshot::Sender<Published>,
    },
    /// A full-text change; an unknown document is opened.
    Change {
        uri: Url,
        version: i32,
        text: String,
        reply: oneshot::Sender<Published>,
    },
    /// Forgets the document; the reply clears its diagnostics.
    Close {
        uri: Url,
        reply: oneshot::Sender<Published>,
    },
    Hover {
        uri: Url,
        pos: Position,
        reply: oneshot::Sender<Option<Hover>>,
    },
    Complete {
        uri: Url,
        pos: Position,
        reply: oneshot::Sender<Vec<CompletionItem>>,
    },
    Format {
        uri: Url,
        reply: oneshot::Sender<Option<Vec<TextEdit>>>,
    },
    /// A runtime `diag` body from an attached session socket; the reply
    /// holds every document whose diagnostics changed.
    RuntimeDiags {
        body: DiagBody,
        reply: oneshot::Sender<Vec<Published>>,
    },
}

/// The read-only package view: the workspace lock and the verified cache.
#[derive(Debug, Default)]
pub struct PkgConfig {
    pub lock: Option<LockFile>,
    pub cache: Option<FsCache>,
}

impl PkgConfig {
    /// `<root>/vactrol.lock` when it exists and parses, and the package
    /// cache at `cache_root` only when it and its staging directory
    /// already exist, so opening it creates nothing.
    #[must_use]
    pub fn load(root: Option<&Path>, cache_root: &Path) -> PkgConfig {
        let lock = root
            .and_then(|r| std::fs::read_to_string(r.join(LOCK_FILE)).ok())
            .and_then(|t| LockFile::parse(&t).ok());
        let cache = (cache_root.join(STAGING_DIR).is_dir())
            .then(|| FsCache::new(cache_root).ok())
            .flatten();
        PkgConfig { lock, cache }
    }
}

struct Doc {
    version: i32,
    text: String,
    file: FileId,
    analysis: Analysis,
    /// Reader, expander, checker, package and directive-lint diagnostics.
    diags: Vec<LspDiag>,
    /// Runtime diagnostics from an attached session, as received.
    runtime: Vec<WireDiag>,
}

/// All analysis state; lives on the analysis thread.
pub struct Analyzer {
    pkgs: PkgConfig,
    docs: HashMap<Url, Doc>,
    next_file: u32,
    /// Top-level names of fetched packages, by package path.
    pkg_names: HashMap<Rc<str>, Vec<String>>,
}

/// The name a top-level definition binds.
fn defined_name(form: &Node) -> Option<&str> {
    if !matches!(form.kind, NodeKind::Call) {
        return None;
    }
    match form.children.first()?.sym_name()? {
        "let" | "var" | "fn" | "inst" | "struct" | "enum" | "bus" | "look" => {
            form.children.get(1)?.sym_name()
        }
        _ => None,
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '?' | '!' | '*' | '+' | '/')
}

/// True when the runtime diagnostic's `file` names the document `uri`: the
/// same URI, the same path, or a relative path that ends the URI's path.
fn names_doc(uri: &Url, file: &str) -> bool {
    if uri.as_str() == file {
        return true;
    }
    let Ok(path) = uri.to_file_path() else {
        return false;
    };
    let f = Path::new(file);
    path == f || (f.is_relative() && !file.is_empty() && path.ends_with(f))
}

impl Analyzer {
    /// An analyzer over the package view `pkgs`, with no documents.
    #[must_use]
    pub fn new(pkgs: PkgConfig) -> Analyzer {
        Analyzer {
            pkgs,
            docs: HashMap::new(),
            next_file: 1,
            pkg_names: HashMap::new(),
        }
    }

    /// Replaces the package view.
    pub fn configure(&mut self, pkgs: PkgConfig) {
        self.pkgs = pkgs;
        self.pkg_names.clear();
    }

    fn analyze(&self, text: &str, file: FileId) -> (Analysis, Vec<LspDiag>) {
        let view = PackageView {
            lock: self.pkgs.lock.as_ref(),
            cache: self.pkgs.cache.as_ref().map(|c| c as &dyn CacheBackend),
        };
        let a = analyze(text, file, &view);
        let (_, lint) = build_table(text, file, &a.nodes, &a.trivia, &a.manifest);
        let diags = a
            .diags
            .iter()
            .chain(lint.iter())
            .map(|d| {
                let mut l = to_lsp(text, d);
                if d.span.file != file {
                    l.range = range(text, 0, 0);
                }
                l
            })
            .collect();
        (a, diags)
    }

    /// Opens or replaces a document and returns its diagnostics.
    pub fn open(&mut self, uri: Url, version: i32, text: String) -> Published {
        let (file, runtime) = match self.docs.remove(&uri) {
            Some(d) => (d.file, d.runtime),
            None => {
                let f = FileId::new(self.next_file);
                self.next_file = self.next_file.saturating_add(1);
                (f, Vec::new())
            }
        };
        let (analysis, diags) = self.analyze(&text, file);
        self.docs.insert(
            uri.clone(),
            Doc {
                version,
                text,
                file,
                analysis,
                diags,
                runtime,
            },
        );
        self.published(&uri)
    }

    /// Forgets a document; the result clears its diagnostics.
    pub fn close(&mut self, uri: Url) -> Published {
        self.docs.remove(&uri);
        Published {
            uri,
            version: None,
            diagnostics: Vec::new(),
        }
    }

    fn published(&self, uri: &Url) -> Published {
        let Some(d) = self.docs.get(uri) else {
            return Published {
                uri: uri.clone(),
                version: None,
                diagnostics: Vec::new(),
            };
        };
        let mut diagnostics = d.diags.clone();
        diagnostics.extend(d.runtime.iter().map(|w| wire_to_lsp(&d.text, w)));
        Published {
            uri: uri.clone(),
            version: Some(d.version),
            diagnostics,
        }
    }

    /// The type of the innermost checked node at `pos`.
    #[must_use]
    pub fn hover(&self, uri: &Url, pos: Position) -> Option<Hover> {
        let d = self.docs.get(uri)?;
        let at = u32::try_from(offset(&d.text, pos)).ok()?;
        let types = &d.analysis.types;
        let mut best: Option<(u32, String, u32, u32)> = None;
        for f in &d.analysis.forms {
            f.walk(&mut |n: &Node| {
                let s = n.span;
                if s.file != d.file || s.start > at || at > s.end {
                    return;
                }
                let Some(ty) = types.get(&n.id) else {
                    return;
                };
                if best.as_ref().is_none_or(|(len, ..)| s.len() <= *len) {
                    let text = match n.sym_name() {
                        Some(name) => format!("{name}: {ty}"),
                        None => ty.to_string(),
                    };
                    best = Some((s.len(), text, s.start, s.end));
                }
            });
        }
        let (_, text, start, end) = best?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```vactrol\n{text}\n```"),
            }),
            range: Some(range(&d.text, start as usize, end as usize)),
        })
    }

    /// The top-level names of the fetched package at `path` (cached).
    fn package_names(&mut self, path: &Rc<str>) -> Vec<String> {
        if let Some(names) = self.pkg_names.get(path) {
            return names.clone();
        }
        let (Some(lock), Some(cache)) = (self.pkgs.lock.as_ref(), self.pkgs.cache.as_ref()) else {
            return Vec::new();
        };
        let Ok(sources) = locked_sources(lock, cache, &PackageId(Rc::clone(path))) else {
            return Vec::new();
        };
        let mut names = BTreeSet::new();
        for (k, (file, bytes)) in sources.files.iter().enumerate() {
            let Some(text) = file
                .ends_with(".vact")
                .then(|| std::str::from_utf8(bytes).ok())
                .flatten()
            else {
                continue;
            };
            let id = FileId::new(u32::MAX - u32::try_from(k).unwrap_or(0));
            let r = read(text, id, &AliasEnv::new());
            let mut ecx = ExpandCx::new(r.next_node_id());
            for n in &r.nodes {
                if let Ok(f) = expand(n, &mut ecx) {
                    names.extend(defined_name(&f).map(str::to_string));
                }
            }
        }
        let names: Vec<String> = names.into_iter().collect();
        self.pkg_names.insert(Rc::clone(path), names.clone());
        names
    }

    /// Completion at `pos`: prelude natives, document top-level names,
    /// manifest keywords (`:name`), package prefixes and qualified names,
    /// filtered by the word before the cursor.
    pub fn complete(&mut self, uri: &Url, pos: Position) -> Vec<CompletionItem> {
        let Some(d) = self.docs.get(uri) else {
            return Vec::new();
        };
        let at = offset(&d.text, pos);
        let word_start = d.text[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_word(*c))
            .last()
            .map_or(at, |(k, _)| k);
        let word = d.text[word_start..at].to_string();
        let mut items: BTreeMap<String, (CompletionItemKind, String)> = BTreeMap::new();
        let mut add = |label: String, kind, detail: String| {
            items.entry(label).or_insert((kind, detail));
        };
        for f in &d.analysis.forms {
            if let Some(name) = defined_name(f) {
                let kind = match f.children.first().and_then(Node::sym_name) {
                    Some("fn") => CompletionItemKind::FUNCTION,
                    Some("struct" | "enum") => CompletionItemKind::STRUCT,
                    _ => CompletionItemKind::VARIABLE,
                };
                let detail = f
                    .children
                    .get(2)
                    .and_then(|v| d.analysis.types.get(&v.id))
                    .map_or_else(|| "document".to_string(), ToString::to_string);
                add(name.to_string(), kind, detail);
            }
        }
        for (_, sig) in NativeTable::global().iter() {
            let detail = sig.ty.first().map_or("prelude", |t| t).to_string();
            add(sig.name.to_string(), CompletionItemKind::FUNCTION, detail);
        }
        let m = &d.analysis.manifest;
        for (set, what) in [
            (&m.sounds, "sound"),
            (&m.synths, "synth"),
            (&m.controls, "control"),
        ] {
            if let KeySet::Of(keys) = set {
                for k in keys {
                    add(
                        format!(":{k}"),
                        CompletionItemKind::KEYWORD,
                        what.to_string(),
                    );
                }
            }
        }
        let prefixes: Vec<(Rc<str>, Rc<str>)> = d
            .analysis
            .alias_env
            .prefixes
            .iter()
            .map(|(p, path)| (Rc::clone(p), Rc::clone(path)))
            .collect();
        for (prefix, path) in &prefixes {
            add(
                prefix.to_string(),
                CompletionItemKind::MODULE,
                path.to_string(),
            );
        }
        for (prefix, path) in prefixes {
            for name in self.package_names(&path) {
                items
                    .entry(format!("{prefix}.{name}"))
                    .or_insert((CompletionItemKind::FIELD, path.to_string()));
            }
        }
        items
            .into_iter()
            .filter(|(label, _)| label.starts_with(&word))
            .map(|(label, (kind, detail))| CompletionItem {
                label,
                kind: Some(kind),
                detail: Some(detail),
                ..CompletionItem::default()
            })
            .collect()
    }

    /// The whitespace-only formatting edits of the document.
    #[must_use]
    pub fn format(&self, uri: &Url) -> Option<Vec<TextEdit>> {
        self.docs.get(uri).map(|d| format_edits(&d.text))
    }

    /// Merges a runtime `diag` body: every `clear` slot drops that slot's
    /// runtime diagnostics everywhere, and every added diagnostic joins the
    /// document its `file` names (an unmatched one is dropped). Returns the
    /// documents whose diagnostics changed.
    pub fn runtime_diags(&mut self, body: &DiagBody) -> Vec<Published> {
        let mut touched: BTreeSet<String> = BTreeSet::new();
        for c in &body.clear {
            for (uri, d) in &mut self.docs {
                let before = d.runtime.len();
                d.runtime
                    .retain(|w| w.slot.as_deref() != Some(c.slot.as_str()));
                if d.runtime.len() != before {
                    touched.insert(uri.to_string());
                }
            }
        }
        for w in &body.add {
            if let Some((uri, d)) = self.docs.iter_mut().find(|(u, _)| names_doc(u, &w.file)) {
                if !d.runtime.contains(w) {
                    d.runtime.push(w.clone());
                    touched.insert(uri.to_string());
                }
            }
        }
        touched
            .iter()
            .filter_map(|u| Url::parse(u).ok())
            .map(|u| self.published(&u))
            .collect()
    }

    /// Serves one request.
    pub fn handle(&mut self, req: AnalysisReq) {
        // A dropped reply receiver (a cancelled request) is not an error.
        match req {
            AnalysisReq::Configure { root } => {
                self.configure(PkgConfig::load(root.as_deref(), &pkg_cache_root()));
            }
            AnalysisReq::Open {
                uri,
                version,
                text,
                reply,
            }
            | AnalysisReq::Change {
                uri,
                version,
                text,
                reply,
            } => {
                let _ = reply.send(self.open(uri, version, text));
            }
            AnalysisReq::Close { uri, reply } => {
                let _ = reply.send(self.close(uri));
            }
            AnalysisReq::Hover { uri, pos, reply } => {
                let _ = reply.send(self.hover(&uri, pos));
            }
            AnalysisReq::Complete { uri, pos, reply } => {
                let _ = reply.send(self.complete(&uri, pos));
            }
            AnalysisReq::Format { uri, reply } => {
                let _ = reply.send(self.format(&uri));
            }
            AnalysisReq::RuntimeDiags { body, reply } => {
                let _ = reply.send(self.runtime_diags(&body));
            }
        }
    }
}

/// Starts the analysis thread; it ends when every sender is dropped.
///
/// # Errors
/// The OS refused to start the thread.
pub fn spawn() -> std::io::Result<(mpsc::Sender<AnalysisReq>, thread::JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel::<AnalysisReq>();
    let handle = thread::Builder::new()
        .name("vactrol-lsp-analysis".into())
        .spawn(move || {
            let mut an = Analyzer::new(PkgConfig::default());
            while let Ok(req) = rx.recv() {
                an.handle(req);
            }
        })?;
    Ok((tx, handle))
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Whitespace-only formatting (14.5.11): strips trailing spaces and tabs
/// and ends the text with exactly one newline. A line containing `#@`
/// stays byte-identical, and nothing else changes.
#[must_use]
pub fn format_edits(text: &str) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut strips: Vec<(usize, usize)> = Vec::new();
    // End of the last line with content, and that line's terminator.
    let mut last: Option<(usize, &str)> = None;
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let (body, term) = match line.strip_suffix("\r\n") {
            Some(b) => (b, "\r\n"),
            None => match line.strip_suffix('\n') {
                Some(b) => (b, "\n"),
                None => (line, ""),
            },
        };
        let keep = if body.contains("#@") {
            body.len()
        } else {
            body.trim_end_matches(is_blank).len()
        };
        if keep < body.len() {
            strips.push((start + keep, start + body.len()));
        }
        if keep > 0 {
            last = Some((start + keep, if term.is_empty() { "\n" } else { term }));
        }
        start += line.len();
    }
    let (tail_start, want) = last.unwrap_or((0, ""));
    for (a, b) in strips {
        // The tail edit below covers everything from `tail_start` on.
        if b <= tail_start {
            edits.push(TextEdit::new(range(text, a, b), String::new()));
        }
    }
    if text[tail_start..] != *want {
        edits.push(TextEdit::new(
            range(text, tail_start, text.len()),
            want.to_string(),
        ));
    }
    edits
}
