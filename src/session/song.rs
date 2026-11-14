//! Isolated, whole-document finite-song construction. No effects reach the
//! active runtime and no active namespace/registry/loader is cloned.
mod cells;
pub(crate) mod freeze;
mod inventory;
mod shape_preparation;
mod source_uses;
mod transport;
/// Genuine fixture inventory from the same original evaluator and frozen Song.
#[cfg(test)]
pub(crate) fn capture_original_test_routing(
    evaluator: &crate::ns::evaluator::Evaluator,
    song: &crate::song::Song,
    limits: crate::song::assets::SongAssetLimits,
) -> Result<crate::song::snapshot::FrozenRoutingInventory, crate::vm::fail::Failure> {
    inventory::capture_routing(evaluator, song, limits)
}
use super::eval::package_sources;
use crate::host::caps::{InstResolver, Route, SampleSrc};
use crate::ns::evaluator::Evaluator;
use crate::ns::namespace::Prelude;
use crate::ns::pkg::{ImportBinding, PackageId, PkgNs};
use crate::ns::stage::{EffectSink, StagedEffect};
use crate::pkg::cache::CacheBackend;
use crate::pkg::load::{asset_banks, default_prefix};
use crate::pkg::lock::LockFile;
use crate::reader::span::FileId;
use crate::reader::{prescan_imports, read, AliasEnv, ImportDecl};
use crate::song::assets::{
    SongAssetFactory, SongAssetLimits, SongAssetPreparation, SongAssetSelector, SongSourceFile,
};
use crate::song::{PreparedSong, SnapshotEpoch, SongCandidate};
use crate::types::diag::{Diagnostic, Severity};
use crate::types::manifest::HostManifest;
use crate::value::intern::{intern_kw, intern_sym, KwId};
use crate::value::key::Key;
use crate::value::value::{PathVal, Sound, Value};
use crate::vm::fail::{FailCode, Failure};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
pub(super) use transport::SongRequests;

