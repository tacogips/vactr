//! Once-only retained callback discovery and closed shape admission.
use crate::pattern::pat::{Pat, PatNode};
use crate::song::assets::SongAssetLimits;
use crate::song::source_uses::FrozenUseReason as R;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
mod callable_results;
use callable_results::{FixedCallableResults, PendingFixedResults};
#[cfg(test)]
thread_local! { static SHAPE_CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
#[derive(Default)]
pub(super) struct PreparedShapes {
    fixed: FixedCallableResults,
    pub(super) patterns: BTreeMap<(u8, usize, usize), Rc<Pat>>,
    pub(super) rejected: BTreeMap<(u8, usize, usize), R>,
    #[cfg(test)]
    pub(super) consumed_work: u32,
}
impl PreparedShapes {
    pub(super) fn fixed_pattern<'a>(
        &'a self,
        callable: &Value,
        depth: u32,
        remaining: &mut u32,
        limits: SongAssetLimits,
    ) -> Result<Option<&'a Rc<Pat>>, Failure> {
        self.fixed.pattern(callable, depth, remaining, limits)
    }
    pub(super) fn dependency_roots(
        &self,
        pattern: &Rc<Pat>,
        remaining: &mut u32,
        limits: SongAssetLimits,
    ) -> Result<Vec<Value>, Failure> {
        admit(remaining, 1)?;
        let mut roots = vec![Value::Pattern(pattern.clone())];
        let mut seen = BTreeSet::new();
        self.dependency_pat(pattern, remaining, limits, 0, &mut seen, &mut roots)?;
        Ok(roots)
    }
    fn dependency_pat(
        &self,
        p: &Pat,
        remaining: &mut u32,
        limits: SongAssetLimits,
        depth: u32,
        seen: &mut BTreeSet<usize>,
        roots: &mut Vec<Value>,
    ) -> Result<(), Failure> {
        admit(remaining, 1)?;
        if depth >= limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "prepared dependency depth exhausted",
            ));
        }
        if !seen.insert(p as *const Pat as usize) {
            return Ok(());
        }
        let pair = match &p.node {
            PatNode::Every(_, f, p)
            | PatNode::WhenMod(_, _, f, p)
            | PatNode::SometimesBy(_, f, p)
            | PatNode::Superimpose(p, f)
            | PatNode::Off(p, _, f)
            | PatNode::Jux(p, f)
            | PatNode::Chunk(p, _, f) => Some((f, p)),
            _ => None,
        };
        if let Some((f, input)) = pair {
            if let Some(output) = shape_key(f, input).and_then(|key| self.patterns.get(&key)) {
                admit(remaining, 1)?;
                roots.push(Value::Pattern(output.clone()));
                self.dependency_pat(output, remaining, limits, depth + 1, seen, roots)?;
            }
        }
        admit(remaining, super::child_count(p))?;
        for child in super::freeze::children(p) {
            self.dependency_pat(child, remaining, limits, depth + 1, seen, roots)?;
        }
        match &p.node {
            PatNode::Pure(step) => {
                self.dependency_value(&step.value, remaining, limits, depth + 1, seen, roots)?
            }
            PatNode::Steps(steps) => {
                admit(remaining, steps.len())?;
                for step in steps.iter() {
                    self.dependency_value(&step.value, remaining, limits, depth + 1, seen, roots)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn dependency_value(
        &self,
        value: &Value,
        remaining: &mut u32,
        limits: SongAssetLimits,
        depth: u32,
        seen: &mut BTreeSet<usize>,
        roots: &mut Vec<Value>,
    ) -> Result<(), Failure> {
        admit(remaining, 1)?;
        if depth >= limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "prepared value depth exhausted",
            ));
        }
        match value {
            Value::Fn(_) => {
                if let Some(result) = self.fixed_pattern(value, depth, remaining, limits)? {
                    admit(remaining, 1)?;
                    roots.push(Value::Pattern(result.clone()));
                    self.dependency_pat(result, remaining, limits, depth + 1, seen, roots)?;
                }
                Ok(())
            }
            Value::Pattern(p) => self.dependency_pat(p, remaining, limits, depth + 1, seen, roots),
            Value::List(list) => {
                admit(remaining, list.items.len())?;
                for v in list.items.iter() {
                    self.dependency_value(v, remaining, limits, depth + 1, seen, roots)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
pub(super) fn shape_key(value: &Value, pat: &Pat) -> Option<(u8, usize, usize)> {
    let (kind, identity) = match value {
        Value::Fn(f) => (0, Rc::as_ptr(f) as usize),
        Value::Native(id) => (1, id.get() as usize),
        Value::VarRef(slot) => (2, Rc::as_ptr(&slot.0) as usize),
        _ => return None,
    };
    Some((kind, identity, pat as *const Pat as usize))
}
pub(super) struct PendingShape {
    callback: Value,
    input: Rc<Pat>,
    outcome: PendingShapeOutcome,
}
pub(super) enum PendingShapeOutcome {
    Pattern(Rc<Pat>),
    Rejected(R),
}
#[derive(Default)]
pub(super) struct PendingShapes {
    fixed: PendingFixedResults,
    entries: BTreeMap<(u8, usize, usize), PendingShape>,
    retained_instruments: BTreeSet<crate::dsp::graph::InstId>,
    dependency_roots: Vec<Value>,
}
pub(super) struct FrozenPendingShapes {
    fixed: PendingFixedResults,
    entries: Vec<PendingShape>,
}
pub(super) fn discover_shapes(
    evaluator: &mut crate::ns::evaluator::Evaluator,
    song: &crate::song::Song,
    limits: SongAssetLimits,
    remaining: &mut u32,
) -> Result<PendingShapes, Failure> {
    let (vm, ns) = evaluator.vm_and_ns();
    let mut state = ShapePreparation {
        vm,
        ns,
        limits,
        remaining,
        seen: BTreeSet::new(),
        out: PendingShapes::default(),
    };
    state.part(song.part(), 0)?;
    Ok(state.out)
}
pub(super) fn dependencies(
    roots: &[Value],
    limits: SongAssetLimits,
    remaining: &mut u32,
) -> Result<crate::song::assets::SongAssetDependencies, Failure> {
    if *remaining == 0 {
        return Err(Failure::new(
            FailCode::FuelExhausted,
            "aggregate shape dependency work exhausted",
        ));
    }
    let mut reduced = limits;
    reduced.max_walk_nodes = *remaining;
    let deps = crate::song::assets::song_asset_dependencies(roots, reduced)?;
    admit(remaining, deps.consumed_work() as usize)?;
    Ok(deps)
}
pub(super) fn charge(remaining: &mut u32, n: usize) -> Result<(), Failure> {
    admit(remaining, n)
}
fn admit(remaining: &mut u32, n: usize) -> Result<(), Failure> {
    let n =
        u32::try_from(n).map_err(|_| Failure::new(FailCode::Overflow, "shape work overflow"))?;
    *remaining = remaining
        .checked_sub(n)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "aggregate shape work exhausted"))?;
    Ok(())
}
impl PendingShapes {
    pub(super) fn retained_count(&self) -> usize {
        self.retained_instruments.len()
    }
    pub(super) fn take_dependency_roots(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.dependency_roots)
    }
    pub(super) fn record_count(&self) -> usize {
        self.entries.len() + self.fixed.len()
    }
    pub(super) fn retained_instruments(
        &self,
    ) -> impl Iterator<Item = crate::dsp::graph::InstId> + '_ {
        self.retained_instruments.iter().copied()
    }
    pub(super) fn freeze_records(
        self,
        freeze: &mut super::freeze::Freeze<'_>,
        remaining: &mut u32,
    ) -> Result<FrozenPendingShapes, Failure> {
        admit(remaining, self.entries.len())?;
        let mut entries = Vec::with_capacity(self.entries.len());
        for pending in self.entries.into_values() {
            let callback = freeze.value(&pending.callback)?;
            let Value::Pattern(input) = freeze.value(&Value::Pattern(pending.input))? else {
                return Err(super::failure("shape input copy invariant"));
            };
            let outcome = match pending.outcome {
                PendingShapeOutcome::Pattern(p) => {
                    let Value::Pattern(p) = freeze.value(&Value::Pattern(p))? else {
                        return Err(super::failure("shape output copy invariant"));
                    };
                    PendingShapeOutcome::Pattern(p)
                }
                PendingShapeOutcome::Rejected(r) => PendingShapeOutcome::Rejected(r),
            };
            entries.push(PendingShape {
                callback,
                input,
                outcome,
            });
        }
        let fixed = self.fixed.freeze_records(freeze, remaining)?;
        Ok(FrozenPendingShapes { entries, fixed })
    }
}
impl FrozenPendingShapes {
    pub(super) fn admit_closed(
        self,
        evaluator: &mut crate::ns::evaluator::Evaluator,
        assets: &mut crate::song::assets::PinnedSongAssets,
        limits: SongAssetLimits,
        remaining: &mut u32,
    ) -> Result<PreparedShapes, Failure> {
        let mut out = PreparedShapes {
            fixed: self
                .fixed
                .admit_closed(evaluator, assets, limits, remaining)?,
            ..PreparedShapes::default()
        };
        let (vm, ns) = evaluator.vm_and_ns();
        for pending in self.entries {
            admit(remaining, 1)?;
            let key = shape_key(&pending.callback, &pending.input)
                .ok_or_else(|| super::failure("copied shape callback invariant"))?;
            match pending.outcome {
                PendingShapeOutcome::Rejected(reason) => {
                    out.rejected.insert(key, reason);
                }
                PendingShapeOutcome::Pattern(p) => {
                    if *remaining == 0 {
                        return Err(Failure::new(
                            FailCode::FuelExhausted,
                            "closed shape dependency work exhausted",
                        ));
                    }
                    let mut reduced = limits;
                    reduced.max_walk_nodes = *remaining;
                    let deps = crate::song::assets::song_asset_dependencies(
                        &[Value::Pattern(p.clone())],
                        reduced,
                    )?;
                    admit(remaining, deps.consumed_work() as usize)?;
                    let accepted = super::ClosedShapeCtx {
                        vm,
                        ns,
                        assets,
                        remaining,
                        limits,
                        depth: 0,
                    }
                    .accepts(&deps)?;
                    if accepted {
                        out.patterns.insert(key, p);
                    } else {
                        out.rejected.insert(key, R::UnclosedResource);
                    }
                }
            }
        }
        Ok(out)
    }
}
struct ShapePreparation<'a> {
    vm: &'a mut crate::vm::vm::Vm,
    ns: &'a crate::ns::namespace::Namespace,
    limits: SongAssetLimits,
    remaining: &'a mut u32,
    seen: BTreeSet<(u8, usize)>,
    out: PendingShapes,
}
impl ShapePreparation<'_> {
    fn admit(&mut self, n: usize, depth: u32) -> Result<(), Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "shape preparation depth exhausted",
            ));
        }
        let n = u32::try_from(n)
            .map_err(|_| Failure::new(FailCode::Overflow, "shape work overflow"))?;
        *self.remaining = self.remaining.checked_sub(n).ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "aggregate shape preparation exhausted",
            )
        })?;
        Ok(())
    }
    fn part(&mut self, part: &crate::song::Part, depth: u32) -> Result<(), Failure> {
        self.admit(1, depth)?;
        if !self
            .seen
            .insert((0, part as *const crate::song::Part as usize))
        {
            return Ok(());
        }
        if let crate::song::PartNode::Repeat { child, .. } = part.node() {
            return self.part(child, depth + 1);
        }
        self.part_inner(part, depth)
    }
    #[inline(never)]
    fn part_inner(&mut self, part: &crate::song::Part, depth: u32) -> Result<(), Failure> {
        use crate::song::{PartEdit, PartNode};
        match part.node() {
            PartNode::Capture(tracks) => {
                self.admit(tracks.len(), depth)?;
                for p in tracks.values() {
                    self.pat(p, depth + 1)?;
                }
            }
            PartNode::Sequence(children) => {
                self.admit(children.len(), depth)?;
                for child in children {
                    self.part(child, depth + 1)?;
                }
            }
            PartNode::Repeat { .. } => {}
            PartNode::Edit { source, edit } => {
                match edit {
                    PartEdit::TransformInstrument { selector, .. }
                    | PartEdit::InstrumentFx { selector, .. } => self.selector(selector, depth)?,
                    _ => {}
                }
                self.part(source, depth + 1)?;
                match edit {
                    PartEdit::ReplaceTrack { pattern, .. }
                    | PartEdit::OverwriteRegion { pattern, .. }
                    | PartEdit::TransformInstrument { pattern, .. } => {
                        self.pat(pattern, depth + 1)?
                    }
                    _ => {}
                }
            }
        };
        Ok(())
    }
    fn pat(&mut self, pat: &Pat, depth: u32) -> Result<(), Failure> {
        self.admit(1, depth)?;
        if !self.seen.insert((1, pat as *const Pat as usize)) {
            return Ok(());
        }
        match &pat.node {
            PatNode::Fast(p, _)
            | PatNode::Slow(p, _)
            | PatNode::Hurry(p, _)
            | PatNode::Rev(p)
            | PatNode::DegradeBy(p, _)
            | PatNode::Maybe(p, _)
            | PatNode::Voicing(p)
            | PatNode::Fit(p)
            | PatNode::ScaleNotes(_, _, p)
            | PatNode::Strum(p, _, _, _)
            | PatNode::Harp { subject: p, .. }
            | PatNode::Inversion { subject: p, .. } => self.pat(p, depth + 1),
            _ => self.pat_inner(pat, depth),
        }
    }
    #[inline(never)]
    fn pat_inner(&mut self, pat: &Pat, depth: u32) -> Result<(), Failure> {
        match &pat.node {
            PatNode::Every(_, f, p)
            | PatNode::WhenMod(_, _, f, p)
            | PatNode::SometimesBy(_, f, p)
            | PatNode::Superimpose(p, f)
            | PatNode::Off(p, _, f)
            | PatNode::Jux(p, f)
            | PatNode::Chunk(p, _, f) => self.callback(f, p, depth)?,
            PatNode::SongSource(source) => {
                self.selector(source.selector(), depth)?;
                self.part(source.part(), depth + 1)?;
            }
            PatNode::Pure(step) => self.value(&step.value, depth + 1)?,
            PatNode::Tune { .. } => {}
            PatNode::Steps(steps) => {
                self.admit(steps.len(), depth)?;
                for step in steps.iter() {
                    self.value(&step.value, depth + 1)?;
                }
            }
            _ => {}
        }
        self.admit(super::child_count(pat), depth)?;
        for child in super::freeze::children(pat) {
            self.pat(child, depth + 1)?;
        }
        Ok(())
    }
    fn value(&mut self, value: &Value, depth: u32) -> Result<(), Failure> {
        self.admit(1, depth)?;
        match value {
            Value::Fn(_) => {
                if let Some(result) =
                    self.out
                        .fixed
                        .discover(value, depth, self.remaining, self.limits)?
                {
                    self.admit(1, depth)?;
                    self.out
                        .dependency_roots
                        .push(Value::Pattern(result.clone()));
                    self.pat(&result, depth + 1)?;
                }
                Ok(())
            }
            Value::Pattern(p) => self.pat(p, depth + 1),
            Value::List(l) => {
                self.admit(l.items.len(), depth)?;
                for v in l.items.iter() {
                    self.value(v, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn selector(
        &mut self,
        selector: &crate::song::InstrumentSelector,
        depth: u32,
    ) -> Result<(), Failure> {
        self.admit(selector.family().len(), depth)?;
        self.out.dependency_roots.extend(
            selector
                .family()
                .iter()
                .map(|sound| Value::Sound(Rc::new(sound.clone()))),
        );
        Ok(())
    }
    fn retain(
        &mut self,
        key: (u8, usize, usize),
        callback: &Value,
        input: &Rc<Pat>,
        outcome: PendingShapeOutcome,
    ) {
        self.out.entries.insert(
            key,
            PendingShape {
                callback: callback.clone(),
                input: input.clone(),
                outcome,
            },
        );
    }
    fn discover_owners(
        &mut self,
        deps: &crate::song::assets::SongAssetDependencies,
        depth: u32,
    ) -> Result<(), Failure> {
        use crate::host::caps::{InstResolver, Route};
        use crate::value::key::Key;
        use crate::value::value::Sound;
        let registry = self
            .vm
            .dsp
            .registry
            .clone()
            .ok_or_else(|| super::failure("shape registry missing"))?;
        self.admit(
            deps.sound_keywords.len()
                + deps.instruments.len()
                + deps.sample_paths.len()
                + deps.buffers.len(),
            depth,
        )?;
        let mut roots: Vec<_> = deps
            .instruments
            .iter()
            .map(|id| (Value::Inst(*id), depth))
            .collect();
        for path in &deps.sample_paths {
            self.admit(1, depth)?;
            roots.push((Value::Sound(Rc::new(Sound::Sample(path.clone()))), depth));
        }
        for buffer in &deps.buffers {
            self.admit(1, depth)?;
            roots.push((Value::Sound(Rc::new(Sound::Buffer(buffer.clone()))), depth));
        }
        let kit = self
            .ns
            .lookup(crate::value::intern::intern_sym("sound-kit"))
            .map(|slot| slot.slot().get());
        for kw in &deps.sound_keywords {
            let family = match &kit {
                Some(Value::Dict(dict)) => dict.get(&Key::Kw(*kw)).cloned(),
                Some(Value::VarRef(slot)) => match slot.get() {
                    Value::Dict(dict) => dict.get(&Key::Kw(*kw)).cloned(),
                    _ => None,
                },
                _ => None,
            };
            roots.push((
                family
                    .or_else(|| registry.borrow().sound(*kw))
                    .unwrap_or_else(|| Value::Sound(Rc::new(Sound::Builtin(*kw)))),
                depth,
            ));
        }
        while let Some((value, level)) = roots.pop() {
            self.admit(1, level)?;
            let sound = match value {
                Value::Inst(id) => Sound::Inst(id),
                Value::Sound(sound) => (*sound).clone(),
                Value::List(list) => {
                    self.admit(list.items.len(), level)?;
                    roots.extend(list.items.iter().cloned().map(|v| (v, level + 1)));
                    continue;
                }
                _ => return Err(super::failure("generated sound family is not audio")),
            };
            let Route::Audio { inst, .. } = registry.borrow().route(&sound)? else {
                return Err(super::failure("generated family is external"));
            };
            self.out.retained_instruments.insert(inst);
        }
        Ok(())
    }
    fn callback(&mut self, value: &Value, pat: &Rc<Pat>, depth: u32) -> Result<(), Failure> {
        let Some(key) = shape_key(value, pat) else {
            return Ok(());
        };
        if self.out.entries.contains_key(&key) {
            return Ok(());
        }
        self.admit(1, depth)?;
        if *self.remaining == 0 {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "shape dependency work exhausted",
            ));
        }
        let mut limits = self.limits;
        limits.max_walk_nodes = *self.remaining;
        let deps = crate::song::assets::song_asset_dependencies(
            &[value.clone(), Value::Pattern(pat.clone())],
            limits,
        )?;
        self.admit(deps.consumed_work() as usize, depth)?;
        self.admit(deps.native_functions.len(), depth)?;
        let permitted = deps.native_functions.iter().all(|id| {
            self.ns.prelude().native(*id).is_some_and(|entry| {
                !entry.sig.effectful
                    && !super::forbidden_native(entry.sig.name)
                    && !matches!(entry.sig.name, "query" | "part-events")
            })
        });
        if !permitted {
            self.retain(
                key,
                value,
                pat,
                PendingShapeOutcome::Rejected(R::UncertifiedCallback),
            );
            return Ok(());
        }
        let saved_fuel = self.vm.fuel();
        let before = self.vm.take_output();
        self.vm.set_fuel(u64::from(*self.remaining));
        #[cfg(test)]
        SHAPE_CALLS.with(|calls| calls.set(calls.get() + 1));
        let result = self
            .vm
            .with_effect_mode(crate::vm::vm::EffectMode::Query, |vm| {
                vm.call_value(
                    self.ns,
                    value,
                    vec![Value::Pattern(pat.clone())],
                    Vec::new(),
                )
            });
        let after = self.vm.fuel();
        let emitted = !self.vm.take_output().is_empty();
        self.vm.put_output(before);
        self.vm.set_fuel(saved_fuel);
        *self.remaining = u32::try_from(after)
            .map_err(|_| Failure::new(FailCode::Overflow, "shape fuel accounting overflow"))?;
        if emitted {
            self.retain(
                key,
                value,
                pat,
                PendingShapeOutcome::Rejected(R::UncertifiedCallback),
            );
            return Ok(());
        }
        let prepared = match result {
            Ok(Value::Pattern(p)) => p,
            Ok(Value::List(l)) => {
                self.admit(l.items.len(), depth)?;
                Rc::new(crate::pattern::step::steps(
                    crate::pattern::step::steps_of_list(&l),
                    None,
                ))
            }
            _ => {
                self.retain(
                    key,
                    value,
                    pat,
                    PendingShapeOutcome::Rejected(R::UncertifiedCallback),
                );
                return Ok(());
            }
        };
        if *self.remaining == 0 {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "shape dependency work exhausted",
            ));
        }
        let mut limits = self.limits;
        limits.max_walk_nodes = *self.remaining;
        let prepared_deps = crate::song::assets::song_asset_dependencies(
            &[Value::Pattern(prepared.clone())],
            limits,
        )?;
        self.admit(prepared_deps.consumed_work() as usize, depth)?;
        let source_comparisons = prepared_deps
            .selected_sources()
            .len()
            .checked_mul(deps.selected_sources().len())
            .and_then(|n| {
                prepared_deps
                    .buffers
                    .len()
                    .checked_mul(deps.buffers.len())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(|| super::failure("shape identity comparison overflow"))?;
        self.admit(source_comparisons, depth)?;
        let closed = prepared_deps
            .selected_sources()
            .iter()
            .all(|s| deps.selected_sources().iter().any(|old| Rc::ptr_eq(s, old)))
            && prepared_deps
                .buffers
                .iter()
                .all(|b| deps.buffers.iter().any(|old| Rc::ptr_eq(b, old)))
            && !prepared_deps.has_live_signals
            && !prepared_deps.has_external_sounds;
        if !closed {
            self.retain(
                key,
                value,
                pat,
                PendingShapeOutcome::Rejected(R::UnclosedResource),
            );
            return Ok(());
        }
        self.discover_owners(&prepared_deps, depth)?;
        self.admit(1, depth)?;
        self.out
            .dependency_roots
            .push(Value::Pattern(prepared.clone()));
        self.retain(
            key,
            value,
            pat,
            PendingShapeOutcome::Pattern(prepared.clone()),
        );
        self.pat(&prepared, depth + 1)
    }
}

#[cfg(test)]
mod closed_resource_tests {
    use super::*;
    #[test]
    fn dynamic_keywords_use_closed_complete_bank_and_default_family() {
        use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
        use crate::song::assets::{
            DecodedSongAssetFactory, SongAssetDependencies, SongAssetFactory, SongAssetSelector,
            SongSourceFile,
        };
        use crate::value::intern::intern_kw;
        use std::sync::Arc;
        let data = Arc::new(SampleData {
            rate: 48000,
            channels: 2,
            frames: vec![0.1, 0.2].into(),
        });
        let factory = DecodedSongAssetFactory::new(
            BTreeMap::from([("a.wav".into(), data.clone()), ("b.wav".into(), data)]),
            BTreeMap::from([(intern_kw("bd"), vec!["a.wav".into(), "b.wav".into()])]),
            BTreeMap::new(),
        );
        let limits = SongAssetLimits {
            max_resources: 32,
            max_pcm_bytes: 1_000_000,
            max_source_files: 8,
            max_source_bytes: 10_000,
            max_banks: 8,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        };
        let mut preparation = factory
            .begin(
                SongSourceFile {
                    file: crate::reader::span::FileId::new(0),
                    path: crate::value::value::PathVal {
                        text: "score.vact".into(),
                        file: None,
                    },
                },
                limits,
            )
            .unwrap();
        let loader = preparation.source_loader();
        preparation
            .pin(&[SongAssetSelector::Bank(intern_kw("bd"))])
            .unwrap();
        let mut assets = preparation.close().unwrap();
        assert_eq!(assets.resource_count(), 2);
        assert!(assets
            .load(&SampleSrc::Bank {
                kw: intern_kw("bd"),
                index: 1
            })
            .is_ok());
        let mut evaluator = crate::ns::evaluator::Evaluator::new(
            crate::ns::namespace::Prelude::core(),
            loader,
            Box::new(crate::ns::stage::RecordingSink::default()),
        );
        let (vm, ns) = evaluator.vm_and_ns();
        let mut remaining = limits.max_walk_nodes;
        let mut state = super::super::ClosedShapeCtx {
            vm,
            ns,
            assets: &mut assets,
            limits,
            remaining: &mut remaining,
            depth: 0,
        };
        let mut dependencies = SongAssetDependencies::default();
        dependencies.sound_keywords = vec![intern_kw(&["b", "d"].concat())];
        assert!(state.accepts(&dependencies).unwrap());
        dependencies.sound_keywords = vec![intern_kw("analog")];
        assert!(state.accepts(&dependencies).unwrap());
        dependencies.sound_keywords = vec![intern_kw(&["un", "pinned"].concat())];
        assert!(!state.accepts(&dependencies).unwrap());
        assert!(state
            .assets
            .load(&SampleSrc::Bank {
                kw: intern_kw("unpinned"),
                index: 0
            })
            .is_err());
    }
}

#[cfg(test)]
mod retained_shape_tests;
