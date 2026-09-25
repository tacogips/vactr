//! `BindingPersistence`: one interface, two implementations (design 13,
//! 13.5, 14.5.8; pending-session-questions S4).
//!
//! - `DirectivePersistence` (the adjudicated default) keeps panel
//!   membership and MIDI mappings IN the source as `#@` comments. `save`
//!   returns directive text edits and NEVER writes overlay values.
//! - `ExternalFilePersistence` keeps them in `<doc>.bindings.json`
//!   (`{"v":1,"bindings":[...]}`, keys spelled `label.site.n.param`), with
//!   overlays. The caller reads and writes the file; nothing here does IO.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::directives::key::{BindingIdent, BindingKey};
use crate::directives::parse::DirectiveBody;
use crate::directives::writeback::{
    append_after, is_def_param, override_head, owner_target, render_pairs,
};
use crate::directives::DirectiveTable;
use crate::reader::span::Span;

/// A MIDI mapping.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Midi {
    pub cc: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<u8>,
}

/// One persisted binding.
#[derive(Clone, PartialEq, Debug)]
pub struct BindingEntry {
    pub key: BindingIdent,
    pub panel: bool,
    pub midi: Option<Midi>,
    pub overlay: Option<f64>,
}

/// The bindings of one document.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BindingSet {
    pub entries: Vec<BindingEntry>,
}

impl BindingSet {
    /// The entries sorted by identity, for comparison.
    #[must_use]
    pub fn sorted(&self) -> Vec<BindingEntry> {
        let mut v = self.entries.clone();
        v.sort_by(|a, b| a.key.cmp(&b.key));
        v
    }

    /// The same set with every overlay removed.
    #[must_use]
    pub fn without_overlays(&self) -> BindingSet {
        BindingSet {
            entries: self
                .entries
                .iter()
                .map(|e| BindingEntry {
                    overlay: None,
                    ..e.clone()
                })
                .collect(),
        }
    }

    /// The entry of `key`.
    #[must_use]
    pub fn get(&self, key: &BindingIdent) -> Option<&BindingEntry> {
        self.entries.iter().find(|e| &e.key == key)
    }
}

/// A validated text edit: the editor applies it only while the text at
/// `span` still equals `expected`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextEdit {
    pub span: (u32, u32),
    pub expected: String,
    pub text: String,
}

/// What `save` produced.
#[derive(Clone, PartialEq, Debug)]
pub enum Persisted {
    /// Directive mode: edits to the source.
    Edits(Vec<TextEdit>),
    /// ExternalFile mode: the new session-file text.
    File(String),
}

/// One document as persistence sees it.
#[derive(Clone, Copy, Debug)]
pub struct DocInput<'a> {
    pub text: &'a str,
    pub table: &'a DirectiveTable,
}

/// Loads and saves the binding set of one document.
pub trait BindingPersistence {
    fn load(&self, doc: &DocInput<'_>) -> BindingSet;
    fn save(&mut self, doc: &DocInput<'_>, set: &BindingSet) -> Persisted;
}

/// Why edits do not apply.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditError {
    /// The text at the span differs from `expected` (drift: decline).
    Mismatch { span: (u32, u32) },
    /// Spans overlap or fall outside the text.
    BadSpan { span: (u32, u32) },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Mismatch { span } => {
                write!(f, "text at {}..{} changed; edit declined", span.0, span.1)
            }
            EditError::BadSpan { span } => write!(f, "bad edit span {}..{}", span.0, span.1),
        }
    }
}

impl std::error::Error for EditError {}

/// Applies `edits` to `text`, verifying each `expected` first.
///
/// # Errors
/// `Mismatch` on drift, `BadSpan` for overlapping or out-of-range spans.
pub fn apply_edits(text: &str, edits: &[TextEdit]) -> Result<String, EditError> {
    let mut order: Vec<&TextEdit> = edits.iter().collect();
    order.sort_by_key(|e| e.span);
    let mut prev_end = 0u32;
    for e in &order {
        let (s, t) = e.span;
        if s < prev_end || s > t || t as usize > text.len() {
            return Err(EditError::BadSpan { span: e.span });
        }
        if text.get(s as usize..t as usize) != Some(e.expected.as_str()) {
            return Err(EditError::Mismatch { span: e.span });
        }
        prev_end = t;
    }
    let mut out = text.to_string();
    for e in order.iter().rev() {
        out.replace_range(e.span.0 as usize..e.span.1 as usize, &e.text);
    }
    Ok(out)
}

