//! Copied symbolic routing inventory, bounded selected-source policies and immutable descriptors.
use super::freeze::children;
use super::{child_count, family};
use crate::pattern::pat::{Pat, PatNode};
use crate::song::assets::SongAssetLimits;
use crate::song::Song;
use crate::value::value::{Sound, Value};
use crate::vm::fail::{FailCode, Failure};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
fn fail(message: &str) -> Failure {
    Failure::new(FailCode::HostUnavailable, message)
}
fn failure(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}
#[cfg(test)]
pub(super) fn capture_routing(
    evaluator: &crate::ns::evaluator::Evaluator,
    song: &Song,
    limits: SongAssetLimits,
) -> Result<crate::song::snapshot::FrozenRoutingInventory, Failure> {
    capture_routing_prepared(
        evaluator,
        song,
        limits,
        &super::source_uses::PreparedShapes::default(),
    )
}
#[cfg(test)]
pub(super) fn capture_routing_prepared(
    evaluator: &crate::ns::evaluator::Evaluator,
    song: &Song,
    limits: SongAssetLimits,
    shapes: &super::source_uses::PreparedShapes,
) -> Result<crate::song::snapshot::FrozenRoutingInventory, Failure> {
    let remaining_work = limits
        .max_walk_nodes
        .checked_sub(shapes.consumed_work)
        .ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "shape preparation exhausted inventory work",
            )
        })?;
    let mut remaining = remaining_work;
    capture_routing_shared(evaluator, song, limits, shapes, &mut remaining)
}
pub(super) fn capture_routing_shared(
    evaluator: &crate::ns::evaluator::Evaluator,
    song: &Song,
    limits: SongAssetLimits,
    shapes: &super::source_uses::PreparedShapes,
    remaining: &mut u32,
) -> Result<crate::song::snapshot::FrozenRoutingInventory, Failure> {
    let remaining_work = *remaining;
    let mut builder = Inventory {
        evaluator,
        shapes,
        limits,
        remaining_work,
        payloads: BTreeMap::new(),
        out: Default::default(),
        seen: BTreeMap::new(),
        visiting: BTreeSet::new(),
        depth: 0,
        used: BTreeSet::new(),
    };
    builder.out.root_part = builder.part(song.part())?;
    let registry = evaluator
        .insts()
        .ok_or_else(|| fail("candidate registry unavailable"))?;
    let registry = registry.borrow();
    let used = std::mem::take(&mut builder.used);
    for id in used {
        let entry = registry
            .entry(id)
            .ok_or_else(|| fail("candidate instrument missing"))?;
        if !entry.signals.is_empty()
            || entry.def.nodes.iter().any(|n| {
                matches!(
                    n,
                    crate::dsp::graph::UGenSpec::HostInputL
                        | crate::dsp::graph::UGenSpec::HostInputR
                )
            })
        {
            return Err(fail("song instrument reads live input"));
        }
        builder.admit(entry.cells.len() + entry.params.len() + 1)?;
        let defaults = entry
            .cells
            .iter()
            .map(|(cell, _)| {
                entry
                    .default_cell_value(*cell)
                    .filter(|v| v.is_finite())
                    .map(|v| (*cell, v))
                    .ok_or_else(|| fail("instrument default is not finite"))
            })
            .collect::<Result<_, _>>()?;
        builder
            .out
            .instruments
            .push(crate::song::snapshot::FrozenInstrument {
                name: entry.name,
                graph: entry.def.clone(),
                parameters: entry.params.clone(),
                defaults,
                resource: entry.resource,
            });
    }
    for (name, entry) in registry.buses() {
        builder.admit(1)?;
        if !entry.signals.is_empty() {
            return Err(fail("song bus reads live input"));
        }
        builder.out.buses.push((name, entry.def.clone()));
    }
    builder.out.cells =
        super::cells::capture_frozen_cells(&builder.out, &mut builder.remaining_work)?;
    *remaining = builder.remaining_work;
    Ok(builder.out)
}
pub(super) struct Inventory<'a> {
    shapes: &'a super::source_uses::PreparedShapes,
    pub(super) evaluator: &'a crate::ns::evaluator::Evaluator,
    pub(super) limits: SongAssetLimits,
    pub(super) out: crate::song::snapshot::FrozenRoutingInventory,
    pub(super) remaining_work: u32,
    payloads: BTreeMap<usize, crate::song::snapshot::FrozenPattern>,
    pub(super) seen: BTreeMap<usize, usize>,
    pub(super) visiting: BTreeSet<usize>,
    pub(super) depth: u32,
    used: BTreeSet<crate::dsp::graph::InstId>,
}
impl Inventory<'_> {
    fn dependencies(
        &mut self,
        roots: &[Value],
    ) -> Result<crate::song::assets::SongAssetDependencies, Failure> {
        if self.remaining_work == 0 {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "aggregate inventory work exhausted",
            ));
        }
        let mut limits = self.limits;
        limits.max_walk_nodes = self.remaining_work;
        let dependencies = crate::song::assets::song_asset_dependencies(roots, limits)?;
        let spent = dependencies.consumed_work();
        self.remaining_work = self
            .remaining_work
            .checked_sub(spent)
            .ok_or_else(|| fail("metadata accounting underflow"))?;
        Ok(dependencies)
    }
    pub(super) fn payload(
        &mut self,
        pattern: &Rc<Pat>,
    ) -> Result<crate::song::snapshot::FrozenPattern, Failure> {
        use crate::host::caps::{InstResolver, Route};
        use crate::song::snapshot::{FrozenAudioRoute, FrozenPattern, FrozenSound};
        let identity = Rc::as_ptr(pattern) as usize;
        if let Some(payload) = self.payloads.get(&identity) {
            let cost = payload_copy_work(payload)?;
            self.admit(cost)?;
            return self
                .payloads
                .get(&identity)
                .cloned()
                .ok_or_else(|| fail("cached payload missing"));
        }
        let mut roots =
            self.shapes
                .dependency_roots(pattern, &mut self.remaining_work, self.limits)?;
        let first = self.dependencies(&roots)?;
        for keyword in first
            .sound_keywords
            .iter()
            .chain(first.literal_keywords.iter())
        {
            if let Some(family) = family(self.evaluator, *keyword)? {
                self.admit(1)?;
                roots.push(family);
            }
        }
        let deps = self.dependencies(&roots)?;
        self.admit(deps.selected_sources().len())?;
        let mut sources = Vec::new();
        for source in deps.selected_sources() {
            self.admit(source.selector().family().len())?;
            sources.push(crate::song::snapshot::FrozenSelectedSource {
                root_part: self.part(source.part())?,
                track: source.track(),
                family: source
                    .selector()
                    .family()
                    .iter()
                    .map(FrozenSound::from_sound)
                    .collect::<Result<_, Failure>>()?,
            });
        }
        self.admit(deps.selected_sources().len())?;
        let deps_selected = deps.selected_sources().to_vec();
        let mut sounds: Vec<_> = deps
            .sound_keywords
            .into_iter()
            .map(Sound::Builtin)
            .collect();
        sounds.extend(deps.instruments.into_iter().map(Sound::Inst));
        sounds.extend(deps.sample_paths.into_iter().map(Sound::Sample));
        sounds.extend(deps.buffers.into_iter().map(Sound::Buffer));
        let registry = self
            .evaluator
            .insts()
            .ok_or_else(|| fail("candidate registry unavailable"))?;
        let registry = registry.borrow();
        let mut families = BTreeMap::new();
        for sound in sounds {
            let Route::Audio { inst, sample } = registry.route(&sound)? else {
                return Err(fail("external song route"));
            };
            self.used.insert(inst);
            families.insert(
                FrozenSound::from_sound(&sound)?,
                FrozenAudioRoute {
                    instrument: inst,
                    sample,
                },
            );
        }
        let mut named_buses = BTreeSet::new();
        let mut work = vec![pattern.as_ref()];
        let mut seen = BTreeSet::new();
        while let Some(p) = work.pop() {
            if !seen.insert(p as *const Pat as usize) {
                continue;
            }
            if seen.len() > self.limits.max_walk_nodes as usize {
                return Err(fail("routing pattern work exceeded"));
            }
            if let PatNode::Control(name, value, _) = &p.node {
                if *name == crate::value::intern::intern_kw("bus") {
                    let selected = self.dependencies(&[Value::Pattern(value.clone())])?;
                    if selected.literal_keywords.is_empty() {
                        return Err(fail("dynamic bus destination cannot be certified"));
                    }
                    for name in selected.literal_keywords {
                        if registry.bus(name).is_none() {
                            return Err(fail("explicit named bus is undeclared"));
                        }
                        named_buses.insert(name);
                    }
                }
            }
            let pending = work
                .len()
                .checked_add(child_count(p))
                .ok_or_else(|| fail("routing queue overflow"))?;
            if pending > self.limits.max_walk_nodes as usize {
                return Err(Failure::new(
                    FailCode::FuelExhausted,
                    "routing queue exceeds remaining work",
                ));
            }
            work.extend(children(p));
        }
        if named_buses.len() > 1 {
            return Err(fail("conflicting named destinations in one song payload"));
        }
        let source_uses = super::source_uses::analyze(
            pattern,
            &deps_selected,
            &mut self.remaining_work,
            self.limits,
            self.shapes,
        )?;
        let index_timing = super::source_uses::timing::capture_index_timing(
            pattern,
            self.shapes,
            &mut self.remaining_work,
            self.limits,
        )?;
        self.admit(index_timing.retained_copy_work()? as usize)?;
        let payload = FrozenPattern {
            index_timing: Some(index_timing),
            id: pattern.id,
            families: families.into_iter().collect(),
            named_buses: named_buses.into_iter().collect(),
            sources,
            source_uses,
        };
        self.payloads.insert(identity, payload.clone());
        Ok(payload)
    }
}
impl Inventory<'_> {
    pub(super) fn admit(&mut self, count: usize) -> Result<(), Failure> {
        let count =
            u32::try_from(count).map_err(|_| failure("metadata collection length overflow"))?;
        self.remaining_work = self.remaining_work.checked_sub(count).ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "source metadata admission exhausted",
            )
        })?;
        Ok(())
    }
    pub(super) fn part(&mut self, part: &Rc<crate::song::Part>) -> Result<usize, Failure> {
        let identity = Rc::as_ptr(part) as usize;
        if let Some(index) = self.seen.get(&identity) {
            return Ok(*index);
        }
        if self.depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source policy depth exhausted",
            ));
        }
        if !self.visiting.insert(identity) {
            return Err(failure("cyclic selected source policy"));
        }
        self.depth += 1;
        let result = if let crate::song::PartNode::Repeat {
            child,
            count,
            seed_mode,
        } = part.node()
        {
            self.admit(
                part.tracks()
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| failure("repeat metadata length overflow"))?,
            )?;
            let child = self.part(child)?;
            Ok(self.finish_part(
                part,
                crate::song::snapshot::FrozenPartNode::Repeat {
                    child,
                    count: *count,
                    seed_mode: *seed_mode,
                },
            ))
        } else {
            self.part_inner(part)
        };
        self.depth -= 1;
        self.visiting.remove(&identity);
        result
    }
    fn finish_part(
        &mut self,
        part: &Rc<crate::song::Part>,
        node: crate::song::snapshot::FrozenPartNode,
    ) -> usize {
        let index = self.out.parts.len();
        self.seen.insert(Rc::as_ptr(part) as usize, index);
        self.out.parts.push(crate::song::snapshot::FrozenPart {
            revision: part.revision(),
            duration: part.duration(),
            tracks: part.tracks().iter().copied().collect(),
            node,
        });
        index
    }
    fn part_inner(&mut self, part: &Rc<crate::song::Part>) -> Result<usize, Failure> {
        use crate::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenSound};
        use crate::song::{PartEdit, PartNode};
        self.admit(
            part.tracks()
                .len()
                .checked_add(match part.node() {
                    PartNode::Capture(v) => v.len(),
                    PartNode::Sequence(v) => v.len(),
                    PartNode::Edit {
                        edit:
                            PartEdit::TransformInstrument { selector, .. }
                            | PartEdit::InstrumentFx { selector, .. },
                        ..
                    } => selector.family().len(),
                    _ => 1,
                })
                .ok_or_else(|| failure("part metadata length overflow"))?,
        )?;
        let family = |selector: &crate::song::InstrumentSelector| {
            selector
                .family()
                .iter()
                .map(FrozenSound::from_sound)
                .collect::<Result<Vec<_>, Failure>>()
        };
        let node = match part.node() {
            PartNode::Capture(tracks) => FrozenPartNode::Capture(
                tracks
                    .iter()
                    .map(|(k, p)| Ok((*k, self.payload(p)?)))
                    .collect::<Result<_, Failure>>()?,
            ),
            PartNode::Sequence(children) => {
                let mut offset = crate::value::ratio::Ratio64::ZERO;
                let mut placements = Vec::new();
                for child in children {
                    placements.push((offset, self.part(child)?));
                    offset = offset.checked_add(child.duration())?;
                }
                FrozenPartNode::Sequence(placements)
            }
            PartNode::Repeat {
                child,
                count,
                seed_mode,
            } => FrozenPartNode::Repeat {
                child: self.part(child)?,
                count: *count,
                seed_mode: *seed_mode,
            },
            PartNode::Edit { source, edit } => {
                let edit = match edit {
                    PartEdit::ReplaceTrack { track, pattern } => FrozenEdit::Replace {
                        track: *track,
                        payload: self.payload(pattern)?,
                    },
                    PartEdit::TransformInstrument {
                        track,
                        selector,
                        pattern,
                    } => FrozenEdit::Transform {
                        track: *track,
                        family: family(selector)?,
                        cutoff: source.revision(),
                        payload: self.payload(pattern)?,
                    },
                    PartEdit::DeleteEvent(handle) => FrozenEdit::Delete(handle.clone()),
                    PartEdit::OverwriteRegion {
                        track,
                        region,
                        pattern,
                    } => FrozenEdit::Overwrite {
                        track: *track,
                        region: *region,
                        payload: self.payload(pattern)?,
                    },
                    PartEdit::InstrumentFx {
                        track,
                        selector,
                        template,
                    } => {
                        let registry = self
                            .evaluator
                            .insts()
                            .ok_or_else(|| failure("candidate registry missing"))?;
                        if registry.borrow().bus(*template).is_none() {
                            return Err(failure("private effect template is undeclared"));
                        }
                        FrozenEdit::InstrumentFx {
                            track: *track,
                            family: family(selector)?,
                            template: *template,
                        }
                    }
                };
                FrozenPartNode::Edit {
                    source: self.part(source)?,
                    edit,
                }
            }
        };
        Ok(self.finish_part(part, node))
    }
}
fn payload_copy_work(payload: &crate::song::snapshot::FrozenPattern) -> Result<usize, Failure> {
    let graph = &payload.source_uses;
    let mut graph_cost = graph.nodes.len();
    if let Some(recipe) = payload.index_timing() {
        graph_cost = graph_cost
            .checked_add(recipe.retained_copy_work()? as usize)
            .ok_or_else(|| failure("index timing pointer retention overflow"))?;
    }
    for node in &graph.nodes {
        if let crate::song::source_uses::FrozenUseMapping::Slices { starts, .. } = &node.mapping {
            graph_cost = graph_cost
                .checked_add(starts.len())
                .ok_or_else(|| failure("slice metadata copy overflow"))?;
        }
        graph_cost = graph_cost
            .checked_add(node.edges.len())
            .ok_or_else(|| failure("graph copy overflow"))?;
        for edge in &node.edges {
            graph_cost = graph_cost
                .checked_add(edge.trace.len())
                .ok_or_else(|| failure("graph trace copy overflow"))?;
        }
    }
    payload.sources.iter().try_fold(
        payload
            .families
            .len()
            .checked_add(graph_cost)
            .and_then(|n| n.checked_add(payload.named_buses.len()))
            .and_then(|n| n.checked_add(payload.sources.len()))
            .ok_or_else(|| failure("payload length overflow"))?,
        |count, source| {
            count
                .checked_add(source.family.len())
                .ok_or_else(|| failure("payload family length overflow"))
        },
    )
}