/// Explicit configuration-only loaders, remaining budgets and verified cache.
/// These capabilities never supply an active evaluator or mutable sound values.
pub struct CandidateBuildCtx<'a> {
    pub assets: &'a dyn SongAssetFactory,
    pub asset_limits: SongAssetLimits,
    pub lock: Option<&'a LockFile>,
    pub cache: Option<&'a dyn CacheBackend>,
}
#[derive(Default)]
struct Sources {
    files: BTreeMap<FileId, crate::song::snapshot::FrozenSource>,
    loaded: BTreeSet<(Option<FileId>, Rc<str>)>,
    origins: BTreeMap<FileId, Rc<str>>,
    bytes: u64,
}
struct CandidateLoader {
    inner: Box<dyn crate::ns::load::SourceLoader>,
    sources: Rc<RefCell<Sources>>,
    limits: SongAssetLimits,
}
impl crate::ns::load::SourceLoader for CandidateLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        let mapped = normalize_path(path, &self.sources.borrow().origins);
        let (file, text) = self.inner.read(&mapped)?;
        let mut sources = self.sources.borrow_mut();
        if !sources.files.contains_key(&file) {
            let bytes = sources
                .bytes
                .checked_add(text.len() as u64)
                .ok_or_else(|| failure("source bytes overflow"))?;
            if bytes > self.limits.max_source_bytes
                || sources.files.len() >= self.limits.max_source_files as usize
            {
                return Err(Failure::new(
                    FailCode::FuelExhausted,
                    "candidate source inventory exhausted",
                ));
            }
            sources.bytes = bytes;
            sources.files.insert(
                file,
                crate::song::snapshot::FrozenSource {
                    file,
                    path: mapped.text.clone(),
                    text: text.clone(),
                },
            );
        }
        sources.loaded.insert((path.file, path.text.clone()));
        Ok((file, text))
    }
}
fn normalize_path(path: &PathVal, origins: &BTreeMap<FileId, Rc<str>>) -> PathVal {
    let origin = path.file.and_then(|file| origins.get(&file));
    if (path.text.starts_with("./") || path.text.starts_with("../")) && origin.is_some() {
        let base = origin.map_or("", |x| x.as_ref());
        let parent = base.rsplit_once('/').map_or("", |(parent, _)| parent);
        PathVal {
            file: None,
            text: Rc::from(if parent.is_empty() {
                path.text.to_string()
            } else {
                format!("{parent}/{}", path.text)
            }),
        }
    } else {
        path.clone()
    }
}
struct Staging(Rc<RefCell<Vec<StagedEffect>>>);
impl EffectSink for Staging {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().push(effect);
    }
}
fn failure(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}
fn diagnostics(diags: &[Diagnostic]) -> Result<(), Failure> {
    if let Some(d) = diags.iter().find(|d| d.severity == Severity::Error) {
        return Err(failure(format!("candidate diagnostic: {}", d.message)));
    }
    Ok(())
}
fn warning_messages(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.severity != Severity::Error)
        .map(|d| d.message.to_string())
        .collect()
}
fn check_effects(effects: &[StagedEffect]) -> Result<(), Failure> {
    for effect in effects {
        if !matches!(
            effect,
            StagedEffect::Install(_)
                | StagedEffect::Bindings(_)
                | StagedEffect::TweakRefresh(_)
                | StagedEffect::PlaySong(_)
        ) {
            return Err(Failure::new(
                FailCode::EffectInQuery,
                "song candidate contains a disallowed effect",
            ));
        }
    }
    Ok(())
}
/// Evaluate all forms in a fresh private runtime and close its asset inventory.
/// Failure drops the entire candidate. This prepares ownership, not host Ready.
/// The standalone API uses edit epoch zero; Session supplies its addressed epoch.
pub fn evaluate_song_candidate(
    code: &str,
    file: &str,
    revision: u64,
    epoch: SnapshotEpoch,
    cx: &CandidateBuildCtx<'_>,
) -> Result<SongCandidate, Failure> {
    evaluate(code, file, revision, 0, epoch, cx)
}
/// Classify an entrypoint in isolated staged evaluation; never run live effects.
/// Returns the original candidate for a Song and None for valid legacy code.
pub fn probe_song_candidate(
    code: &str,
    file: &str,
    revision: u64,
    epoch: SnapshotEpoch,
    cx: &CandidateBuildCtx<'_>,
) -> Result<Option<SongCandidate>, Failure> {
    evaluate_inner(code, file, revision, 0, epoch, cx, true)
}
fn evaluate(
    code: &str,
    file: &str,
    revision: u64,
    edit_epoch: u64,
    epoch: SnapshotEpoch,
    cx: &CandidateBuildCtx<'_>,
) -> Result<SongCandidate, Failure> {
    evaluate_inner(code, file, revision, edit_epoch, epoch, cx, false)?
        .ok_or_else(|| failure("candidate requires exactly one play-song entrypoint"))
}
fn evaluate_inner(
    code: &str,
    file: &str,
    revision: u64,
    edit_epoch: u64,
    epoch: SnapshotEpoch,
    cx: &CandidateBuildCtx<'_>,
    probe: bool,
) -> Result<Option<SongCandidate>, Failure> {
    let limits = cx.asset_limits.validate()?;
    if file.is_empty() || file.contains('\0') || code.len() as u64 > limits.max_source_bytes {
        return Err(failure("candidate source path/size is invalid"));
    }
    let source = SongSourceFile {
        file: FileId::new(0),
        path: PathVal {
            text: Rc::from(file),
            file: None,
        },
    };
    let mut assets = cx.assets.begin(source, limits)?;
    let sources = Rc::new(RefCell::new(Sources::default()));
    {
        let mut state = sources.borrow_mut();
        state.bytes = code.len() as u64;
        state.files.insert(
            FileId::new(0),
            crate::song::snapshot::FrozenSource {
                file: FileId::new(0),
                path: Rc::from(file),
                text: Rc::from(code),
            },
        );
    }
    let effects = Rc::new(RefCell::new(Vec::new()));
    let mut evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(CandidateLoader {
            inner: assets.source_loader(),
            sources: sources.clone(),
            limits,
        }),
        Box::new(Staging(effects.clone())),
    );
    if evaluator
        .insts()
        .is_some_and(|r| !r.borrow().template_errors().is_empty())
    {
        return Err(failure("candidate prelude template installation failed"));
    }
    check_effects(&effects.borrow())?;
    evaluator
        .insts()
        .ok_or_else(|| failure("candidate registry unavailable"))?
        .borrow_mut()
        .enable_closed_song_resources();
    let mut packages = Packages {
        cx,
        assets: &mut assets,
        cache: BTreeMap::new(),
        stack: Vec::new(),
        next_file: 1,
        source_bytes: code.len() as u64,
        source_files: 1,
        sources: sources.clone(),
    };
    for decl in prescan_imports(code) {
        let pkg = packages.load(&PackageId(decl.path.clone()), &mut evaluator)?;
        evaluator.ns().import(
            ImportBinding {
                prefix: intern_sym(&decl.prefix),
                pkg: pkg.id.clone(),
                open: decl.open,
            },
            pkg,
        );
    }
    let mut aliases = AliasEnv::new();
    for decl in prescan_imports(code) {
        aliases.bind(decl.prefix, decl.path);
    }
    let parsed = read(code, FileId::new(0), &aliases);
    diagnostics(&parsed.diags)?;
    let mut warnings = warning_messages(&parsed.diags);
    let imports = prescan_imports(code);
    for node in &parsed.nodes {
        let mut invalid_order = false;
        node.walk(&mut |child| {
            if let crate::reader::node::NodeKind::Atom(crate::reader::node::Atom::Qualified {
                prefix,
                ..
            }) = &child.kind
            {
                if imports.iter().any(|d| d.prefix == *prefix)
                    && !imports
                        .iter()
                        .any(|d| d.prefix == *prefix && d.span.start <= child.span.start)
                {
                    invalid_order = true;
                }
            }
        });
        if invalid_order {
            return Err(failure("qualified name used before its import"));
        }
    }
    let mut expand = crate::expand::ExpandCx::new(parsed.next_node_id());
    for node in parsed.nodes {
        let form = crate::expand::expand(&node, &mut expand).map_err(|d| failure(d.message))?;
        let result = evaluator.eval_form(&form);
        diagnostics(&result.diags)?;
        warnings.extend(warning_messages(&result.diags));
        result.value?;
        if !probe {
            check_effects(&effects.borrow())?;
        }
    }
    let load_diags = crate::ns::load::take_load_diags(evaluator.vm_mut());
    diagnostics(&load_diags)?;
    warnings.extend(warning_messages(&load_diags));
    let entries: Vec<_> = effects
        .borrow()
        .iter()
        .filter_map(|e| {
            if let StagedEffect::PlaySong(song) = e {
                Some(song.clone())
            } else {
                None
            }
        })
        .collect();
    if probe && entries.is_empty() {
        return Ok(None);
    }
    check_effects(&effects.borrow())?;
    if entries.len() != 1 {
        return Err(failure(
            "candidate requires exactly one play-song entrypoint",
        ));
    }
    let song = entries[0].clone();
    let mut remaining = limits.max_walk_nodes;
    let mut pending =
        shape_preparation::discover_shapes(&mut evaluator, &song, limits, &mut remaining)?;
    shape_preparation::charge(&mut remaining, pending.retained_count())?;
    let mut retained: BTreeSet<_> = pending.retained_instruments().collect();
    shape_preparation::charge(&mut remaining, 1)?;
    let mut roots = vec![Value::Song(song.clone())];
    roots.extend(pending.take_dependency_roots());
    // Retain only symbolic roots, not every prelude native or a full query scan.
    let dependencies = shape_preparation::dependencies(&roots, limits, &mut remaining)?;
    if dependencies.has_external_sounds || dependencies.has_live_signals {
        return Err(failure("candidate contains external output or live input"));
    }
    let registry = evaluator
        .insts()
        .ok_or_else(|| failure("candidate registry unavailable"))?;
    let mut known = BTreeSet::new();
    for keyword in dependencies
        .sound_keywords
        .iter()
        .chain(dependencies.literal_keywords.iter())
    {
        if !known.insert(*keyword) {
            continue;
        }
        let family = family(&evaluator, *keyword)?;
        if let Some(family) = family {
            shape_preparation::charge(&mut remaining, 1)?;
            roots.push(family);
        }
    }
    let dependencies = shape_preparation::dependencies(&roots, limits, &mut remaining)?;
    if dependencies.has_external_sounds || dependencies.has_live_signals {
        return Err(failure(
            "candidate family contains external output or live input",
        ));
    }
    for id in dependencies.native_functions {
        let name = evaluator
            .ns()
            .prelude()
            .native(id)
            .map(|entry| entry.sig.name)
            .ok_or_else(|| failure("unknown candidate native"))?;
        if evaluator
            .ns()
            .prelude()
            .native(id)
            .is_some_and(|entry| entry.sig.effectful)
            || forbidden_native(name)
        {
            return Err(failure(format!(
                "candidate callback references disallowed native `{name}`"
            )));
        }
    }
    shape_preparation::charge(
        &mut remaining,
        dependencies.sample_paths.len()
            + dependencies.literal_paths.len()
            + dependencies.sound_keywords.len()
            + dependencies.instruments.len(),
    )?;
    let mut selections = Vec::new();
    for path in &dependencies.sample_paths {
        selections.push(SongAssetSelector::Path(normalize_path(
            path,
            &sources.borrow().origins,
        )));
    }
    for path in dependencies.literal_paths {
        // Source literals already evaluated by load are retained in preparation;
        // audio paths have explicit WAV identity, otherwise late I/O is rejected.
        if path.text.to_ascii_lowercase().ends_with(".wav") {
            selections.push(SongAssetSelector::Path(normalize_path(
                &path,
                &sources.borrow().origins,
            )));
        } else if !sources
            .borrow()
            .loaded
            .contains(&(path.file, path.text.clone()))
        {
            return Err(failure(
                "ambiguous resource path has no closed classification",
            ));
        }
    }
    let mut sounds: Vec<_> = dependencies
        .sound_keywords
        .into_iter()
        .map(Sound::Builtin)
        .collect();
    sounds.extend(dependencies.instruments.into_iter().map(Sound::Inst));
    shape_preparation::charge(
        &mut remaining,
        dependencies.sample_paths.len() + dependencies.buffers.len(),
    )?;
    sounds.extend(dependencies.sample_paths.iter().cloned().map(Sound::Sample));
    sounds.extend(dependencies.buffers.iter().cloned().map(Sound::Buffer));
    let mut event_banks = BTreeSet::new();
    for sound in sounds {
        let Route::Audio { inst, sample } = registry.borrow().route(&sound)? else {
            return Err(failure("candidate sound is not audio"));
        };
        shape_preparation::charge(&mut remaining, 1)?;
        retained.insert(inst);
        let r = registry.borrow();
        let entry = r
            .entry(inst)
            .ok_or_else(|| failure("candidate instrument missing"))?;
        if !entry.signals.is_empty()
            || entry.def.nodes.iter().any(|n| {
                matches!(
                    n,
                    crate::dsp::graph::UGenSpec::HostInputL
                        | crate::dsp::graph::UGenSpec::HostInputR
                )
            })
        {
            return Err(failure("candidate instrument reads live input"));
        }
        if let Some(sample) = sample {
            match sample {
                SampleSrc::Path(path) => {
                    shape_preparation::charge(&mut remaining, 1)?;
                    selections.push(SongAssetSelector::Path(normalize_path(
                        &path,
                        &sources.borrow().origins,
                    )));
                }
                SampleSrc::Bank { kw, .. } => {
                    shape_preparation::charge(&mut remaining, 2)?;
                    event_banks.insert((inst, kw));
                    selections.push(SongAssetSelector::Bank(kw));
                }
                SampleSrc::Buffer { .. } => {}
            }
        }
    }
    shape_preparation::charge(&mut remaining, retained.len())?;
    let retained: Vec<_> = retained.into_iter().collect();
    let graph_selections = crate::song::snapshot::FrozenGraphResources::pin_selections(
        &registry.borrow(),
        &retained,
        &event_banks,
        &mut remaining,
    )?;
    shape_preparation::charge(&mut remaining, graph_selections.len())?;
    selections.extend(graph_selections);
    for buffer in &dependencies.buffers {
        assets.pin_buffer(buffer)?;
    }
    assets.pin(&selections)?;
    let mut closed = assets.close()?;
    shape_preparation::charge(&mut remaining, dependencies.buffers.len())?;
    let copies: Vec<Rc<crate::value::sample::SampleBuf>> = dependencies
        .buffers
        .iter()
        .filter_map(|b| closed.buffer_copy(b.id))
        .collect();
    let mut reduced = limits;
    shape_preparation::charge(
        &mut remaining,
        sources.borrow().origins.len() + copies.len(),
    )?;
    reduced.max_walk_nodes = remaining;
    let mut frozen = freeze::Freeze::new(&closed, reduced);
    frozen.keep_copies(&copies);
    frozen.origins = sources.borrow().origins.clone();
    // Every retained candidate namespace slot is fresh; rewriting in place keeps
    // recursive function/global linkage and never touches an active namespace.
    for slot in evaluator.candidate_slots() {
        frozen.slot(&slot)?;
    }
    let work = frozen.consumed_work();
    drop(frozen);
    shape_preparation::charge(&mut remaining, work as usize)?;
    shape_preparation::charge(
        &mut remaining,
        sources.borrow().origins.len() + copies.len(),
    )?;
    reduced.max_walk_nodes = remaining
        .checked_sub(
            u32::try_from(pending.record_count())
                .map_err(|_| failure("shape record count overflow"))?,
        )
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "shape record storage exhausted"))?;
    let mut reachable = freeze::Freeze::new(&closed, reduced);
    reachable.audit = true;
    reachable.keep_copies(&copies);
    reachable.origins = sources.borrow().origins.clone();
    let Value::Song(song) = reachable.value(&Value::Song(song))? else {
        return Err(failure("candidate song freeze invariant"));
    };
    let pending = pending.freeze_records(&mut reachable, &mut remaining)?;
    let work = reachable.consumed_work();
    drop(reachable);
    shape_preparation::charge(&mut remaining, work as usize)?;
    let shapes = pending.admit_closed(&mut evaluator, &mut closed, limits, &mut remaining)?;
    let mut inventory =
        inventory::capture_routing_shared(&evaluator, &song, limits, &shapes, &mut remaining)?;
    inventory.resources = crate::song::snapshot::FrozenGraphResources::capture(
        &inventory,
        &registry.borrow(),
        &mut closed,
        &event_banks,
        &mut remaining,
    )?;
    let mut candidate = SongCandidate::from_isolated_evaluation(
        evaluator,
        song,
        closed,
        copies,
        file.into(),
        revision,
        edit_epoch,
        epoch,
    )?;
    shape_preparation::charge(&mut remaining, sources.borrow().files.len())?;
    inventory.sources = sources.borrow().files.values().cloned().collect();
    candidate.set_routing(inventory);
    candidate.set_warnings(warnings);
    Ok(Some(candidate))
}
fn forbidden_native(name: &str) -> bool {
    matches!(
        name,
        "load"
            | "print"
            | "capture"
            | "render"
            | "once"
            | "at"
            | "hush"
            | "stop"
            | "upd"
            | "set!"
            | "midi-notes"
            | "midi-out"
            | "osc"
    )
}
fn family(evaluator: &Evaluator, keyword: KwId) -> Result<Option<Value>, Failure> {
    let kit = evaluator
        .ns()
        .lookup(intern_sym("sound-kit"))
        .map(|slot| slot.slot().get());
    match kit {
        Some(Value::Dict(kit)) => Ok(kit
            .get(&Key::Kw(keyword))
            .cloned()
            .or_else(|| evaluator.insts().and_then(|r| r.borrow().sound(keyword)))),
        Some(Value::VarRef(slot)) => match slot.get() {
            Value::Dict(kit) => Ok(kit.get(&Key::Kw(keyword)).cloned()),
            _ => Err(failure("candidate sound kit is not a dictionary")),
        },
        _ => Ok(evaluator.insts().and_then(|r| r.borrow().sound(keyword))),
    }
}
struct Packages<'a, 'b> {
    cx: &'a CandidateBuildCtx<'a>,
    assets: &'b mut SongAssetPreparation,
    cache: BTreeMap<PackageId, Rc<PkgNs>>,
    stack: Vec<PackageId>,
    next_file: u32,
    source_bytes: u64,
    source_files: u32,
    sources: Rc<RefCell<Sources>>,
}
impl Packages<'_, '_> {
    fn load(&mut self, id: &PackageId, evaluator: &mut Evaluator) -> Result<Rc<PkgNs>, Failure> {
        if let Some(pkg) = self.cache.get(id) {
            return Ok(pkg.clone());
        }
        if self.stack.len() >= 16 || self.stack.contains(id) {
            return Err(failure("candidate package recursion/cycle"));
        }
        let sources =
            package_sources(self.cx.lock, self.cx.cache, id).map_err(|e| failure(e.to_string()))?;
        self.stack.push(id.clone());
        let mut deps = BTreeMap::new();
        for (path, bytes) in &sources.files {
            if !path.ends_with(".vact") {
                continue;
            }
            self.source_files = self
                .source_files
                .checked_add(1)
                .ok_or_else(|| failure("package file count overflow"))?;
            self.source_bytes = self
                .source_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| failure("package source size overflow"))?;
            if self.source_files > self.cx.asset_limits.max_source_files
                || self.source_bytes > self.cx.asset_limits.max_source_bytes
            {
                return Err(failure("package source budget exhausted"));
            }
            let text =
                std::str::from_utf8(bytes).map_err(|_| failure("package source is not UTF-8"))?;
            for decl in prescan_imports(text) {
                let dep = self.load(&PackageId(decl.path.clone()), evaluator)?;
                deps.insert(decl.path.clone(), dep);
            }
        }
        self.sources.borrow_mut().bytes = self.source_bytes;
        let mut manifest = HostManifest::spec_default();
        for (bank, files) in asset_banks(&sources, &default_prefix(id)) {
            let kw = intern_kw(&bank);
            self.assets.sample_loader().register_bank(kw, files)?;
            manifest = manifest.with_sounds([bank.as_ref()]);
            for name in ["sound-kit", "default-sound-kit"] {
                if let Some(slot) = evaluator.ns().prelude().slot(intern_sym(name)) {
                    if let Value::Dict(kit) = slot.get() {
                        let mut kit = (*kit).clone();
                        kit.insert(Key::Kw(kw), Value::Sound(Rc::new(Sound::Builtin(kw))));
                        slot.set(Value::Dict(Rc::new(kit)));
                    }
                }
            }
        }
        let file_count = sources
            .files
            .iter()
            .filter(|(path, _)| path.ends_with(".vact"))
            .count();
        let file_count =
            u32::try_from(file_count).map_err(|_| failure("package file count overflow"))?;
        self.next_file
            .checked_add(file_count)
            .ok_or_else(|| failure("package file identity overflow"))?;
        let mut next = self.next_file;
        let retained = self.sources.clone();
        let mut next_file = |path: &str| {
            let file = FileId::new(next);
            next += 1;
            let full = sources
                .root
                .as_ref()
                .map_or_else(|| path.to_string(), |root| format!("{root}/{path}"));
            let text = sources
                .files
                .iter()
                .find(|(name, _)| name.as_ref() == path)
                .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
                .map(Rc::from)
                .unwrap_or_else(|| Rc::from(""));
            let mut retained = retained.borrow_mut();
            retained.origins.insert(file, Rc::from(full.as_str()));
            retained.files.insert(
                file,
                crate::song::snapshot::FrozenSource {
                    file,
                    path: Rc::from(full),
                    text,
                },
            );
            file
        };
        let mut imports = |decl: &ImportDecl| deps.get(&decl.path).cloned();
        let prelude = evaluator.ns().prelude().clone();
        let (pkg, diags) = PkgNs::load(
            id.clone(),
            prelude,
            evaluator.vm_mut(),
            &sources,
            &manifest,
            &mut next_file,
            &mut imports,
        );
        self.next_file = next;
        diagnostics(&diags)?;
        // Package installs remain local; every other package effect is checked.
        let staged = evaluator.vm_mut().effects_mut().take();
        check_effects(&staged)?;
        if staged
            .iter()
            .any(|e| matches!(e, StagedEffect::PlaySong(_)))
        {
            return Err(failure("package cannot declare a song entrypoint"));
        }
        self.stack.pop();
        self.cache.insert(id.clone(), pkg.clone());
        Ok(pkg)
    }
}
/// Cancel the addressed pending candidate on this thread; active songs are not
/// touched. The session-owned pending candidate is removed by its owner below.
pub fn cancel_song_candidate(
    pending: &mut PreparedSong,
    epoch: SnapshotEpoch,
) -> Result<(), Failure> {
    pending.cancel(epoch)
}

