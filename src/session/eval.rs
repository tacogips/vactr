//! The document pipeline (design 14.5.4 steps 1-6) and LSP-only analysis
//! (14.5.7 "LSP analysis").
//!
//! `Session::eval` runs, in order: the phase-1 import prescan; package
//! loading through the lock and the verified cache; the phase-2 read of the WHOLE document with every
//! prescanned prefix bound, plus the ORDER check that turns a qualified atom
//! before its import into `unbound-qualifier`; expansion; the directive
//! table; then each top-level form in the requested span: evaluated against
//! the session manifest, then `Runtime::drain`. A reader or expander error
//! in one form never stops the others.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::directives::attach::extent;
use crate::directives::{build_table, DirectiveTable};
use crate::expand::{expand, ExpandCx};
use crate::ns::namespace::FormGen;
use crate::ns::pkg::PackageId;
use crate::ns::tweak::{TweakId, TweakSite};
use crate::pkg::cache::CacheBackend;
use crate::pkg::load::{asset_banks, default_prefix, locked_sources};
use crate::pkg::lock::LockFile;
use crate::pkg::store::{PkgError, PkgSources};
use crate::reader::node::{Atom, Node, NodeKind, Trivia};
use crate::reader::span::{FileId, NodeId, Span};
use crate::reader::{prescan_imports, read, AliasEnv, ImportDecl};
use crate::session::protocol::{
    EvalResultBody, ServerMsg, WireBinding, WireDirective, WireDirectives, WireFileLevel, WireForm,
    WireLabel, WireSpan,
};
use crate::session::publish::{bindings_from, diag_wire, failure_wire, site_wire, wire_span};
use crate::session::session::Session;
use crate::types::check::check;
use crate::types::diag::{DiagCode, Diagnostic, Severity};
use crate::types::manifest::HostManifest;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Ty};
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// One evaluated top-level form.
#[derive(Clone, Debug)]
pub struct FormResult {
    pub span: Span,
    pub value: Option<Value>,
    pub failure: Option<Failure>,
    pub form_gen: FormGen,
}

/// What one `eval` did (14.5.4 "EvalOutcome").
#[derive(Clone, Debug)]
pub struct EvalOutcome {
    pub file: FileId,
    pub doc_revision: u64,
    pub forms: Vec<FormResult>,
    /// Reader, expander, checker, load, package and directive diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Dry-run and bind failures from the drains (the old binding plays).
    pub faults: Vec<Failure>,
    /// Console output of the evaluated forms.
    pub console: Vec<String>,
    /// The `eval-result` body.
    pub wire: EvalResultBody,
}

impl EvalOutcome {
    /// True when a form failed or an error-severity diagnostic was
    /// reported.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.forms.iter().any(|f| f.failure.is_some())
            || !self.faults.is_empty()
            || self
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error)
    }
}

/// The phase-2 alias environment: the session's bound prefixes plus every
/// prescanned import of the document.
#[must_use]
pub fn alias_env_for(session: &AliasEnv, imports: &[ImportDecl]) -> AliasEnv {
    let mut env = session.clone();
    for d in imports {
        env.bind(Rc::clone(&d.prefix), Rc::clone(&d.path));
    }
    env
}

/// The imports of `src` with their spans in `file`.
fn imports_of(src: &str, file: FileId) -> Vec<ImportDecl> {
    prescan_imports(src)
        .into_iter()
        .map(|mut d| {
            d.span.file = file;
            d
        })
        .collect()
}

/// `unbound-qualifier` for every qualified atom whose prefix only an import
/// AFTER it binds (`elsewhere` holds the prefixes bound before this read).
fn order_check(
    nodes: &[Node],
    imports: &[ImportDecl],
    elsewhere: &dyn Fn(&str) -> bool,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for n in nodes {
        n.walk(&mut |c: &Node| {
            let NodeKind::Atom(Atom::Qualified { prefix, name }) = &c.kind else {
                return;
            };
            let later = imports.iter().any(|d| d.prefix == *prefix);
            let earlier = imports
                .iter()
                .any(|d| d.prefix == *prefix && d.span.start <= c.span.start);
            if later && !earlier && !elsewhere(prefix) {
                out.push(Diagnostic::error(
                    DiagCode::UnboundQualifier,
                    c.span,
                    format!("`{prefix}.{name}` is used before `import ... {prefix}`"),
                ));
            }
        });
    }
    out
}

