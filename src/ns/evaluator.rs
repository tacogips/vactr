//! The top-level driver (design 7.1.3 "Top-level driver") and the reactive
//! pass (5.6 revised).
//!
//! `Evaluator` owns the namespace, the dependency graph, the VM, the
//! staging and the `FormGen` counter. `eval_form` runs one expanded form as
//! a whole-form transaction; a form that writes a top-level slot (a
//! redefinition or a top-level `upd`) triggers a reactive pass over the
//! forms that eagerly read it. The pass runs in rounds, gates every eager
//! read at the dereference (dirty-read abort, `Failed`/`Blocked` owner),
//! commits provisionally through the pass journal, validates, rolls back
//! invalid forms, and only then releases every host-visible effect,
//! followed by ONE `bindings` batch.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::compile::proto::FnProto;
use crate::compile::{compile, CompileCx};
use crate::ns::depgraph::{DepGraph, Edge, FormId, FormRec, FormState, WriteItem};
use crate::ns::insts::{inst_target, load_templates, realize_inst, InstRegistry};
use crate::ns::journal::{Restored, Snapshot};
use crate::ns::load::{read_forms, register_load, take_load_diags, LoaderHost, SourceLoader};
use crate::ns::namespace::{FormGen, Namespace, Prelude, VarSlotRef};
use crate::ns::stage::{EffectSink, SlotKey, StagedEffect};
use crate::ns::tweak::{SiteOrigin, SiteTier, TweakId, TweakSite};
use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::FileId;
use crate::types::check::{check, CheckResult};
use crate::types::diag::Diagnostic;
use crate::types::manifest::HostManifest;
use crate::value::eq::deep_eq;
use crate::value::intern::{intern_sym, name_of_sym, SymId};
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::pattern::reject_input_lanes;
use crate::vm::natives::register_domain;
use crate::vm::ops::Op;
use crate::vm::vm::{ReadObserver, Vm};

fn has_free_session_symbol(form: &Node) -> bool {
    let definition = if matches!(form.kind, NodeKind::Call) {
        let head = form.children.first();
        let name = form.children.get(1);
        let head_name = head.and_then(|node| match &node.kind {
            NodeKind::Atom(Atom::Sym(name) | Atom::Builtin(name)) => Some(name.as_ref()),
            _ => None,
        });
        let is_definition = matches!(
            head_name,
            Some("let" | "var" | "fn" | "inst" | "bus" | "look" | "struct" | "enum")
        );
        is_definition.then_some((head, name))
    } else {
        None
    };
    let mut found = false;
    form.walk(&mut |node| {
        if matches!(node.kind, NodeKind::Atom(Atom::Qualified { .. })) {
            found = true;
        } else if node.sym_name().is_some() {
            let is_definition_token = definition.is_some_and(|(head, name)| {
                head.is_some_and(|candidate| std::ptr::eq(candidate, node))
                    || name.is_some_and(|candidate| std::ptr::eq(candidate, node))
            });
            found |= !is_definition_token;
        }
    });
    found
}

/// The outcome of one top-level form.
#[derive(Debug)]
pub struct FormOutcome {
    pub value: Result<Value, Failure>,
    pub diags: Vec<Diagnostic>,
    pub form_gen: FormGen,
}

/// One step of a reactive pass, for tests and the protocol layer.
#[derive(Clone, Debug)]
pub enum PassEvent {
    /// A PROVISIONAL commit and the values it wrote.
    Commit {
        form: FormId,
        values: Vec<(SymId, Value)>,
    },
    /// A dirty read of a scheduled, not yet recomputed owner.
    DirtyAbort {
        form: FormId,
        owner: FormId,
    },
    /// A read of a `Failed`/`Blocked` owner's slot.
    BlockedAbort {
        form: FormId,
        on: SymId,
    },
    Failed {
        form: FormId,
        code: FailCode,
    },
    /// Round-end validation found a stale read.
    Stale {
        form: FormId,
    },
    Cycle {
        forms: Vec<FormId>,
    },
    /// The journal restored a slot of an invalid form.
    Restore(Restored),
}