#[cfg(test)]
mod source_inventory_test_support {
    use super::*;
    use crate::pattern::pat::{Pat, PatNode};
    use crate::song::{InstrumentSelector, Part, SongSource};
    pub(super) fn evaluator() -> crate::ns::evaluator::Evaluator {
        let mut ev = crate::ns::evaluator::Evaluator::new(
            crate::ns::namespace::Prelude::core(),
            Box::new(crate::ns::load::NoopHost),
            Box::new(crate::ns::stage::RecordingSink::default()),
        );
        assert!(ev
            .eval_str(
                "bus :plate-room:\n\tplate mix: 1\nbus :spring-room:\n\tspring-reverb mix: 1",
                crate::reader::span::FileId::new(0)
            )
            .unwrap()
            .iter()
            .all(|f| f.value.is_ok()));
        ev
    }
    pub(super) fn limits() -> SongAssetLimits {
        SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 1_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        }
    }
    pub(super) fn base() -> Rc<Part> {
        let pat = Rc::new(Pat::new(
            PatNode::Pure(crate::pattern::step::Step::bare(Value::Sound(Rc::new(
                Sound::Builtin(crate::value::intern::intern_kw("analog")),
            )))),
            None,
            false,
        ));
        Rc::new(
            crate::song::capture_part(
                BTreeMap::from([(crate::value::intern::intern_kw("drums"), pat)]),
                crate::value::Ratio64::ONE,
            )
            .unwrap(),
        )
    }
    pub(super) fn selected(part: Rc<Part>) -> Rc<Part> {
        let track = crate::value::intern::intern_kw("drums");
        let selector = InstrumentSelector::new(vec![
            Sound::Builtin(crate::value::intern::intern_kw("analog")),
            Sound::Builtin(crate::value::intern::intern_kw("bd")),
        ])
        .unwrap();
        let source = Rc::new(SongSource::new(part, track, selector).unwrap());
        Rc::new(
            crate::song::capture_part(
                BTreeMap::from([(
                    track,
                    Rc::new(Pat::new(PatNode::SongSource(source), None, false)),
                )]),
                crate::value::Ratio64::ONE,
            )
            .unwrap(),
        )
    }
}

