//! Selected-source cover consumers and causal detector admission.

use super::*;
use crate::vm::fail::{FailCode, Failure};

/// Each detector sees the completed pre-track-effect sum of private outputs.
/// Add these dependencies to the restricted audio topology before accepting
/// template instances; a template reused on another branch is another target.
pub(super) fn validate_detectors(
    tracks: &[SongTrackRoute],
    branches: &[SongBranchRoute],
    bindings: &[SongSidechainRoute],
) -> Result<(), Failure> {
    let master = branches
        .len()
        .checked_add(tracks.len().checked_mul(2).ok_or_else(capacity_overflow)?)
        .ok_or_else(capacity_overflow)?;
    let mut edges = vec![Vec::new(); master.checked_add(1).ok_or_else(capacity_overflow)?];
    let track_index = |track| {
        tracks
            .iter()
            .position(|t| t.track == track)
            .map(|index| branches.len() + index)
            .ok_or_else(|| Failure::new(FailCode::Type, "detector track is outside song topology"))
    };
    for (index, branch) in branches.iter().enumerate() {
        edges[index].push(track_index(branch.track)?);
    }
    for index in 0..tracks.len() {
        let input = branches.len() + index;
        let output = branches.len() + tracks.len() + index;
        edges[input].push(output);
        edges[output].push(master);
    }
    for binding in bindings {
        let source = track_index(binding.source_track)?;
        let target = match binding.target {
            SongDetectorTarget::Branch(id) => branches
                .iter()
                .position(|b| b.id == id)
                .ok_or_else(|| Failure::new(FailCode::Type, "invalid detector branch target"))?,
            SongDetectorTarget::Track(track) => track_index(track)? + tracks.len(),
            SongDetectorTarget::Master => master,
        };
        edges[source].push(target);
    }
    let mut incoming = vec![0usize; edges.len()];
    for edge in &edges {
        for &target in edge {
            incoming[target] = incoming[target]
                .checked_add(1)
                .ok_or_else(capacity_overflow)?;
        }
    }
    let mut ready: std::collections::VecDeque<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, &n)| (n == 0).then_some(index))
        .collect();
    let mut completed = 0;
    while let Some(index) = ready.pop_front() {
        completed += 1;
        for &target in &edges[index] {
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push_back(target);
            }
        }
    }
    if completed != edges.len() {
        let owner = bindings
            .iter()
            .find(|b| incoming[track_index(b.source_track).unwrap_or(master)] > 0);
        let addressed = owner.map_or_else(
            || "unknown".into(),
            |b| {
                format!(
                    "source_track={}, target={:?}",
                    crate::value::intern::name_of_kw(b.source_track),
                    b.target
                )
            },
        );
        return Err(Failure::new(
            FailCode::BeyondCapability,
            format!("song detector causal cycle: {addressed}"),
        ));
    }
    Ok(())
}

