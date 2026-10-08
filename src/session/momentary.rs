//! Bounded session-side overrides for live performance controls.

use crate::dsp::cells::CellId;
use crate::dsp::controls::{row_by_id, ScalarType};
use crate::ns::namespace::{FormGen, VarSlotRef};
use crate::ns::stage::StagedEffect;
use crate::ns::tweak::{NumTy, SiteOrigin, SiteTier, TweakId, TweakSite};
use crate::reader::span::FileId;
use crate::sched::cells::CellMap;
use crate::session::authority::check_tweak;
use crate::session::protocol::{
    MomentaryBody, ServerMsg, StaleBindingBody, StaleReason, StaleTarget, WireNum,
};
use crate::session::session::Session;
use crate::value::num::NumKind;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

/// Maximum simultaneous momentary sites.
pub const MAX_MOMENTARY: usize = 16;
/// Shortest allowed audio ramp, seconds.
pub const MIN_RAMP_S: f64 = 0.005;
/// Evaluator-side override refresh interval, seconds.
pub const EVAL_PUSH_S: f64 = 0.016;
/// Maximum accepted glide duration, milliseconds.
pub const MAX_RAMP_MS: u32 = 10_000;

#[derive(Clone, Copy, Debug)]
enum Target {
    Value(f64),
    Base,
}

#[derive(Clone, Copy, Debug)]
struct Ramp {
    from: f64,
    to: Target,
    start: f64,
    dur: f64,
}

impl Ramp {
    fn value(self, now: f64, base: f64) -> f64 {
        let to = match self.to {
            Target::Value(value) => value,
            Target::Base => base,
        };
        let t = ((now - self.start) / self.dur).clamp(0.0, 1.0);
        self.from + (to - self.from) * t
    }

    fn end(self) -> f64 {
        self.start + self.dur
    }
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    id: CellId,
    epoch: u32,
    base: f32,
    target: f32,
    acked: bool,
    stepped: bool,
}

#[derive(Debug)]
struct Entry {
    file: FileId,
    id: TweakId,
    alias: Option<TweakId>,
    slot: VarSlotRef,
    index: u32,
    ty: NumTy,
    origin: SiteOrigin,
    form_gen: FormGen,
    ramp: Ramp,
    releasing: bool,
    last_push: f64,
    version: u64,
    cells: Vec<Cell>,
}

/// Session-local momentary state, bounded by `MAX_MOMENTARY`.
#[derive(Debug, Default)]
pub struct MomentaryTable {
    entries: Vec<Entry>,
    last_cuts: u64,
    stale_drops: u64,
}