#[cfg(test)]
mod source_inventory_tests {
    use self::source_inventory_test_support::*;
    use super::*;
    use crate::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenSound};
    use crate::song::source::SongSource;
    use crate::song::{InstrumentSelector, PartEdit, RepeatSeedMode, SongSettings};
    #[test]
    fn external_source_policies_keep_offsets_repeats_nonadjacent_fx_and_root_identity() {
        let track = crate::value::intern::intern_kw("drums");
        let seed = base();
        let selector = InstrumentSelector::new(vec![Sound::Builtin(
            crate::value::intern::intern_kw("analog"),
        )])
        .unwrap();
        let fx = |name| {
            Rc::new(
                seed.clone()
                    .edit(PartEdit::InstrumentFx {
                        track,
                        selector: selector.clone(),
                        template: crate::value::intern::intern_kw(name),
                    })
                    .unwrap(),
            )
        };
        let plate = fx("plate-room");
        let spring = fx("spring-room");
        let sequence =
            Rc::new(crate::song::sequence(vec![plate.clone(), seed, spring, plate]).unwrap());
        let external =
            Rc::new(crate::song::part_repeat(sequence, 5, RepeatSeedMode::Same).unwrap());
        let root = selected(external.clone());
        let song = Song::new(root.clone(), SongSettings::default()).unwrap();
        let inventory = capture_routing(&evaluator(), &song, limits()).unwrap();
        assert_eq!(
            inventory.parts[inventory.root_part].revision,
            root.revision()
        );
        let FrozenPartNode::Capture(tracks) = &inventory.parts[inventory.root_part].node else {
            panic!()
        };
        let source = &tracks[0].1.sources[0];
        assert_eq!(source.track, track);
        assert_eq!(source.family.len(), 2);
        assert_eq!(
            source.family[0],
            FrozenSound::Builtin(crate::value::intern::intern_kw("analog"))
        );
        assert_eq!(
            inventory.parts[source.root_part].revision,
            external.revision()
        );
        let FrozenPartNode::Repeat { child, count, .. } = inventory.parts[source.root_part].node
        else {
            panic!()
        };
        assert_eq!(count, 5);
        let FrozenPartNode::Sequence(rows) = &inventory.parts[child].node else {
            panic!()
        };
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[3].0, crate::value::Ratio64::from_int(3));
        assert_eq!(rows[0].1, rows[3].1);
        assert!(inventory.parts.iter().any(|p|matches!(&p.node,FrozenPartNode::Edit{edit:FrozenEdit::InstrumentFx{template,..},..} if *template==crate::value::intern::intern_kw("spring-room"))));
    }
    #[test]
    fn cyclic_selected_policies_reject_and_descriptor_admission_is_aggregate() {
        use crate::ns::namespace::{SlotKind, VarSlotRef};
        let track = crate::value::intern::intern_kw("drums");
        let slot = VarSlotRef::new(
            crate::value::intern::intern_sym("source-cycle"),
            SlotKind::Var,
            Value::Nil,
        );
        let pattern = Rc::new(Pat::new(
            PatNode::Sound {
                src: crate::pattern::pat::PParam::Late(slot.clone()),
                kit: None,
            },
            None,
            false,
        ));
        let part = Rc::new(
            crate::song::capture_part(
                BTreeMap::from([(track, pattern)]),
                crate::value::Ratio64::ONE,
            )
            .unwrap(),
        );
        let source = Rc::new(
            SongSource::new(
                part.clone(),
                track,
                InstrumentSelector::new(vec![Sound::Builtin(crate::value::intern::intern_kw(
                    "analog",
                ))])
                .unwrap(),
            )
            .unwrap(),
        );
        slot.set(Value::Pattern(Rc::new(Pat::new(
            PatNode::SongSource(source),
            None,
            false,
        ))));
        let result = capture_routing(
            &evaluator(),
            &Song::new(part, SongSettings::default()).unwrap(),
            limits(),
        );
        slot.set(Value::Nil);
        let error = result.unwrap_err();
        assert_eq!(error.code, FailCode::Type);
        assert!(error.message.contains("cyclic"));
        let ev = evaluator();
        let mut builder = Inventory {
            shapes: &super::super::source_uses::PreparedShapes::default(),
            evaluator: &ev,
            limits: limits(),
            out: Default::default(),
            remaining_work: 16,
            payloads: BTreeMap::new(),
            seen: BTreeMap::new(),
            visiting: BTreeSet::new(),
            depth: 0,
            used: BTreeSet::new(),
        };
        builder.admit(10).unwrap();
        assert_eq!(builder.remaining_work, 6);
        assert_eq!(builder.admit(7).unwrap_err().code, FailCode::FuelExhausted);
    }
    #[test]
    fn timing_wrapper_sources_honor_depth_two_hundred_and_three_hundred() {
        let track = crate::value::intern::intern_kw("drums");
        for depth in [200, 300] {
            let mut pattern = Rc::new(Pat::new(
                PatNode::SongSource(Rc::new(
                    SongSource::new(
                        base(),
                        track,
                        InstrumentSelector::new(vec![Sound::Builtin(
                            crate::value::intern::intern_kw("analog"),
                        )])
                        .unwrap(),
                    )
                    .unwrap(),
                )),
                None,
                false,
            ));
            for i in 0..depth {
                let param = crate::pattern::pat::PParam::Const(Value::Int(1));
                pattern = Rc::new(Pat::new(
                    if i % 2 == 0 {
                        PatNode::Fast(pattern, param)
                    } else {
                        PatNode::Slow(pattern, param)
                    },
                    None,
                    false,
                ));
            }
            let song = Song::new(
                Rc::new(
                    crate::song::capture_part(
                        BTreeMap::from([(track, pattern)]),
                        crate::value::Ratio64::ONE,
                    )
                    .unwrap(),
                ),
                SongSettings::default(),
            )
            .unwrap();
            let result = capture_routing(&evaluator(), &song, limits());
            if depth == 200 {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
            }
        }
    }
    #[test]
    fn selected_policy_depth_two_hundred_is_bounded_on_normal_stack() {
        let mut source = base();
        for _ in 0..200 {
            source = Rc::new(crate::song::part_repeat(source, 1, RepeatSeedMode::Same).unwrap());
        }
        let root = selected(source);
        let song = Song::new(root, SongSettings::default()).unwrap();

        let inventory = capture_routing(&evaluator(), &song, limits()).unwrap();

        assert_eq!(inventory.parts.len(), 202);
        let mut bound = limits();
        bound.max_walk_depth = 100;
        assert_eq!(
            capture_routing(&evaluator(), &song, bound)
                .unwrap_err()
                .code,
            FailCode::DepthExceeded
        );
        let track = crate::value::intern::intern_kw("drums");
        let mut pattern = Rc::new(Pat::new(
            PatNode::SongSource(Rc::new(
                SongSource::new(
                    base(),
                    track,
                    InstrumentSelector::new(vec![Sound::Builtin(crate::value::intern::intern_kw(
                        "analog",
                    ))])
                    .unwrap(),
                )
                .unwrap(),
            )),
            None,
            false,
        ));
        for _ in 0..300 {
            pattern = Rc::new(Pat::new(PatNode::Rev(pattern), None, false));
        }
        let deep = Song::new(
            Rc::new(
                crate::song::capture_part(
                    BTreeMap::from([(track, pattern)]),
                    crate::value::Ratio64::ONE,
                )
                .unwrap(),
            ),
            SongSettings::default(),
        )
        .unwrap();

        assert_eq!(
            capture_routing(&evaluator(), &deep, limits())
                .unwrap_err()
                .code,
            FailCode::DepthExceeded
        );
    }
}

