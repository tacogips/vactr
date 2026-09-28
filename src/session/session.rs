//! The `Session` (design 14.1, 14.5.4): one `Evaluator` and one
//! `sched::Runtime` plus document state, package state, directive tables,
//! console registers, write authority and the outbox. It adds no second
//! evaluator loop and no second scheduler, and it never does file, network,
//! thread or process I/O: the caller hands it the lock, the cache and the
//! loaders, so it builds for wasm32.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::clock::clock::ClockSource;
use crate::directives::key::KeyTable;
use crate::directives::persist::BindingSet;
use crate::directives::DirectiveTable;
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{AnalysisCx, Hosts, InstResolver};
use crate::host::noop::NoopHost;
use crate::ns::depgraph::FormId;
use crate::ns::evaluator::Evaluator;
use crate::ns::insts::InstRegistry;
use crate::ns::load::SourceLoader;
use crate::ns::namespace::{FormGen, Prelude};
use crate::ns::pkg::{ImportBinding, PackageId, PkgNs};
use crate::ns::stage::{EffectSink, SlotKey, StagedEffect};
use crate::ns::tweak::TweakId;
use crate::pkg::cache::CacheBackend;
use crate::pkg::load::{asset_banks, default_prefix};
use crate::pkg::lock::LockFile;
use crate::reader::span::{FileId, Span};
use crate::reader::{prescan_imports, ImportDecl};
use crate::sched::runtime::{Runtime, RuntimeConfig, RuntimeSink};
use crate::session::authority::PendingWrites;
use crate::session::changes::ChangeSet;
use crate::session::eval::{package_sources, pkg_diag};
use crate::session::protocol::{
    ClientMsg, Empty, Envelope, ErrorCode, ManifestBody, ProtocolError, Route, ServerMsg,
    Subscription, Topic,
};
use crate::session::{codec, publish};
use crate::types::diag::{DiagCode, Diagnostic};
use crate::types::manifest::HostManifest;
use crate::types::ty::KeySet;
use crate::value::intern::{intern_kw, intern_sym, KwId, SymId};
use crate::value::key::Key;
use crate::value::ratio::Ratio64;
use crate::value::value::{PathVal, Sound, Value};
use crate::vm::fail::{FailCode, Failure};

/// Where learned bindings are persisted (13.5, 14.5.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PersistenceMode {
    /// Directive comments in the source (the default).
    #[default]
    Directive,
    /// `<doc>.bindings.json`, held in memory by the session.
    ExternalFile,
}

/// What a session is built from.
pub struct SessionConfig {
    /// The capability tier.
    pub caps: CapabilitySet,
    /// Scheduler settings (the cell tier lives here).
    pub runtime: RuntimeConfig,
    /// The contents of `vactrol.lock`, when the project has one.
    pub lock: Option<LockFile>,
    /// The verified package cache.
    pub cache: Option<Box<dyn CacheBackend>>,
    /// The source loader of `load`; the session wraps it (14.5.9).
    pub loader: Box<dyn SourceLoader>,
    pub persistence: PersistenceMode,
    /// The instrument registry to share with a host that resolves bus
    /// names (`NativeAudioHost::set_bus_names`); a fresh one when `None`.
    pub insts: Option<Rc<RefCell<InstRegistry>>>,
}

impl SessionConfig {
    /// `caps` with default scheduling, no packages, no source loader and
    /// directive persistence.
    #[must_use]
    pub fn new(caps: CapabilitySet) -> SessionConfig {
        SessionConfig {
            caps,
            runtime: RuntimeConfig::default(),
            lock: None,
            cache: None,
            loader: Box::new(NoopHost),
            persistence: PersistenceMode::Directive,
            insts: None,
        }
    }
}

/// The session's source loader: the caller's loader plus the analysis
/// context the self-analysis natives reach (14.5.9).
struct SessionLoader {
    inner: Box<dyn SourceLoader>,
    cx: AnalysisCx,
}

impl SourceLoader for SessionLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        self.inner.read(path)
    }

    fn analysis(&mut self) -> Option<&mut AnalysisCx> {
        Some(&mut self.cx)
    }
}

/// Write authority of one tweak site (14.5.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SiteAuth {
    /// In the document's current authority revision (`DocState::rev`).
    pub span: (u32, u32),
    pub form_gen: FormGen,
    pub invalid: bool,
}