impl MomentaryTable {
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Session {
    pub(super) fn on_momentary(
        &mut self,
        _conn: u32,
        _seq: u64,
        body: MomentaryBody,
    ) -> Vec<ServerMsg> {
        let id = TweakId::new(body.id);
        let Some(file) = self.lookup_file(&body.file) else {
            return Vec::new();
        };
        if body.target.is_none() {
            self.release_momentary(file, id, body.ramp_ms);
            return Vec::new();
        }

        let site = self.ev.ns().tweaks().borrow().get(id).cloned();
        let current = site.as_ref().map(|site| site.form_gen);
        let valid = check_tweak(
            self.docs.get(&file),
            current,
            id,
            body.form_gen,
            body.edit_epoch,
        );
        if valid.is_err() {
            self.momentary.stale_drops = self.momentary.stale_drops.saturating_add(1);
            return Vec::new();
        }
        let Some(site) = site else {
            self.momentary.stale_drops = self.momentary.stale_drops.saturating_add(1);
            return Vec::new();
        };
        if site.tier != SiteTier::Direct
            || !matches!(
                site.ty,
                NumKind::Int | NumKind::Int64 | NumKind::Float | NumKind::Float64 | NumKind::Ratio
            )
        {
            return vec![momentary_stale(id, StaleReason::MomentaryIneligible)];
        }
        let Some(target) = body.target.and_then(|n| coerce_target(site.ty, n)) else {
            return vec![crate::session::session::bad_body("a bad momentary target")];
        };
        let existing = self
            .momentary
            .entries
            .iter()
            .position(|entry| entry.file == file && entry.id == id);
        if existing.is_none() && self.momentary.entries.len() >= MAX_MOMENTARY {
            return vec![momentary_stale(id, StaleReason::MomentaryCapacity)];
        }
        let now = self.last_host_now;
        let prior_push =
            existing.map_or(f64::NEG_INFINITY, |i| self.momentary.entries[i].last_push);
        let base = numeric(&site.slot.base()).unwrap_or(0.0);
        let from = existing.map_or(base, |i| self.momentary.entries[i].ramp.value(now, base));
        let dur = duration(body.ramp_ms);
        let ramp = Ramp {
            from,
            to: Target::Value(numeric(&target).unwrap_or(from)),
            start: now,
            dur,
        };
        let cells = self.cells_for(&site, &target);
        if !self.rt.ramps.can_post_all(
            cells
                .iter()
                .filter(|cell| cell.acked)
                .map(|cell| (cell.id, cell.epoch)),
        ) {
            return vec![momentary_stale(id, StaleReason::MomentaryCapacity)];
        }
        let mut entry = existing.map_or_else(
            || Entry {
                file,
                id,
                alias: None,
                slot: site.slot.clone(),
                index: site.index,
                ty: site.ty,
                origin: site.origin,
                form_gen: site.form_gen,
                ramp,
                releasing: false,
                last_push: now,
                version: site.slot.version(),
                cells: cells.clone(),
            },
            |i| {
                let entry = &mut self.momentary.entries[i];
                entry.slot = site.slot.clone();
                entry.form_gen = site.form_gen;
                entry.ramp = ramp;
                entry.releasing = false;
                entry.version = site.slot.version();
                entry.cells = cells.clone();
                Entry {
                    file: entry.file,
                    id: entry.id,
                    alias: entry.alias,
                    slot: entry.slot.clone(),
                    index: entry.index,
                    ty: entry.ty,
                    origin: entry.origin,
                    form_gen: entry.form_gen,
                    ramp: entry.ramp,
                    releasing: entry.releasing,
                    last_push: entry.last_push,
                    version: entry.version,
                    cells: entry.cells.clone(),
                }
            },
        );
        let current_value = ramp.value(now, base);
        let slot = entry.slot.clone();
        let slot_value = value_for(site.ty, current_value);
        let changed = !same_numeric(&entry.slot.momentary(), &Some(slot_value.clone()));
        let needs_push = changed && (existing.is_none() || now - prior_push >= EVAL_PUSH_S);
        if needs_push {
            entry.slot.set_momentary(Some(slot_value.clone()));
            self.set_cell_overrides(&cells, &site, current_value);
        }
        self.post_entry_ramps(&cells, target, ramp.end(), false, now);
        entry.cells = cells;
        if needs_push {
            entry.last_push = now;
        }
        if let Some(index) = existing {
            self.momentary.entries[index] = entry;
        } else {
            self.momentary.entries.push(entry);
        }
        if needs_push {
            self.stage_slot_value(&slot, Some(slot_value));
        }
        Vec::new()
    }

    pub(super) fn momentary_tick(&mut self, now: f64) {
        let cuts = self.rt.output.cuts();
        if cuts != self.momentary.last_cuts {
            self.momentary_clear();
            self.momentary.last_cuts = cuts;
        }
        let mut index = 0;
        while index < self.momentary.entries.len() {
            let mut entry = self.momentary.entries.remove(index);
            let base_value = numeric(&entry.slot.base()).unwrap_or(0.0);
            if entry.releasing && entry.slot.version() != entry.version {
                let remaining = (entry.ramp.end() - now).max(MIN_RAMP_S);
                let from = entry.ramp.value(now, base_value);
                entry.ramp = Ramp {
                    from,
                    to: Target::Base,
                    start: now,
                    dur: remaining,
                };
                entry.version = entry.slot.version();
                self.post_entry_ramps(&entry.cells, entry.slot.base(), entry.ramp.end(), true, now);
            }
            let value = entry.ramp.value(now, base_value);
            let ended = now >= entry.ramp.end();
            if now - entry.last_push >= EVAL_PUSH_S || ended {
                let effective = if entry.releasing && ended {
                    None
                } else {
                    Some(value_for(entry.ty, value))
                };
                let slot = entry.slot.clone();
                let changed = !same_numeric(&slot.momentary(), &effective);
                slot.set_momentary(effective.clone());
                let site = { self.ev.ns().tweaks().borrow().get(entry.id).cloned() };
                if let Some(site) = site {
                    let cells = self.cells_for(&site, &value_for(entry.ty, value));
                    self.set_cell_overrides(&cells, &site, value);
                    for cell in &cells {
                        if cell.stepped {
                            let rounded = cell.target.round();
                            if rounded.to_bits() != cell.base.to_bits() && cell.acked {
                                self.post_single_ramp(cell, rounded, 0, false, now);
                            }
                        }
                    }
                    entry.cells = cells;
                }
                entry.last_push = now;
                if changed {
                    self.stage_slot_value(&slot, effective);
                }
            }
            if entry.releasing && ended {
                let slot = entry.slot.clone();
                let changed = slot.momentary().is_some();
                slot.set_momentary(None);
                for cell in &entry.cells {
                    self.rt.cells.set_momentary(cell.id, cell.epoch, None);
                }
                if changed {
                    self.stage_slot_value(&slot, None);
                }
            } else {
                self.momentary.entries.insert(index, entry);
                index += 1;
            }
        }
        let sample_rate = self.rt.cfg.sample_rate;
        let frame = host_frame(now, sample_rate);
        let resend = self.rt.cfg.resend_ticks;
        let messages = self.rt.ramps.tick(frame, resend, &self.rt.cells);
        for message in messages {
            self.rt.cells.post_ctl(message, &mut *self.rt.hosts.audio);
        }
    }

    pub(super) fn momentary_clear(&mut self) {
        let entries = std::mem::take(&mut self.momentary.entries);
        let now = self.last_host_now;
        for entry in &entries {
            let changed = entry.slot.momentary().is_some();
            entry.slot.set_momentary(None);
            for cell in &entry.cells {
                self.rt.cells.set_momentary(cell.id, cell.epoch, None);
                if cell.acked {
                    self.post_single_ramp(cell, cell.base, 0, true, now);
                }
            }
            if changed {
                self.stage_slot_value(&entry.slot, None);
            }
        }
    }

    pub(super) fn momentary_rebase(&mut self) {
        let migrations = self.ev.take_migrations();
        if migrations.is_empty() {
            return;
        }
        let now = self.last_host_now;
        let mut entries = std::mem::take(&mut self.momentary.entries);
        let mut rebased = Vec::with_capacity(entries.len());
        let mut refresh = Vec::with_capacity(entries.len());
        for mut entry in entries.drain(..) {
            let migrated = migrations
                .iter()
                .find(|(old, _)| *old == entry.id)
                .map(|(_, new)| *new);
            let Some(new_id) = migrated else {
                let still_current = self
                    .ev
                    .ns()
                    .tweaks()
                    .borrow()
                    .get(entry.id)
                    .is_some_and(|site| site.form_gen == entry.form_gen);
                if still_current {
                    rebased.push(entry);
                    continue;
                }
                entry.slot.set_momentary(None);
                for cell in &entry.cells {
                    self.rt.cells.set_momentary(cell.id, cell.epoch, None);
                    if cell.acked {
                        self.post_single_ramp(cell, cell.base, 0, true, now);
                    }
                }
                refresh.push((entry.slot, None));
                continue;
            };
            let Some(site) = self.ev.ns().tweaks().borrow().get(new_id).cloned() else {
                entry.slot.set_momentary(None);
                for cell in &entry.cells {
                    self.rt.cells.set_momentary(cell.id, cell.epoch, None);
                    if cell.acked {
                        self.post_single_ramp(cell, cell.base, 0, true, now);
                    }
                }
                refresh.push((entry.slot, None));
                continue;
            };
            let old_id = entry.id;
            let old_slot = entry.slot.clone();
            let old_cells = entry.cells.clone();
            let base = numeric(&site.slot.base()).unwrap_or(0.0);
            let current = entry.ramp.value(now, base);
            let cells = self.cells_for(&site, &value_for(site.ty, current));
            for old in &old_cells {
                if !cells
                    .iter()
                    .any(|new| new.id == old.id && new.epoch == old.epoch)
                {
                    self.rt.cells.set_momentary(old.id, old.epoch, None);
                    if old.acked {
                        self.post_single_ramp(old, old.base, 0, true, now);
                    }
                }
            }
            let slot_value = value_for(site.ty, current);
            if !old_slot.same(&site.slot) {
                old_slot.set_momentary(None);
                refresh.push((old_slot, None));
            }
            site.slot.set_momentary(Some(slot_value.clone()));
            refresh.push((site.slot.clone(), Some(slot_value)));
            self.set_cell_overrides(&cells, &site, current);
            entry.alias = Some(old_id);
            entry.id = site.id;
            entry.slot = site.slot.clone();
            entry.form_gen = site.form_gen;
            entry.index = site.index;
            entry.ty = site.ty;
            entry.version = site.slot.version();
            entry.cells = cells.clone();
            if entry.releasing {
                let remaining = (entry.ramp.end() - now).max(MIN_RAMP_S);
                entry.ramp = Ramp {
                    from: current,
                    to: Target::Base,
                    start: now,
                    dur: remaining,
                };
                self.post_entry_ramps(&cells, site.slot.base(), entry.ramp.end(), true, now);
            } else {
                let target = match entry.ramp.to {
                    Target::Value(value) => value,
                    Target::Base => current,
                };
                entry.ramp = Ramp {
                    from: current,
                    to: Target::Value(target),
                    start: now,
                    dur: MIN_RAMP_S,
                };
                for cell in &cells {
                    if cell.acked {
                        self.post_single_ramp(cell, cell.target, 0, false, now);
                    }
                }
            }
            rebased.push(entry);
        }
        self.momentary.entries = rebased;
        for (slot, value) in refresh {
            self.stage_slot_value(&slot, value);
        }
    }

    fn release_momentary(&mut self, file: FileId, id: TweakId, ramp_ms: u32) {
        let Some(index) =
            self.momentary.entries.iter().position(|entry| {
                entry.file == file && (entry.id == id || entry.alias == Some(id))
            })
        else {
            return;
        };
        let now = self.last_host_now;
        let base = self.momentary.entries[index].slot.base();
        let base_num = numeric(&base).unwrap_or(0.0);
        let (cells, slot, end) = {
            let entry = &mut self.momentary.entries[index];
            let from = entry.ramp.value(now, base_num);
            entry.ramp = Ramp {
                from,
                to: Target::Base,
                start: now,
                dur: duration(ramp_ms),
            };
            entry.releasing = true;
            entry.version = entry.slot.version();
            (entry.cells.clone(), entry.slot.clone(), entry.ramp.end())
        };
        self.post_entry_ramps(&cells, slot.base(), end, true, now);
    }

    fn stage_slot_value(&mut self, slot: &VarSlotRef, value: Option<Value>) {
        let effective = value.unwrap_or_else(|| slot.base());
        self.stage_now(StagedEffect::CellUpdate {
            slot: slot.clone(),
            value: effective,
        });
    }

    fn cells_for(&self, site: &TweakSite, value: &Value) -> Vec<Cell> {
        let mut cells = Vec::new();
        let base = site.slot.base();
        for (id, epoch, row, map, acked) in self.rt.cells.site_cells(site.slot.id()) {
            let target = self.rt.cells.encode_for(id, value);
            let base_value = self.rt.cells.encode_for(id, &base);
            if let (Some(target), Some(base_value)) = (target, base_value) {
                cells.push(Cell {
                    id,
                    epoch,
                    target,
                    base: base_value,
                    acked,
                    stepped: crate::sched::cells::ControlCells::stepped(row)
                        || matches!(map, CellMap::Direct)
                            && row.domain != crate::dsp::controls::CtlDomain::Float,
                });
            }
        }
        if let Some(registry) = self.ev.insts() {
            for inst in registry.borrow().entries() {
                for (id, slot) in &inst.cells {
                    if !slot.same(&site.slot) {
                        continue;
                    }
                    let prior = slot.momentary();
                    slot.set_momentary(Some(value.clone()));
                    let target = inst.default_cell_value(*id);
                    slot.set_momentary(Some(base.clone()));
                    let base_value = inst.default_cell_value(*id);
                    slot.set_momentary(prior);
                    if let (Some(target), Some(base_value), Some((epoch, acked, _))) =
                        (target, base_value, self.rt.cells.cell_state(*id))
                    {
                        let ctl = inst.def.params.iter().find_map(|(ctl, cell)| {
                            (*cell == crate::host::wire::Ctl::Cell(*id)).then_some(*ctl)
                        });
                        let row = ctl.and_then(row_by_id);
                        let int = inst.params.iter().any(|param| {
                            Some(param.ctl) == ctl
                                && matches!(param.ty, ScalarType::Int | ScalarType::Bool)
                        });
                        cells.push(Cell {
                            id: *id,
                            epoch,
                            target,
                            base: base_value,
                            acked,
                            stepped: int
                                || row.is_some_and(crate::sched::cells::ControlCells::stepped),
                        });
                    }
                }
            }
        }
        cells
    }

    fn set_cell_overrides(&mut self, cells: &[Cell], _site: &TweakSite, value: f64) {
        for cell in cells {
            let encoded = if cell.stepped {
                value.round() as f32
            } else {
                value as f32
            };
            self.rt
                .cells
                .set_momentary(cell.id, cell.epoch, Some(encoded));
        }
    }

    fn post_entry_ramps(
        &mut self,
        cells: &[Cell],
        target: Value,
        end: f64,
        release: bool,
        now: f64,
    ) {
        let sr = self.rt.cfg.sample_rate;
        let frames = host_frame((end - now).max(0.0), sr).min(u64::from(u32::MAX)) as u32;
        for cell in cells {
            if !cell.acked {
                continue;
            }
            let encoded = if release { cell.base } else { cell.target };
            let frames = if cell.stepped { 0 } else { frames };
            self.post_single_ramp(cell, encoded, frames, release, now);
        }
        let _ = target;
    }

    fn post_single_ramp(&mut self, cell: &Cell, target: f32, frames: u32, release: bool, now: f64) {
        let sample_rate = self.rt.cfg.sample_rate;
        let frame = host_frame(now, sample_rate);
        let message = if release && frames == 0 {
            self.rt
                .ramps
                .drop_cell(cell.id, cell.epoch, target, frame, &self.rt.cells)
        } else {
            self.rt
                .ramps
                .post(cell.id, cell.epoch, target, frames, release, frame)
                .ok()
        };
        if let Some(msg) = message {
            self.rt.cells.post_ctl(msg, &mut *self.rt.hosts.audio);
        }
    }
}

fn duration(ms: u32) -> f64 {
    f64::from(ms.min(MAX_RAMP_MS))
        .mul_add(0.001, 0.0)
        .max(MIN_RAMP_S)
}

fn numeric(value: &Value) -> Option<f64> {
    match value {
        Value::Int(n) => Some(f64::from(*n)),
        Value::Int64(n) => Some(*n as f64),
        Value::Float(n) => Some(f64::from(*n)),
        Value::Float64(n) => Some(*n),
        Value::Ratio(n) => Some(n.to_f64()),
        _ => None,
    }
}

fn same_numeric(left: &Option<Value>, right: &Option<Value>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => match (numeric(left), numeric(right)) {
            (Some(left), Some(right)) => left.to_bits() == right.to_bits(),
            _ => false,
        },
        _ => false,
    }
}

