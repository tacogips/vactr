//! Opaque borrowed original invocation addresses. No query or caller key dictionary.
pub(crate) mod authority;
use super::*;
use crate::pattern::eval::song_clock::{ProjectedInvocation, ProjectionBudget, SourceBoundaryRef};
use crate::pattern::eval::song_observation::OwnerInvocation;
use crate::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenSelectedSource};
use crate::value::Ratio64;
use authority::{authenticate_request, policy_path};

#[derive(Clone, Copy)]
pub(super) enum LookupAuthority<'a> {
    Snapshot(&'a SongSnapshot),
    Issued(&'a super::route_view::RouteAuthorityView),
}
pub(super) struct AuthorityRecord<'a> {
    request: &'a CanonicalIndexRequest,
    invocations: &'a [Rc<OwnerInvocation>],
}
enum AuthorityRecords<'a> {
    Snapshot(std::slice::Iter<'a, super::RetainedCanonicalIndex>),
    Issued(std::slice::Iter<'a, super::route_view::PublishedRetainedIndex>),
}
impl<'a> Iterator for AuthorityRecords<'a> {
    type Item = AuthorityRecord<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Snapshot(records) => records.next().map(|record| AuthorityRecord {
                request: &record.request,
                invocations: &record.invocations,
            }),
            Self::Issued(records) => records.next().map(|record| AuthorityRecord {
                request: &record.request,
                invocations: &record.invocations,
            }),
        }
    }
}
impl<'a> LookupAuthority<'a> {
    fn original(self) -> &'a Rc<Song> {
        match self {
            Self::Snapshot(snapshot) => &snapshot.song,
            Self::Issued(view) => view.original(),
        }
    }
    fn inventory(self) -> &'a super::super::FrozenRoutingInventory {
        match self {
            Self::Snapshot(snapshot) => &snapshot.routing,
            Self::Issued(view) => view.inventory(),
        }
    }
    fn records(self) -> AuthorityRecords<'a> {
        match self {
            Self::Snapshot(snapshot) => AuthorityRecords::Snapshot(snapshot.occupancy.iter()),
            Self::Issued(view) => AuthorityRecords::Issued(view.records().iter()),
        }
    }
    fn same_snapshot(self, snapshot: &SongSnapshot) -> bool {
        matches!(self,Self::Snapshot(original) if std::ptr::eq(original,snapshot))
    }
}
pub(crate) struct RetainedOwnerAddress<'s> {
    authority: LookupAuthority<'s>,
    request: &'s CanonicalIndexRequest,
    invocation: &'s OwnerInvocation,
}
impl RetainedOwnerAddress<'_> {
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn owner(&self) -> &crate::pattern::eval::song_observation::CanonicalOwnerFrame {
        self.invocation.lookup_owner()
    }
}
pub(crate) struct RetainedIndexAddress<'s> {
    owner: RetainedOwnerAddress<'s>,
}
pub(crate) struct RetainedSourceMember<'s> {
    edges: Vec<u32>,
    origin: &'s crate::song::source::SongEventOrigin,
}
pub(crate) struct RetainedProjectedIndexRow<'s> {
    observation: &'s CanonicalIndexObservation,
    projected: ProjectedInvocation<'s>,
}

