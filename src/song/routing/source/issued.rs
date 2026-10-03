//! Authentication of source-side evidence carried by issued song events.

use crate::pattern::eval::{
    song_observation::SharedIndexWork, song_provenance::IssuedQueryTranscript,
};
use crate::song::{
    routing::{PreparedRoutes, SongResolvedRoute},
    snapshot::issued::{FrozenIssuedBatch, FrozenIssuedSongEvent},
    source_uses::{copy_origin, origin::FrozenIssuedSourceContribution},
};
use crate::vm::fail::{FailCode, Failure};
use std::rc::Rc;

fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, format!("issued route source: {message}"))
}

/// Resolve an issued event only after the batch, invocation seals, and source
/// contribution members have been authenticated against the prepared Song.
pub(in crate::song::routing) fn resolve_issued(
    prepared: &PreparedRoutes,
    batch: &FrozenIssuedBatch,
    event_index: usize,
    work: &SharedIndexWork,
    depth: u32,
) -> Result<SongResolvedRoute, Failure> {
    let transcript = batch.transcript();
    let original = prepared.authority_original();
    let first_seal = batch
        .events()
        .first()
        .and_then(|event| event.invocations().first())
        .ok_or_else(|| invalid("issued batch has no authentic invocation"))?;
    if !transcript.authentic(original, first_seal, work, depth)? {
        return Err(invalid("issued batch belongs to another Song"));
    }

    let issued = batch
        .events()
        .get(event_index)
        .ok_or_else(|| invalid("issued event index is outside batch"))?;
    if issued.invocations().is_empty() {
        return Err(invalid("issued event has no invocation seals"));
    }
    for seal in issued.invocations() {
        if !transcript.authentic(original, seal, work, depth)? {
            return Err(invalid("issued event has a foreign invocation seal"));
        }
    }
    authenticate_contributions(issued, transcript, work, depth)?;

    let event = issued.descriptor();
    let plan = prepared.plan();
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

    let template = event
        .route
        .as_ref()
        .map(|(_, route_template)| *route_template);
    let branch_scan = u64::try_from(plan.branches.len())
        .map_err(|_| invalid("route branch count overflow"))?
        .checked_add(1)
        .ok_or_else(|| invalid("route branch count overflow"))?;
    work.borrow_mut().charge(branch_scan)?;

    let mut agreed: Option<SongResolvedRoute> = None;
    for branch in &plan.branches {
        if branch.track != event.track
            || branch.instrument != event.instrument
            || branch.effect_template != template
        {
            continue;
        }
        let interval = crate::song::snapshot::occupancy::lookup::authority::with_work(
            work,
            |limits, remaining| {
                let mut budget = super::ResolutionBudget::new(crate::song::SongLimits {
                    max_nodes: *remaining,
                    ..limits
                });
                let result = super::branch_interval(plan, branch, event, &mut budget);
                *remaining = budget.remaining;
                result
            },
        )?;
        let Some((configuration, offset)) = interval else {
            continue;
        };
        let matched = super::super::nested::issued::resolve_issued_sources(
            super::super::nested::issued::IssuedSourceRequest {
                prepared,
                issued,
                transcript,
                branch,
                offset,
                configuration,
                work,
                depth,
            },
        )?;
        let placement_words = event.handle.placement().0.len();
        let route_words = placement_words
            .checked_add(matched.sources.len())
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| invalid("resolved route size overflow"))?;
        work.borrow_mut().charge(
            u64::try_from(route_words).map_err(|_| invalid("resolved route size overflow"))?,
        )?;
        let candidate = SongResolvedRoute {
            branch: branch.id,
            placement: event.handle.placement().clone(),
            sources: matched.sources,
            configuration: matched.configuration,
        };
        if let Some(previous) = &agreed {
            let compare_words = previous
                .sources
                .len()
                .checked_add(candidate.sources.len())
                .and_then(|count| count.checked_add(1))
                .ok_or_else(|| invalid("resolved route comparison overflow"))?;
            work.borrow_mut().charge(
                u64::try_from(compare_words)
                    .map_err(|_| invalid("resolved route comparison overflow"))?,
            )?;
            if previous != &candidate {
                return Err(invalid("authenticated route candidates conflict"));
            }
        } else {
            agreed = Some(candidate);
        }
    }
    agreed.ok_or_else(|| invalid("event does not match admitted route topology"))
}

/// Authenticate every retained source contribution before an issued route is
/// considered. The copied slots must be a one-to-one account of genuine sealed
/// members, and every augmented origin must preserve each member's raw origin.
pub(super) fn authenticate_contributions(
    event: &FrozenIssuedSongEvent,
    transcript: &IssuedQueryTranscript,
    work: &SharedIndexWork,
    depth: u32,
) -> Result<(), Failure> {
    work.borrow_mut()
        .charge(event.source_contributions().len() as u64 + 1)?;
    for contribution in event.source_contributions() {
        authenticate_contribution(contribution, transcript, work, depth)?;
    }
    Ok(())
}

