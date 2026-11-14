//! Issued nested source resolution using fresh transcript-bound Index evidence.
mod members;

use super::*;
use crate::song::routing::PreparedRoutes;
use crate::{
    pattern::{eval::song_observation::SharedIndexWork, query::TimeSpan},
    song::{
        snapshot::{issued::FrozenIssuedSongEvent, FrozenSongEvent},
        source_uses::{
            resolve_source_use, resolve_source_use_frame, FrozenSliceStructure, FrozenUseMapping,
        },
    },
    vm::fail::Failure,
};

/// Resolve one authenticated event's inherited source chain. The caller
/// authenticates every source contribution and copied member slot first.
pub(in crate::song::routing) struct IssuedSourceRequest<'a> {
    pub(in crate::song::routing) prepared: &'a PreparedRoutes,
    pub(in crate::song::routing) issued: &'a FrozenIssuedSongEvent,
    pub(in crate::song::routing) transcript:
        &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    pub(in crate::song::routing) branch: &'a SongBranchRoute,
    pub(in crate::song::routing) offset: Ratio64,
    pub(in crate::song::routing) configuration: TimeSpan,
    pub(in crate::song::routing) work: &'a SharedIndexWork,
    pub(in crate::song::routing) depth: u32,
}

pub(in crate::song::routing) fn resolve_issued_sources(
    request: IssuedSourceRequest<'_>,
) -> Result<MatchedSources, Failure> {
    let IssuedSourceRequest {
        prepared,
        issued,
        transcript,
        branch,
        offset,
        configuration,
        work,
        depth,
    } = request;
    let (limits, remaining) = {
        let ledger = work.borrow();
        if depth >= ledger.limits.max_depth {
            return Err(Failure::new(
                crate::vm::fail::FailCode::DepthExceeded,
                "issued nested route depth",
            ));
        }
        (ledger.limits, ledger.remaining())
    };
    let mut budget = ResolutionBudget::new(crate::song::SongLimits {
        max_nodes: remaining,
        ..limits
    });
    let mut bridge = WorkBridge {
        work,
        synced_remaining: remaining,
    };
    let result = (|| {
        let event = issued.descriptor();
        budget.charge(
            issued
                .invocations()
                .len()
                .checked_add(1)
                .ok_or_else(capacity_overflow)?,
        )?;
        let mut event_seals = issued.invocations().to_vec();
        for contribution in issued.source_contributions() {
            for seal in contribution.leaves().contributors() {
                if !event_seals
                    .iter()
                    .any(|prior| std::rc::Rc::ptr_eq(prior, seal))
                {
                    budget.charge(1)?;
                    event_seals.push(seal.clone());
                }
            }
        }
        let mut resolved = Vec::new();
        let mut descriptor_anchors = OriginAnchors::default();
        if let Some(origin) = event.source_origin.as_ref() {
            let context = IssuedResolution {
                prepared,
                transcript,
                seals: &event_seals,
                branch,
                offset,
                depth,
                anchor_parent: None,
            };
            // Descriptor provenance can identify a prepared nested stage, but
            // never contributes a route. Every returned route below is still
            // resolved from a separately authenticated source contribution.
            let _descriptor_result = resolve_origin_chain(
                &context,
                event,
                origin,
                configuration,
                &mut budget,
                &mut bridge,
                &mut descriptor_anchors,
            );
        }
        for contribution in issued.source_contributions() {
            budget.charge(
                contribution
                    .leaves()
                    .contributors()
                    .len()
                    .checked_add(1)
                    .ok_or_else(capacity_overflow)?,
            )?;
            let seals = event_seals.clone();
            let anchor_index = descriptor_anchors
                .stages
                .iter()
                .position(|anchor| anchor.matches(contribution.augmented_origin()));
            let anchor_parent = anchor_index
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| descriptor_anchors.stages.get(index))
                .map(|parent| AnchorParent {
                    scope: parent.output_scope,
                    track: parent.output_track,
                    source: parent.identity.policy as usize,
                });
            let context = IssuedResolution {
                prepared,
                transcript,
                seals: &seals,
                branch,
                offset,
                depth,
                anchor_parent,
            };
            let mut contribution_anchors = OriginAnchors {
                stages: descriptor_anchors.stages.clone(),
                start_index: anchor_index,
            };
            let contribution_result = resolve_origin_chain(
                &context,
                event,
                contribution.augmented_origin(),
                configuration,
                &mut budget,
                &mut bridge,
                &mut contribution_anchors,
            );
            resolved.push(contribution_result?);
        }
        if resolved.is_empty() {
            verify_policy(prepared.plan(), event, None, &mut budget)?;
            return Ok(MatchedSources {
                sources: Vec::new(),
                configuration,
            });
        }
        let mut combined = resolved.remove(0);
        for candidate in resolved {
            budget.charge(
                candidate
                    .sources
                    .len()
                    .checked_add(1)
                    .ok_or_else(capacity_overflow)?,
            )?;
            if candidate.configuration != combined.configuration {
                return Err(invalid(
                    "issued source contributions resolve conflicting routes",
                ));
            }
            for source in candidate.sources {
                budget.charge(1)?;
                if !combined.sources.contains(&source) {
                    combined.sources.push(source);
                }
            }
        }
        Ok(combined)
    })();
    sync_local_work(&mut bridge, &budget)?;
    result
}