/// The directive-mode set of a table: every resolved binding on the panel,
/// its CC mapping, no overlay.
fn table_set(table: &DirectiveTable) -> BindingSet {
    BindingSet {
        entries: table
            .resolved
            .iter()
            .map(|r| BindingEntry {
                key: r.ident.clone(),
                panel: true,
                midi: r.cc.map(|cc| Midi { cc, ch: r.ch }),
                overlay: None,
            })
            .collect(),
    }
}

/// The directive-rendering unit of an identity: one call site, or one
/// definition parameter.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Group {
    Site(Rc<str>, Rc<str>, u16),
    Param(BindingKey),
    Positional(u32, u32),
}

fn group_of(ident: &BindingIdent) -> Group {
    match ident {
        BindingIdent::Key(k) => match &k.site {
            Some((site, n)) => Group::Site(Rc::clone(&k.label), Rc::clone(site), *n),
            None => Group::Param(k.clone()),
        },
        BindingIdent::Positional { span, .. } => Group::Positional(span.0, span.1),
    }
}

/// The comparable state of an entry in Directive mode.
fn state(e: &BindingEntry) -> Option<Option<Midi>> {
    (e.panel || e.midi.is_some()).then_some(e.midi)
}

/// Directive mode, the adjudicated default.
#[derive(Clone, Copy, Debug, Default)]
pub struct DirectivePersistence;

impl BindingPersistence for DirectivePersistence {
    fn load(&self, doc: &DocInput<'_>) -> BindingSet {
        table_set(doc.table)
    }

    fn save(&mut self, doc: &DocInput<'_>, set: &BindingSet) -> Persisted {
        let table = doc.table;
        let current = table_set(table);
        let mut groups: BTreeMap<Group, (Vec<&BindingEntry>, Vec<&BindingEntry>)> = BTreeMap::new();
        for e in &current.entries {
            groups.entry(group_of(&e.key)).or_default().0.push(e);
        }
        for e in set.entries.iter().filter(|e| state(e).is_some()) {
            groups.entry(group_of(&e.key)).or_default().1.push(e);
        }
        let mut edits = Vec::new();
        let mut tail: Vec<String> = Vec::new();
        for (group, (cur, want)) in &groups {
            let same = cur.len() == want.len()
                && want
                    .iter()
                    .all(|w| cur.iter().any(|c| c.key == w.key && state(c) == state(w)));
            if same {
                continue;
            }
            if let Some(mut more) = save_group(doc, group, cur, want, &mut edits) {
                tail.append(&mut more);
            }
        }
        if !tail.is_empty() {
            let end = u32::try_from(doc.text.len()).unwrap_or(u32::MAX);
            let mut text = String::new();
            if !doc.text.is_empty() && !doc.text.ends_with('\n') {
                text.push('\n');
            }
            for l in tail {
                text.push_str(&l);
                text.push('\n');
            }
            edits.push(TextEdit {
                span: (end, end),
                expected: String::new(),
                text,
            });
        }
        edits.sort_by_key(|e| e.span);
        Persisted::Edits(edits)
    }
}

/// The directives that name identities of `group`, and whether each names
/// only this group.
fn contributors(table: &DirectiveTable, group: &Group) -> Vec<(usize, bool)> {
    let mut ds: BTreeSet<usize> = BTreeSet::new();
    for h in &table.hits {
        if &group_of(&h.ident) == group {
            ds.insert(h.directive);
        }
    }
    ds.into_iter()
        .map(|d| {
            let exclusive = table
                .hits
                .iter()
                .filter(|h| h.directive == d)
                .all(|h| &group_of(&h.ident) == group);
            (d, exclusive)
        })
        .collect()
}

/// The declared parameters and the site/parameter span of a group.
fn group_target(
    table: &DirectiveTable,
    group: &Group,
    any: &BindingIdent,
) -> Option<(Vec<Rc<str>>, Span)> {
    match (group, any) {
        (Group::Site(..) | Group::Param(_), BindingIdent::Key(k)) => table.key_target(k),
        (Group::Positional(s, e), _) => {
            let file = table.doc.file?;
            let span = Span::new(file, *s, *e);
            if let Some(site) = table.doc.sites.iter().find(|x| x.head == span) {
                return Some((site.params.clone(), span));
            }
            is_def_param(table, span).then(|| (Vec::new(), span))
        }
        _ => None,
    }
}

