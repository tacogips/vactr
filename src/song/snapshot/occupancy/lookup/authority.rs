//! Original lookup authentication, extracted before issued backend adoption.
use super::*;
pub(in crate::song::snapshot::occupancy) fn payload_for<'s>(
    authority: LookupAuthority<'s>,
    request: &CanonicalIndexRequest,
    budget: &mut ProjectionBudget<'_>,
) -> Result<&'s crate::song::snapshot::FrozenPattern, Failure> {
    let part = authority
        .inventory()
        .parts
        .get(request.scope)
        .ok_or_else(|| invalid("lookup owner scope"))?;
    if part.revision != request.revision {
        return Err(invalid("lookup owner revision"));
    }
    budget.charge(match &part.node {
        FrozenPartNode::Capture(tracks) => tracks.len() as u64 + 1,
        _ => 1,
    })?;
    match &part.node {
        FrozenPartNode::Capture(tracks) => tracks
            .iter()
            .find_map(|(track, p)| (*track == request.track).then_some(p)),
        FrozenPartNode::Edit {
            edit:
                FrozenEdit::Transform { track, payload, .. }
                | FrozenEdit::Replace { track, payload }
                | FrozenEdit::Overwrite { track, payload, .. },
            ..
        } if *track == request.track => Some(payload),
        _ => None,
    }
    .ok_or_else(|| invalid("lookup owner payload"))
}
pub(in crate::song::snapshot::occupancy) fn authenticate_request(
    snapshot: &SongSnapshot,
    request: &CanonicalIndexRequest,
    depth: u32,
    budget: &mut ProjectionBudget<'_>,
) -> Result<(), Failure> {
    authenticate_authority(LookupAuthority::Snapshot(snapshot), request, depth, budget)
}
pub(in crate::song::snapshot::occupancy) fn authenticate_authority(
    authority: LookupAuthority<'_>,
    request: &CanonicalIndexRequest,
    depth: u32,
    budget: &mut ProjectionBudget<'_>,
) -> Result<(), Failure> {
    budget.enter(depth)?;
    budget.charge(request.prefix.len() as u64 + 1)?;
    if !Rc::ptr_eq(authority.original(), &request.original) {
        return Err(invalid("foreign lookup original Song"));
    }
    let payload = payload_for(authority, request, budget)?;
    if payload.id != request.root
        || !payload
            .index_timing
            .as_ref()
            .is_some_and(|recipe| Rc::ptr_eq(recipe, &request.recipe))
    {
        return Err(invalid("lookup original recipe or root mismatch"));
    }
    // The request constructor is private-field authority, already issued by the
    // owning prepared Slice matcher. Validate its retained immutable attachment.
    if site_count(
        &request.recipe,
        request.recipe.root(),
        request.issuer,
        &request.prefix,
        0,
        depth,
        budget,
    )? != 1
    {
        return Err(invalid("missing or ambiguous complete original Slice site"));
    }

    Ok(())
}

pub(in crate::song::snapshot::occupancy) fn policy_path(
    graph: &crate::song::source_uses::FrozenSourceUseGraph,
    node: u32,
    policy: usize,
    trace: &[crate::pattern::occ::ProducerStep],
    cursor: usize,
    depth: u32,
    budget: &mut ProjectionBudget<'_>,
) -> Result<Option<Vec<u32>>, Failure> {
    use crate::song::source_uses::{FrozenUseMapping, FrozenUseTraceTerm};
    budget.enter(depth)?;
    let node = graph
        .nodes
        .get(node as usize)
        .ok_or_else(|| invalid("source use node absent"))?;
    if let FrozenUseMapping::Source { policy: actual } = node.mapping {
        budget.charge(1)?;
        return Ok((actual as usize == policy && cursor == trace.len()).then(Vec::new));
    }
    let mut found = None;
    for (ordinal, edge) in node.edges.iter().enumerate() {
        budget.charge(edge.trace.len() as u64 + 1)?;
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(|| invalid("source trace overflow"))?;
        let Some(actual) = trace.get(cursor..end) else {
            continue;
        };
        if edge
            .trace
            .iter()
            .zip(actual)
            .all(|(term, step)| match term {
                FrozenUseTraceTerm::Exact(exact) => exact == step,
                FrozenUseTraceTerm::Copies { kind, count } => {
                    *kind == step.kind && step.ordinal < *count
                }
            })
        {
            if let Some(mut edges) = policy_path(
                graph,
                edge.child,
                policy,
                trace,
                end,
                depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("source use depth overflow"))?,
                budget,
            )? {
                if found.is_some() {
                    return Err(invalid("ambiguous full original source use trace"));
                }
                budget.charge(edges.len() as u64 + 1)?;
                edges.insert(
                    0,
                    u32::try_from(ordinal).map_err(|_| invalid("source edge ordinal overflow"))?,
                );
                found = Some(edges);
            }
        }
    }
    Ok(found)
}