/// What one pass did.
#[derive(Clone, Debug, Default)]
pub struct PassReport {
    pub events: Vec<PassEvent>,
    /// Final state of every form the pass touched.
    pub states: Vec<(FormId, FormState)>,
    pub diags: Vec<Diagnostic>,
    /// The published `bindings` batch.
    pub bindings: Vec<(SymId, Value)>,
}

/// One eager read an evaluation performed.
#[derive(Clone, Debug)]
pub(super) struct Read {
    pub(super) slot: VarSlotRef,
    pub(super) version: u64,
    pub(super) owner: Option<FormId>,
}

/// Why an evaluation was aborted at a dereference.
#[derive(Clone, Debug)]
pub(super) enum Abort {
    Dirty(FormId),
    Blocked(VarSlotRef),
}

/// The deref-time gate the read observer consults (5.6 joins).
#[derive(Default)]
struct Gate {
    active: bool,
    owners: BTreeMap<u64, FormId>,
    dirty: BTreeSet<FormId>,
    bad: BTreeSet<FormId>,
    /// Slots the running form defines: neither gated nor recorded.
    own: BTreeSet<u64>,
    reads: Vec<Read>,
    abort: Option<Abort>,
}

struct GateObserver(Rc<RefCell<Gate>>);

impl ReadObserver for GateObserver {
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        let mut g = self.0.borrow_mut();
        if !g.active || g.own.contains(&slot.id()) {
            return Ok(());
        }
        let owner = g.owners.get(&slot.id()).copied();
        if let Some(o) = owner {
            let abort = if g.dirty.contains(&o) {
                Some(Abort::Dirty(o))
            } else if g.bad.contains(&o) {
                Some(Abort::Blocked(slot.clone()))
            } else {
                None
            };
            if let Some(a) = abort {
                g.abort = Some(a);
                return Err(Failure::new(
                    FailCode::Blocked,
                    format!("blocked on `{}`", name_of_sym(slot.name())),
                ));
            }
        }
        if !g.reads.iter().any(|r| r.slot.same(slot)) {
            g.reads.push(Read {
                slot: slot.clone(),
                version: slot.version(),
                owner,
            });
        }
        Ok(())
    }
}

/// One evaluation of a form (standalone or rebuild).
pub(super) struct Attempt {
    pub(super) value: Result<Value, Failure>,
    pub(super) snap: Snapshot,
    pub(super) reads: Vec<Read>,
    pub(super) abort: Option<Abort>,
    pub(super) effects: Vec<StagedEffect>,
    pub(super) diags: Vec<Diagnostic>,
}

/// The top-level driver.
pub(super) struct DynamicManifestCacheEntry {
    pub(super) base: HostManifest,
    pub(super) controls: Rc<BTreeSet<Rc<str>>>,
    pub(super) manifest: Rc<HostManifest>,
}

pub struct Evaluator {
    pub(super) ns: Namespace,
    pub(super) graph: DepGraph,
    pub(super) vm: Vm,
    form_gen: u64,
    sink: Box<dyn EffectSink>,
    gate: Rc<RefCell<Gate>>,
    pub(super) dynamic_manifest_cache: RefCell<Vec<DynamicManifestCacheEntry>>,
    queued: Vec<(VarSlotRef, Value)>,
    last_pass: Option<PassReport>,
    last_migrations: Vec<(TweakId, TweakId)>,
}

impl std::fmt::Debug for Evaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Evaluator")
            .field("form_gen", &self.form_gen)
            .finish_non_exhaustive()
    }
}