fn authenticate_contribution(
    contribution: &FrozenIssuedSourceContribution,
    transcript: &IssuedQueryTranscript,
    work: &SharedIndexWork,
    depth: u32,
) -> Result<(), Failure> {
    let members = transcript.source_members(contribution.leaves(), work, depth)?;
    let copied = contribution.members();
    if members.is_empty() || members.len() != copied.len() {
        return Err(invalid("missing or incomplete source member slots"));
    }
    work.borrow_mut().charge(copied.len() as u64 + 1)?;
    let mut seen = vec![false; members.len()];
    for copied_member in copied {
        let slot = copied_member.member_slot();
        let Some(seen_slot) = seen.get_mut(slot) else {
            return Err(invalid("source member slot is outside transcript"));
        };
        if *seen_slot {
            return Err(invalid("duplicate source member slot"));
        }
        *seen_slot = true;

        let member = members
            .get(slot)
            .ok_or_else(|| invalid("source member slot is outside transcript"))?;
        if !member.is_sealed_member(work, depth)?
            || !member
                .member()
                .issued_leaves
                .as_ref()
                .is_some_and(|leaves| Rc::ptr_eq(leaves, contribution.leaves()))
        {
            return Err(invalid("source member is not sealed to its contribution"));
        }

        let raw = crate::song::snapshot::issued::with_copy_budget(
            work,
            depth,
            |remaining, max_depth| copy_origin(member.member(), remaining, max_depth),
        )?;
        crate::song::snapshot::issued::with_copy_budget(work, depth, |remaining, _| {
            copied_member.origin().admit_raw_comparison(&raw, remaining)
        })?;
        if copied_member.origin() != &raw {
            return Err(invalid("copied source member differs from transcript"));
        }
        let augmented = contribution.augmented_origin();
        crate::song::snapshot::issued::with_copy_budget(work, depth, |remaining, _| {
            augmented.admit_raw_comparison(&raw, remaining)
        })?;
        if !augmented.preserves_raw_member(&raw) {
            return Err(invalid("augmented origin does not preserve source member"));
        }
    }
    if seen.iter().any(|present| !present) {
        return Err(invalid("source member slot is missing"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::eval::song_observation::{CanonicalIndexCollector, SharedIndexWork};
    use crate::song::routing::{prepare_routes_issued, SongHostCapacities};
    use crate::song::snapshot::issued::FrozenIssuedBatch;
    use crate::song::snapshot::occupancy::route_view::{
        capture_test_route_authority, RouteAuthorityView,
    };
    use crate::song::SongLimits;
    use crate::vm::fail::Failure;
    use std::rc::Rc;

    const PLAIN: &str =
        "let base {part [drums: {s :analog}] duration: 2}\nsong base tail-seconds: 0 > play-song";
    const SLICE: &str = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> s :analog > chord [:c :five]} 2 [cut nil]\nlet base {part [drums: {s :analog}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";

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
        let mut remaining = limits().max_nodes;
        capture_test_route_authority(code, limits(), &mut remaining, 0)
    }

    fn prepared(authority: Rc<RouteAuthorityView>) -> Result<PreparedRoutes, Failure> {
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

    #[test]
    fn genuine_plain_event_matches_legacy_route() -> Result<(), Failure> {
        let (authority, batch) = capture(PLAIN)?;
        let routes = prepared(authority.clone())?;
        let work = attached_work(&authority)?;
        let event = batch
            .events()
            .first()
            .ok_or_else(|| invalid("plain fixture produced no issued event"))?;
        let issued = resolve_issued(&routes, &batch, 0, &work, 0)?;
        let legacy = super::super::resolve_route(routes.plan(), event.descriptor(), limits())?;
        assert_eq!(issued, legacy);
        Ok(())
    }

    #[test]
    fn genuine_source_contributions_reject_a_foreign_transcript() -> Result<(), Failure> {
        let (authority, batch) = capture(SLICE)?;
        let (_, foreign_batch) = capture(SLICE)?;
        let event = batch
            .events()
            .first()
            .ok_or_else(|| invalid("Slice fixture produced no issued event"))?;
        if event.source_contributions().is_empty() {
            return Err(invalid("Slice fixture produced no source contribution"));
        }
        let work = attached_work(&authority)?;
        authenticate_contributions(event, batch.transcript(), &work, 0)?;
        assert!(authenticate_contributions(event, foreign_batch.transcript(), &work, 0).is_err());
        Ok(())
    }

    #[test]
    fn exact_work_succeeds_and_one_less_returns_no_route() -> Result<(), Failure> {
        let (authority, batch) = capture(PLAIN)?;
        let routes = prepared(authority.clone())?;
        let measured = attached_work(&authority)?;
        resolve_issued(&routes, &batch, 0, &measured, 0)?;
        let spent = limits().max_nodes - measured.borrow().remaining();
        let exact = CanonicalIndexCollector::new(spent, limits())?;
        exact.borrow_mut().original = Some(authority.original().clone());
        resolve_issued(&routes, &batch, 0, &exact, 0)?;
        assert_eq!(exact.borrow().remaining(), 0);

        let short = CanonicalIndexCollector::new(spent - 1, limits())?;
        short.borrow_mut().original = Some(authority.original().clone());
        let before = short.borrow().remaining();
        assert!(resolve_issued(&routes, &batch, 0, &short, 0).is_err());
        assert!(short.borrow().remaining() < before);
        Ok(())
    }
}