pub(in crate::song::snapshot::occupancy) fn site_count(
    recipe: &FrozenIndexTiming,
    index: u32,
    issuer: NodeId,
    path: &[FrozenUseTraceTerm],
    cursor: usize,
    depth: u32,
    budget: &mut ProjectionBudget<'_>,
) -> Result<u32, Failure> {
    budget.enter(depth)?;
    let node = recipe
        .nodes()
        .get(index as usize)
        .ok_or_else(|| invalid("lookup timing node"))?;
    let mut count = u32::from(
        node.issuer() == issuer
            && cursor == path.len()
            && matches!(
                node.operation(),
                crate::song::source_uses::FrozenUseOperation::Slice
                    | crate::song::source_uses::FrozenUseOperation::Splice
            ),
    );
    for child in node.children() {
        budget.charge(child.trace().len() as u64 + 1)?;
        let end = cursor
            .checked_add(child.trace().len())
            .ok_or_else(|| invalid("site path overflow"))?;
        if path.get(cursor..end) == Some(child.trace()) {
            count = count
                .checked_add(site_count(
                    recipe,
                    child.child(),
                    issuer,
                    path,
                    end,
                    depth
                        .checked_add(1)
                        .ok_or_else(|| invalid("site depth overflow"))?,
                    budget,
                )?)
                .ok_or_else(|| invalid("site count overflow"))?;
        }
    }
    Ok(count)
}

/// Bridge a caller collector without retaining its RefCell borrow across callbacks.
fn with_work<T>(
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    f: impl FnOnce(SongLimits, &mut u32) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let (limits, before) = {
        let ledger = work.borrow();
        (ledger.limits, ledger.remaining())
    };
    let mut remaining = before;
    let result = f(limits, &mut remaining);
    let spent = before
        .checked_sub(remaining)
        .ok_or_else(|| invalid("route work was replenished"))?;
    work.borrow_mut().charge(u64::from(spent))?;
    result
}
/// Bind the actual fresh issued invocation, retaining its rebound entry clock.
pub(crate) fn bind_issued_owner<'a>(
    site: crate::song::routing::PreparedSiteRef<'a>,
    issuer: NodeId,
    prefix: &[FrozenUseTraceTerm],
    window: TimeSpan,
    transcript: &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    seal: &'a Rc<crate::pattern::eval::song_provenance::InvocationSeal>,
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<RetainedOwnerAddress<'a>, Failure> {
    let view = site.authority();
    let authority = LookupAuthority::Issued(view);
    let original_payload = with_work(work, |limits, remaining| {
        site.bind_original(limits, remaining, depth)
    })?;
    let before = work.borrow().remaining();
    if !transcript.authentic(view.original(), seal, work, depth)? {
        return Err(invalid("foreign route invocation"));
    }
    let charged_scan = before
        .checked_sub(work.borrow().remaining())
        .ok_or_else(|| invalid("route auth debit"))?;
    // invocation() authenticates again and then searches the same immutable vector.
    work.borrow_mut().charge(u64::from(charged_scan))?;
    let actual = transcript.invocation(seal, work, depth)?;
    let candidates = with_work(work, |limits, remaining| {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(view.records().len() as u64 + prefix.len() as u64 + 1)?;
        let mut candidates = Vec::new();
        for record in view.records() {
            let request = &record.request;
            budget.charge(request.prefix.len() as u64 + 1)?;
            if !site.authenticates_request(request) {
                continue;
            }
            if request.scope != site.scope()
                || request.track != site.track()
                || request.issuer != issuer
                || request.prefix != prefix
                || request.window.begin > window.begin
                || request.window.end < window.end
            {
                continue;
            }
            authenticate_authority(authority, request, depth, &mut budget)?;
            if !std::ptr::eq(
                original_payload,
                payload_for(authority, request, &mut budget)?,
            ) {
                return Err(invalid("foreign original prepared site"));
            }
            for retained in &record.invocations {
                let old = retained.lookup_owner();
                let fresh = actual.lookup_owner();
                budget.charge(
                    old.placement.0.len() as u64
                        + fresh.placement.0.len() as u64
                        + retained.lookup_entry().steps.len() as u64
                        + actual.lookup_entry().steps.len() as u64
                        + 2,
                )?;
                if old != fresh
                    || retained.lookup_seed() != actual.lookup_seed()
                    || retained.lookup_entry() != actual.lookup_entry()
                {
                    continue;
                }
                budget.charge(1)?;
                limits.check_events(
                    candidates
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| invalid("route candidates overflow"))?,
                )?;
                candidates.push((request, retained.as_ref()));
            }
        }
        Ok(candidates)
    })?;
    let mut found = None;
    for (request, retained) in candidates {
        if transcript.authentic_retained_invocation(view.original(), seal, retained, work, depth)? {
            if actual.lookup_depth() > work.borrow().limits.max_depth {
                return Err(Failure::new(
                    FailCode::DepthExceeded,
                    "issued route execution depth",
                ));
            }
            // Repeated retained coverage is harmless for this exact fresh invocation.
            found.get_or_insert(request);
        }
    }
    let request =
        found.ok_or_else(|| invalid("required issued execution is not retained at site"))?;
    with_work(work, |limits, remaining| {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        actual
            .lookup_clock()
            .retained_sources(actual.lookup_owner(), depth, &mut budget)?;
        Ok(())
    })?;
    Ok(RetainedOwnerAddress {
        authority,
        request,
        invocation: actual,
    })
}
