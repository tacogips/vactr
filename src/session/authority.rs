//! Write authority (design 14.4, 14.5.6 rules 1-7) and write coalescing.
//!
//! A write is validated when it arrives AND again when the coalesced write
//! is applied at the tick:
//! 1. `set-tweak`: the site's `form_gen` must be the message's
//!    (`stale-form-gen`);
//! 2. the site or the definition must not be edit-invalidated;
//! 3. the write's `edit_epoch` must not be newer than the document's
//!    reconciled epoch (`unreconciled-edit`);
//! 4. `set-var`: the name's CURRENT defining form must have
//!    `defining_form_gen` (`superseded-definition`);
//! 5. `doc-changed` on the stored revision maps every stored span forward
//!    and invalidates touched ones and those intersecting a dirty span;
//! 6. `doc-changed` on another revision invalidates the whole file;
//! 7. an eval clears invalidation for the forms it rebuilds (fresh sites).
//!
//! Accepted writes coalesce latest-wins per target until the tick, which
//! runs every `set-var` as `queue_upd` plus ONE `run_pass`, then each
//! `set-tweak` through `Evaluator::set_tweak`, publishing every pass.

use std::collections::BTreeMap;

use crate::directives::key::{BindingIdent, BindingKey, KeyTable};
use crate::directives::persist::{BindingEntry, Midi};
use crate::directives::writeback::{learn_edit, LearnError};
use crate::directives::DirectiveTable;
use crate::ns::namespace::{FormGen, SlotKind};
use crate::ns::tweak::TweakId;
use crate::reader::span::FileId;
use crate::session::changes::{Change, ChangeSet, Mapped};
use crate::session::eval::site_keys;
use crate::session::protocol::{
    DirectiveEditBody, DocChangedBody, LearnBody, LearnTarget, ServerMsg, SetTweakBody, SetVarBody,
    StaleBindingBody, StaleReason, StaleTarget, WireNum, WireSpan, WireValue,
};
use crate::session::session::{bad_body, DefAuth, DocState, PersistenceMode, Session, SiteAuth};
use crate::value::intern::{intern_sym, SymId};
use crate::value::num::NumKind;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

/// A write rejected by rule `reason`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Stale {
    pub reason: StaleReason,
    pub current_form_gen: Option<u64>,
}

impl Stale {
    fn new(reason: StaleReason) -> Stale {
        Stale {
            reason,
            current_form_gen: None,
        }
    }
}

/// A coalesced `set-var`.
#[derive(Clone, Debug)]
struct VarWrite {
    conn: u32,
    file: FileId,
    name: String,
    value: WireValue,
    defining_form_gen: u64,
    epoch: u64,
}

/// A coalesced `set-tweak`.
#[derive(Clone, Debug)]
struct TweakWrite {
    conn: u32,
    file: FileId,
    id: TweakId,
    form_gen: u64,
    value: WireNum,
    epoch: u64,
}

/// Accepted writes, latest-wins per target, until the next tick.
#[derive(Debug, Default)]
pub struct PendingWrites {
    vars: Vec<VarWrite>,
    tweaks: Vec<TweakWrite>,
}

impl PendingWrites {
    /// Writes waiting for the tick.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vars.len() + self.tweaks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn put_var(&mut self, w: VarWrite) {
        match self.vars.iter_mut().find(|v| v.name == w.name) {
            Some(v) => *v = w,
            None => self.vars.push(w),
        }
    }

    fn put_tweak(&mut self, w: TweakWrite) {
        match self.tweaks.iter_mut().find(|t| t.id == w.id) {
            Some(t) => *t = w,
            None => self.tweaks.push(w),
        }
    }

    /// Drops the writes of a closed connection.
    pub fn forget(&mut self, conn: u32) {
        self.vars.retain(|v| v.conn != conn);
        self.tweaks.retain(|t| t.conn != conn);
    }
}

/// Rule 3.
fn check_epoch(doc: Option<&DocState>, epoch: u64) -> Result<(), Stale> {
    let reconciled = doc.map_or(0, |d| d.epoch_reconciled);
    if epoch > reconciled {
        return Err(Stale::new(StaleReason::UnreconciledEdit));
    }
    Ok(())
}