/// Write authority of one defined name (14.5.6 rule 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DefAuth {
    pub span: (u32, u32),
    pub form_gen: FormGen,
    pub invalid: bool,
}

/// The state of one open document.
#[derive(Debug)]
pub struct DocState {
    pub file: FileId,
    /// The revision the authority spans are in (advanced by `doc-changed`).
    pub rev: u64,
    /// The last evaluated text and its revision.
    pub text: String,
    pub text_rev: u64,
    /// The highest `edit_epoch` any `eval` or `doc-changed` carried.
    pub epoch_reconciled: u64,
    pub directives: DirectiveTable,
    pub keys: KeyTable,
    /// The ExternalFile-mode binding set.
    pub bindings: BindingSet,
    pub sites: BTreeMap<TweakId, SiteAuth>,
    pub defs: BTreeMap<SymId, DefAuth>,
    /// The edits since `text_rev`, composed; `None` once a base mismatch
    /// made them unmappable.
    pub since_eval: Option<ChangeSet>,
}

impl DocState {
    #[must_use]
    pub fn new(file: FileId) -> DocState {
        DocState {
            file,
            rev: 0,
            text: String::new(),
            text_rev: 0,
            epoch_reconciled: 0,
            directives: DirectiveTable::default(),
            keys: KeyTable::default(),
            bindings: BindingSet::default(),
            sites: BTreeMap::new(),
            defs: BTreeMap::new(),
            since_eval: Some(ChangeSet::default()),
        }
    }
}

/// Where an outgoing message goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dest {
    /// One connection (a reply).
    Conn(u32),
    /// Every connection subscribed to the topic.
    Topic(Topic),
}

/// One outgoing, routed message.
#[derive(Clone, PartialEq, Debug)]
pub struct Outgoing {
    pub to: Dest,
    pub env: Envelope<ServerMsg>,
}

/// The session.
pub struct Session {
    pub(super) ev: Evaluator,
    pub(super) rt: Runtime,
    /// A clone of the evaluator's sink (package effects, `hush`, `stop`).
    pub(super) sink: RuntimeSink,
    pub(super) caps: CapabilitySet,
    /// The spec default plus the registered package banks.
    pub(super) manifest: HostManifest,
    /// Registered banks and the package that owns each.
    pub(super) banks: BTreeMap<Rc<str>, Rc<str>>,
    pub(super) lock: Option<LockFile>,
    pub(super) cache: Option<Box<dyn CacheBackend>>,
    pub(super) persistence: PersistenceMode,
    /// File names by `FileId` (index 0 is the console).
    pub(super) files: Vec<Rc<str>>,
    pub(super) docs: BTreeMap<FileId, DocState>,
    /// Bound import prefixes: package path and the files that imported it.
    pub(super) prefixes: BTreeMap<Rc<str>, (Rc<str>, BTreeSet<FileId>)>,
    /// The file and revision each recorded form was last evaluated at.
    pub(super) form_revs: BTreeMap<FormId, (FileId, u64)>,
    /// The file and revision of every form generation the session ran.
    pub(super) gen_revs: BTreeMap<u64, (FileId, u64)>,
    pub(super) pending: PendingWrites,
    /// Completed reactive passes published so far.
    pub(super) passes: u64,
    pub(super) out_seq: u64,
    pub(super) subs: BTreeMap<u32, Subscription>,
    /// Console registers bound so far (`_1` is the first).
    pub(super) registers: u32,
    /// Broadcasts produced outside `tick` (console evals), sent at the next
    /// tick.
    pub(super) outbox: Vec<Outgoing>,
    /// Console lines not yet taken.
    pub(super) console: Vec<String>,
    pub(super) last_levels: Option<f64>,
    /// bpm, beats per cycle, clock source and its `locked` state (TASK-010
    /// G4: a clock change alone must also emit `tempo`).
    pub(super) last_tempo: Option<(Ratio64, Ratio64, ClockSource, Option<bool>)>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("files", &self.files)
            .field("passes", &self.passes)
            .finish_non_exhaustive()
    }
}

/// The console's file name.
pub const CONSOLE_FILE: &str = "<console>";