/// A package error as a load diagnostic at `span`.
pub(super) fn pkg_diag(e: &PkgError, span: Span) -> Diagnostic {
    let code = e.code();
    let mut message = e.to_string();
    if matches!(e, PkgError::NotLocked(_)) {
        message.push_str("; run `vactrol get`");
    }
    Diagnostic {
        span,
        severity: code.default_severity(),
        code,
        message,
        origin: None,
    }
}

/// A package's cached sources through the lock (never a fetch).
pub(super) fn package_sources(
    lock: Option<&LockFile>,
    cache: Option<&dyn CacheBackend>,
    id: &PackageId,
) -> Result<PkgSources, PkgError> {
    let lock = lock.ok_or_else(|| PkgError::NotLocked(id.clone()))?;
    let cache = cache.ok_or_else(|| {
        PkgError::NotFetched(format!("`{}` (no package cache is configured)", id.0))
    })?;
    locked_sources(lock, cache, id)
}

/// The directive table as `eval-result` carries it.
fn directives_wire(table: &DirectiveTable) -> WireDirectives {
    let s = table.summary();
    let span = |a: [u32; 2]| WireSpan::new(a[0], a[1]);
    WireDirectives {
        file_level: WireFileLevel { midi_ch: s.midi_ch },
        entries: table
            .entries
            .iter()
            .map(|p| WireDirective {
                span: wire_span(p.directive.span),
                kind: match p.directive.kind {
                    crate::directives::parse::DirectiveKind::Positional => "positional",
                    crate::directives::parse::DirectiveKind::Addressed => "addressed",
                }
                .to_string(),
                target: p
                    .attach
                    .and_then(|t| table.doc.targets.get(t))
                    .map(|t| wire_span(t.extent)),
                trailing: p.trailing,
            })
            .collect(),
        labels: s
            .labels
            .into_iter()
            .map(|l| WireLabel {
                name: l.name,
                spans: l.spans.into_iter().map(span).collect(),
                ambiguous: l.ambiguous,
            })
            .collect(),
        bindings: s
            .bindings
            .into_iter()
            .map(|b| WireBinding {
                key: b.key,
                span: span(b.span),
                param: b.param,
                cc: b.cc,
                ch: b.ch,
                directive: span(b.directive),
            })
            .collect(),
    }
}

/// The `BindingKey` of each labeled tweak site: the literal at the
/// parameter's position among the literals of the keyed call site.
pub(super) fn site_keys(table: &DirectiveTable, sites: &[TweakSite]) -> BTreeMap<TweakId, String> {
    let mut out = BTreeMap::new();
    let heads = &table.doc.sites;
    for r in &table.resolved {
        let Some(key) = &r.key else { continue };
        if key.site.is_none() {
            continue;
        }
        let Some(pos) = heads.iter().position(|c| c.head == r.target_span) else {
            continue;
        };
        let end = heads
            .iter()
            .skip(pos + 1)
            .map(|c| c.head.start)
            .find(|s| *s > r.target_span.end)
            .unwrap_or(u32::MAX);
        let mut lits: Vec<&TweakSite> = sites
            .iter()
            .filter(|s| {
                s.span.file == r.target_span.file
                    && s.span.start >= r.target_span.end
                    && s.span.start < end
            })
            .collect();
        lits.sort_by_key(|s| s.span.start);
        if let Some(s) = lits.get(usize::from(r.param_index)) {
            out.insert(s.id, key.to_string());
        }
    }
    out
}

fn in_span(form: &Node, span: Option<(u32, u32)>) -> bool {
    let Some((s, e)) = span else { return true };
    let x = extent(form);
    let (fs, fe) = (x.start, x.end);
    if s == e {
        fs <= s && s <= fe
    } else {
        fs < e && s < fe
    }
}