pub(super) fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, format!("song route: {message}"))
}
pub(super) struct ResolutionBudget {
    remaining: u32,
    max_depth: u32,
}
impl ResolutionBudget {
    pub(super) fn new(limits: crate::song::SongLimits) -> Self {
        Self {
            remaining: limits.max_nodes,
            max_depth: limits.max_depth,
        }
    }
    pub(super) fn limits(&self) -> crate::song::SongLimits {
        crate::song::SongLimits {
            max_nodes: self.remaining,
            max_depth: self.max_depth,
            ..Default::default()
        }
    }
    pub(super) fn with_remaining<T>(
        &mut self,
        f: impl FnOnce(&mut u32) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        f(&mut self.remaining)
    }
    pub(super) fn charge(&mut self, count: usize) -> Result<(), Failure> {
        let count = u32::try_from(count).map_err(|_| capacity_overflow())?;
        self.remaining = self.remaining.checked_sub(count).ok_or_else(|| {
            Failure::new(FailCode::FuelExhausted, "route resolution work exhausted")
        })?;
        Ok(())
    }
    pub(super) fn enter(&mut self, depth: u32) -> Result<(), Failure> {
        if depth > self.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "route placement depth exhausted",
            ));
        }
        self.charge(1)
    }
}
pub(super) fn node_count(
    inventory: &FrozenRoutingInventory,
    index: usize,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<u32, Failure> {
    use crate::song::snapshot::FrozenPartNode as P;
    budget.enter(depth)?;
    let part = inventory
        .parts
        .get(index)
        .ok_or_else(|| invalid("invalid placement Part"))?;
    let mut count = 1u32;
    match &part.node {
        P::Capture(_) => {}
        P::Sequence(children) => {
            for (_, child) in children {
                count = count
                    .checked_add(node_count(inventory, *child, depth + 1, budget)?)
                    .ok_or_else(capacity_overflow)?;
            }
        }
        P::Repeat { child, .. } | P::Edit { source: child, .. } => {
            count = count
                .checked_add(node_count(inventory, *child, depth + 1, budget)?)
                .ok_or_else(capacity_overflow)?;
        }
    }
    Ok(count)
}
fn branch_interval(
    plan: &SongRoutePlan,
    branch: &SongBranchRoute,
    event: &crate::song::snapshot::FrozenSongEvent,
    budget: &mut ResolutionBudget,
) -> Result<Option<(crate::pattern::TimeSpan, Ratio64)>, Failure> {
    use crate::pattern::TimeSpan;
    use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
    let path = &event.handle.placement().0;
    let mut cursor = 0usize;
    let mut offset = Ratio64::ZERO;
    let mut regions = Vec::new();
    let mut depth = 0u32;
    for edge in &branch.placement {
        if matches!(edge, SongRoutePlacement::Region { .. }) {
            budget.charge(1)?;
        } else {
            depth = depth.checked_add(1).ok_or_else(capacity_overflow)?;
            budget.enter(depth)?;
        }
        match edge {
            SongRoutePlacement::Sequence {
                child,
                offset: shift,
                ..
            } => {
                if path.get(cursor..cursor + 2) != Some(&[1, *child]) {
                    return Ok(None);
                }
                cursor += 2;
                offset = offset.checked_add(*shift)?;
            }
            SongRoutePlacement::Repeat { part, count } => {
                let Some(&[2, ordinal]) = path.get(cursor..cursor + 2) else {
                    return Ok(None);
                };
                if ordinal >= *count {
                    return Ok(None);
                }
                let P::Repeat { child, .. } = plan
                    .topology
                    .parts
                    .get(*part)
                    .ok_or_else(|| invalid("repeat scope"))?
                    .node
                else {
                    return Err(invalid("repeat recipe"));
                };
                let duration = plan
                    .topology
                    .parts
                    .get(child)
                    .ok_or_else(|| invalid("repeat child"))?
                    .duration;
                offset = offset
                    .checked_add(duration.checked_mul(Ratio64::from_int(i64::from(ordinal)))?)?;
                cursor += 2;
            }
            SongRoutePlacement::Region { part, span, inside } => {
                let owner = plan
                    .topology
                    .parts
                    .get(*part)
                    .ok_or_else(|| invalid("region owner"))?;
                let P::Edit {
                    edit: E::Overwrite { track, .. },
                    ..
                } = &owner.node
                else {
                    return Err(invalid("region recipe is not overwrite"));
                };
                if *track == branch.track {
                    budget.charge(1)?;
                    regions.push((span.map(|t| t.checked_add(offset))?, *inside));
                }
            }
            SongRoutePlacement::Edit { .. } => {}
        }
    }
    let scope = plan
        .topology
        .parts
        .get(branch.scope_part)
        .ok_or_else(|| invalid("branch owner scope"))?;
    if matches!(scope.node, P::Capture(_)) {
        budget.enter(depth.checked_add(1).ok_or_else(capacity_overflow)?)?;
    }
    if let P::Edit { edit, .. } = &scope.node {
        let tag = match edit {
            E::Replace { .. } => 3,
            E::Overwrite { .. } => 4,
            E::Transform { .. } => 5,
            _ => return Ok(None),
        };
        let count = node_count(&plan.topology, branch.scope_part, depth.max(1), budget)?;
        if path.get(cursor..cursor + 2) != Some(&[tag, count]) {
            return Ok(None);
        }
        cursor += 2;
    }
    if cursor != path.len() {
        return Ok(None);
    }
    let birth = event.handle.occurrence().onset;
    let mut span = TimeSpan::new(offset, offset.checked_add(scope.duration)?)?;
    if birth < span.begin || birth >= span.end {
        return Ok(None);
    }
    for (region, inside) in regions {
        if inside {
            if birth < region.begin || birth >= region.end {
                return Ok(None);
            }
            span = TimeSpan::new(span.begin.max(region.begin), span.end.min(region.end))?;
        } else if birth < region.begin {
            span = TimeSpan::new(span.begin, span.end.min(region.begin))?;
        } else if birth >= region.end {
            span = TimeSpan::new(span.begin.max(region.end), span.end)?;
        } else {
            return Ok(None);
        }
    }
    Ok(Some((span, offset)))
}
/// Authenticate the realized root and typed placement before route selection.
/// Configuration keys contain intrinsic supports, never individual note/tone.
pub fn resolve_route(
    plan: &SongRoutePlan,
    event: &crate::song::snapshot::FrozenSongEvent,
    limits: crate::song::SongLimits,
) -> Result<SongResolvedRoute, Failure> {
    limits.validate()?;
    let root = plan
        .topology
        .parts
        .get(plan.topology.root_part)
        .ok_or_else(|| invalid("missing prepared root"))?;
    if event.handle.revision() != root.revision
        || event.handle.track() != event.track
        || !root.tracks.contains(&event.track)
    {
        return Err(invalid(
            "event root revision or track does not match prepared topology",
        ));
    }
    let template = event.route.as_ref().map(|(_, template)| *template);
    let mut budget = ResolutionBudget::new(limits);
    budget.charge(root.tracks.len())?;
    for branch in &plan.branches {
        budget.charge(1)?;
        if branch.track != event.track
            || branch.instrument != event.instrument
            || branch.effect_template != template
        {
            continue;
        }
        let Some((mut configuration, offset)) = branch_interval(plan, branch, event, &mut budget)?
        else {
            continue;
        };
        let matched = super::nested::resolve_sources(
            plan,
            event,
            branch,
            offset,
            configuration,
            &mut budget,
        )?;
        configuration = matched.configuration;
        let sources = matched.sources;
        budget.charge(event.handle.placement().0.len())?;
        return Ok(SongResolvedRoute {
            branch: branch.id,
            placement: event.handle.placement().clone(),
            sources,
            configuration,
        });
    }
    Err(invalid(
        "event does not match admitted track/family/placement/configuration",
    ))
}

pub(super) fn reserve_source_search(
    cover: &crate::song::source_uses::FrozenSourceUseCover,
    payload: &crate::song::snapshot::FrozenPattern,
    origin: crate::song::source_uses::OriginView<'_>,
    base_depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<crate::song::SongLimits, Failure> {
    budget.charge(crate::song::source_uses::origin::SOURCE_AUTHORITY_INSPECTION_WORK as usize)?;
    let remaining_depth = budget
        .max_depth
        .checked_sub(base_depth)
        .filter(|d| *d > 0)
        .ok_or_else(|| {
            Failure::new(
                FailCode::DepthExceeded,
                "nested source matcher depth exhausted",
            )
        })?;
    let authority_work = origin.authority_work()?;
    let timing_before = budget.remaining;
    budget.with_remaining(|remaining| {
        crate::song::source::slices::validate_frozen_slice_timings(
            origin.slice_timings,
            origin.issued_handle,
            remaining,
            remaining_depth,
        )
    })?;
    let timing_work = timing_before
        .checked_sub(budget.remaining)
        .ok_or_else(capacity_overflow)?;
    // The validator probe was actually spent. Reserve a separate equal debit
    // for resolve_origin's validation; neither pass borrows a fresh quota.
    let before = budget.remaining;
    budget.charge(authority_work as usize)?;
    budget.charge(timing_work as usize)?;
    let placement_words = origin.handle().placement().0.len();
    let mut probe_work = 0;
    search_allowance(
        cover,
        payload,
        SearchPosition {
            index: cover.graph().root,
            depth: base_depth.checked_add(1).ok_or_else(capacity_overflow)?,
            placement_words,
            copy_words: 0,
        },
        budget,
        &mut probe_work,
    )?;
    Ok(crate::song::SongLimits {
        max_nodes: before
            .checked_sub(budget.remaining)
            .and_then(|work| work.checked_sub(probe_work))
            .ok_or_else(capacity_overflow)?
            .max(1),
        max_depth: remaining_depth,
        ..Default::default()
    })
}

pub(super) fn reserve_grid_search(
    recipe: crate::song::source_uses::FrozenStaticSampling,
    budget: &mut ResolutionBudget,
) -> Result<u32, Failure> {
    let before = budget.limits().max_nodes;
    let phases = budget.with_remaining(|remaining| {
        crate::song::source_uses::sampling::enabled_phases(recipe, remaining)
    })?;
    let mask_work = before
        .checked_sub(budget.limits().max_nodes)
        .ok_or_else(capacity_overflow)?;
    // The probe was actually spent, separate from two matcher masks and scans.
    budget.charge(
        usize::try_from(mask_work.checked_mul(2).ok_or_else(capacity_overflow)?)
            .map_err(|_| capacity_overflow())?
            .checked_add(phases.len().checked_mul(5).ok_or_else(capacity_overflow)?)
            .ok_or_else(capacity_overflow)?,
    )?;
    Ok(mask_work)
}

/// Reserve a full symbolic traversal upper bound, including duplicate DAG edges.
/// The existing borrowed matcher cannot spend beyond this allowance; each
/// nested call will reserve from the same outer budget before allocating.
struct SearchPosition {
    index: u32,
    depth: u32,
    placement_words: usize,
    copy_words: usize,
}
fn search_allowance(
    cover: &crate::song::source_uses::FrozenSourceUseCover,
    payload: &crate::song::snapshot::FrozenPattern,
    position: SearchPosition,
    budget: &mut ResolutionBudget,
    probe_work: &mut u32,
) -> Result<(), Failure> {
    let SearchPosition {
        index,
        depth,
        placement_words,
        copy_words,
    } = position;
    use crate::song::source_uses::FrozenUseMapping as M;
    budget.enter(depth)?;
    let node = cover
        .graph()
        .nodes
        .get(index as usize)
        .ok_or_else(|| invalid("source matcher node"))?;
    if let M::SampleGrid { sampling, .. } = node.mapping {
        *probe_work = probe_work
            .checked_add(reserve_grid_search(sampling, budget)?)
            .ok_or_else(capacity_overflow)?;
    }
    if let M::Source { policy } = node.mapping {
        let source = payload
            .sources
            .get(policy as usize)
            .ok_or_else(|| invalid("source matcher policy"))?;
        budget.charge(source.family.len())?;
        budget.charge(
            placement_words
                .checked_add(copy_words)
                .and_then(|n| n.checked_add(depth as usize))
                .ok_or_else(capacity_overflow)?,
        )?;
    }
    for edge in &node.edges {
        budget.charge(
            edge.trace
                .len()
                .checked_add(edge.layout.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(capacity_overflow)?,
        )?;
        search_allowance(
            cover,
            payload,
            SearchPosition {
                index: edge.child,
                depth: depth.checked_add(1).ok_or_else(capacity_overflow)?,
                placement_words,
                copy_words: copy_words
                    .checked_add(
                        edge.trace
                            .iter()
                            .filter(|term| {
                                matches!(
                                    term,
                                    crate::song::source_uses::FrozenUseTraceTerm::Copies { .. }
                                )
                            })
                            .count(),
                    )
                    .ok_or_else(capacity_overflow)?,
            },
            budget,
            probe_work,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod matcher_budget_tests {
    use super::*;
    use crate::pattern::eval::{InputCells, QueryVm};
    use crate::pattern::{
        pat::{PParam, Pat, PatNode},
        step::Step,
        TimeSpan,
    };
    use crate::song::source_uses::*;
    use crate::song::{capture_part, query_part, InstrumentSelector, SongLimits, SongQueryCtx};
    use crate::value::{
        intern::intern_kw,
        value::{Sound, Value},
    };
    use std::rc::Rc;
    struct NoCalls;
    impl QueryVm for NoCalls {
        fn call(&mut self, _: &Value, _: &[Value]) -> Result<Value, Failure> {
            Err(invalid("unexpected callback"))
        }
        fn deref(&mut self, r: &crate::ns::namespace::VarSlotRef) -> Result<Value, Failure> {
            Ok(r.get())
        }
        fn take_output(&mut self) -> Vec<(crate::vm::fail::Origin, Rc<str>)> {
            Vec::new()
        }
        fn put_output(&mut self, out: Vec<(crate::vm::fail::Origin, Rc<str>)>) {
            assert!(out.is_empty());
        }
        fn sound_kit(&mut self) -> Result<Value, Failure> {
            Ok(Value::dict(Default::default()))
        }
    }
    #[test]
    fn genuine_repeat_origin_matches_flat_symbolic_copy_recipe() {
        let track = intern_kw("drums");
        let sound = Sound::Builtin(intern_kw("analog"));
        let original = Rc::new(
            capture_part(
                [(
                    track,
                    Rc::new(crate::pattern::build::pure(
                        Value::Sound(Rc::new(sound.clone())),
                        None,
                    )),
                )]
                .into(),
                Ratio64::ONE,
            )
            .unwrap(),
        );
        let source = Rc::new(
            crate::song::source::SongSource::new(
                original.clone(),
                track,
                InstrumentSelector::new(vec![sound]).unwrap(),
            )
            .unwrap(),
        );
        let mut value = Value::Pattern(Rc::new(Pat::new(PatNode::SongSource(source), None, true)));
        for _ in 0..16 {
            let child = Rc::new(Pat::new(
                PatNode::Steps(vec![Step::bare(value)].into()),
                None,
                true,
            ));
            value = Value::Pattern(Rc::new(Pat::new(
                PatNode::Repeat(child, PParam::Const(Value::Int(1))),
                None,
                true,
            )));
        }
        let part = capture_part(
            [(
                track,
                Rc::new(Pat::new(
                    PatNode::Steps(vec![Step::bare(value)].into()),
                    None,
                    true,
                )),
            )]
            .into(),
            Ratio64::ONE,
        )
        .unwrap();
        let mut vm = NoCalls;
        let cells = InputCells::new();
        let limits = SongLimits::default();
        let rows = query_part(
            &part,
            TimeSpan::cycle(0).unwrap(),
            &mut SongQueryCtx {
                vm: &mut vm,
                cells: &cells,
                seed: 1,
                tempo: crate::clock::tempo::Tempo::default(),
                limits: &limits,
            },
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        let mut copy_work = 10000;
        let origin = copy_origin(
            rows[0].event.song_source.as_ref().unwrap(),
            &mut copy_work,
            256,
        )
        .unwrap();
        let trace: Vec<_> = origin
            .entry_trace
            .iter()
            .map(|step| {
                if step.kind == crate::pattern::occ::ProducerKind::GeneratedBranch {
                    FrozenUseTraceTerm::Copies {
                        kind: step.kind,
                        count: 1,
                    }
                } else {
                    FrozenUseTraceTerm::Exact(*step)
                }
            })
            .collect();
        let layout: Vec<_> = trace
            .iter()
            .enumerate()
            .filter_map(|(index, term)| {
                matches!(term, FrozenUseTraceTerm::Copies { .. }).then_some(
                    layout::FrozenUseSlotLayout {
                        prefix: Ratio64::ZERO,
                        width: Ratio64::ONE,
                        copy_trace_term: Some(index as u32),
                    },
                )
            })
            .collect();
        assert_eq!(layout.len(), 16);
        let empty = FrozenPattern {
            index_timing: None,
            id: crate::reader::span::NodeId::new(0),
            families: vec![(
                FrozenSound::Builtin(intern_kw("analog")),
                crate::song::snapshot::FrozenAudioRoute {
                    instrument: crate::dsp::graph::InstId::new(0),
                    sample: None,
                },
            )],
            named_buses: Vec::new(),
            sources: Vec::new(),
            source_uses: FrozenSourceUseGraph {
                root: 0,
                nodes: vec![FrozenSourceUseNode {
                    operation: FrozenUseOperation::Pure,
                    mapping: FrozenUseMapping::Empty,
                    edges: Vec::new(),
                }],
            },
        };
        let inventory = FrozenRoutingInventory {
            parts: vec![crate::song::snapshot::FrozenPart {
                revision: original.revision(),
                duration: Ratio64::ONE,
                tracks: vec![track],
                node: crate::song::snapshot::FrozenPartNode::Capture(vec![(track, empty)]),
            }],
            root_part: 0,
            ..Default::default()
        };
        let payload = FrozenPattern {
            index_timing: None,
            id: crate::reader::span::NodeId::new(1),
            families: Vec::new(),
            named_buses: Vec::new(),
            sources: vec![FrozenSelectedSource {
                root_part: 0,
                track,
                family: vec![FrozenSound::Builtin(intern_kw("analog"))],
            }],
            source_uses: FrozenSourceUseGraph {
                root: 0,
                nodes: vec![
                    FrozenSourceUseNode {
                        operation: FrozenUseOperation::Steps,
                        mapping: FrozenUseMapping::Preserve,
                        edges: vec![FrozenSourceUseEdge {
                            trace,
                            layout,
                            child: 1,
                        }],
                    },
                    FrozenSourceUseNode {
                        operation: FrozenUseOperation::Source,
                        mapping: FrozenUseMapping::Source { policy: 0 },
                        edges: Vec::new(),
                    },
                ],
            },
        };
        let cover =
            certify_source_uses(&inventory, &payload, TimeSpan::cycle(0).unwrap(), limits).unwrap();
        let mut budget = ResolutionBudget {
            remaining: limits.max_nodes,
            max_depth: limits.max_depth,
        };
        let matcher =
            reserve_source_search(&cover, &payload, origin.borrowed_view(), 0, &mut budget)
                .unwrap();
        let allowance = limits.max_nodes - budget.remaining;
        let identity = resolve_source_use(
            &cover,
            &origin,
            SongLimits {
                max_nodes: matcher.max_nodes,
                ..limits
            },
        )
        .unwrap();
        assert_eq!(identity.copies.len(), 16);
        assert_eq!(identity.edges.len(), 1);
        let mut tiny = ResolutionBudget {
            remaining: allowance - 1,
            max_depth: limits.max_depth,
        };
        assert_eq!(
            reserve_source_search(&cover, &payload, origin.borrowed_view(), 0, &mut tiny)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
    }

    #[test]
    fn genuine_index_timing_reserves_probe_and_matcher_with_exact_quota_and_depth() {
        let track = intern_kw("drums");
        let sound = Sound::Builtin(intern_kw("analog"));
        let original = Rc::new(
            capture_part(
                [(
                    track,
                    Rc::new(crate::pattern::build::pure(
                        Value::Sound(Rc::new(sound.clone())),
                        None,
                    )),
                )]
                .into(),
                Ratio64::ONE,
            )
            .unwrap(),
        );
        let selected = Rc::new(Pat::new(
            PatNode::SongSource(Rc::new(
                crate::song::SongSource::new(
                    original.clone(),
                    track,
                    InstrumentSelector::new(vec![sound]).unwrap(),
                )
                .unwrap(),
            )),
            None,
            false,
        ));
        let slicing = Rc::new(crate::pattern::combinators::region::slice(
            selected,
            crate::pattern::pat::SliceCuts::Equal(PParam::Const(Value::Int(2))),
            Rc::new(crate::pattern::build::value_steps(&[Value::Int(0)])),
            None,
        ));
        let part = capture_part([(track, slicing.clone())].into(), Ratio64::ONE).unwrap();
        let mut vm = NoCalls;
        let cells = InputCells::new();
        let limits = SongLimits::default();
        let rows = query_part(
            &part,
            TimeSpan::cycle(0).unwrap(),
            &mut SongQueryCtx {
                vm: &mut vm,
                cells: &cells,
                seed: 1,
                tempo: crate::clock::tempo::Tempo::default(),
                limits: &limits,
            },
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        let origin = copy_origin(
            rows[0].event.song_source.as_ref().unwrap(),
            &mut 100000,
            limits.max_depth,
        )
        .unwrap();
        assert_eq!(origin.slice_timings().len(), 1);
        assert_eq!(origin.slice_timings()[0].issuer(), slicing.id);
        assert_eq!(
            origin.slice_timings()[0].index_whole(),
            rows[0].event.whole.unwrap()
        );
        assert_eq!(origin.slice_timings()[0].subject_handle(), &origin.handle);

        // This matcher-only recipe uses the actual queried subject trace.
        // Full capture/preparation/route equality is covered by inventory fixtures.
        let trace = origin
            .entry_trace
            .iter()
            .copied()
            .map(FrozenUseTraceTerm::Exact)
            .collect();
        let empty = FrozenPattern {
            index_timing: None,
            id: crate::reader::span::NodeId::new(0),
            families: vec![(
                FrozenSound::Builtin(intern_kw("analog")),
                crate::song::snapshot::FrozenAudioRoute {
                    instrument: crate::dsp::graph::InstId::new(0),
                    sample: None,
                },
            )],
            named_buses: Vec::new(),
            sources: Vec::new(),
            source_uses: FrozenSourceUseGraph {
                root: 0,
                nodes: vec![FrozenSourceUseNode {
                    operation: FrozenUseOperation::Pure,
                    mapping: FrozenUseMapping::Empty,
                    edges: Vec::new(),
                }],
            },
        };
        let inventory = FrozenRoutingInventory {
            parts: vec![crate::song::snapshot::FrozenPart {
                revision: original.revision(),
                duration: Ratio64::ONE,
                tracks: vec![track],
                node: crate::song::snapshot::FrozenPartNode::Capture(vec![(track, empty)]),
            }],
            root_part: 0,
            ..Default::default()
        };
        let payload = FrozenPattern {
            index_timing: None,
            id: slicing.id,
            families: Vec::new(),
            named_buses: Vec::new(),
            sources: vec![FrozenSelectedSource {
                root_part: 0,
                track,
                family: vec![FrozenSound::Builtin(intern_kw("analog"))],
            }],
            source_uses: FrozenSourceUseGraph {
                root: 0,
                nodes: vec![
                    FrozenSourceUseNode {
                        operation: FrozenUseOperation::Slice,
                        mapping: FrozenUseMapping::Slices {
                            starts: vec![Ratio64::ZERO, Ratio64::new(1, 2).unwrap()],
                            splice: false,
                            structure: FrozenSliceStructure::Index { issuer: slicing.id },
                        },
                        edges: vec![FrozenSourceUseEdge {
                            trace,
                            layout: Vec::new(),
                            child: 1,
                        }],
                    },
                    FrozenSourceUseNode {
                        operation: FrozenUseOperation::Source,
                        mapping: FrozenUseMapping::Source { policy: 0 },
                        edges: Vec::new(),
                    },
                ],
            },
        };
        let cover =
            certify_source_uses(&inventory, &payload, TimeSpan::cycle(0).unwrap(), limits).unwrap();
        let base_depth = 3;
        let mut budget = ResolutionBudget::new(limits);
        let matcher = reserve_source_search(
            &cover,
            &payload,
            origin.borrowed_view(),
            base_depth,
            &mut budget,
        )
        .unwrap();
        let used = limits.max_nodes - budget.remaining;
        let mut validator_remaining = limits.max_nodes;
        crate::song::source::slices::validate_frozen_slice_timings(
            origin.slice_timings(),
            &origin.handle,
            &mut validator_remaining,
            matcher.max_depth,
        )
        .unwrap();
        let timing_work = limits.max_nodes - validator_remaining;
        assert!(timing_work > 0);
        assert_eq!(
            used,
            matcher.max_nodes
                + timing_work
                + crate::song::source_uses::origin::SOURCE_AUTHORITY_INSPECTION_WORK
        );
        let expected = resolve_source_use(&cover, &origin, matcher).unwrap();
        let mut exact = ResolutionBudget {
            remaining: used,
            max_depth: limits.max_depth,
        };
        let reserved = reserve_source_search(
            &cover,
            &payload,
            origin.borrowed_view(),
            base_depth,
            &mut exact,
        )
        .unwrap();
        assert_eq!(exact.remaining, 0);
        assert_eq!(reserved.max_nodes, matcher.max_nodes);
        assert_eq!(reserved.max_depth, limits.max_depth - base_depth);
        assert_eq!(
            resolve_source_use(&cover, &origin, reserved).unwrap(),
            expected
        );
        let mut one_less = ResolutionBudget {
            remaining: used - 1,
            max_depth: limits.max_depth,
        };
        assert_eq!(
            reserve_source_search(
                &cover,
                &payload,
                origin.borrowed_view(),
                base_depth,
                &mut one_less
            )
            .unwrap_err()
            .code,
            FailCode::FuelExhausted
        );
        let mut twice = ResolutionBudget {
            remaining: used * 2,
            max_depth: limits.max_depth,
        };
        for _ in 0..2 {
            reserve_source_search(
                &cover,
                &payload,
                origin.borrowed_view(),
                base_depth,
                &mut twice,
            )
            .unwrap();
        }
        assert_eq!(twice.remaining, 0);
        let timing_depth = origin.slice_timings()[0].admission_depth();
        let mut shallow = ResolutionBudget {
            remaining: limits.max_nodes,
            max_depth: base_depth + timing_depth - 1,
        };
        assert_eq!(
            reserve_source_search(
                &cover,
                &payload,
                origin.borrowed_view(),
                base_depth,
                &mut shallow
            )
            .unwrap_err()
            .code,
            FailCode::DepthExceeded
        );
    }
}