fn value_for(ty: NumTy, value: f64) -> Value {
    match ty {
        NumKind::Int => Value::Int(value.round() as i32),
        NumKind::Int64 => Value::Int64(value.round() as i64),
        NumKind::Float => Value::Float(value as f32),
        NumKind::Float64 => Value::Float64(value),
        NumKind::Ratio => {
            Ratio64::from_f64_exact(value).map_or(Value::Float64(value), Value::Ratio)
        }
    }
}

fn coerce_target(ty: NumTy, target: WireNum) -> Option<Value> {
    let value = target.as_f64();
    if !value.is_finite() {
        return None;
    }
    Some(match ty {
        NumKind::Int => Value::Int(i32::try_from(value.round() as i64).ok()?),
        NumKind::Int64 => Value::Int64(value.round() as i64),
        NumKind::Float => Value::Float(value as f32),
        NumKind::Float64 => Value::Float64(value),
        NumKind::Ratio => match target {
            WireNum::Int(n) => Value::Ratio(Ratio64::from_int(n)),
            WireNum::Float(n) => Value::Ratio(Ratio64::from_f64_exact(n)?),
        },
    })
}

fn momentary_stale(id: TweakId, reason: StaleReason) -> ServerMsg {
    ServerMsg::StaleBinding(StaleBindingBody {
        target: StaleTarget::Id(id.get()),
        reason,
        current_form_gen: None,
    })
}

fn host_frame(seconds: f64, sample_rate: u32) -> u64 {
    let frame = seconds * f64::from(sample_rate);
    if frame.is_finite() && frame > 0.0 {
        frame.round() as u64
    } else {
        0
    }
}