#[cfg_attr(not(test), allow(dead_code))] // Next configuration consumer; owning fixtures exercise this entry.
pub(crate) fn owner_addresses<'s>(
    snapshot: &'s SongSnapshot,
    request: &'s CanonicalIndexRequest,
    depth: u32,
    limits: SongLimits,
    remaining: &mut u32,
) -> Result<Vec<RetainedOwnerAddress<'s>>, Failure> {
    let mut budget = ProjectionBudget::new(limits, remaining)?;
    authenticate_request(snapshot, request, depth, &mut budget)?;
    let mut addresses = Vec::new();
    for record in &snapshot.occupancy {
        budget.charge(record.request.prefix.len() as u64 + 1)?;
        if !Rc::ptr_eq(&record.request.original, &snapshot.song)
            || record.request.window.begin > request.window.begin
            || record.request.window.end < request.window.end
        {
            continue;
        }
        for invocation in &record.invocations {
            let owner = invocation.lookup_owner();
            budget.charge(
                owner.placement.0.len() as u64 + invocation.lookup_entry().steps.len() as u64 + 1,
            )?;
            if !invocation.authentic_original(&snapshot.song) {
                return Err(invalid("foreign stored invocation"));
            }
            let addressed = owner.revision == request.revision
                && owner.track == request.track
                && owner.root == request.root;
            if !addressed
                && !invocation.lookup_clock().has_parent_owner(
                    request.revision,
                    request.track,
                    request.root,
                    depth,
                    &mut budget,
                )?
            {
                continue;
            }
            if invocation.lookup_depth() > limits.max_depth {
                return Err(Failure::new(
                    FailCode::DepthExceeded,
                    "original invocation exceeds lookup depth",
                ));
            }
            // Multiple actual invocation contexts remain distinct tokens. No
            // geometric grouping can discard a seed or selected source entry.
            budget.charge(1)?;
            limits.check_events(
                addresses
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| invalid("lookup addresses overflow"))?,
            )?;
            addresses.push(RetainedOwnerAddress {
                authority: LookupAuthority::Snapshot(snapshot),
                request,
                invocation,
            });
        }
    }
    if addresses.is_empty() {
        return Err(invalid("required retained owner coverage missing"));
    }
    Ok(addresses)
}
#[cfg_attr(not(test), allow(dead_code))] // Next configuration consumer; owning fixtures exercise this entry.
pub(crate) fn bind_index<'s>(
    snapshot: &'s SongSnapshot,
    request: &'s CanonicalIndexRequest,
    mut owner: RetainedOwnerAddress<'s>,
    depth: u32,
    limits: SongLimits,
    remaining: &mut u32,
) -> Result<RetainedIndexAddress<'s>, Failure> {
    let mut budget = ProjectionBudget::new(limits, remaining)?;
    authenticate_request(snapshot, request, depth, &mut budget)?;
    budget.charge(request.prefix.len() as u64 + owner.request.prefix.len() as u64 + 1)?;
    let actual = owner.invocation.lookup_owner();
    if !owner.authority.same_snapshot(snapshot)
        || owner.request.window.begin > request.window.begin
        || owner.request.window.end < request.window.end
        || actual.revision != request.revision
        || actual.track != request.track
        || actual.root != request.root
    {
        return Err(invalid(
            "retained address belongs to a different original site or owner",
        ));
    }
    owner.request = request;
    // No observation is required: genuine successful empty q still binds policy.
    owner.invocation.lookup_clock().retained_sources(
        owner.invocation.lookup_owner(),
        depth,
        &mut budget,
    )?;
    Ok(RetainedIndexAddress { owner })
}
impl<'s> RetainedIndexAddress<'s> {
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(not(test), allow(dead_code))] // Actual geometry owning seam, awaiting production route-authority integration.
    pub(crate) fn validate_prepared_site(
        &self,
        request: &CanonicalIndexRequest,
        prepared: &crate::song::snapshot::FrozenPattern,
        issuer: NodeId,
        prefix: &[FrozenUseTraceTerm],
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        authority::authenticate_authority(self.owner.authority, request, depth, &mut budget)?;
        let original = self.owner.request;
        budget.charge(request.prefix.len() as u64 + original.prefix.len() as u64 + 1)?;
        if request.scope != original.scope
            || request.revision != original.revision
            || request.track != original.track
            || request.root != original.root
            || request.issuer != original.issuer
            || request.prefix != original.prefix
            || !Rc::ptr_eq(&request.recipe, &original.recipe)
        {
            return Err(invalid("configuration site differs from token"));
        }
        budget.charge(prefix.len() as u64 + request.prefix.len() as u64 + 1)?;
        if !std::ptr::eq(
            prepared,
            authority::payload_for(self.owner.authority, request, &mut budget)?,
        ) || issuer != request.issuer
            || prefix != request.prefix
        {
            return Err(invalid(
                "prepared operands differ from original retained site",
            ));
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(not(test), allow(dead_code))] // Actual geometry owning seam, awaiting production route-authority integration.
    pub(crate) fn row_source_window(
        &self,
        row: &RetainedProjectedIndexRow<'_>,
        selected: Option<&FrozenSelectedSource>,
        source: TimeSpan,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<bool, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        if let Some(selected) = selected {
            let part = self
                .owner
                .authority
                .inventory()
                .parts
                .get(selected.root_part)
                .ok_or_else(|| invalid("configuration selected root absent"))?;
            let mut eligible = None;
            for (boundary, hop) in row.projected.sources.iter().zip(&row.projected.hops) {
                if !boundary.policy_matches(part.revision, part.duration, selected, &mut budget)?
                    || self
                        .selected_policy(selected, boundary, depth, &mut budget)?
                        .is_none()
                {
                    continue;
                }
                if eligible.is_some() {
                    return Err(invalid("ambiguous source-window basis"));
                }
                budget.charge(1)?;
                eligible = Some(match hop.orientation {
                    crate::pattern::eval::song_clock::ClockOrientation::At => {
                        source.begin <= hop.start && hop.start < source.end
                    }
                    crate::pattern::eval::song_clock::ClockOrientation::Before => {
                        source.begin < hop.start && hop.start <= source.end
                    }
                });
            }
            eligible.ok_or_else(|| invalid("configuration source-window basis absent"))
        } else {
            if !row.projected.sources.is_empty() {
                return Err(invalid("direct issuer has selected source boundary"));
            }
            budget.charge(1)?;
            Ok(source.begin <= row.issuer_start() && row.issuer_start() < source.end)
        }
    }
    #[allow(dead_code)] // Narrow-window API retained; geometry uses validate_prepared_site until route issuance integration.
    pub(crate) fn validate_site(
        &self,
        request: &CanonicalIndexRequest,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        authority::authenticate_authority(self.owner.authority, request, depth, &mut budget)?;
        let original = self.owner.request;
        budget.charge(request.prefix.len() as u64 + original.prefix.len() as u64 + 1)?;
        if request.scope != original.scope
            || request.revision != original.revision
            || request.track != original.track
            || request.root != original.root
            || request.issuer != original.issuer
            || request.prefix != original.prefix
            || !Rc::ptr_eq(&request.recipe, &original.recipe)
            || request.window.begin < original.window.begin
            || request.window.end > original.window.end
        {
            return Err(invalid(
                "configuration request differs from retained original site",
            ));
        }
        Ok(())
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn seed(&self) -> u64 {
        self.owner.invocation.lookup_seed()
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn owner(&self) -> &crate::pattern::eval::song_observation::CanonicalOwnerFrame {
        self.owner.invocation.lookup_owner()
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn entry(&self) -> &ProducerTrace {
        self.owner.invocation.lookup_entry()
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn rows(
        &self,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<Vec<RetainedProjectedIndexRow<'s>>, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        let mut out = Vec::new();
        for row in self.owner.invocation.observations() {
            budget.charge(
                row.owner.placement.0.len() as u64
                    + row.prefix.steps.len() as u64
                    + self.owner.request.prefix.len() as u64
                    + 1,
            )?;
            if !self.owner.request.matches(row) {
                continue;
            }
            if &row.owner != self.owner.invocation.lookup_owner() {
                return Err(invalid("Index row differs from actual invocation key"));
            }
            let Some(whole) = row.event.whole else {
                return Err(invalid("continuous Index whole absent"));
            };
            let projected = row
                .clock
                .project_retained(
                    &row.owner,
                    whole,
                    row.issuer_sample_start,
                    depth,
                    &mut budget,
                )?
                .ok_or_else(|| invalid("unknown retained Index clock"))?;
            budget.charge(1)?;
            limits.check_events(
                out.len()
                    .checked_add(1)
                    .ok_or_else(|| invalid("lookup rows overflow"))?,
            )?;
            out.push(RetainedProjectedIndexRow {
                observation: row,
                projected,
            });
        }
        Ok(out)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn related_rows(
        &self,
        request: &CanonicalIndexRequest,
        owner: TimeSpan,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<Vec<RetainedProjectedIndexRow<'s>>, Failure> {
        // Site authentication is independent of root query coverage. Separate
        // published partitions may cover the requested immutable owner domain.
        {
            let mut budget = ProjectionBudget::new(limits, remaining)?;
            authority::authenticate_authority(self.owner.authority, request, depth, &mut budget)?;
            budget
                .charge(request.prefix.len() as u64 + self.owner.request.prefix.len() as u64 + 1)?;
            if request.scope != self.owner.request.scope
                || request.revision != self.owner.request.revision
                || request.track != self.owner.request.track
                || request.root != self.owner.request.root
                || request.issuer != self.owner.request.issuer
                || request.prefix != self.owner.request.prefix
                || !Rc::ptr_eq(&request.recipe, &self.owner.request.recipe)
            {
                return Err(invalid("configuration site differs from issued token"));
            }
        }
        let sources = self.source_boundaries(depth, limits, remaining)?;
        let basis = if let Some(source) = sources.last() {
            source.policy_parent()?
        } else {
            self.owner()
        };
        if owner.begin < Ratio64::ZERO || owner.end > basis.duration {
            return Err(invalid(
                "configuration owner extent differs from authentic final basis",
            ));
        }
        let required = owner.map(|point| point.checked_add(basis.offset))?;
        let mut coverage = Vec::new();
        let mut out = Vec::new();
        for record in self.owner.authority.records() {
            {
                let mut budget = ProjectionBudget::new(limits, remaining)?;
                budget.enter(depth)?;
                budget.charge(record.request.prefix.len() as u64 + 1)?;
                if !Rc::ptr_eq(&record.request.original, self.owner.authority.original()) {
                    return Err(invalid("foreign configuration record attachment"));
                }
                authority::authenticate_authority(
                    self.owner.authority,
                    record.request,
                    depth,
                    &mut budget,
                )?;
            }
            let mut relevant = false;
            for invocation in record.invocations {
                {
                    let mut budget = ProjectionBudget::new(limits, remaining)?;
                    budget.enter(depth)?;
                    let actual = invocation.lookup_owner();
                    budget.charge(
                        actual.placement.0.len() as u64
                            + self.owner().placement.0.len() as u64
                            + invocation.lookup_entry().steps.len() as u64
                            + self.entry().steps.len() as u64
                            + 1,
                    )?;
                    if !invocation.authentic_original(self.owner.authority.original()) {
                        return Err(invalid("foreign configuration invocation"));
                    }
                    if !same_intrinsic_owner(actual, self.owner())
                        || invocation.lookup_seed() != self.seed()
                        || invocation.lookup_entry() != self.entry()
                    {
                        continue;
                    }
                    if !invocation.lookup_clock().configuration_sources_match(
                        self.owner.invocation.lookup_clock(),
                        depth,
                        &mut budget,
                    )? {
                        continue;
                    }
                    if invocation.lookup_depth() > limits.max_depth {
                        return Err(Failure::new(
                            FailCode::DepthExceeded,
                            "configuration invocation depth",
                        ));
                    }
                }
                relevant = true;
                let related = RetainedIndexAddress {
                    owner: RetainedOwnerAddress {
                        authority: self.owner.authority,
                        request: self.owner.request,
                        invocation,
                    },
                };
                for row in related.rows(depth, limits, remaining)? {
                    let mut budget = ProjectionBudget::new(limits, remaining)?;
                    budget.charge(1)?;
                    limits.check_events(
                        out.len()
                            .checked_add(1)
                            .ok_or_else(|| invalid("configuration rows overflow"))?,
                    )?;
                    out.push(row);
                }
            }
            if relevant {
                let mut budget = ProjectionBudget::new(limits, remaining)?;
                insert_coverage(&mut coverage, record.request.window, &mut budget)?;
            }
        }
        let mut covered = false;
        for interval in &coverage {
            let mut budget = ProjectionBudget::new(limits, remaining)?;
            budget.charge(1)?;
            if interval.begin <= required.begin && interval.end >= required.end {
                covered = true;
                break;
            }
        }
        if !covered {
            return Err(invalid(
                "complete immutable configuration extent coverage absent",
            ));
        }
        Ok(out)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn source_boundaries(
        &self,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<Vec<SourceBoundaryRef<'s>>, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        self.owner
            .invocation
            .lookup_clock()
            .retained_sources(self.owner(), depth, &mut budget)
    }
    fn selected_policy(
        &self,
        selected: &FrozenSelectedSource,
        boundary: &SourceBoundaryRef<'_>,
        depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<Option<Vec<u32>>, Failure> {
        let parent = boundary.policy_parent()?;
        let mut found = None;
        for part in &self.owner.authority.inventory().parts {
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
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn empty_source(
        &self,
        selected: &FrozenSelectedSource,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<bool, Failure> {
        let boundaries = self.source_boundaries(depth, limits, remaining)?;
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        let part = self
            .owner
            .authority
            .inventory()
            .parts
            .get(selected.root_part)
            .ok_or_else(|| invalid("selected source root missing"))?;
        let mut found = None;
        for boundary in boundaries {
            if boundary.policy_matches(part.revision, part.duration, selected, &mut budget)? {
                if self
                    .selected_policy(selected, &boundary, depth, &mut budget)?
                    .is_none()
                {
                    continue;
                }
                if found.is_some() {
                    return Err(invalid("ambiguous original source boundary"));
                }
                found = Some(boundary.is_empty()?);
            }
        }
        found.ok_or_else(|| invalid("required original source boundary missing"))
    }
}
#[cfg(test)]
pub(super) type GeometrySourceHop = (
    TimeSpan,
    TimeSpan,
    Ratio64,
    crate::pattern::eval::song_clock::ClockOrientation,
    Ratio64,
    crate::pattern::eval::song_clock::ClockOrientation,
);
impl RetainedProjectedIndexRow<'_> {
    #[cfg(test)]
    pub(super) fn source_hops(&self) -> impl Iterator<Item = GeometrySourceHop> + '_ {
        self.projected.hops.iter().map(|hop| {
            (
                hop.domain,
                hop.query,
                hop.start,
                hop.orientation,
                hop.outgoing_start,
                hop.outgoing_orientation,
            )
        })
    }

    #[cfg_attr(not(test), allow(dead_code))] // Actual geometry owning seam, awaiting production route-authority integration.
    pub(crate) fn same_configuration_group(
        &self,
        other: &Self,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<bool, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        budget.enter(depth)?;
        budget.charge(
            self.observation.prefix.steps.len() as u64
                + other.observation.prefix.steps.len() as u64
                + self.observation.owner.placement.0.len() as u64
                + other.observation.owner.placement.0.len() as u64
                + 1,
        )?;
        if !same_intrinsic_owner(&self.observation.owner, &other.observation.owner)
            || self.observation.seed != other.observation.seed
            || self.observation.issuer != other.observation.issuer
            || self.observation.prefix != other.observation.prefix
            || self.projected.hops.len() != other.projected.hops.len()
        {
            return Ok(false);
        }
        if !self.observation.clock.configuration_sources_match(
            &other.observation.clock,
            depth,
            &mut budget,
        )? {
            return Ok(false);
        }
        for (left, right) in self.projected.hops.iter().zip(&other.projected.hops) {
            budget.charge(
                left.producer.steps.len() as u64
                    + right.producer.steps.len() as u64
                    + left.owner.placement.0.len() as u64
                    + right.owner.placement.0.len() as u64
                    + 1,
            )?;
            if !same_intrinsic_owner(left.owner, right.owner) || left.producer != right.producer {
                return Ok(false);
            }
        }
        Ok(true)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Actual geometry owning seam, awaiting production route-authority integration.
    pub(crate) fn source_eligible(
        &self,
        depth: u32,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<bool, Failure> {
        let mut budget = ProjectionBudget::new(limits, remaining)?;
        for (ordinal, hop) in self.projected.hops.iter().enumerate() {
            budget.enter(
                depth
                    .checked_add(
                        u32::try_from(ordinal).map_err(|_| invalid("source hop depth overflow"))?,
                    )
                    .ok_or_else(|| invalid("source hop depth overflow"))?,
            )?;
            budget
                .charge(hop.owner.placement.0.len() as u64 + hop.producer.steps.len() as u64 + 1)?;
            let eligible = match hop.orientation {
                crate::pattern::eval::song_clock::ClockOrientation::At => {
                    hop.domain.begin <= hop.start && hop.start < hop.domain.end
                }
                crate::pattern::eval::song_clock::ClockOrientation::Before => {
                    hop.domain.begin < hop.start && hop.start <= hop.domain.end
                }
            };
            let (next_start, next_orientation) =
                if let Some(next) = self.projected.hops.get(ordinal + 1) {
                    (next.start.checked_sub(next.owner.offset)?, next.orientation)
                } else {
                    (
                        self.projected.footprint.sample_start,
                        self.projected.footprint.orientation,
                    )
                };
            if hop.outgoing_start != next_start || hop.outgoing_orientation != next_orientation {
                return Err(invalid(
                    "retained source hop projection differs from next authentic basis",
                ));
            }
            if !eligible || !hop.applicable || sect(hop.whole, hop.query).is_none() {
                return Ok(false);
            }
        }
        Ok(self.projected.footprint.applicable)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Actual geometry owning seam, awaiting production route-authority integration.
    pub(crate) fn original_whole(&self) -> TimeSpan {
        self.observation.event.whole.unwrap()
    }
    #[cfg_attr(not(test), allow(dead_code))] // Actual geometry owning seam, awaiting production route-authority integration.
    pub(crate) fn original_producer(&self) -> Option<&ProducerTrace> {
        self.observation.event.producer.as_ref()
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn issuer_start(&self) -> crate::value::Ratio64 {
        self.projected.issuer_start
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn footprint(
        &self,
    ) -> &crate::pattern::eval::song_clock::CanonicalClockFootprint<'_> {
        &self.projected.footprint
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn issuer(&self) -> NodeId {
        self.observation.issuer
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn prefix(&self) -> &ProducerTrace {
        &self.observation.prefix
    }
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn source_count(&self) -> usize {
        self.projected.sources.len()
    }
}
#[cfg_attr(not(test), allow(dead_code))] // Next configuration consumer; owning fixtures exercise this entry.
pub(crate) fn bind_member<'s>(
    index: &RetainedIndexAddress<'s>,
    selected: &FrozenSelectedSource,
    handle: &crate::song::EventHandle,
    depth: u32,
    limits: SongLimits,
    remaining: &mut u32,
) -> Result<RetainedSourceMember<'s>, Failure> {
    let boundaries = index.source_boundaries(depth, limits, remaining)?;
    let mut budget = ProjectionBudget::new(limits, remaining)?;
    let part = index
        .owner
        .authority
        .inventory()
        .parts
        .get(selected.root_part)
        .ok_or_else(|| invalid("member source root"))?;
    let mut found = None;
    for boundary in boundaries {
        if !boundary.policy_matches(part.revision, part.duration, selected, &mut budget)? {
            continue;
        }
        let Some(edges) = index.selected_policy(selected, &boundary, depth, &mut budget)? else {
            continue;
        };
        if let Some(origin) = boundary.member(handle, depth, &mut budget)? {
            if found.is_some() {
                return Err(invalid("ambiguous original member binding"));
            }
            found = Some((origin, edges));
        }
    }
    found
        .map(|(origin, edges)| RetainedSourceMember { origin, edges })
        .ok_or_else(|| invalid("original source membership missing"))
}
impl RetainedSourceMember<'_> {
    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn use_edges(&self) -> &[u32] {
        &self.edges
    }

    #[cfg_attr(not(test), allow(dead_code))] // Borrowed opaque interface for next geometry consumer.
    pub(crate) fn origin(&self) -> &crate::song::source::SongEventOrigin {
        self.origin
    }
}

fn same_intrinsic_owner(
    left: &crate::pattern::eval::song_observation::CanonicalOwnerFrame,
    right: &crate::pattern::eval::song_observation::CanonicalOwnerFrame,
) -> bool {
    left.revision == right.revision
        && left.track == right.track
        && left.root == right.root
        && left.placement == right.placement
        && left.offset == right.offset
        && left.duration == right.duration
}

fn insert_coverage(
    intervals: &mut Vec<TimeSpan>,
    mut span: TimeSpan,
    budget: &mut ProjectionBudget<'_>,
) -> Result<(), Failure> {
    let mut i = 0;
    while i < intervals.len() {
        budget.charge(1)?;
        let previous = intervals[i];
        if previous.end < span.begin {
            i += 1;
            continue;
        }
        if span.end < previous.begin {
            break;
        }
        span = TimeSpan::new(span.begin.min(previous.begin), span.end.max(previous.end))?;
        budget.charge((intervals.len() - i) as u64)?;
        intervals.remove(i);
    }
    budget.charge((intervals.len() - i + 1) as u64)?;
    intervals.insert(i, span);
    Ok(())
}