impl Session {
    /// Evaluates the document `file` (the FULL text at `doc_revision`; the
    /// forms inside `span`, or all of them). Returns the outcome and the
    /// `bindings` batches the forms triggered, which follow the
    /// `eval-result` in order.
    pub fn eval(
        &mut self,
        src: &str,
        file: &str,
        doc_revision: u64,
        edit_epoch: u64,
        span: Option<(u32, u32)>,
    ) -> (EvalOutcome, Vec<ServerMsg>) {
        let fid = self.file_id(file);
        self.eval_doc(src, fid, doc_revision, edit_epoch, span)
    }

    /// The session's bound prefixes as an alias environment.
    #[must_use]
    pub fn session_aliases(&self) -> AliasEnv {
        let mut env = AliasEnv::new();
        for (prefix, (path, _)) in &self.prefixes {
            env.bind(Rc::clone(prefix), Rc::clone(path));
        }
        env
    }

    pub(super) fn eval_doc(
        &mut self,
        src: &str,
        fid: FileId,
        rev: u64,
        epoch: u64,
        span: Option<(u32, u32)>,
    ) -> (EvalOutcome, Vec<ServerMsg>) {
        let mut diags = Vec::new();
        let mut faults = Vec::new();
        let mut console = Vec::new();
        // 1-2. Phase 1 and the packages, before any form runs.
        let imports = imports_of(src, fid);
        let elsewhere: BTreeSet<Rc<str>> = self
            .prefixes
            .iter()
            .filter(|(_, (_, files))| files.iter().any(|f| *f != fid))
            .map(|(p, _)| Rc::clone(p))
            .collect();
        for decl in &imports {
            self.import_package(decl, fid, &mut diags, &mut faults);
        }
        // 3. Phase 2: the whole document, then the ORDER check.
        let env = alias_env_for(&self.session_aliases(), &imports);
        let r = read(src, fid, &env);
        diags.extend(r.diags.iter().cloned());
        diags.extend(order_check(&r.nodes, &imports, &|p| elsewhere.contains(p)));
        // 4. Expand, form by form.
        let mut ecx = ExpandCx::new(r.next_node_id());
        let mut forms = Vec::new();
        for n in &r.nodes {
            match expand(n, &mut ecx) {
                Ok(f) => forms.push(f),
                Err(d) => diags.push(d),
            }
        }
        // 5. The directive table (lint only; evaluation never reads it).
        let (table, lint) = build_table(src, fid, &r.nodes, &r.trivia, &self.manifest);
        diags.extend(lint);
        self.prepare_doc(fid, src, rev, epoch, table);
        // 6. The forms in span: eval, drain, publish.
        let mut results = Vec::new();
        let mut batches = Vec::new();
        for form in &forms {
            if !in_span(form, span) {
                continue;
            }
            if matches!(form.kind, NodeKind::Import(_)) {
                // Bound in step 2; the checker still sees it (`import-collision`).
                let env = self.ev.ns().check_env();
                diags.extend(check(std::slice::from_ref(form), &env, &self.manifest).diags);
                continue;
            }
            let out = self.ev.eval_form_in(form, &self.manifest);
            self.note_revision(out.form_gen, fid, rev);
            let report = self.ev.last_pass().cloned();
            let drained = self.rt.drain(&mut self.ev);
            diags.extend(out.diags);
            diags.extend(drained.diags);
            faults.extend(drained.faults);
            console.extend(drained.console.iter().map(ToString::to_string));
            if let Some(report) = report {
                if let Some(b) = self.publish_pass(&report) {
                    batches.push(b);
                }
            }
            let (value, failure) = match out.value {
                Ok(v) => (Some(v), None),
                Err(e) => (None, Some(e)),
            };
            results.push(FormResult {
                span: extent(form),
                value,
                failure,
                form_gen: out.form_gen,
            });
        }
        // A pass may have rebuilt forms of other documents too.
        self.refresh_all_auth();
        let wire = self.eval_wire(fid, rev, &results, &diags, &faults);
        let outcome = EvalOutcome {
            file: fid,
            doc_revision: rev,
            forms: results,
            diagnostics: diags,
            faults,
            console,
            wire,
        };
        (outcome, batches)
    }