/// Rules 1-3 for a `set-tweak` against the site's current state.
///
/// # Errors
/// The first rule the write breaks.
pub fn check_tweak(
    doc: Option<&DocState>,
    current: Option<FormGen>,
    id: TweakId,
    form_gen: u64,
    epoch: u64,
) -> Result<(), Stale> {
    let Some(cur) = current else {
        return Err(Stale::new(StaleReason::StaleFormGen));
    };
    if cur.get() != form_gen {
        return Err(Stale {
            reason: StaleReason::StaleFormGen,
            current_form_gen: Some(cur.get()),
        });
    }
    if doc
        .and_then(|d| d.sites.get(&id))
        .is_some_and(|s| s.invalid)
    {
        return Err(Stale::new(StaleReason::EditInvalidated));
    }
    check_epoch(doc, epoch)
}

/// Rules 2-4 for a `set-var`.
///
/// # Errors
/// The first rule the write breaks.
pub fn check_var(
    doc: Option<&DocState>,
    current: Option<FormGen>,
    name: &str,
    defining_form_gen: u64,
    epoch: u64,
) -> Result<(), Stale> {
    let Some(cur) = current.filter(|g| g.get() == defining_form_gen) else {
        return Err(Stale {
            reason: StaleReason::SupersededDefinition,
            current_form_gen: current.map(FormGen::get),
        });
    };
    let sym = intern_sym(name);
    if doc
        .and_then(|d| d.defs.get(&sym))
        .is_some_and(|d| d.invalid && d.form_gen == cur)
    {
        return Err(Stale::new(StaleReason::EditInvalidated));
    }
    check_epoch(doc, epoch)
}

/// True when the closed or half-open ranges overlap (an empty dirty span
/// intersects a span strictly containing it).
fn intersects(span: (u32, u32), dirty: &WireSpan) -> bool {
    if dirty.start == dirty.end {
        span.0 < dirty.start && dirty.start < span.1
    } else {
        span.0 < dirty.end && dirty.start < span.1
    }
}

/// Rule 5 for one stored span.
fn remap(span: (u32, u32), cs: &ChangeSet, dirty: &[WireSpan]) -> Option<(u32, u32)> {
    match cs.map_span(span.0, span.1) {
        Mapped::Moved { start, end } if !dirty.iter().any(|d| intersects((start, end), d)) => {
            Some((start, end))
        }
        _ => None,
    }
}

/// `later` after `first`, as ONE change set over `first`'s base. Each
/// region either set touches becomes one replacement; regions that touch
/// merge, so the result is never narrower than the true composition.
#[must_use]
pub fn compose_sets(first: &ChangeSet, later: &ChangeSet) -> ChangeSet {
    let a = first.changes();
    // Maps a middle-revision offset back to the base: inside an inserted
    // region it widens to the replaced base range.
    let back = |p: u32, hi: bool| -> u32 {
        let mut delta: i64 = 0;
        for c in a {
            let start = i64::from(c.from) + delta;
            let end = start + i64::from(c.insert_len);
            let p64 = i64::from(p);
            if p64 < start {
                break;
            }
            if p64 <= end {
                return if hi { c.to } else { c.from };
            }
            delta += c.delta();
        }
        u32::try_from(i64::from(p) - delta).unwrap_or(0)
    };
    // (base from, base to, delta)
    let mut regions: Vec<(u32, u32, i64)> = a.iter().map(|c| (c.from, c.to, c.delta())).collect();
    for c in later.changes() {
        regions.push((back(c.from, false), back(c.to, true), c.delta()));
    }
    regions.sort_by_key(|r| (r.0, r.1));
    let mut merged: Vec<(u32, u32, i64)> = Vec::new();
    for (f, t, d) in regions {
        match merged.last_mut() {
            Some(m) if f <= m.1 => {
                m.1 = m.1.max(t);
                m.2 += d;
            }
            _ => merged.push((f, t, d)),
        }
    }
    let changes = merged
        .into_iter()
        .map(|(f, t, d)| Change {
            from: f,
            to: t,
            insert_len: u32::try_from((i64::from(t) - i64::from(f) + d).max(0)).unwrap_or(0),
        })
        .collect();
    ChangeSet::new(changes).unwrap_or_default()
}