pub(super) fn prototype_copy_work(
    proto: &crate::compile::proto::FnProto,
) -> Result<usize, Failure> {
    let mut count = 0usize;
    for n in [
        proto.code.len(),
        proto.consts.len(),
        proto.protos.len(),
        proto.globals.len(),
        proto.spans.len(),
        proto.defs.len(),
        proto.arity.names.len(),
        proto.arity.scalar_types.len(),
        proto.masks.len(),
        proto.call_sites.len(),
        proto.list_sites.len(),
        proto.shapes.len(),
    ] {
        count = count
            .checked_add(n)
            .ok_or_else(|| failure("prototype length overflow"))?;
    }
    for n in proto
        .call_sites
        .iter()
        .map(|s| s.args.len())
        .chain(proto.list_sites.iter().map(|s| s.items.len()))
        .chain(proto.shapes.iter().map(|s| s.fields.len()))
        .chain(proto.masks.iter().map(|m| m.len()))
    {
        count = count
            .checked_add(n)
            .ok_or_else(|| failure("prototype length overflow"))?;
    }
    for mask in &proto.masks {
        for entry in mask.0.iter() {
            if let crate::types::masks::MaskEntry::Forward { links } = entry {
                count = count
                    .checked_add(links.len())
                    .ok_or_else(|| failure("prototype length overflow"))?;
            }
        }
    }
    Ok(count)
}