/// The pair texts for the wanted entries: one per distinct channel.
fn pair_texts(
    want: &[&BindingEntry],
    params: &[Rc<str>],
    single: bool,
    file_ch: Option<u8>,
) -> Vec<String> {
    let mut by_ch: BTreeMap<Option<u8>, Vec<Option<u8>>> = BTreeMap::new();
    for w in want {
        let Some(m) = w.midi else {
            continue;
        };
        let param = match &w.key {
            BindingIdent::Key(k) => Rc::clone(&k.param),
            BindingIdent::Positional { param, .. } => Rc::clone(param),
        };
        let slot = if single {
            0
        } else {
            match params.iter().position(|p| *p == param) {
                Some(k) => k,
                None => continue,
            }
        };
        let list = by_ch
            .entry(m.ch.filter(|c| Some(*c) != file_ch))
            .or_default();
        if list.len() <= slot {
            list.resize(slot + 1, None);
        }
        list[slot] = Some(m.cc);
    }
    if by_ch.is_empty() {
        return vec![String::new()];
    }
    by_ch
        .into_iter()
        .map(|(ch, ccs)| render_pairs(&ccs, ch))
        .collect()
}

/// Saves one changed group: rewrites its exclusive directive in place,
/// deletes it when the group left the panel, or adds lines (returned for the
/// end of the file when addressed; inserted after the statement when
/// positional).
fn save_group(
    doc: &DocInput<'_>,
    group: &Group,
    cur: &[&BindingEntry],
    want: &[&BindingEntry],
    edits: &mut Vec<TextEdit>,
) -> Option<Vec<String>> {
    let table = doc.table;
    let any = want.first().or(cur.first()).map(|e| e.key.clone())?;
    let (params, span) = group_target(table, group, &any)?;
    let single = matches!(group, Group::Param(_)) || is_def_param(table, span);
    let ds = contributors(table, group);
    let exclusive: Vec<usize> = ds.iter().filter(|(_, x)| *x).map(|(d, _)| *d).collect();
    let shared = ds.iter().any(|(_, x)| !*x);
    if want.is_empty() {
        for d in exclusive {
            edits.push(remove_directive(doc, d));
        }
        return None;
    }
    let pairs = pair_texts(want, &params, single, table.file_level.midi_ch);
    let (first, rest) = pairs.split_first()?;
    let mut lines: Vec<String> = Vec::new();
    if let (Some(&d), false) = (exclusive.first(), shared) {
        edits.push(rewrite_pairs(doc, d, first));
        for &extra in exclusive.iter().skip(1) {
            edits.push(remove_directive(doc, extra));
        }
        lines.extend(rest.iter().cloned());
    } else {
        lines.extend(pairs.iter().cloned());
    }
    if lines.is_empty() {
        return None;
    }
    match &any {
        BindingIdent::Key(k) => {
            let head = override_head(table, k)?;
            Some(lines.iter().map(|p| format!("#@ {head}{p}")).collect())
        }
        BindingIdent::Positional { param, .. } => {
            let t = owner_target(table, span)?;
            let head = positional_name(table, t, span, param);
            let lines: Vec<String> = lines.iter().map(|p| format!("#@ {head}{p}")).collect();
            edits.push(append_after(doc.text, table, t, &lines));
            None
        }
    }
}

/// `lpf`, `lpf.2` or the parameter name, within target `t`.
fn positional_name(table: &DirectiveTable, t: usize, span: Span, param: &str) -> String {
    let Some(site) = table.doc.sites.iter().find(|s| s.head == span) else {
        return param.to_string();
    };
    let same: Vec<Span> = table
        .doc
        .sites_in(table.doc.targets[t].extent)
        .filter(|(_, s)| s.name == site.name)
        .map(|(_, s)| s.head)
        .collect();
    match same.iter().position(|s| *s == span) {
        Some(k) if same.len() > 1 => format!("{}.{}", site.name, k + 1),
        _ => site.name.to_string(),
    }
}

/// Replaces the pairs of directive `d` (everything after its head).
fn rewrite_pairs(doc: &DocInput<'_>, d: usize, pairs: &str) -> TextEdit {
    let dir = &doc.table.entries[d].directive;
    let text = doc
        .text
        .get(dir.span.start as usize..dir.span.end as usize)
        .unwrap_or("");
    let head = doc
        .text
        .get(dir.span.start as usize..dir.head_end as usize)
        .unwrap_or(text);
    TextEdit {
        span: (dir.span.start, dir.span.end),
        expected: text.to_string(),
        text: format!("{head}{pairs}"),
    }
}