#[cfg(test)]
mod selected_source_tests {
    use super::*;
    use crate::pattern::pat::{Pat, PatNode};
    use crate::song::assets::song_asset_dependencies;
    use crate::song::{capture_part, InstrumentSelector, SongSource};
    use crate::value::value::{Sound, Value};
    #[test]
    fn selected_source_aliases_are_private_unique_and_keep_exact_walk_work() {
        let track = crate::value::intern::intern_kw("drums");
        let part = Rc::new(
            capture_part(
                BTreeMap::from([(
                    track,
                    Rc::new(Pat::new(
                        PatNode::Pure(crate::pattern::step::Step::bare(Value::Nil)),
                        None,
                        false,
                    )),
                )]),
                crate::value::Ratio64::ONE,
            )
            .unwrap(),
        );
        let source = Rc::new(
            SongSource::new(
                part,
                track,
                InstrumentSelector::new(vec![Sound::Builtin(crate::value::intern::intern_kw(
                    "analog",
                ))])
                .unwrap(),
            )
            .unwrap(),
        );
        let pattern = Rc::new(Pat::new(PatNode::SongSource(source.clone()), None, false));
        let limits = SongAssetLimits {
            max_resources: 1,
            max_pcm_bytes: 1024,
            max_source_files: 1,
            max_source_bytes: 1024,
            max_banks: 1,
            max_walk_nodes: 1000,
            max_walk_depth: 256,
        };
        let once = song_asset_dependencies(&[Value::Pattern(pattern.clone())], limits).unwrap();
        let twice = song_asset_dependencies(
            &[Value::Pattern(pattern.clone()), Value::Pattern(pattern)],
            limits,
        )
        .unwrap();
        assert_eq!(once.selected_sources().len(), 1);
        assert_eq!(twice.selected_sources().len(), 1);
        assert!(Rc::ptr_eq(&once.selected_sources()[0], &source));
        assert_eq!(once.consumed_work(), 5);
        assert_eq!(twice.consumed_work(), 7);
    }
}

#[cfg(test)]
mod index_occupancy;