/// True for an effect a rebuild may never replay (5.6 "Effects").
pub(super) fn one_shot(e: &StagedEffect) -> bool {
    matches!(
        e,
        StagedEffect::OneShot { .. }
            | StagedEffect::Console(_)
            | StagedEffect::Revoke(_)
            | StagedEffect::StopAll
            | StagedEffect::Cut
            | StagedEffect::PlaySong(_)
    )
}

pub(super) fn changed(old: &Value, new: &Value) -> bool {
    !deep_eq(old, new).unwrap_or(false)
}

fn def_targets(proto: &FnProto) -> Vec<VarSlotRef> {
    proto
        .defs
        .iter()
        .filter_map(|(g, _)| proto.globals.get(*g as usize).cloned())
        .collect()
}

impl Evaluator {
    /// An evaluator over `prelude` (the `load` native and the domain natives
    /// and prelude values are registered here), reading sources through
    /// `loader` and releasing effects to `sink`, with its own instrument
    /// registry.
    #[must_use]
    pub fn new(
        prelude: Prelude,
        loader: Box<dyn SourceLoader>,
        sink: Box<dyn EffectSink>,
    ) -> Evaluator {
        Evaluator::with_insts(prelude, loader, sink, InstRegistry::shared())
    }

    /// `new` over a shared instrument registry (the one the runtime
    /// resolves sounds with, 12.8.3). The prelude templates are realized
    /// into it here and their `Install` effects released to `sink`.
    #[must_use]
    pub fn with_insts(
        mut prelude: Prelude,
        loader: Box<dyn SourceLoader>,
        mut sink: Box<dyn EffectSink>,
        insts: Rc<RefCell<InstRegistry>>,
    ) -> Evaluator {
        register_load(&mut prelude);
        register_domain(&mut prelude);
        let gate = Rc::new(RefCell::new(Gate::default()));
        let mut vm = Vm::new();
        vm.set_host(Some(Box::new(LoaderHost(loader, Vec::new()))));
        vm.dsp.registry = Some(insts);
        let ns = Namespace::new(prelude);
        load_templates(&mut vm, ns.prelude(), sink.as_mut());
        vm.set_read_observer(Some(Box::new(GateObserver(Rc::clone(&gate)))));
        Evaluator {
            ns,
            graph: DepGraph::default(),
            vm,
            form_gen: 0,
            sink,
            gate,
            dynamic_manifest_cache: RefCell::new(Vec::new()),
            queued: Vec::new(),
            last_pass: None,
            last_migrations: Vec::new(),
        }
    }

    #[must_use]
    pub fn ns(&self) -> &Namespace {
        &self.ns
    }

    #[must_use]
    pub fn graph(&self) -> &DepGraph {
        &self.graph
    }

    /// The instrument registry (hand the same one to the runtime).
    #[must_use]
    pub fn insts(&self) -> Option<Rc<RefCell<InstRegistry>>> {
        self.vm.dsp.registry.clone()
    }

