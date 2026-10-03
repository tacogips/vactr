//! Original lookup authentication, extracted before issued backend adoption.
use super::*;
use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
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

pub(in crate::song::snapshot::occupancy) fn selected_policy_in(
    authority: LookupAuthority<'_>,
    selected: &FrozenSelectedSource,
    boundary: &SourceBoundaryRef<'_>,
    depth: u32,
    budget: &mut ProjectionBudget<'_>,
) -> Result<Option<Vec<u32>>, Failure> {
    let parent = boundary.policy_parent()?;
    let mut found = None;
    for part in &authority.inventory().parts {
        budget.charge(1)?;
        if part.revision != parent.revision {
            continue;
        }
        let pattern = match &part.node {
            FrozenPartNode::Capture(patterns) => {
                budget.charge(patterns.len() as u64 + 1)?;
                patterns.iter().find_map(|(track, p)| {
                    (*track == parent.track && p.id == parent.root).then_some(p)
                })
            }
            FrozenPartNode::Edit {
                edit:
                    FrozenEdit::Transform { track, payload, .. }
                    | FrozenEdit::Replace { track, payload }
                    | FrozenEdit::Overwrite { track, payload, .. },
                ..
            } if *track == parent.track && payload.id == parent.root => Some(payload),
            _ => None,
        };
        if let Some(pattern) = pattern {
            budget.charge(pattern.sources.len() as u64 + 1)?;
            let Some(policy) = pattern
                .sources
                .iter()
                .position(|source| std::ptr::eq(source, selected))
            else {
                // Frozen copies can share logical parent identity. Only the
                // allocation owning this exact descriptor can certify its use.
                continue;
            };
            if let Some(edges) = policy_path(
                &pattern.source_uses,
                pattern.source_uses.root,
                policy,
                &boundary.producer().steps,
                0,
                depth,
                budget,
            )? {
                if found.is_some() {
                    return Err(invalid("ambiguous exact selected policy parent use"));
                }
                found = Some(edges);
            } else {
                return Err(invalid(
                    "source boundary producer does not authenticate original use",
                ));
            }
        }
    }
    Ok(found)
}

pub(crate) struct IssuedMemberQuery<'a, 'p> {
    pub(crate) site: crate::song::routing::PreparedSiteRef<'a>,
    pub(crate) owner: &'p crate::pattern::eval::song_observation::CanonicalOwnerFrame,
    pub(crate) selected: &'p FrozenSelectedSource,
    pub(crate) handle: &'p crate::song::EventHandle,
}

pub(crate) fn bind_issued_member<'a>(
    query: &IssuedMemberQuery<'a, '_>,
    transcript: &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    seals: &'a [Rc<crate::pattern::eval::song_provenance::InvocationSeal>],
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<RetainedSourceMember<'a>, Failure> {
    let authority = LookupAuthority::Issued(query.site.authority());
    let part = authority
        .inventory()
        .parts
        .get(query.selected.root_part)
        .ok_or_else(|| invalid("member source root"))?;
    let mut found: Option<(&'a crate::song::source::SongEventOrigin, Vec<u32>)> = None;
    for seal in seals {
        let actual = transcript.invocation(seal, work, depth)?;
        let matched = with_work(work, |limits, remaining| {
            let mut budget = ProjectionBudget::new(limits, remaining)?;
            let boundaries = actual.lookup_clock().retained_sources(
                actual.lookup_owner(),
                depth,
                &mut budget,
            )?;
            let mut matched: Option<(&'a crate::song::source::SongEventOrigin, Vec<u32>)> = None;
            for boundary in boundaries {
                if !super::same_intrinsic_owner(boundary.policy_parent()?, query.owner) {
                    continue;
                }
                if !boundary.policy_matches(
                    part.revision,
                    part.duration,
                    query.selected,
                    &mut budget,
                )? {
                    continue;
                }
                let Some(edges) =
                    selected_policy_in(authority, query.selected, &boundary, depth, &mut budget)?
                else {
                    continue;
                };
                let Some(origin) = boundary.member(query.handle, depth, &mut budget)? else {
                    continue;
                };
                if let Some((previous, previous_edges)) = matched.as_ref() {
                    if !std::ptr::eq(origin, *previous) || edges != *previous_edges {
                        return Err(invalid("ambiguous original member binding"));
                    }
                } else {
                    matched = Some((origin, edges));
                }
            }
            Ok(matched)
        })?;
        if let Some((origin, edges)) = matched {
            if let Some((previous, previous_edges)) = found.as_ref() {
                if !std::ptr::eq(origin, *previous) || edges != *previous_edges {
                    return Err(invalid("ambiguous original member binding"));
                }
            } else {
                found = Some((origin, edges));
            }
        }
    }
    found
        .map(|(origin, edges)| RetainedSourceMember { origin, edges })
        .ok_or_else(|| invalid("original source membership missing"))
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
pub(crate) fn with_work<T>(
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
pub(crate) struct IssuedOwnerSelector<'a, 'p> {
    pub(crate) site: crate::song::routing::PreparedSiteRef<'a>,
    pub(crate) issuer: NodeId,
    pub(crate) prefix: &'p [FrozenUseTraceTerm],
    pub(crate) owner_window: TimeSpan,
}

/// Opaque operands authenticated from one actual issued invocation.
pub(crate) struct IssuedIndexOperand<'a> {
    request: &'a CanonicalIndexRequest,
    address: RetainedIndexAddress<'a>,
    site: crate::song::routing::PreparedSiteRef<'a>,
    policy: Option<crate::song::routing::PreparedPolicyRef<'a>>,
}
impl<'a> IssuedIndexOperand<'a> {
    pub(crate) fn address(&self) -> &RetainedIndexAddress<'a> {
        &self.address
    }

    pub(crate) fn site(&self) -> crate::song::routing::PreparedSiteRef<'a> {
        self.site
    }

    pub(crate) fn policy(&self) -> Option<&crate::song::routing::PreparedPolicyRef<'a>> {
        self.policy.as_ref()
    }

    pub(crate) fn authenticates_prepared_site(
        &self,
        issuer: NodeId,
        prefix: &[FrozenUseTraceTerm],
        original: &crate::song::snapshot::FrozenPattern,
    ) -> bool {
        self.site.authenticates_request(self.request)
            && self.request.scope == self.site.scope()
            && self.request.track == self.site.track()
            && self.request.issuer == issuer
            && self.request.prefix == prefix
            && self.request.root == original.id
            && original
                .index_timing
                .as_ref()
                .is_some_and(|recipe| Rc::ptr_eq(recipe, &self.request.recipe))
    }
}