    /// Publishes one completed pass: the ONE `bindings` batch, if any.
    pub(super) fn publish_pass(
        &mut self,
        report: &crate::ns::evaluator::PassReport,
    ) -> Option<ServerMsg> {
        let msg = bindings_from(report, &self.ev, self.passes + 1, &self.files)?;
        self.passes += 1;
        Some(msg)
    }

    /// Records the revision a form generation was evaluated at.
    pub(super) fn note_revision(&mut self, gen: FormGen, file: FileId, rev: u64) {
        self.gen_revs.insert(gen.get(), (file, rev));
        if let Some(f) = self.ev.graph().form_of_gen(gen) {
            self.form_revs.insert(f, (file, rev));
        }
    }

    fn eval_wire(
        &self,
        fid: FileId,
        rev: u64,
        results: &[FormResult],
        diags: &[Diagnostic],
        faults: &[Failure],
    ) -> EvalResultBody {
        let files = &self.files;
        let mut diagnostics: Vec<_> = diags.iter().map(|d| diag_wire(files, d)).collect();
        diagnostics.extend(
            faults
                .iter()
                .map(|f| failure_wire(files, f, Span::new(fid, 0, 0))),
        );
        let sites: Vec<TweakSite> = {
            let table = self.ev.ns().tweaks().borrow();
            let mut v: Vec<TweakSite> = table
                .iter()
                .filter(|s| s.span.file == fid)
                .cloned()
                .collect();
            v.sort_by_key(|s| (s.span.start, s.id));
            v
        };
        let doc = self.docs.get(&fid);
        let keys = doc
            .map(|d| site_keys(&d.directives, &sites))
            .unwrap_or_default();
        EvalResultBody {
            file: self.file_name(fid),
            doc_revision: rev,
            forms: results
                .iter()
                .map(|r| WireForm {
                    span: wire_span(r.span),
                    value: r.value.as_ref().map(ToString::to_string),
                    failure: r.failure.as_ref().map(|f| failure_wire(files, f, r.span)),
                    form_gen: r.form_gen.get(),
                })
                .collect(),
            diagnostics,
            sites: sites
                .iter()
                .map(|s| site_wire(&self.ev, s, keys.get(&s.id).cloned()))
                .collect(),
            directives: doc
                .map(|d| directives_wire(&d.directives))
                .unwrap_or_default(),
        }
    }
}

/// Where `analyze` reads packages from: the lock and the verified cache,
/// never the network.
pub struct PackageView<'a> {
    pub lock: Option<&'a LockFile>,
    pub cache: Option<&'a dyn CacheBackend>,
}

/// The result of `analyze`: diagnostics and types, no execution.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub diags: Vec<Diagnostic>,
    /// The type of every checked node (`TypedInfo`, 14.3).
    pub types: HashMap<NodeId, Ty>,
    pub alias_env: AliasEnv,
    /// The read (unexpanded) nodes and their trivia (for the directive
    /// lint).
    pub nodes: Vec<Node>,
    pub trivia: Trivia,
    /// The expanded forms that were checked.
    pub forms: Vec<Node>,
    /// The manifest the forms were checked against.
    pub manifest: HostManifest,
}

/// The name a top-level definition binds, with its kind.
fn defined_name(form: &Node) -> Option<(Rc<str>, BindKind)> {
    if !matches!(form.kind, NodeKind::Call) {
        return None;
    }
    let kind = match form.children.first()?.sym_name()? {
        "let" | "bus" | "look" => BindKind::Let,
        "var" => BindKind::Var,
        "fn" => BindKind::Fn,
        "inst" => BindKind::Inst,
        "struct" => BindKind::Struct,
        "enum" => BindKind::Enum,
        _ => return None,
    };
    let name = form.children.get(1)?.sym_name()?;
    Some((Rc::from(name), kind))
}