impl Session {
    /// Builds the Evaluator + Runtime pair (12.8.3): the runtime first, so
    /// its sink exists; then the evaluator over the same instrument
    /// registry with the session loader carrying the `AnalysisCx`; then one
    /// drain installs the prelude templates.
    #[must_use]
    pub fn new(cfg: SessionConfig, hosts: Hosts) -> Session {
        let reg = cfg.insts.unwrap_or_else(InstRegistry::shared);
        let resolver: Rc<dyn InstResolver> = Rc::new(Rc::clone(&reg));
        let (mut rt, sink) = Runtime::new(hosts, resolver, cfg.caps, cfg.runtime);
        let loader = SessionLoader {
            inner: cfg.loader,
            cx: AnalysisCx {
                caps: cfg.caps,
                taps: rt.tap_reader(),
            },
        };
        let mut ev = Evaluator::with_insts(
            Prelude::core(),
            Box::new(loader),
            Box::new(sink.clone()),
            Rc::clone(&reg),
        );
        let _ = rt.drain(&mut ev);
        let mut docs = BTreeMap::new();
        docs.insert(FileId::CONSOLE, DocState::new(FileId::CONSOLE));
        Session {
            ev,
            rt,
            sink,
            caps: cfg.caps,
            manifest: HostManifest::spec_default(),
            banks: BTreeMap::new(),
            lock: cfg.lock,
            cache: cfg.cache,
            persistence: cfg.persistence,
            files: vec![Rc::from(CONSOLE_FILE)],
            docs,
            prefixes: BTreeMap::new(),
            form_revs: BTreeMap::new(),
            gen_revs: BTreeMap::new(),
            pending: PendingWrites::default(),
            passes: 0,
            out_seq: 0,
            subs: BTreeMap::new(),
            registers: 0,
            outbox: Vec::new(),
            console: Vec::new(),
            last_levels: None,
            last_tempo: None,
        }
    }

    /// The evaluator.
    #[must_use]
    pub fn evaluator(&self) -> &Evaluator {
        &self.ev
    }

    /// The evaluator, mutably (tests and front ends).
    pub fn evaluator_mut(&mut self) -> &mut Evaluator {
        &mut self.ev
    }