/// Bind the actual fresh issued invocation, retaining its rebound entry clock.
pub(crate) fn bind_issued_owner<'a>(
    selector: &IssuedOwnerSelector<'a, '_>,
    transcript: &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    seal: &'a Rc<crate::pattern::eval::song_provenance::InvocationSeal>,
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<RetainedOwnerAddress<'a>, Failure> {
    bind_issued_owner_if_matching(selector, transcript, seal, work, depth)?
        .ok_or_else(|| invalid("required issued execution is not retained at site"))
}

fn bind_issued_owner_if_matching<'a>(
    selector: &IssuedOwnerSelector<'a, '_>,
    transcript: &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    seal: &'a Rc<crate::pattern::eval::song_provenance::InvocationSeal>,
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<Option<RetainedOwnerAddress<'a>>, Failure> {
    let view = selector.site.authority();
    let authority = LookupAuthority::Issued(view);
    let original_payload = with_work(work, |limits, remaining| {
        selector.site.bind_original(limits, remaining, depth)
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
    let (site_requests, candidates) = with_work(work, |limits, remaining| {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(view.records().len() as u64 + selector.prefix.len() as u64 + 1)?;
        let mut site_requests = Vec::new();
        let mut candidates = Vec::new();
        for record in view.records() {
            let request = &record.request;
            budget.charge(request.prefix.len() as u64 + 1)?;
            if !selector.site.authenticates_request(request) {
                continue;
            }
            if request.scope != selector.site.scope()
                || request.track != selector.site.track()
                || request.issuer != selector.issuer
                || request.prefix != selector.prefix
                || request.window.begin > selector.owner_window.begin
                || request.window.end < selector.owner_window.end
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
            site_requests.push(request);
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
        Ok((site_requests, candidates))
    })?;
    if site_requests.is_empty() {
        return Ok(None);
    }
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
    if found.is_none() {
        let fallback = with_work(work, |limits, remaining| {
            let mut budget = ProjectionBudget::new(limits, remaining)?;
            budget.enter(depth)?;
            budget.charge(view.records().len() as u64 + 1)?;
            let mut candidates = Vec::new();
            for record in view.records() {
                budget.charge(record.request.prefix.len() as u64 + 1)?;
                authenticate_authority(authority, &record.request, depth, &mut budget)?;
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
                    candidates.push(retained.as_ref());
                }
            }
            Ok(candidates)
        })?;
        for retained in fallback {
            if transcript.authentic_retained_invocation(
                view.original(),
                seal,
                retained,
                work,
                depth,
            )? {
                if actual.lookup_depth() > work.borrow().limits.max_depth {
                    return Err(Failure::new(
                        FailCode::DepthExceeded,
                        "issued route execution depth",
                    ));
                }
                found = site_requests.first().copied();
                break;
            }
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
    Ok(Some(RetainedOwnerAddress {
        authority,
        request,
        invocation: actual,
    }))
}

/// Authenticate the fresh invocation and bind the selected original request.
pub(crate) fn bind_issued_index<'a>(
    selector: &IssuedOwnerSelector<'a, '_>,
    transcript: &'a crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    seal: &'a Rc<crate::pattern::eval::song_provenance::InvocationSeal>,
    policy: Option<crate::song::routing::PreparedPolicyRef<'a>>,
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<Option<IssuedIndexOperand<'a>>, Failure> {
    let Some(owner) = bind_issued_owner_if_matching(selector, transcript, seal, work, depth)?
    else {
        return Ok(None);
    };
    let request = owner.request;
    Ok(Some(IssuedIndexOperand {
        request,
        address: RetainedIndexAddress { owner },
        site: selector.site,
        policy,
    }))
}