/// A `set-var` value coerced to the var's current type.
fn coerce_var(current: &Value, w: WireValue) -> Option<Value> {
    #[allow(clippy::cast_precision_loss)]
    let x = match w {
        WireValue::Bool(b) => {
            return matches!(current, Value::Bool(_)).then_some(Value::Bool(b));
        }
        WireValue::Int(n) => n as f64,
        WireValue::Float(x) => x,
    };
    coerce_num(NumKind::of(current)?, x, w)
}

/// A number coerced to `kind` (an int kind rounds).
fn coerce_num(kind: NumKind, x: f64, w: WireValue) -> Option<Value> {
    if !x.is_finite() {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    Some(match kind {
        NumKind::Int => Value::Int(i32::try_from(x.round() as i64).ok()?),
        NumKind::Int64 => Value::Int64(x.round() as i64),
        NumKind::Float => Value::Float(x as f32),
        NumKind::Float64 => Value::Float64(x),
        NumKind::Ratio => match w {
            WireValue::Int(n) => Value::Ratio(Ratio64::from_int(n)),
            _ => Value::Ratio(Ratio64::from_f64_exact(x)?),
        },
    })
}

fn stale_msg(target: StaleTarget, s: Stale) -> ServerMsg {
    ServerMsg::StaleBinding(StaleBindingBody {
        target,
        reason: s.reason,
        current_form_gen: s.current_form_gen,
    })
}

impl Session {
    fn current_site_gen(&self, id: TweakId) -> Option<FormGen> {
        self.ev.ns().tweaks().borrow().get(id).map(|s| s.form_gen)
    }

    fn current_def_gen(&self, name: &str) -> Option<FormGen> {
        let f = self.ev.form_of(name)?;
        self.ev.graph().get(f).map(|r| r.gen)
    }

    fn validate_tweak(&self, w: &TweakWrite) -> Result<(), Stale> {
        let doc = self.docs.get(&w.file);
        check_tweak(doc, self.current_site_gen(w.id), w.id, w.form_gen, w.epoch)
    }

    fn validate_var(&self, w: &VarWrite) -> Result<(), Stale> {
        let doc = self.docs.get(&w.file);
        check_var(
            doc,
            self.current_def_gen(&w.name),
            &w.name,
            w.defining_form_gen,
            w.epoch,
        )
    }

    /// `set-tweak` arriving: validated now, stored latest-wins.
    pub(super) fn on_set_tweak(&mut self, conn: u32, _seq: u64, b: SetTweakBody) -> Vec<ServerMsg> {
        let Some(file) = self.lookup_file(&b.file) else {
            return vec![stale_msg(
                StaleTarget::Id(b.id),
                Stale::new(StaleReason::StaleFormGen),
            )];
        };
        let w = TweakWrite {
            conn,
            file,
            id: TweakId::new(b.id),
            form_gen: b.form_gen,
            value: b.value,
            epoch: b.edit_epoch,
        };
        if let Err(s) = self.validate_tweak(&w) {
            return vec![stale_msg(StaleTarget::Id(b.id), s)];
        }
        self.pending.put_tweak(w);
        Vec::new()
    }

    /// `set-var` arriving: validated now, stored latest-wins.
    pub(super) fn on_set_var(&mut self, conn: u32, _seq: u64, b: SetVarBody) -> Vec<ServerMsg> {
        let file = self.file_id(&b.file);
        let w = VarWrite {
            conn,
            file,
            name: b.name.clone(),
            value: b.value,
            defining_form_gen: b.defining_form_gen,
            epoch: b.edit_epoch,
        };
        if let Err(s) = self.validate_var(&w) {
            return vec![stale_msg(StaleTarget::Name(b.name), s)];
        }
        let is_var = self
            .ev
            .ns()
            .session_slot(intern_sym(&b.name))
            .is_some_and(|s| s.kind() == SlotKind::Var);
        if !is_var {
            return vec![bad_body(format!("`{}` is not a var", b.name))];
        }
        self.pending.put_var(w);
        Vec::new()
    }

    /// The tick's first step: every pending write re-validated, then the
    /// set-vars as ONE pass and each set-tweak as its own. Returns the
    /// replies (with their connection) and the batches, in order.
    pub(super) fn apply_pending(&mut self) -> Vec<(Option<u32>, ServerMsg)> {
        let mut out = Vec::new();
        if self.pending.is_empty() {
            return out;
        }
        let vars = std::mem::take(&mut self.pending.vars);
        let tweaks = std::mem::take(&mut self.pending.tweaks);
        let mut queued = false;
        for w in vars {
            if let Err(s) = self.validate_var(&w) {
                out.push((Some(w.conn), stale_msg(StaleTarget::Name(w.name), s)));
                continue;
            }
            let current = self.ev.ns().session_value(&w.name).unwrap_or(Value::Nil);
            let Some(v) = coerce_var(&current, w.value) else {
                out.push((
                    Some(w.conn),
                    bad_body(format!("a bad value for `{}`", w.name)),
                ));
                continue;
            };
            match self.ev.queue_upd(&w.name, v) {
                Ok(()) => queued = true,
                Err(e) => out.push((Some(w.conn), bad_body(e.to_string()))),
            }
        }
        if queued {
            let report = self.ev.run_pass();
            if let Some(b) = self.publish_pass(&report) {
                out.push((None, b));
            }
        }
        for w in tweaks {
            if let Err(s) = self.validate_tweak(&w) {
                out.push((Some(w.conn), stale_msg(StaleTarget::Id(w.id.get()), s)));
                continue;
            }
            let ty = self.ev.ns().tweaks().borrow().get(w.id).map(|s| s.ty);
            let wv = match w.value {
                WireNum::Int(n) => WireValue::Int(n),
                WireNum::Float(x) => WireValue::Float(x),
            };
            let Some(v) = ty.and_then(|k| coerce_num(k, w.value.as_f64(), wv)) else {
                out.push((Some(w.conn), bad_body("a bad tweak value")));
                continue;
            };
            match self.ev.set_tweak(w.id, FormGen::new(w.form_gen), v) {
                Ok(Some(report)) => {
                    if let Some(b) = self.publish_pass(&report) {
                        out.push((None, b));
                    }
                }
                Ok(None) => {}
                Err(_) => out.push((
                    Some(w.conn),
                    stale_msg(
                        StaleTarget::Id(w.id.get()),
                        Stale {
                            reason: StaleReason::StaleFormGen,
                            current_form_gen: self.current_site_gen(w.id).map(FormGen::get),
                        },
                    ),
                )),
            }
        }
        self.refresh_all_auth();
        out
    }

    /// `doc-changed` (rules 5 and 6).
    pub(super) fn on_doc_changed(&mut self, b: &DocChangedBody) -> Vec<ServerMsg> {
        let cs = match ChangeSet::new(b.changes.clone()) {
            Ok(cs) => cs,
            Err(e) => return vec![bad_body(format!("changes: {e}"))],
        };
        let fid = self.file_id(&b.file);
        let doc = self.docs.entry(fid).or_insert_with(|| DocState::new(fid));
        doc.epoch_reconciled = doc.epoch_reconciled.max(b.edit_epoch);
        if b.base_revision == doc.rev {
            for s in doc.sites.values_mut() {
                match remap(s.span, &cs, &b.dirty) {
                    Some(sp) => s.span = sp,
                    None => s.invalid = true,
                }
            }
            for d in doc.defs.values_mut() {
                match remap(d.span, &cs, &b.dirty) {
                    Some(sp) => d.span = sp,
                    None => d.invalid = true,
                }
            }
            doc.since_eval = doc.since_eval.as_ref().map(|prev| compose_sets(prev, &cs));
        } else {
            for s in doc.sites.values_mut() {
                s.invalid = true;
            }
            for d in doc.defs.values_mut() {
                d.invalid = true;
            }
            doc.since_eval = None;
        }
        doc.rev = b.doc_revision;
        Vec::new()
    }

    /// Before an eval's forms run: the text, revision, reconciled epoch,
    /// directive table and key migration. When the stored authority spans
    /// are not in this revision, every existing site and definition is
    /// invalidated (the rebuilt ones come back fresh).
    pub(super) fn prepare_doc(
        &mut self,
        fid: FileId,
        src: &str,
        rev: u64,
        epoch: u64,
        table: DirectiveTable,
    ) {
        let doc = self.docs.entry(fid).or_insert_with(|| DocState::new(fid));
        doc.epoch_reconciled = doc.epoch_reconciled.max(epoch);
        let fresh = doc.text.is_empty() && doc.keys.iter().next().is_none();
        let keys = match (&doc.since_eval, fresh) {
            (_, true) => KeyTable::from_table(&table, rev),
            (Some(cs), false) if doc.rev == rev => {
                let mut k = doc.keys.clone();
                k.migrate(cs, &table);
                k
            }
            _ if doc.text == src => {
                let mut k = doc.keys.clone();
                k.migrate(&ChangeSet::default(), &table);
                k
            }
            _ => KeyTable::from_table(&table, rev),
        };
        if doc.rev != rev && doc.text != src {
            for s in doc.sites.values_mut() {
                s.invalid = true;
            }
            for d in doc.defs.values_mut() {
                d.invalid = true;
            }
        }
        doc.keys = keys;
        doc.directives = table;
        doc.text = src.to_string();
        doc.text_rev = rev;
        doc.rev = rev;
        doc.since_eval = Some(ChangeSet::default());
    }

    /// Rule 7 and the site table: every current site and definition of the
    /// file; a new generation gets a fresh, valid entry at its source span
    /// (mapped through the edits since the eval), an unchanged one keeps
    /// its mapped span and invalidation.
    pub(super) fn refresh_auth(&mut self, fid: FileId) {
        let sites: Vec<(TweakId, (u32, u32), FormGen)> = self
            .ev
            .ns()
            .tweaks()
            .borrow()
            .iter()
            .filter(|s| s.span.file == fid)
            .map(|s| (s.id, (s.span.start, s.span.end), s.form_gen))
            .collect();
        let g = self.ev.graph();
        let defs: Vec<(SymId, (u32, u32), FormGen)> = g
            .ids()
            .filter_map(|f| g.get(f).map(|r| (f, r)))
            .filter(|(_, r)| r.node.span.file == fid)
            .flat_map(|(f, r)| {
                let span = (r.node.span.start, r.node.span.end);
                r.names()
                    .filter(|s| g.owner_of_slot(s.id()) == Some(f))
                    .map(|s| (s.name(), span, r.gen))
                    .collect::<Vec<_>>()
            })
            .collect();
        let Some(doc) = self.docs.get_mut(&fid) else {
            return;
        };
        let since = doc.since_eval.clone();
        let place = |span: (u32, u32)| match &since {
            Some(cs) => match cs.map_span(span.0, span.1) {
                Mapped::Moved { start, end } => (Some((start, end)), false),
                Mapped::Touched => (None, true),
            },
            None => (None, true),
        };
        let mut new_sites = BTreeMap::new();
        for (id, span, gen) in sites {
            let entry = match doc.sites.get(&id) {
                Some(old) if old.form_gen == gen => *old,
                _ => {
                    let (sp, invalid) = place(span);
                    SiteAuth {
                        span: sp.unwrap_or(span),
                        form_gen: gen,
                        invalid,
                    }
                }
            };
            new_sites.insert(id, entry);
        }
        doc.sites = new_sites;
        let mut new_defs = BTreeMap::new();
        for (sym, span, gen) in defs {
            let entry = match doc.defs.get(&sym) {
                Some(old) if old.form_gen == gen => *old,
                _ => {
                    let (sp, invalid) = place(span);
                    DefAuth {
                        span: sp.unwrap_or(span),
                        form_gen: gen,
                        invalid,
                    }
                }
            };
            new_defs.insert(sym, entry);
        }
        doc.defs = new_defs;
    }

    /// `refresh_auth` for every open document (after passes that may have
    /// rebuilt forms).
    pub(super) fn refresh_all_auth(&mut self) {
        let files: Vec<FileId> = self.docs.keys().copied().collect();
        for f in files {
            self.refresh_auth(f);
        }
    }

    /// The binding identity a `learn` names.
    fn learn_ident(&self, doc: &DocState, target: &LearnTarget) -> Option<BindingIdent> {
        match target {
            LearnTarget::Key(k) => k.parse::<BindingKey>().ok().map(BindingIdent::Key),
            LearnTarget::Id(id) => {
                let site = self
                    .ev
                    .ns()
                    .tweaks()
                    .borrow()
                    .get(TweakId::new(*id))
                    .cloned()?;
                if let Some(key) =
                    site_keys(&doc.directives, std::slice::from_ref(&site)).remove(&site.id)
                {
                    return key.parse::<BindingKey>().ok().map(BindingIdent::Key);
                }
                // Positional: the call site whose literals include this one.
                let heads = &doc.directives.doc.sites;
                let pos = heads.iter().rposition(|c| c.head.end <= site.span.start)?;
                let head = &heads[pos];
                let end = heads.get(pos + 1).map_or(u32::MAX, |c| c.head.start);
                let mut lits: Vec<(u32, TweakId)> = self
                    .ev
                    .ns()
                    .tweaks()
                    .borrow()
                    .iter()
                    .filter(|s| {
                        s.span.file == doc.file
                            && s.span.start >= head.head.end
                            && s.span.start < end
                    })
                    .map(|s| (s.span.start, s.id))
                    .collect();
                lits.sort_unstable();
                let k = lits.iter().position(|(_, i)| *i == site.id)?;
                let param = head.params.get(k)?;
                Some(BindingIdent::positional(head.head, param))
            }
        }
    }

    /// `learn` (13.5 write-back, 14.5.8): rule 3, then the minimal
    /// directive edit (Directive mode) or an update of the in-memory
    /// binding set (ExternalFile mode, no reply).
    pub(super) fn on_learn(&mut self, b: &LearnBody) -> Vec<ServerMsg> {
        let target = match &b.binding {
            LearnTarget::Id(id) => StaleTarget::Id(*id),
            LearnTarget::Key(k) => StaleTarget::Name(k.clone()),
        };
        let Some(fid) = self.lookup_file(&b.file) else {
            return vec![bad_body(format!("`{}` was never evaluated", b.file))];
        };
        let Some(doc) = self.docs.get(&fid) else {
            return vec![bad_body(format!("`{}` was never evaluated", b.file))];
        };
        if let Err(s) = check_epoch(Some(doc), b.edit_epoch) {
            return vec![stale_msg(target, s)];
        }
        if doc.rev != doc.text_rev {
            // Edited since the last eval: the stored text is not current.
            return vec![stale_msg(target, Stale::new(StaleReason::EditInvalidated))];
        }
        let Some(ident) = self.learn_ident(doc, &b.binding) else {
            return vec![bad_body("the binding does not resolve in this revision")];
        };
        if b.cc > 127 || b.ch.is_some_and(|c| !(1..=16).contains(&c)) {
            return vec![bad_body("cc is 0..127 and ch is 1..16")];
        }
        match self.persistence {
            PersistenceMode::Directive => {
                match learn_edit(&doc.text, &doc.directives, &doc.keys, &ident, b.cc, b.ch) {
                    Ok(edit) => vec![ServerMsg::DirectiveEdit(DirectiveEditBody {
                        file: b.file.clone(),
                        doc_revision: doc.text_rev,
                        span: WireSpan::new(edit.span.0, edit.span.1),
                        expected: edit.expected,
                        text: edit.text,
                    })],
                    Err(LearnError::Stale) => {
                        vec![stale_msg(target, Stale::new(StaleReason::EditInvalidated))]
                    }
                    Err(e) => vec![bad_body(e.to_string())],
                }
            }
            PersistenceMode::ExternalFile => {
                if let BindingIdent::Key(k) = &ident {
                    if doc.keys.state(k) == Some(crate::directives::key::KeyState::Stale) {
                        return vec![stale_msg(target, Stale::new(StaleReason::EditInvalidated))];
                    }
                }
                let midi = Some(Midi { cc: b.cc, ch: b.ch });
                if let Some(doc) = self.docs.get_mut(&fid) {
                    match doc.bindings.entries.iter_mut().find(|e| e.key == ident) {
                        Some(e) => {
                            e.midi = midi;
                            e.panel = true;
                        }
                        None => doc.bindings.entries.push(BindingEntry {
                            key: ident,
                            panel: true,
                            midi,
                            overlay: None,
                        }),
                    }
                }
                Vec::new()
            }
        }
    }
}