fn child_count(pattern: &crate::pattern::pat::Pat) -> usize {
    match &pattern.node {
        crate::pattern::pat::PatNode::Choose(ps)
        | crate::pattern::pat::PatNode::Stack(ps)
        | crate::pattern::pat::PatNode::Cat(ps)
        | crate::pattern::pat::PatNode::FastCat(ps) => ps.len(),
        _ => 2,
    }
}

pub(super) struct ClosedShapeCtx<'a> {
    pub(super) vm: &'a crate::vm::vm::Vm,
    pub(super) ns: &'a crate::ns::namespace::Namespace,
    pub(super) assets: &'a mut crate::song::assets::PinnedSongAssets,
    pub(super) remaining: &'a mut u32,
    pub(super) limits: SongAssetLimits,
    pub(super) depth: u32,
}
impl ClosedShapeCtx<'_> {
    fn admit(&mut self, count: usize, depth: u32) -> Result<(), Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "closed callback family depth exceeded",
            ));
        }
        let count = u32::try_from(count)
            .map_err(|_| Failure::new(FailCode::Overflow, "closed callback work overflow"))?;
        *self.remaining = self.remaining.checked_sub(count).ok_or_else(|| {
            Failure::new(FailCode::FuelExhausted, "closed callback work exhausted")
        })?;
        Ok(())
    }
    pub(super) fn accepts(
        &mut self,
        deps: &crate::song::assets::SongAssetDependencies,
    ) -> Result<bool, Failure> {
        use crate::host::caps::{InstResolver, Route, SampleLoader};
        use crate::value::key::Key;
        use crate::value::value::Sound;
        let mut roots = Vec::new();
        self.admit(
            deps.sound_keywords
                .len()
                .checked_add(deps.instruments.len())
                .and_then(|n| n.checked_add(deps.sample_paths.len()))
                .and_then(|n| n.checked_add(deps.buffers.len()))
                .ok_or_else(|| {
                    Failure::new(FailCode::Overflow, "callback family admission overflow")
                })?,
            self.depth,
        )?;
        let kit = self
            .ns
            .lookup(crate::value::intern::intern_sym("sound-kit"))
            .map(|slot| slot.slot().get());
        let registry = self.vm.dsp.registry.clone().ok_or_else(|| {
            Failure::new(
                FailCode::HostUnavailable,
                "closed callback registry missing",
            )
        })?;
        for kw in &deps.sound_keywords {
            let family = match &kit {
                Some(Value::Dict(kit)) => kit.get(&Key::Kw(*kw)).cloned(),
                _ => None,
            };
            roots.push((
                family
                    .or_else(|| registry.borrow().sound(*kw))
                    .unwrap_or_else(|| Value::Sound(Rc::new(Sound::Builtin(*kw)))),
                self.depth,
            ));
        }
        for id in &deps.instruments {
            roots.push((Value::Inst(*id), self.depth));
        }
        for path in &deps.sample_paths {
            roots.push((
                Value::Sound(Rc::new(Sound::Sample(path.clone()))),
                self.depth,
            ));
        }
        for buffer in &deps.buffers {
            roots.push((
                Value::Sound(Rc::new(Sound::Buffer(buffer.clone()))),
                self.depth,
            ));
        }
        while let Some((value, level)) = roots.pop() {
            self.admit(1, level)?;
            let sound = match value {
                Value::Sound(sound) => (*sound).clone(),
                Value::Inst(id) => Sound::Inst(id),
                Value::List(list) => {
                    self.admit(list.items.len(), level)?;
                    roots.extend(list.items.iter().cloned().map(|v| (v, level + 1)));
                    continue;
                }
                _ => return Ok(false),
            };
            let route = registry.borrow().route(&sound)?;
            let Route::Audio { inst, sample } = route else {
                return Ok(false);
            };
            let registry = registry.borrow();
            let Some(entry) = registry.entry(inst) else {
                return Ok(false);
            };
            if !entry.signals.is_empty()
                || entry.def.nodes.iter().any(|node| {
                    matches!(
                        node,
                        crate::dsp::graph::UGenSpec::HostInputL
                            | crate::dsp::graph::UGenSpec::HostInputR
                    )
                })
            {
                return Ok(false);
            }
            drop(registry);
            if let Some(sample) = sample {
                if self.assets.load(&sample).is_err() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}