#[derive(Clone, Copy)]
struct IssuedResolution<'a> {
    prepared: &'a PreparedRoutes,
    transcript: &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    seals: &'a [std::rc::Rc<crate::pattern::eval::song_provenance::InvocationSeal>],
    branch: &'a SongBranchRoute,
    offset: Ratio64,
    depth: u32,
    anchor_parent: Option<AnchorParent>,
}
#[derive(Clone, Copy)]
struct AnchorParent {
    scope: usize,
    track: KwId,
    source: usize,
}
struct WorkBridge<'a> {
    work: &'a SharedIndexWork,
    synced_remaining: u32,
}

#[derive(Default)]
struct OriginAnchors<'a> {
    stages: Vec<OriginAnchor<'a>>,
    start_index: Option<usize>,
}

#[derive(Clone)]
struct OriginAnchor<'a> {
    handle: &'a crate::song::EventHandle,
    source_part: TimeSpan,
    original_instrument: &'a FrozenSound,
    entry_trace: &'a [crate::pattern::occ::ProducerStep],
    source_whole: Option<TimeSpan>,
    payload: &'a crate::song::snapshot::FrozenPattern,
    output_scope: usize,
    output_track: KwId,
    cover: &'a crate::song::source_uses::FrozenSourceUseCover,
    identity: crate::song::source_uses::FrozenSourceUseIdentity,
    output_handle: &'a crate::song::EventHandle,
    base_depth: u32,
    output_onset: Ratio64,
    output_whole: Option<TimeSpan>,
    output_offset: Ratio64,
    incoming_sampled: Option<SourceMembership>,
}

impl OriginAnchor<'_> {
    fn matches(&self, origin: &crate::song::source_uses::FrozenSourceOrigin) -> bool {
        self.handle == &origin.handle
            && self.source_part == origin.source_part
            && self.original_instrument == &origin.original_instrument
            && self.entry_trace == origin.entry_trace
            && self.source_whole == origin.source_whole()
    }
}