    /// The VM (limits are configurable here).
    pub fn vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }

    /// The VM and the namespace together (a `QueryVm` implementation or a
    /// test calls back into the VM against this session).
    pub fn vm_and_ns(&mut self) -> (&mut Vm, &Namespace) {
        (&mut self.vm, &self.ns)
    }

    /// Candidate-only slot roots. The caller constructs this evaluator fresh;
    /// this does not clone an active namespace or grant public mutation access.
    pub(crate) fn candidate_slots(&self) -> Vec<VarSlotRef> {
        let mut slots: Vec<_> = self
            .ns
            .session_names()
            .into_iter()
            .filter_map(|name| self.ns.session_slot(name))
            .collect();
        slots.extend(
            self.ns
                .prelude()
                .names()
                .filter_map(|name| self.ns.prelude().slot(name)),
        );
        slots
    }
    /// The report of the most recent reactive pass.
    #[must_use]
    pub fn last_pass(&self) -> Option<&PassReport> {
        self.last_pass.as_ref()
    }

    /// Takes tweak-site migrations accumulated since the previous take.
    pub fn take_migrations(&mut self) -> Vec<(TweakId, TweakId)> {
        std::mem::take(&mut self.last_migrations)
    }

    pub(super) fn next_gen(&mut self) -> FormGen {
        self.form_gen += 1;
        FormGen::new(self.form_gen)
    }

    /// Reads, expands and evaluates every top-level form of `text`.
    ///
    /// # Errors
    /// The first reader or expander error; nothing is evaluated then.
    pub fn eval_str(&mut self, text: &str, file: FileId) -> Result<Vec<FormOutcome>, Diagnostic> {
        let forms = read_forms(text, file)?;
        Ok(forms.iter().map(|f| self.eval_form(f)).collect())
    }

    /// Runs one expanded top-level form as a whole-form transaction and,
    /// when it wrote a top-level slot, the reactive pass it triggers. The
    /// form is checked first against the session; check diagnostics never
    /// gate compile or run (7.1.1).
    pub fn eval_form(&mut self, form: &Node) -> FormOutcome {
        let checked = check(
            std::slice::from_ref(form),
            &self.check_env_for(form),
            &self.dynamic_manifest(&HostManifest::spec_default()),
        );
        self.eval_form_checked(form, checked)
    }

    pub(super) fn eval_form_checked(&mut self, form: &Node, checked: CheckResult) -> FormOutcome {
        self.last_pass = None;
        let checked_inputs = (!checked.callables.is_empty())
            .then(|| self.ns.checked_inputs())
            .flatten();
        let gen = self.next_gen();
        let bad = if has_free_session_symbol(form) {
            self.durable_bad()
        } else {
            BTreeSet::new()
        };
        let migrate_from = self
            .graph
            .ids()
            .find_map(|id| {
                let previous = self.graph.get(id)?;
                (self.graph.is_current(id) && previous.node.span == form.span)
                    .then_some(previous.gen)
            })
            .or_else(|| {
                self.ns
                    .tweaks()
                    .borrow()
                    .iter()
                    .filter(|site| {
                        site.form_gen.get() < gen.get()
                            && site.span.file == form.span.file
                            && site.span.start >= form.span.start
                            && site.span.end <= form.span.end
                    })
                    .max_by_key(|site| site.form_gen.get())
                    .map(|site| site.form_gen)
            });
        let a = self.attempt(form, gen, migrate_from, &[], BTreeSet::new(), bad);
        let checked_callables = checked.callables;
        let mut diags = checked.diags;
        diags.extend(a.diags);
        let value = match a.value {
            Ok(v) if a.abort.is_none() => v,
            other => {
                let e = match (&a.abort, other) {
                    (Some(Abort::Blocked(s)), _) => Failure::new(
                        FailCode::Blocked,
                        format!("blocked on `{}`", name_of_sym(s.name())),
                    ),
                    (_, Err(e)) => e,
                    (_, Ok(_)) => Failure::new(FailCode::Blocked, "aborted"),
                };
                self.ns.tweaks().borrow_mut().retire(gen);
                return FormOutcome {
                    value: Err(e),
                    diags,
                    form_gen: gen,
                };
            }
        };
        let writes = a.snap.changed();
        let defined: Vec<VarSlotRef> = writes
            .iter()
            .filter(|s| s.slot.owner() == Some(gen))
            .map(|s| s.slot.clone())
            .collect();
        let upds: Vec<(VarSlotRef, Value)> = writes
            .iter()
            .filter(|s| s.slot.owner() != Some(gen))
            .map(|s| (s.slot.clone(), s.value.clone()))
            .collect();
        let mut members: Vec<WriteItem> = defined.iter().cloned().map(WriteItem::Name).collect();
        for e in &a.effects {
            if let StagedEffect::SlotBind { slot, .. } = e {
                if !members
                    .iter()
                    .any(|m| matches!(m, WriteItem::Bind(k) if k == slot))
                {
                    members.push(WriteItem::Bind(*slot));
                }
            }
        }
        let non_replayable = !upds.is_empty() || a.effects.iter().any(one_shot);
        let mut triggers: Vec<VarSlotRef> = Vec::new();
        let mut status: Vec<u64> = Vec::new();
        for s in &writes {
            if changed(&s.value, &s.slot.get()) || !s.bound {
                triggers.push(s.slot.clone());
            }
        }
        if !members.is_empty() {
            let own: BTreeSet<u64> = defined.iter().map(VarSlotRef::id).collect();
            let edges = a
                .reads
                .iter()
                .filter(|r| !own.contains(&r.slot.id()))
                .map(|r| Edge {
                    slot: r.slot.clone(),
                    version: r.version,
                })
                .collect();
            let mut rec = FormRec::new(Rc::new(form.clone()), gen, edges, members);
            rec.non_replayable = non_replayable;
            let f = self.graph.add(rec);
            for g in self.graph.claim(f) {
                if self.graph.get(g).is_some_and(|r| r.state.is_bad()) {
                    status.extend(self.graph.owned_slots(f));
                }
                self.refresh_tiers(g);
            }
            self.refresh_tiers(f);
        }
        let mut lead = a.effects;
        let mut names: Vec<SymId> = defined.iter().map(VarSlotRef::name).collect();
        for (slot, _) in &upds {
            lead.push(StagedEffect::CellUpdate {
                slot: slot.clone(),
                value: slot.get(),
            });
            names.push(slot.name());
        }
        if triggers.is_empty() && status.is_empty() {
            let _ = self.release(lead, &names);
        } else {
            let report = self.propagate(&triggers, &status, &[], lead, names);
            diags.extend(report.diags.iter().cloned());
            self.last_pass = Some(report);
        }
        if !diags
            .iter()
            .any(|d| d.severity == crate::types::diag::Severity::Error)
        {
            if let Some(inputs) = checked_inputs.as_ref() {
                self.ns
                    .install_checked_callables(&checked_callables, inputs, gen);
            }
        }
        FormOutcome {
            value: Ok(value),
            diags,
            form_gen: gen,
        }
    }

    /// The checker environment needed by one form. Forms without free
    /// session symbols cannot observe session globals, so avoid cloning the
    /// complete namespace for those independent declarations.
    pub(super) fn check_env_for(&self, form: &Node) -> crate::types::ty::CheckEnv {
        if has_free_session_symbol(form) {
            self.ns.check_env()
        } else {
            crate::types::ty::CheckEnv::empty()
        }
    }

    /// Queues a controller-rate `upd`; queued writes coalesce latest-wins
    /// per slot until `run_pass`.
    ///
    /// # Errors
    /// `undefined-name` or `upd-immutable` for a name that is not a `var`.
    pub fn queue_upd(&mut self, name: &str, value: Value) -> Result<(), Failure> {
        let slot = self
            .ns
            .session_slot(intern_sym(name))
            .filter(VarSlotRef::is_bound)
            .ok_or_else(|| {
                Failure::new(FailCode::UndefinedName, format!("`{name}` is not defined"))
            })?;
        if slot.kind() != crate::ns::namespace::SlotKind::Var {
            return Err(Failure::new(
                FailCode::UpdImmutable,
                format!("`{name}` is not a var"),
            ));
        }
        match self.queued.iter_mut().find(|(s, _)| s.same(&slot)) {
            Some(q) => q.1 = value,
            None => self.queued.push((slot, value)),
        }
        Ok(())
    }

    /// Applies the queued `upd`s (one write per slot, the latest value) and
    /// runs ONE reactive pass for all of them.
    pub fn run_pass(&mut self) -> PassReport {
        let queued = std::mem::take(&mut self.queued);
        let mut triggers = Vec::new();
        let mut lead = Vec::new();
        let mut names = Vec::new();
        for (slot, value) in queued {
            let old = slot.get();
            if self.ns.write_var(&slot, value.clone()).is_err() {
                continue;
            }
            lead.push(StagedEffect::CellUpdate {
                slot: slot.clone(),
                value: value.clone(),
            });
            names.push(slot.name());
            if changed(&old, &value) {
                triggers.push(slot);
            }
        }
        let report = self.propagate(&triggers, &[], &[], lead, names);
        self.last_pass = Some(report.clone());
        report
    }

    /// `set-tweak`: writes a site's slot. A `Reeval` site rebuilds its
    /// owning form (a binding site re-evaluates the binding's dependents);
    /// a `Direct` site is heard at the next late read; a `Manual` site only
    /// updates its slot.
    ///
    /// # Errors
    /// `undefined-name` for an unknown or stale site, `type` for a
    /// non-number.
    pub fn set_tweak(
        &mut self,
        id: TweakId,
        form_gen: FormGen,
        value: Value,
    ) -> Result<Option<PassReport>, Failure> {
        let site = {
            let table = self.ns.tweaks().borrow();
            let site = table.get(id).cloned();
            match site {
                Some(s) if s.form_gen == form_gen => {
                    table.write(id, value)?;
                    s
                }
                _ => {
                    return Err(Failure::new(
                        FailCode::UndefinedName,
                        "stale or unknown tweak site",
                    ))
                }
            }
        };
        let owner = self.graph.form_of_gen(site.form_gen);
        let tier = self.tier_of(&site);
        if tier == SiteTier::Manual {
            return Ok(None);
        }
        let forced: Vec<FormId> = match (tier, site.origin) {
            (SiteTier::Reeval, SiteOrigin::PatternLiteral | SiteOrigin::InstDefault) => {
                owner.into_iter().collect()
            }
            _ => Vec::new(),
        };
        let lead = vec![
            StagedEffect::CellUpdate {
                slot: site.slot.clone(),
                value: site.slot.get(),
            },
            StagedEffect::TweakRefresh(id),
        ];
        let names = match site.origin {
            SiteOrigin::Binding => vec![site.slot.name()],
            _ => Vec::new(),
        };
        let report = self.propagate(&[site.slot.clone()], &[], &forced, lead, names);
        self.last_pass = Some(report.clone());
        Ok(Some(report))
    }

    /// The effective tier of a site: `Manual` once its owner is
    /// non-replayable or no longer the current owner of its write set.
    #[must_use]
    pub fn site_tier(&self, id: TweakId) -> Option<SiteTier> {
        let site = self.ns.tweaks().borrow().get(id).cloned()?;
        Some(self.tier_of(&site))
    }

    fn tier_of(&self, site: &TweakSite) -> SiteTier {
        match self.graph.form_of_gen(site.form_gen) {
            Some(f) if !self.graph.eligible(f) => SiteTier::Manual,
            _ => site.tier,
        }
    }

    /// A superseded form's sites: a dead form's (it owns nothing any more)
    /// are retired; an ineligible or non-replayable form's report `manual`.
    pub(super) fn refresh_tiers(&mut self, f: FormId) {
        if self.graph.eligible(f) {
            return;
        }
        let Some(gen) = self.graph.get(f).map(|r| r.gen) else {
            return;
        };
        let mut t = self.ns.tweaks().borrow_mut();
        if self.graph.owns_any(f) {
            for s in t.sites_of(gen) {
                t.set_tier(s.id, SiteTier::Manual);
            }
        } else {
            t.retire(gen);
        }
    }

    /// The tweak sites of the form that owns `name` (current generation).
    #[must_use]
    pub fn sites_of(&self, name: &str) -> Vec<TweakSite> {
        self.form_of(name)
            .and_then(|f| self.graph.get(f))
            .map_or_else(Vec::new, |r| self.ns.tweaks().borrow().sites_of(r.gen))
    }

    /// The form that owns the session name `name`.
    #[must_use]
    pub fn form_of(&self, name: &str) -> Option<FormId> {
        let slot = self.ns.session_slot(intern_sym(name))?;
        self.graph.owner_of_slot(slot.id())
    }

    /// The form that owns the playing slot `key`.
    #[must_use]
    pub fn form_of_bind(&self, key: SlotKey) -> Option<FormId> {
        self.graph.owner_of_bind(key)
    }

    /// The state of the form that owns `name`.
    #[must_use]
    pub fn form_state(&self, name: &str) -> Option<FormState> {
        self.form_of(name)
            .and_then(|f| self.graph.get(f))
            .map(|r| r.state.clone())
    }

    fn names_of(slots: &[VarSlotRef]) -> Vec<String> {
        let mut v: Vec<String> = slots
            .iter()
            .map(|s| name_of_sym(s.name()).to_string())
            .collect();
        v.sort();
        v
    }

    /// The committed eager-read edges of the form that owns `name`, sorted.
    #[must_use]
    pub fn edges(&self, name: &str) -> Vec<String> {
        let slots: Vec<VarSlotRef> = self
            .form_of(name)
            .and_then(|f| self.graph.get(f))
            .map_or_else(Vec::new, |r| {
                r.edges.iter().map(|e| e.slot.clone()).collect()
            });
        Self::names_of(&slots)
    }

    /// The attempt edge set of the form that owns `name`, sorted.
    #[must_use]
    pub fn attempt_edges(&self, name: &str) -> Vec<String> {
        let slots = self
            .form_of(name)
            .and_then(|f| self.graph.get(f))
            .map_or_else(Vec::new, |r| r.attempt.clone());
        Self::names_of(&slots)
    }

    /// The recovery subscriptions of the form that owns `name`, sorted.
    #[must_use]
    pub fn subscriptions(&self, name: &str) -> Vec<String> {
        let slots = self
            .form_of(name)
            .and_then(|f| self.graph.get(f))
            .map_or_else(Vec::new, |r| r.subs.clone());
        Self::names_of(&slots)
    }

    /// How many times the form that owns `name` has run.
    #[must_use]
    pub fn runs(&self, name: &str) -> u64 {
        self.form_of(name)
            .and_then(|f| self.graph.get(f))
            .map_or(0, |r| r.runs)
    }

    pub(super) fn durable_bad(&self) -> BTreeSet<FormId> {
        self.graph
            .ids()
            .filter(|f| self.graph.get(*f).is_some_and(|r| r.state.is_bad()))
            .collect()
    }

    /// Compiles and runs `form` once with the gate armed. An unsuccessful
    /// evaluation (abort or failure) rolls back its namespace writes and
    /// drops its effects before returning.
    pub(super) fn attempt(
        &mut self,
        form: &Node,
        gen: FormGen,
        migrate_from: Option<FormGen>,
        own: &[u64],
        dirty: BTreeSet<FormId>,
        bad: BTreeSet<FormId>,
    ) -> Attempt {
        let mut cx = CompileCx::new(&self.ns, gen);
        cx.custom_controls = self.dynamic_custom_controls();
        let compiled = compile(form, &mut cx);
        let mut diags = std::mem::take(&mut cx.diags);
        let sites = std::mem::take(&mut cx.sites);
        let proto = match compiled {
            Ok(p) => p,
            Err(d) => {
                let e = Failure::new(FailCode::DepthExceeded, d.message.clone());
                diags.push(d);
                return Attempt {
                    value: Err(e),
                    snap: Snapshot::default(),
                    reads: Vec::new(),
                    abort: None,
                    effects: Vec::new(),
                    diags,
                };
            }
        };
        if let Some(old) = migrate_from {
            self.migrate(old, &sites);
        }
        let targets = def_targets(&proto);
        let bounded_snapshot = proto.code.iter().all(|op| {
            matches!(
                op,
                Op::LoadConst(_) | Op::DefGlobal(_) | Op::Force | Op::Deref | Op::Ret
            )
        }) && proto
            .globals
            .iter()
            .all(|global| targets.iter().any(|target| target.same(global)));
        let snap = if bounded_snapshot {
            Snapshot::take_slots(&targets)
        } else {
            Snapshot::take(&self.ns, &targets)
        };
        {
            let mut g = self.gate.borrow_mut();
            g.active = true;
            g.owners = if bounded_snapshot {
                BTreeMap::new()
            } else {
                self.graph.name_owners().clone()
            };
            g.own = targets
                .iter()
                .map(VarSlotRef::id)
                .chain(own.iter().copied())
                .collect();
            g.dirty = dirty;
            g.bad = bad;
            g.reads.clear();
            g.abort = None;
        }
        let mut value = self.vm.run(proto, &self.ns);
        // An `inst` definition is realized with the gate still armed, so the
        // body's reads are the form's edges (12.8.6).
        if let (Ok(_), Some(slot)) = (&value, inst_target(form, &targets)) {
            value = realize_inst(&mut self.vm, &self.ns, &slot, form.span).map_err(|e| {
                diags.extend(e.diag.map(|d| *d));
                e.failure
            });
        }
        diags.append(&mut self.vm.dsp.diags);
        let (reads, abort) = {
            let mut g = self.gate.borrow_mut();
            g.active = false;
            (std::mem::take(&mut g.reads), g.abort.take())
        };
        let effects = self.vm.effects_mut().take();
        diags.extend(take_load_diags(&mut self.vm));
        if value.is_ok() && abort.is_none() {
            if let Some((d, e)) = reject_input_lanes(&effects, form.span) {
                diags.push(d);
                value = Err(e);
            }
        }
        if value.is_err() || abort.is_some() {
            snap.rollback(&self.ns);
            return Attempt {
                value,
                snap,
                reads,
                abort,
                effects: Vec::new(),
                diags,
            };
        }
        Attempt {
            value,
            snap,
            reads,
            abort,
            effects,
            diags,
        }
    }

    /// Override migration (section 13): a new site matched by index, type
    /// and origin to a site of the previous generation whose slot carries
    /// a controller override starts from the overridden value.
    fn migrate(&mut self, old: FormGen, new: &[TweakSite]) {
        let prev = self.ns.tweaks().borrow().sites_of(old);
        for n in new {
            let hit = prev.iter().find(|p| {
                p.origin == n.origin
                    && if n.origin == SiteOrigin::Binding {
                        p.slot.same(&n.slot) || p.slot.name() == n.slot.name()
                    } else {
                        p.index == n.index && p.ty == n.ty
                    }
            });
            if let Some(p) = hit {
                self.last_migrations.push((p.id, n.id));
                if n.origin != SiteOrigin::Binding {
                    let cur = p.slot.base();
                    if changed(&p.initial, &cur) {
                        n.slot.set(cur);
                    }
                }
            }
        }
    }

    /// Releases `lead` effects and, when names changed, one `bindings`
    /// batch (a form that triggered no pass).
    pub(super) fn release(
        &mut self,
        lead: Vec<StagedEffect>,
        names: &[SymId],
    ) -> Vec<(SymId, Value)> {
        for e in lead {
            self.sink.apply(e);
        }
        let batch = self.batch(names);
        if !batch.is_empty() {
            self.sink.apply(StagedEffect::Bindings(batch.clone()));
        }
        batch
    }

    pub(super) fn batch(&self, names: &[SymId]) -> Vec<(SymId, Value)> {
        let mut out: Vec<(SymId, Value)> = Vec::new();
        for n in names {
            if out.iter().any(|(m, _)| m == n) {
                continue;
            }
            if let Some(s) = self.ns.session_slot(*n).filter(VarSlotRef::is_bound) {
                out.push((*n, s.get()));
            }
        }
        out
    }
}