    /// The runtime.
    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.rt
    }

    /// The capability tier.
    #[must_use]
    pub fn caps(&self) -> CapabilitySet {
        self.caps
    }

    /// The session manifest (the spec default plus package banks).
    #[must_use]
    pub fn manifest(&self) -> &HostManifest {
        &self.manifest
    }

    /// Host seconds on the audio timebase.
    #[must_use]
    pub fn now(&self) -> f64 {
        self.rt.hosts.audio.now()
    }

    /// The persistence mode.
    #[must_use]
    pub fn persistence(&self) -> PersistenceMode {
        self.persistence
    }

    /// The state of the document `file`, when it was ever evaluated or
    /// changed.
    #[must_use]
    pub fn doc(&self, file: &str) -> Option<&DocState> {
        let id = self.lookup_file(file)?;
        self.docs.get(&id)
    }

    /// The `FileId` of a known file name.
    #[must_use]
    pub fn lookup_file(&self, name: &str) -> Option<FileId> {
        let k = self.files.iter().position(|f| &**f == name)?;
        u32::try_from(k).ok().map(FileId::new)
    }

    /// The `FileId` of `name`, assigned on first use (1, 2, ...).
    pub fn file_id(&mut self, name: &str) -> FileId {
        if let Some(id) = self.lookup_file(name) {
            return id;
        }
        self.alloc_file(name)
    }

    pub(super) fn alloc_file(&mut self, name: &str) -> FileId {
        let id = FileId::new(u32::try_from(self.files.len()).unwrap_or(u32::MAX));
        self.files.push(Rc::from(name));
        id
    }

    /// The name of a file (`<file N>` for an unknown id).
    #[must_use]
    pub fn file_name(&self, id: FileId) -> String {
        publish::file_name(&self.files, id)
    }

    /// Console lines (`print`, `at` thunks) since the last call.
    pub fn take_console(&mut self) -> Vec<String> {
        std::mem::take(&mut self.console)
    }

    /// The subscription of connection `conn`.
    #[must_use]
    pub fn subscription(&self, conn: u32) -> Subscription {
        self.subs.get(&conn).copied().unwrap_or_default()
    }

    /// True when a message routed `route` reaches connection `conn` (as a
    /// broadcast; replies are addressed by `Dest::Conn`).
    #[must_use]
    pub fn wants(&self, conn: u32, topic: Topic) -> bool {
        self.subscription(conn).wants(topic)
    }

    /// Forgets a closed connection.
    pub fn disconnect(&mut self, conn: u32) {
        self.subs.remove(&conn);
        self.pending.forget(conn);
    }

    pub(super) fn envelope(&mut self, msg: ServerMsg, re: Option<u64>) -> Envelope<ServerMsg> {
        self.out_seq += 1;
        Envelope::new(self.out_seq, re, msg)
    }

    /// Routes messages produced for requester `conn` answering `re`.
    pub(super) fn route(
        &mut self,
        conn: u32,
        re: Option<u64>,
        msgs: Vec<ServerMsg>,
    ) -> Vec<Outgoing> {
        msgs.into_iter()
            .map(|m| match m.routing() {
                Route::Requester => Outgoing {
                    to: Dest::Conn(conn),
                    env: self.envelope(m, re),
                },
                Route::Broadcast(t) => Outgoing {
                    to: Dest::Topic(t),
                    env: self.envelope(m, None),
                },
            })
            .collect()
    }

    /// Decodes and applies one frame from connection `conn`. A malformed
    /// frame is answered with `protocol-error`; the session never fails.
    pub fn apply_text(&mut self, conn: u32, text: &str) -> Vec<Outgoing> {
        match codec::decode(text) {
            Ok(env) => self.apply_from(conn, env),
            Err(e) => self.route(conn, None, vec![ServerMsg::ProtocolError(e)]),
        }
    }

    /// Applies one client message from connection `conn`: replies go to
    /// `conn`, broadcasts to their topic, in order.
    pub fn apply_from(&mut self, conn: u32, env: Envelope<ClientMsg>) -> Vec<Outgoing> {
        let re = Some(env.seq);
        let msgs = match env.body {
            ClientMsg::Eval(b) => {
                let span = b.span.map(|s| (s.start, s.end));
                let (out, batches) =
                    self.eval(&b.code, &b.file, b.doc_revision, b.edit_epoch, span);
                let mut msgs = vec![ServerMsg::EvalResult(out.wire)];
                msgs.extend(batches);
                msgs
            }
            ClientMsg::Hush(Empty {}) => {
                self.revoke(SlotKey::All);
                Vec::new()
            }
            ClientMsg::Stop(b) => match slot_key(&b.slot) {
                Some(key) => {
                    self.revoke(key);
                    Vec::new()
                }
                None => vec![bad_body(format!("`{}` is not a slot name", b.slot))],
            },
            ClientMsg::SetVar(b) => self.on_set_var(conn, env.seq, b),
            ClientMsg::SetTweak(b) => self.on_set_tweak(conn, env.seq, b),
            ClientMsg::DocChanged(b) => self.on_doc_changed(&b),
            ClientMsg::Learn(b) => self.on_learn(&b),
            ClientMsg::Subscribe(b) => {
                self.subs.insert(conn, b.into());
                Vec::new()
            }
            ClientMsg::ManifestReq(Empty {}) => vec![ServerMsg::Manifest(self.manifest_body())],
        };
        self.route(conn, re, msgs)
    }

    /// `apply_from` for a single client (connection 0): its replies plus
    /// the broadcasts its subscription wants, in order.
    pub fn apply(&mut self, env: Envelope<ClientMsg>) -> Vec<Envelope<ServerMsg>> {
        let sub = self.subscription(0);
        self.apply_from(0, env)
            .into_iter()
            .filter(|o| match o.to {
                Dest::Conn(c) => c == 0,
                Dest::Topic(t) => sub.wants(t),
            })
            .map(|o| o.env)
            .collect()
    }

    /// `tick_routed` without the routing: the broadcasts and replies of one
    /// scheduler step, in order.
    pub fn tick(&mut self, host_now: f64) -> Vec<ServerMsg> {
        self.tick_routed(host_now)
            .into_iter()
            .map(|o| o.env.body)
            .collect()
    }

    /// Stops `key` at once (`hush`, `stop`).
    pub(super) fn revoke(&mut self, key: SlotKey) {
        self.sink.apply(StagedEffect::Revoke(key));
        let rep = self.rt.drain(&mut self.ev);
        self.console
            .extend(rep.console.iter().map(ToString::to_string));
    }

    fn manifest_body(&self) -> ManifestBody {
        let registry = self.ev.insts();
        let dynamic = registry.as_ref().map(|r| r.borrow());
        let manifest = dynamic.as_ref().map_or_else(
            || self.manifest.clone(),
            |r| self.manifest.with_controls(r.declared_names()),
        );
        let mut editors = crate::session::editors::editor_decls();
        if let Some(r) = &dynamic {
            let installed = crate::session::editors::instrument_decls(r);
            editors.retain(|decl| !installed.iter().any(|custom| custom.name == decl.name));
            editors.extend(installed);
        }
        let list = |k: &KeySet| match k {
            KeySet::Open => Vec::new(),
            KeySet::Of(set) => set.iter().map(ToString::to_string).collect(),
        };
        ManifestBody {
            sounds: list(&manifest.sounds),
            synths: list(&manifest.synths),
            controls: list(&manifest.controls),
            editors: Some(editors),
        }
    }
}