fn resolve_origin_chain<'a>(
    context: &IssuedResolution<'a>,
    event: &'a FrozenSongEvent,
    origin: &'a crate::song::source_uses::FrozenSourceOrigin,
    mut configuration: TimeSpan,
    budget: &mut ResolutionBudget,
    bridge: &mut WorkBridge<'_>,
    anchors: &mut OriginAnchors<'a>,
) -> Result<MatchedSources, Failure> {
    let IssuedResolution {
        prepared,
        branch,
        offset,
        ..
    } = *context;
    let plan = prepared.plan();
    let certificate = plan
        .source_covers
        .get(
            branch
                .source_cover
                .ok_or_else(|| invalid("issued source provenance on ordinary branch"))?
                as usize,
        )
        .ok_or_else(|| invalid("issued source cover index"))?;
    let start_anchor = anchors
        .start_index
        .and_then(|index| anchors.stages.get(index));
    let start_identity = start_anchor.map(|anchor| anchor.identity.clone());
    let (mut payload, mut output_scope, mut output_track, mut cover, mut base_depth) =
        if let Some(anchor) = start_anchor {
            (
                anchor.payload,
                anchor.output_scope,
                anchor.output_track,
                anchor.cover,
                anchor.base_depth,
            )
        } else {
            (
                scope_payload(&plan.topology, certificate.scope_part, certificate.track)?,
                certificate.scope_part,
                certificate.track,
                &certificate.cover,
                u32::try_from(
                    branch
                        .placement
                        .iter()
                        .filter(|place| !matches!(place, SongRoutePlacement::Region { .. }))
                        .count(),
                )
                .map_err(|_| capacity_overflow())?
                .checked_add(1)
                .ok_or_else(capacity_overflow)?,
            )
        };
    let count = origin
        .inherited
        .len()
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    budget.charge(count)?;
    let mut stages = Vec::with_capacity(count);
    let mut output_scopes = Vec::with_capacity(count);
    let mut output_handles = Vec::with_capacity(count);
    let mut output_onset = start_anchor.map_or(
        event.handle.occurrence().onset.checked_sub(offset)?,
        |anchor| anchor.output_onset,
    );
    let mut output_offset = start_anchor.map_or(Ratio64::ZERO, |anchor| anchor.output_offset);
    let mut incoming_sampled = start_anchor.and_then(|anchor| anchor.incoming_sampled);
    let mut output_whole = if let Some(anchor) = start_anchor {
        anchor.output_whole
    } else {
        event
            .whole
            .map(|whole| whole.map(|time| time.checked_sub(offset)))
            .transpose()?
    };
    let mut stage_output_handle = start_anchor.map_or(&event.handle, |anchor| anchor.output_handle);

    for ordinal in 0..count {
        let stage_output_scope = output_scope;
        let output_owner = super::super::index::output_operand_owner(
            &plan.topology,
            output_scope,
            output_track,
            budget,
        )?;
        if !std::ptr::eq(output_owner.payload(), payload) {
            return Err(invalid(
                "issued inherited Index payload differs from topology owner",
            ));
        }
        let frame = origin.inherited.get(ordinal.wrapping_sub(1));
        let slice_timings = if ordinal == 0 {
            origin.slice_timings()
        } else {
            frame
                .ok_or_else(|| invalid("issued inherited source frame"))?
                .slice_timings()
        };
        let (handle, instrument, trace, source_whole, source_part, borrowed_view) = if ordinal == 0
        {
            (
                &origin.handle,
                &origin.original_instrument,
                origin.entry_trace.as_slice(),
                origin.source_whole(),
                origin.source_part,
                origin.borrowed_view(),
            )
        } else {
            let frame = frame.ok_or_else(|| invalid("issued inherited source frame"))?;
            (
                &frame.handle,
                &frame.original_instrument,
                frame.entry_trace.as_slice(),
                frame.source_whole(),
                frame.source_part,
                frame.borrowed_view(),
            )
        };
        let current_cover = cover;
        output_handles.push(stage_output_handle);
        let source_limits = super::super::source::reserve_source_search(
            cover,
            payload,
            borrowed_view,
            base_depth,
            budget,
        )?;
        let identity = if ordinal == 0 && start_identity.is_some() {
            start_identity
                .as_ref()
                .ok_or_else(|| invalid("issued source anchor disappeared"))?
                .clone()
        } else if ordinal == 0 {
            resolve_source_use(cover, origin, source_limits)?
        } else {
            resolve_source_use_frame(
                cover,
                frame.ok_or_else(|| invalid("issued inherited source frame"))?,
                source_limits,
            )?
        };
        if anchors.start_index.is_none() {
            anchors.stages.push(OriginAnchor {
                handle,
                source_part,
                original_instrument: instrument,
                entry_trace: trace,
                source_whole,
                payload,
                output_scope,
                output_track,
                cover: current_cover,
                identity: identity.clone(),
                output_handle: stage_output_handle,
                base_depth,
                output_onset,
                output_whole,
                output_offset,
                incoming_sampled,
            });
        }
        let selected = payload
            .sources
            .get(identity.policy as usize)
            .ok_or_else(|| invalid("issued nested selected policy"))?;
        let graph_depth = u32::try_from(identity.edges.len())
            .map_err(|_| capacity_overflow())?
            .checked_add(1)
            .ok_or_else(capacity_overflow)?;
        let policy_depth = base_depth
            .checked_add(graph_depth)
            .ok_or_else(capacity_overflow)?;
        let scope = source_scope(
            &plan.topology,
            selected.root_part,
            handle,
            policy_depth,
            budget,
        )?;
        let scope_offset = scope.offset;
        if ordinal + 1 < count {
            budget.charge(plan.nested_source_covers.len())?;
            let next = plan
                .nested_source_covers
                .iter()
                .find(|candidate| {
                    candidate.root_part == selected.root_part
                        && candidate.scope_part == scope.index
                        && candidate.track == handle.track()
                })
                .ok_or_else(|| invalid("issued inherited origin has no owning payload"))?;
            cover = &next.cover;
            payload = scope.payload;
            base_depth = scope.depth;
            output_scope = scope.index;
            output_track = handle.track();
        }
        let sampled_mapping = map_source_locator(
            current_cover,
            &identity,
            trace,
            output_onset,
            SourceTimingContext {
                incoming_member: incoming_sampled,
                output_whole,
                source_whole,
            },
            budget,
        )?;
        let sampled_locator = match sampled_mapping {
            SourceLocatorMapping::UnchangedBirth | SourceLocatorMapping::NeedsJointGeometry => None,
            SourceLocatorMapping::SampledPoint(input) => {
                let current = incoming_sampled.unwrap_or(SourceMembership {
                    point: output_onset,
                    side: MembershipSide::At,
                });
                incoming_sampled = Some(SourceMembership {
                    point: input.point.checked_sub(scope.offset)?,
                    side: input.side,
                });
                Some(current)
            }
        };
        stages.push(Stage {
            output_owner,
            slice_timings,
            input_root: selected.root_part,
            policy_depth,
            handle,
            cover: current_cover,
            identity,
            trace,
            instrument,
            scope,
            output_onset,
            output_whole,
            output_offset,
            input_onset: handle.occurrence().onset,
            sampled_locator,
            source_whole,
        });
        output_scopes.push(stage_output_scope);
        stage_output_handle = handle;
        output_whole = source_whole
            .map(|whole| whole.map(|time| time.checked_sub(scope_offset)))
            .transpose()?;
        output_onset = handle.occurrence().onset.checked_sub(scope_offset)?;
        output_offset = scope_offset;
    }

    let inherited_route = inherited_policy_route(plan, &stages, budget)?;
    verify_policy(plan, event, inherited_route, budget)?;

    let index_configuration = members::issued_index_stage_configuration(
        context,
        &stages,
        &output_scopes,
        &output_handles,
        configuration,
        budget,
        bridge,
    )?;
    if let Some(index) = index_configuration {
        configuration = index;
    } else if stages.iter().any(|stage| stage.sampled_locator.is_some()) {
        configuration = sampled_stage_configuration(&stages, configuration, offset, budget)?;
    } else if let Some(joint) = joint_stage_configuration(
        &stages,
        configuration,
        offset,
        event.handle.occurrence().onset,
        budget,
    )? {
        configuration = joint;
    } else {
        let mut mapped = None;
        for stage in stages.iter().rev() {
            let mut interval = stage.scope.interval;
            if let Some(inner) = mapped {
                interval = intersect(interval, inner)?;
            }
            let mapped_stage = map_intrinsic_configuration(
                stage.cover,
                &stage.identity,
                stage.trace,
                super::configuration::IntrinsicConfigurationInput {
                    span: interval,
                    anchor: stage
                        .sampled_locator
                        .map_or(stage.output_onset, |m| m.point),
                    source_onset: stage.input_onset,
                    output_whole: stage.output_whole,
                },
                budget,
            )?
            .ok_or_else(|| invalid("issued nested mapping geometry remains unresolved"))?;
            mapped = Some(mapped_stage.map(|time| time.checked_add(stage.output_offset))?);
        }
        if let Some(mapped) = mapped {
            configuration = intersect(configuration, mapped.map(|time| time.checked_add(offset))?)?;
        }
    }

    budget.charge(stages.len())?;
    let mut sources = Vec::with_capacity(stages.len());
    for stage in stages {
        budget.charge(1)?;
        if let FrozenSound::Sample { path, .. } = stage.instrument {
            budget.charge(path.len())?;
        }
        sources.push(SongSourceConfiguration {
            identity: stage.identity,
            original_instrument: stage.instrument.clone(),
            original_configuration: stage.scope.interval,
            configuration,
        });
    }
    Ok(MatchedSources {
        sources,
        configuration,
    })
}