/// A check-only pass over a package's sources: its top-level names.
fn package_names(sources: &PkgSources, manifest: &HostManifest) -> BTreeMap<Rc<str>, GlobalInfo> {
    let mut names = BTreeMap::new();
    for (k, (path, bytes)) in sources.files.iter().enumerate() {
        let Some(text) = path
            .ends_with(".vact")
            .then(|| std::str::from_utf8(bytes).ok())
            .flatten()
        else {
            continue;
        };
        let file = FileId::new(u32::MAX - u32::try_from(k).unwrap_or(0));
        let r = read(text, file, &AliasEnv::new());
        let mut ecx = ExpandCx::new(r.next_node_id());
        let forms: Vec<Node> = r
            .nodes
            .iter()
            .filter_map(|n| expand(n, &mut ecx).ok())
            .collect();
        let _ = check(&forms, &CheckEnv::empty(), manifest);
        for f in &forms {
            if let Some((name, kind)) = defined_name(f) {
                names.insert(
                    name,
                    GlobalInfo {
                        kind,
                        scheme: None,
                        mask: None,
                        span: Some(f.span),
                    },
                );
            }
        }
    }
    names
}

/// LSP-only analysis (14.5.7): the same phase-1/phase-2 frontend and alias
/// environment as `Session::eval`, then expand and check against the spec
/// manifest plus the fetched packages' banks, with NO evaluation. A fetched
/// package's names come from a check-only pass over its cached sources; an
/// unfetched package types its qualified names as `any`, with the warning
/// `package-not-fetched`. The directive lint is `directives::build_table`'s
/// (over `nodes` and `trivia`), which the caller runs.
#[must_use]
pub fn analyze(src: &str, file: FileId, view: &PackageView<'_>) -> Analysis {
    let mut diags = Vec::new();
    let imports = imports_of(src, file);
    let mut env = CheckEnv::empty();
    let mut broken: BTreeSet<Rc<str>> = BTreeSet::new();
    let mut fetched = Vec::new();
    let mut manifest = HostManifest::spec_default();
    for d in &imports {
        let id = PackageId(Rc::clone(&d.path));
        if let Ok(sources) = package_sources(view.lock, view.cache, &id) {
            for (bank, _) in asset_banks(&sources, &default_prefix(&id)) {
                manifest = manifest.with_sounds([&*bank]);
            }
            fetched.push((d, sources));
        }
    }
    for d in &imports {
        let id = PackageId(Rc::clone(&d.path));
        let found = fetched
            .iter()
            .find(|(f, _)| std::ptr::eq(*f, d))
            .map(|(_, s)| s);
        match found.ok_or_else(|| package_sources(view.lock, view.cache, &id).err()) {
            Ok(sources) => {
                let names = package_names(sources, &manifest);
                if d.open {
                    env.opens
                        .push((Rc::clone(&d.prefix), names.keys().cloned().collect()));
                }
                env.qualified.insert(Rc::clone(&d.prefix), names);
                broken.remove(&d.prefix);
            }
            Err(e) => {
                let e = match e {
                    Some(PkgError::NotLocked(_) | PkgError::NotFetched(_)) | None => {
                        PkgError::NotFetched(format!("`{}`", id.0))
                    }
                    Some(other) => other,
                };
                diags.push(pkg_diag(&e, d.span));
                env.qualified.remove(&d.prefix);
                broken.insert(Rc::clone(&d.prefix));
            }
        }
    }
    let alias_env = alias_env_for(&AliasEnv::new(), &imports);
    let r = read(src, file, &alias_env);
    diags.extend(r.diags.iter().cloned());
    diags.extend(order_check(&r.nodes, &imports, &|_| false));
    let mut ecx = ExpandCx::new(r.next_node_id());
    let mut forms = Vec::new();
    for n in &r.nodes {
        match expand(n, &mut ecx) {
            Ok(f) => forms.push(f),
            Err(d) => diags.push(d),
        }
    }
    let checked = check(&forms, &env, &manifest);
    let mut any_spans = BTreeSet::new();
    for f in &forms {
        f.walk(&mut |c: &Node| {
            if let NodeKind::Atom(Atom::Qualified { prefix, .. }) = &c.kind {
                if broken.contains(prefix) {
                    any_spans.insert((c.span.start, c.span.end));
                }
            }
        });
    }
    diags.extend(checked.diags.into_iter().filter(|d| {
        !(d.code == DiagCode::UndefinedName && any_spans.contains(&(d.span.start, d.span.end)))
    }));
    Analysis {
        diags,
        types: checked.types,
        alias_env,
        nodes: r.nodes,
        trivia: r.trivia,
        forms,
        manifest,
    }
}