/// A `protocol-error` `bad-body` reply.
pub(super) fn bad_body(message: impl Into<String>) -> ServerMsg {
    ServerMsg::ProtocolError(ProtocolError::new(ErrorCode::BadBody, message))
}

/// The slot a `stop` names: `d1`..`d9` or a named slot, `[a-z0-9-]+`.
fn slot_key(name: &str) -> Option<SlotKey> {
    let name = name.strip_prefix(':').unwrap_or(name);
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid {
        return None;
    }
    Some(
        match name.strip_prefix('d').and_then(|n| n.parse::<u8>().ok()) {
            Some(n @ 1..=9) if name.len() == 2 => SlotKey::D(n),
            _ => SlotKey::Named(intern_kw(name)),
        },
    )
}

/// How deep package imports may nest.
const MAX_PKG_DEPTH: usize = 16;

/// Package loading (design 14.5.4 step 2, 14.5.7 "Loading", "Assets").
impl Session {
    /// Step 2 for one import: loads the package through the lock and the
    /// verified cache and binds its prefix. A failure is a load diagnostic
    /// at the import span and leaves the prefix bound but broken (an empty
    /// namespace), so qualified uses become `undefined-name`.
    pub(super) fn import_package(
        &mut self,
        decl: &ImportDecl,
        fid: FileId,
        diags: &mut Vec<Diagnostic>,
        faults: &mut Vec<Failure>,
    ) {
        let id = PackageId(Rc::clone(&decl.path));
        let mut stack = Vec::new();
        let pkg = self.load_package(&id, decl.span, diags, faults, &mut stack);
        let binding = ImportBinding {
            prefix: intern_sym(&decl.prefix),
            pkg: id,
            open: decl.open,
        };
        self.ev.ns().import(binding, pkg);
        let entry = self
            .prefixes
            .entry(Rc::clone(&decl.prefix))
            .or_insert_with(|| (Rc::clone(&decl.path), BTreeSet::new()));
        entry.0 = Rc::clone(&decl.path);
        entry.1.insert(fid);
    }