fn inherited_policy_route<'a>(
    plan: &'a SongRoutePlan,
    stages: &[Stage<'a>],
    budget: &mut ResolutionBudget,
) -> Result<Option<BorrowedRoute<'a>>, Failure> {
    let mut route = None;
    for (index, stage) in stages.iter().enumerate().rev() {
        let inner = stages.get(index + 1);
        let query = FxPolicyQuery {
            root: stage.input_root,
            track: stage.handle.track(),
            instrument: inner.map_or(stage.instrument, |inner| inner.instrument),
            onset: stage.input_onset,
            origin_revision: inner.map(|inner| inner.handle.revision()),
            depth: stage.policy_depth,
        };
        if let Some(found) = policy_route(&plan.topology, query, budget)? {
            route = Some(found);
        }
    }
    Ok(route)
}

fn sync_local_work(bridge: &mut WorkBridge<'_>, budget: &ResolutionBudget) -> Result<(), Failure> {
    let spent = bridge
        .synced_remaining
        .checked_sub(budget.remaining())
        .ok_or_else(|| invalid("issued nested local work was replenished"))?;
    bridge.work.borrow_mut().charge(u64::from(spent))?;
    bridge.synced_remaining = budget.remaining();
    Ok(())
}

pub(super) fn missing_issued_index_component() -> Failure {
    invalid("issued Index event has no canonical component")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::routing::PreparedRoutes;
    use crate::{
        pattern::eval::song_observation::{CanonicalIndexCollector, SharedIndexWork},
        song::{
            routing::{prepare_routes_issued, SongHostCapacities},
            snapshot::{
                issued::FrozenIssuedBatch,
                occupancy::route_view::{capture_test_route_authority, RouteAuthorityView},
            },
            PreparedSong, SongLimits,
        },
        vm::fail::Failure,
    };
    use crate::{pattern::query::TimeSpan, value::Ratio64};
    use std::rc::Rc;

    fn limits() -> SongLimits {
        SongLimits {
            max_nodes: 1_000_000,
            ..SongLimits::default()
        }
    }

    fn capacities() -> SongHostCapacities {
        SongHostCapacities {
            sample_rate: 48_000,
            cell_slots: 4096,
            voice_slots: 8,
            template_slots: 256,
            bus_slots: 256,
            sample_resources: 256,
            pcm_bytes: 16_000_000,
            voice_frames: 8 * 192_000,
            bus_frames: 128_000_000,
            ack_slots: 1024,
        }
    }

    fn capture(code: &str) -> Result<(Rc<RouteAuthorityView>, FrozenIssuedBatch), Failure> {
        let policy = limits();
        let mut remaining = policy.max_nodes;
        capture_test_route_authority(code, policy, &mut remaining, 0)
    }

    fn prepare(authority: Rc<RouteAuthorityView>) -> Result<PreparedRoutes, Failure> {
        let mut remaining = limits().max_nodes;
        prepare_routes_issued(
            authority.clone(),
            *authority.original().settings(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacities(),
            limits(),
            &mut remaining,
            0,
        )
    }

    fn attached_work(authority: &RouteAuthorityView) -> Result<SharedIndexWork, Failure> {
        let work = CanonicalIndexCollector::new(limits().max_nodes, limits())?;
        work.borrow_mut().original = Some(authority.original().clone());
        Ok(work)
    }

    fn prepared_song(code: &str) -> Result<PreparedSong, Failure> {
        use crate::{
            session::song::{evaluate_song_candidate, CandidateBuildCtx},
            song::{
                assets::{DecodedSongAssetFactory, SongAssetLimits},
                prepare_song, SnapshotEpoch,
            },
        };
        use std::collections::BTreeMap;

        let factory =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let context = CandidateBuildCtx {
            assets: &factory,
            asset_limits: SongAssetLimits {
                max_resources: 64,
                max_pcm_bytes: 1_000_000,
                max_source_files: 32,
                max_source_bytes: 100_000,
                max_banks: 32,
                max_walk_nodes: limits().max_nodes,
                max_walk_depth: limits().max_depth,
            },
            lock: None,
            cache: None,
        };
        let candidate = evaluate_song_candidate(
            code,
            "nested-issued-partition.vact",
            7,
            SnapshotEpoch(91),
            &context,
        )?;
        prepare_song(candidate)
    }

    fn query_routes(
        prepared: &PreparedRoutes,
        batch: &FrozenIssuedBatch,
        work: &SharedIndexWork,
    ) -> Result<Vec<String>, Failure> {
        let mut routes = Vec::with_capacity(batch.events().len());
        for index in 0..batch.events().len() {
            routes.push(format!(
                "{:?}",
                prepared.resolve_issued_event(batch, index, work, 0)?
            ));
        }
        routes.sort();
        Ok(routes)
    }

    #[test]
    fn cached_nested_slice_events_resolve_from_issued_transcript() -> Result<(), Failure> {
        let program = "fn cut beat:\n\tfirst [0]\nfn inner p:\n\tslice {beat -> p} 2 [0 nil]\nfn outer p:\n\tslice {beat -> p} 2 [0 nil]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inside {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inside :drums :analog outer}\nsong selected tail-seconds: 0 > play-song";
        let (authority, batch) = capture(program)?;
        let routes = prepare(authority.clone())?;
        let work = attached_work(&authority)?;
        assert!(
            !batch.events().is_empty(),
            "nested Slice fixture emits events"
        );
        assert!(batch.events().iter().any(|event| {
            event
                .source_contributions()
                .iter()
                .any(|contribution| !contribution.augmented_origin().inherited.is_empty())
        }));
        for (index, event) in batch.events().iter().enumerate() {
            let issued = routes.resolve_issued_event(&batch, index, &work, 0)?;
            if let Ok(legacy) =
                super::super::source::resolve_route(routes.plan(), event.descriptor(), limits())
            {
                assert_eq!(issued, legacy, "legacy-compatible nested route");
            }
        }
        Ok(())
    }

    #[test]
    fn discarded_augmented_source_origins_are_resolved() -> Result<(), Failure> {
        let program = "fn indexed p:\n\tslow {slice {beat -> p} 1 [0]} 2\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nlet intro {part [drums: nil] duration: 1/2}\nsong {sequence [intro selected]} tail-seconds: 0 > play-song";
        let (authority, batch) = capture(program)?;
        let routes = prepare(authority.clone())?;
        let work = attached_work(&authority)?;
        // Identity evidence is retained by fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority.
        let (index, event) = batch
            .events()
            .iter()
            .enumerate()
            .find(|(_, event)| {
                event
                    .source_contributions()
                    .iter()
                    .filter(|contribution| {
                        let augmented = contribution.augmented_origin();
                        !augmented.slice_timings().is_empty()
                            || augmented
                                .inherited
                                .iter()
                                .any(|frame| !frame.slice_timings().is_empty())
                    })
                    .count()
                    >= 2
            })
            .ok_or_else(|| {
                invalid(
                    "fixture has an equal-handle union with two or more slice-timed contributions",
                )
            })?;
        let remaining_before = work.borrow().remaining();
        routes.resolve_issued_event(&batch, index, &work, 0)?;
        let remaining_after = work.borrow().remaining();
        let minimum_contribution_debit = u32::try_from(event.source_contributions().len())
            .map_err(|_| invalid("source contribution count overflow"))?;
        assert!(
            remaining_before.saturating_sub(remaining_after) >= minimum_contribution_debit,
            "each source contribution is authenticated through caller work"
        );
        Ok(())
    }

    #[test]
    fn issued_joint_geometry_resolves_where_legacy_keeps_its_barrier() -> Result<(), Failure> {
        let program = "fn indexed p:\n\teuclid {slice {beat -> p} 2 [0 nil]} 1 2\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";
        let mut song = prepared_song(program)?;
        let policy = limits();
        let window = TimeSpan::new(Ratio64::ZERO, song.snapshot().duration())?;
        let legacy_plan = crate::song::routing::prepare_routes(
            song.snapshot(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacities(),
        )?;
        let legacy_event = song
            .query(window, &policy)?
            .into_iter()
            .next()
            .ok_or_else(|| invalid("joint-geometry fixture emits no legacy event"))?;
        let mut remaining = policy.max_nodes;
        let authority = song.issue_retained_route_authority(policy, &mut remaining, 0)?;
        let routes = prepare(authority.clone())?;
        let work = attached_work(&authority)?;
        let batch = song.query_issued_with_work(window, &work, 0)?;
        let issued_event = batch
            .events()
            .first()
            .ok_or_else(|| invalid("joint-geometry fixture emits no issued event"))?;
        let legacy = super::super::source::resolve_route(&legacy_plan, &legacy_event, limits())
            .expect_err("legacy route preserves the joint-geometry barrier");
        assert!(
            legacy
                .message
                .contains("sampled context requires joint mapping geometry"),
            "legacy error identifies the NeedsJointGeometry barrier"
        );
        routes.resolve_issued_event(&batch, 0, &work, 0)?;
        assert_eq!(legacy_event.handle, issued_event.descriptor().handle);
        Ok(())
    }

    #[test]
    fn distinct_equal_handle_invocations_are_all_resolved() -> Result<(), Failure> {
        let program = "fn inner p:\n\tslice {beat -> p} 2 [0 nil]\nfn outer p:\n\tstack [{slice {beat -> p} 2 [0 nil]} {slice {beat -> p} 2 [0 nil]}]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inner-part {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inner-part :drums :analog outer}\nsong selected tail-seconds: 0 > play-song";
        let (authority, batch) = capture(program)?;
        let routes = prepare(authority.clone())?;
        let work = attached_work(&authority)?;
        let (index, event) = batch
            .events()
            .iter()
            .enumerate()
            .find(|(_, event)| {
                event.invocations().iter().enumerate().any(|(index, seal)| {
                    event.invocations()[index + 1..]
                        .iter()
                        .any(|other| !Rc::ptr_eq(seal, other))
                })
            })
            .ok_or_else(|| invalid("fixture has no event with distinct invocation seals"))?;
        assert!(
            event.invocations().len() >= 2,
            "coalesced event retains its distinct invocations"
        );
        let remaining_before = work.borrow().remaining();
        routes.resolve_issued_event(&batch, index, &work, 0)?;
        let remaining_after = work.borrow().remaining();
        let minimum_seal_debit = u32::try_from(event.invocations().len())
            .map_err(|_| invalid("invocation seal count overflow"))?;
        assert!(
            remaining_before.saturating_sub(remaining_after) >= minimum_seal_debit,
            "each invocation seal is authenticated through caller work"
        );
        Ok(())
    }

    #[test]
    fn partitioned_nested_issued_queries_equal_the_full_route_set() -> Result<(), Failure> {
        let program = "fn inner p:\n\tslice {beat -> p} 2 [0 nil]\nfn outer p:\n\tslice {beat -> p} 2 [0 nil]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inside {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inside :drums :analog outer}\nsong selected tail-seconds: 0 > play-song";
        let mut song = prepared_song(program)?;
        let policy = limits();
        let mut remaining = policy.max_nodes;
        let authority = song.issue_retained_route_authority(policy, &mut remaining, 0)?;
        let routes = prepare_routes_issued(
            authority.clone(),
            *authority.original().settings(),
            &crate::dsp::caps::CapabilitySet::native(),
            &capacities(),
            limits(),
            &mut remaining,
            0,
        )?;
        let full_work = attached_work(&authority)?;
        let partition_work = attached_work(&authority)?;
        let span = |start, end| TimeSpan::new(Ratio64::from_int(start), Ratio64::from_int(end));

        let full = song.query_issued_with_work(span(0, 2)?, &full_work, 0)?;
        let first = song.query_issued_with_work(span(0, 1)?, &partition_work, 0)?;
        let second = song.query_issued_with_work(span(1, 2)?, &partition_work, 0)?;
        let full_routes = query_routes(&routes, &full, &full_work)?;
        let mut partition_routes = query_routes(&routes, &first, &partition_work)?;
        partition_routes.extend(query_routes(&routes, &second, &partition_work)?);
        partition_routes.sort();
        assert!(!full_routes.is_empty(), "nested fixture has routed events");
        assert_eq!(full_routes, partition_routes);
        Ok(())
    }
}