/// Removes directive `d`: a label definition keeps its label; an own-line
/// directive goes with its line.
fn remove_directive(doc: &DocInput<'_>, d: usize) -> TextEdit {
    let placed = &doc.table.entries[d];
    let dir = &placed.directive;
    let text = doc.text;
    let own = text
        .get(dir.span.start as usize..dir.span.end as usize)
        .unwrap_or("");
    if let DirectiveBody::LabelDef { name, .. } = &dir.body {
        let spelled = if own
            .trim_start_matches("#@")
            .trim_start()
            .starts_with("name ")
        {
            format!("#@ name {name}")
        } else {
            format!("#@ {name}:")
        };
        return TextEdit {
            span: (dir.span.start, dir.span.end),
            expected: own.to_string(),
            text: spelled,
        };
    }
    let lines = &doc.table.doc.lines;
    let line = lines.line_of(dir.span.start);
    let line_start = lines.start(line).unwrap_or(dir.span.start);
    let (start, end) = if placed.trailing {
        let before = &text[..dir.span.start as usize];
        let trimmed = before.trim_end_matches([' ', '\t']).len();
        (
            u32::try_from(trimmed).unwrap_or(dir.span.start),
            dir.span.end,
        )
    } else {
        let end = lines
            .start(line + 1)
            .unwrap_or(u32::try_from(text.len()).unwrap_or(u32::MAX));
        (line_start, end)
    };
    TextEdit {
        span: (start, end),
        expected: text
            .get(start as usize..end as usize)
            .unwrap_or("")
            .to_string(),
        text: String::new(),
    }
}

/// The session-file form (`{"v":1,"bindings":[...]}`).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
struct FileForm {
    v: u32,
    bindings: Vec<FileEntry>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
struct FileEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    param: Option<String>,
    panel: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    midi: Option<Midi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<f64>,
}

/// ExternalFile mode: the session file keeps keys, panel membership,
/// mappings AND overlays; the source is never touched.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ExternalFilePersistence {
    /// The current `<doc>.bindings.json` text, as the caller read it.
    pub file: Option<String>,
}

impl ExternalFilePersistence {
    /// Wraps the text of an existing session file.
    #[must_use]
    pub fn with_file(text: impl Into<String>) -> ExternalFilePersistence {
        ExternalFilePersistence {
            file: Some(text.into()),
        }
    }

    /// Parses session-file text.
    ///
    /// # Errors
    /// Invalid JSON, an unsupported `v`, or a malformed key.
    pub fn parse(text: &str) -> Result<BindingSet, String> {
        let form: FileForm = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if form.v != 1 {
            return Err(format!("unsupported bindings file version {}", form.v));
        }
        let mut entries = Vec::new();
        for b in form.bindings {
            let key = match (b.key, b.span, b.param) {
                (Some(k), None, None) => {
                    BindingIdent::Key(k.parse::<BindingKey>().map_err(|e| e.to_string())?)
                }
                (None, Some([s, e]), Some(p)) => BindingIdent::Positional {
                    span: (s, e),
                    param: Rc::from(p.as_str()),
                },
                _ => return Err("a binding needs `key`, or `span` and `param`".into()),
            };
            entries.push(BindingEntry {
                key,
                panel: b.panel,
                midi: b.midi,
                overlay: b.overlay,
            });
        }
        Ok(BindingSet { entries })
    }

    /// Renders `set` as session-file text.
    #[must_use]
    pub fn render(set: &BindingSet) -> String {
        let form = FileForm {
            v: 1,
            bindings: set
                .sorted()
                .into_iter()
                .map(|e| {
                    let (key, span, param) = match e.key {
                        BindingIdent::Key(k) => (Some(k.to_string()), None, None),
                        BindingIdent::Positional { span, param } => {
                            (None, Some([span.0, span.1]), Some(param.to_string()))
                        }
                    };
                    FileEntry {
                        key,
                        span,
                        param,
                        panel: e.panel,
                        midi: e.midi,
                        overlay: e.overlay,
                    }
                })
                .collect(),
        };
        serde_json::to_string(&form).unwrap_or_else(|_| "{\"v\":1,\"bindings\":[]}".into())
    }
}

impl BindingPersistence for ExternalFilePersistence {
    fn load(&self, _doc: &DocInput<'_>) -> BindingSet {
        self.file
            .as_deref()
            .and_then(|t| Self::parse(t).ok())
            .unwrap_or_default()
    }

    fn save(&mut self, _doc: &DocInput<'_>, set: &BindingSet) -> Persisted {
        let text = Self::render(set);
        self.file = Some(text.clone());
        Persisted::File(text)
    }
}