    /// Loads `id` (and, first, its own imports through the same lock) into
    /// a fresh `PkgNs`; an empty one when it cannot be read.
    fn load_package(
        &mut self,
        id: &PackageId,
        at: Span,
        diags: &mut Vec<Diagnostic>,
        faults: &mut Vec<Failure>,
        stack: &mut Vec<PackageId>,
    ) -> Rc<PkgNs> {
        let prelude = Rc::clone(self.ev.ns().prelude());
        let sources = match package_sources(self.lock.as_ref(), self.cache.as_deref(), id) {
            Ok(s) => s,
            Err(e) => {
                diags.push(pkg_diag(&e, at));
                return Rc::new(PkgNs::new(id.clone(), prelude));
            }
        };
        stack.push(id.clone());
        for (bank, files) in asset_banks(&sources, &default_prefix(id)) {
            self.register_bank(id, &bank, files, at, diags);
        }
        let mut deps: BTreeMap<Rc<str>, Rc<PkgNs>> = BTreeMap::new();
        for (path, bytes) in &sources.files {
            let Some(text) = path
                .ends_with(".vact")
                .then(|| std::str::from_utf8(bytes).ok())
                .flatten()
            else {
                continue;
            };
            for d in prescan_imports(text) {
                let dep = PackageId(Rc::clone(&d.path));
                if deps.contains_key(&d.path)
                    || stack.contains(&dep)
                    || stack.len() >= MAX_PKG_DEPTH
                {
                    continue;
                }
                let p = self.load_package(&dep, at, diags, faults, stack);
                deps.insert(Rc::clone(&d.path), p);
            }
        }
        stack.pop();
        let manifest = self.manifest.clone();
        let files = &mut self.files;
        let tag = format!("{}@{}", id.0, sources.version);
        let mut next_file = |path: &str| {
            let fid = FileId::new(u32::try_from(files.len()).unwrap_or(u32::MAX));
            files.push(Rc::from(format!("{tag}/{path}")));
            fid
        };
        let mut imports = |d: &ImportDecl| deps.get(&d.path).cloned();
        let (pkg, pdiags) = PkgNs::load(
            id.clone(),
            prelude,
            self.ev.vm_mut(),
            &sources,
            &manifest,
            &mut next_file,
            &mut imports,
        );
        for d in pdiags {
            if d.code == DiagCode::PackageLoadFailed {
                diags.push(Diagnostic { span: at, ..d });
            } else {
                diags.push(d);
            }
        }
        // The package forms' effects (instrument installs) go to the runtime.
        for e in self.ev.vm_mut().effects_mut().take() {
            crate::ns::stage::EffectSink::apply(&mut self.sink, e);
        }
        let drained = self.rt.drain(&mut self.ev);
        diags.extend(drained.diags);
        faults.extend(drained.faults);
        pkg
    }

    /// Registers one asset bank `:<prefix>-<bank>`. The first registration
    /// wins; a clash with another package's bank or a builtin sound is
    /// `import-collision` (w).
    fn register_bank(
        &mut self,
        id: &PackageId,
        bank: &Rc<str>,
        files: Vec<PathVal>,
        at: Span,
        diags: &mut Vec<Diagnostic>,
    ) {
        let collision = |message: String| Diagnostic {
            span: at,
            severity: DiagCode::ImportCollision.default_severity(),
            code: DiagCode::ImportCollision,
            message,
            origin: None,
        };
        if let Some(owner) = self.banks.get(bank) {
            if **owner != *id.0 {
                diags.push(collision(format!(
                    "the bank `:{bank}` of `{}` is already registered by `{owner}`",
                    id.0
                )));
            }
            return;
        }
        if self.manifest.sounds.contains(bank) || self.manifest.synths.contains(bank) {
            diags.push(collision(format!(
                "the bank `:{bank}` of `{}` collides with the builtin `:{bank}`",
                id.0
            )));
            return;
        }
        match self.rt.hosts.samples.register_bank(intern_kw(bank), files) {
            Err(e) if e.code != FailCode::HostUnavailable => {
                diags.push(collision(format!("the bank `:{bank}`: {}", e.message)));
            }
            _ => {
                self.banks.insert(Rc::clone(bank), Rc::clone(&id.0));
                self.manifest = self.manifest.with_sounds([&**bank]);
                self.add_to_kits(intern_kw(bank));
            }
        }
    }

    /// Adds a registered bank to the prelude's `default-sound-kit` and
    /// `sound-kit`, so `s :<bank>` resolves at query time like a builtin
    /// host bank (7.1.4; the instrument registry routes it to the sampler).
    fn add_to_kits(&self, kw: KwId) {
        let prelude = self.ev.ns().prelude();
        for name in ["default-sound-kit", "sound-kit"] {
            let Some(slot) = prelude.slot(intern_sym(name)) else {
                continue;
            };
            if let Value::Dict(d) = slot.get() {
                let mut kit = (*d).clone();
                kit.entry(Key::Kw(kw))
                    .or_insert_with(|| Value::Sound(Rc::new(Sound::Builtin(kw))));
                slot.set(Value::Dict(Rc::new(kit)));
            }
        }
    }
}
