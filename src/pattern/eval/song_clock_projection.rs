//! Borrowed-counter projection of sealed original owner and selected-source clocks.
use super::*;

impl CanonicalClockProjection {
    pub(crate) fn configuration_sources_match(
        &self,
        other: &Self,
        depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<bool, Failure> {
        budget.enter(depth)?;
        let (Self::Known(left), Self::Known(right)) = (self, other) else {
            return Ok(false);
        };
        budget.charge(
            left.owner.placement.0.len() as u64
                + right.owner.placement.0.len() as u64
                + left.sources.len() as u64
                + right.sources.len() as u64
                + 1,
        )?;
        if left.owner.revision != right.owner.revision
            || left.owner.track != right.owner.track
            || left.owner.root != right.owner.root
            || left.owner.placement != right.owner.placement
            || left.owner.offset != right.owner.offset
            || left.owner.duration != right.owner.duration
            || left.sources.len() != right.sources.len()
        {
            return Ok(false);
        }
        for (a, b) in left.sources.iter().zip(&right.sources) {
            if a.returned.get().is_none() || b.returned.get().is_none() {
                return Err(invalid("unsealed configuration source"));
            }
            budget.charge(a.producer.steps.len() as u64 + b.producer.steps.len() as u64 + 1)?;
            if a.producer != b.producer
                || a.source.part().revision() != b.source.part().revision()
                || a.source.part().duration() != b.source.part().duration()
                || a.source.track() != b.source.track()
            {
                return Ok(false);
            }
            let af = a.source.selector().family();
            let bf = b.source.selector().family();
            budget.charge(af.len() as u64 + bf.len() as u64 + 1)?;
            if af.len() != bf.len() {
                return Ok(false);
            }
            for (x, y) in af.iter().zip(bf) {
                for sound in [x, y] {
                    if let crate::value::value::Sound::Sample(path) = sound {
                        budget.charge(path.text.len() as u64 + 1)?;
                    }
                }
                if crate::song::snapshot::FrozenSound::from_sound(x)?
                    != crate::song::snapshot::FrozenSound::from_sound(y)?
                {
                    return Ok(false);
                }
            }
            if !a.parent.configuration_sources_match(
                &b.parent,
                depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("configuration source depth overflow"))?,
                budget,
            )? {
                return Ok(false);
            }
        }
        // Frames/query pieces are genuine per-row mapping/applicability evidence,
        // not source/use configuration identity. Each row is projected separately.
        Ok(true)
    }
    pub(super) fn project_charged<'a>(
        &'a self,
        expected_owner: &CanonicalOwnerFrame,
        whole: TimeSpan,
        issuer_start: Ratio64,
        initial_orientation: ClockOrientation,
        charge: &mut impl FnMut(u64) -> Result<(), Failure>,
    ) -> Result<Option<CanonicalClockFootprint<'a>>, Failure> {
        let Self::Known(context) = self else {
            return Ok(None);
        };
        charge(
            context.owner.placement.0.len() as u64
                + expected_owner.placement.0.len() as u64
                + context.frames.len() as u64
                + 1,
        )?;
        if &context.owner != expected_owner {
            return Err(invalid("foreign issuer clock owner basis"));
        }
        let mut whole = whole;
        let mut start = issuer_start;
        let mut orientation = initial_orientation;
        let mut applicable = true;
        for frame in context.frames.iter().rev() {
            applicable &= sect(whole, frame.child_piece).is_some();
            if let ClockBoundary::Sample(relation) = &frame.boundary {
                let Some(structural) = relation.whole else {
                    return Ok(None);
                };
                applicable &= sect(whole, TimeSpan::point(relation.point)).is_some();
                whole = structural;
                start = structural.begin;
                orientation = ClockOrientation::At;
            } else {
                whole = frame.whole(whole)?;
                start = frame.point(start)?;
            }
            if matches!(frame.boundary, ClockBoundary::Rev(_)) {
                orientation = match orientation {
                    ClockOrientation::At => ClockOrientation::Before,
                    ClockOrientation::Before => ClockOrientation::At,
                };
            }
            if matches!(frame.boundary, ClockBoundary::Rev(_)) && frame.parent_piece.is_point() {
                applicable &= sect(whole, frame.parent_piece).is_some();
            }
        }
        Ok(Some(CanonicalClockFootprint {
            owner: &context.owner,
            whole,
            sample_start: start,
            orientation,
            applicable,
        }))
    }
}

/// Scoped original allowance, never a fresh collector or refunded budget.
pub(crate) struct ProjectionBudget<'a> {
    remaining: &'a mut u32,
    limits: crate::song::SongLimits,
}
impl<'a> ProjectionBudget<'a> {
    pub(crate) fn new(
        limits: crate::song::SongLimits,
        remaining: &'a mut u32,
    ) -> Result<Self, Failure> {
        limits.validate()?;
        if *remaining > limits.max_nodes {
            return Err(invalid("projection allowance exceeds original limit"));
        }
        Ok(Self { remaining, limits })
    }
    pub(crate) fn charge(&mut self, cost: u64) -> Result<(), Failure> {
        let cost = u32::try_from(cost).map_err(|_| invalid("projection charge overflow"))?;
        *self.remaining = self.remaining.checked_sub(cost).ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "retained projection work exhausted",
            )
        })?;
        Ok(())
    }
    pub(crate) fn enter(&mut self, depth: u32) -> Result<(), Failure> {
        self.charge(1)?;
        if depth > self.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "retained projection depth exceeded",
            ));
        }
        Ok(())
    }
}

/// Borrowed sealed boundary; it cannot be constructed from routing metadata.
pub(crate) struct SourceBoundaryRef<'a> {
    boundary: &'a SelectedSourceBoundary,
}
impl<'s> SourceBoundaryRef<'s> {
    pub(crate) fn is_empty(&self) -> Result<bool, Failure> {
        Ok(self
            .boundary
            .returned
            .get()
            .ok_or_else(|| invalid("unsealed source boundary"))?
            .is_empty())
    }
    pub(crate) fn policy_matches(
        &self,
        revision: crate::song::PartRevision,
        duration: Ratio64,
        selected: &crate::song::snapshot::FrozenSelectedSource,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<bool, Failure> {
        budget.charge(1)?;
        if self.boundary.source.part().revision() != revision
            || self.boundary.source.part().duration() != duration
            || self.boundary.source.track() != selected.track
        {
            return Ok(false);
        }
        let family = self.boundary.source.selector().family();
        budget.charge(family.len() as u64 + selected.family.len() as u64 + 1)?;
        if family.len() != selected.family.len() {
            return Ok(false);
        }
        for (actual, frozen) in family.iter().zip(&selected.family) {
            if let crate::value::value::Sound::Sample(path) = actual {
                budget.charge(path.text.len() as u64 + 1)?;
            }
            if crate::song::snapshot::FrozenSound::from_sound(actual)? != *frozen {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn policy_parent(&self) -> Result<&CanonicalOwnerFrame, Failure> {
        match &self.boundary.parent {
            CanonicalClockProjection::Known(context) => Ok(&context.owner),
            _ => Err(invalid("unknown source policy parent")),
        }
    }
    #[cfg(test)]
    pub(crate) fn request(&self) -> TimeSpan {
        self.boundary.request
    }
    pub(crate) fn producer(&self) -> &ProducerTrace {
        &self.boundary.producer
    }
    pub(crate) fn member(
        &self,
        handle: &crate::song::EventHandle,
        depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<Option<&'s crate::song::source::SongEventOrigin>, Failure> {
        budget.enter(depth)?;
        let returned = self
            .boundary
            .returned
            .get()
            .ok_or_else(|| invalid("unsealed source membership"))?;
        let mut found = None;
        for member in returned {
            budget.charge(
                member.handle.placement().0.len() as u64
                    + member.handle.occurrence().producer_ordinals.len() as u64
                    + handle.placement().0.len() as u64
                    + handle.occurrence().producer_ordinals.len() as u64
                    + 1,
            )?;
            if &member.handle == handle {
                // Validate the genuine full origin; preserve its original authority.
                let remaining_depth =
                    budget.limits.max_depth.checked_sub(depth).ok_or_else(|| {
                        Failure::new(FailCode::DepthExceeded, "inherited member depth exhausted")
                    })?;
                crate::song::source::copy_origin(member, budget.remaining, remaining_depth)?;
                if found.is_some() {
                    return Err(invalid("ambiguous sealed source member"));
                }
                found = Some(&**member);
            }
        }
        Ok(found)
    }
}

/// Authentic selected-root eligibility and a separate query-piece predicate.
#[cfg_attr(not(test), allow(dead_code))] // Borrowed geometry evidence; next route-authority phase connects the genuine consumer.
pub(crate) struct SourceProjectionHop<'a> {
    pub(crate) owner: &'a CanonicalOwnerFrame,
    pub(crate) domain: TimeSpan,
    pub(crate) query: TimeSpan,
    pub(crate) whole: TimeSpan,
    pub(crate) start: Ratio64,
    pub(crate) orientation: ClockOrientation,
    pub(crate) outgoing_start: Ratio64,
    pub(crate) outgoing_orientation: ClockOrientation,
    pub(crate) producer: &'a ProducerTrace,
    pub(crate) applicable: bool,
}
/// Actual issuer START remains separate from mapped structural sample time.
pub(crate) struct ProjectedInvocation<'a> {
    pub(crate) footprint: CanonicalClockFootprint<'a>,
    pub(crate) issuer_start: Ratio64,
    pub(crate) sources: Vec<SourceBoundaryRef<'a>>,
    #[cfg_attr(not(test), allow(dead_code))]
    // Retained geometry hops; production consumer wiring remains mandatory.
    pub(crate) hops: Vec<SourceProjectionHop<'a>>,
}
impl CanonicalClockProjection {
    pub(crate) fn retained_sources<'a>(
        &'a self,
        expected: &CanonicalOwnerFrame,
        depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<Vec<SourceBoundaryRef<'a>>, Failure> {
        budget.enter(depth)?;
        let Self::Known(context) = self else {
            return Err(invalid("unknown invocation entry clock"));
        };
        budget.charge(
            context.owner.placement.0.len() as u64 + expected.placement.0.len() as u64 + 1,
        )?;
        if &context.owner != expected {
            return Err(invalid("foreign invocation entry owner"));
        }
        self.source_chain(depth, budget)
    }
    pub(crate) fn has_parent_owner(
        &self,
        revision: crate::song::PartRevision,
        track: crate::value::intern::KwId,
        root: crate::reader::span::NodeId,
        depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<bool, Failure> {
        for source in self.source_chain(depth, budget)? {
            let parent = source.policy_parent()?;
            budget.charge(parent.placement.0.len() as u64 + 1)?;
            if parent.revision == revision && parent.track == track && parent.root == root {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn source_chain<'a>(
        &'a self,
        mut depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<Vec<SourceBoundaryRef<'a>>, Failure> {
        let mut next = self;
        let mut sources = Vec::new();
        loop {
            budget.enter(depth)?;
            let Self::Known(context) = next else {
                return Err(invalid("unknown selected source parent relation"));
            };
            budget.charge(context.sources.len() as u64 + 1)?;
            let Some(boundary) = context.sources.first() else {
                break;
            };
            if context.sources.len() != 1 {
                return Err(invalid("ambiguous selected source parent"));
            }
            if boundary.returned.get().is_none() {
                return Err(invalid("unsealed selected source relation"));
            }
            if boundary.source.track() != context.owner.track
                || !owns_source_entry(
                    boundary.source.part(),
                    &context.owner,
                    0,
                    Ratio64::ZERO,
                    depth,
                    budget,
                )?
            {
                return Err(invalid(
                    "selected source does not authenticate invocation placement",
                ));
            }
            budget.charge(boundary.producer.steps.len() as u64 + 1)?;
            sources.push(SourceBoundaryRef { boundary });
            next = &boundary.parent;
            depth = depth
                .checked_add(1)
                .ok_or_else(|| invalid("source projection depth overflow"))?;
        }
        Ok(sources)
    }
    pub(crate) fn project_retained<'a>(
        &'a self,
        expected: &CanonicalOwnerFrame,
        whole: TimeSpan,
        issuer_start: Ratio64,
        depth: u32,
        budget: &mut ProjectionBudget<'_>,
    ) -> Result<Option<ProjectedInvocation<'a>>, Failure> {
        budget.enter(depth)?;
        let Some(mut footprint) = self.project_charged(
            expected,
            whole,
            issuer_start,
            ClockOrientation::At,
            &mut |cost| budget.charge(cost),
        )?
        else {
            return Ok(None);
        };
        let sources = self.source_chain(depth, budget)?;
        let mut hops = Vec::new();
        let mut context = self;
        let mut depth = depth;
        for source in &sources {
            budget.enter(depth)?;
            let Self::Known(current) = context else {
                return Err(invalid("source projection basis"));
            };
            let boundary = source.boundary;
            let selected_whole = footprint
                .whole
                .map(|t| t.checked_add(current.owner.offset))?;
            footprint.applicable &= sect(selected_whole, boundary.request).is_some();
            let Self::Known(parent) = &boundary.parent else {
                return Err(invalid("unknown source parent clock"));
            };
            let Some(projected) = boundary.parent.project_charged(
                &parent.owner,
                selected_whole,
                footprint.sample_start.checked_add(current.owner.offset)?,
                footprint.orientation,
                &mut |cost| budget.charge(cost),
            )?
            else {
                return Ok(None);
            };
            let applicable = footprint.applicable && projected.applicable;
            budget.charge(1)?;
            hops.push(SourceProjectionHop {
                owner: &current.owner,
                domain: TimeSpan::new(Ratio64::ZERO, boundary.source.part().duration())?,
                query: boundary.request,
                whole: selected_whole,
                start: footprint.sample_start.checked_add(current.owner.offset)?,
                orientation: footprint.orientation,
                outgoing_start: projected.sample_start,
                outgoing_orientation: projected.orientation,
                producer: &boundary.producer,
                applicable,
            });
            footprint = projected;
            footprint.applicable = applicable;
            context = &boundary.parent;
            depth = depth
                .checked_add(1)
                .ok_or_else(|| invalid("source projection depth overflow"))?;
        }
        Ok(Some(ProjectedInvocation {
            footprint,
            issuer_start,
            sources,
            hops,
        }))
    }
}

fn owns_source_entry(
    part: &crate::song::Part,
    owner: &CanonicalOwnerFrame,
    cursor: usize,
    offset: Ratio64,
    depth: u32,
    budget: &mut ProjectionBudget<'_>,
) -> Result<bool, Failure> {
    use crate::song::{PartEdit, PartNode};
    budget.enter(depth)?;
    budget.charge(1)?;
    let next_depth = depth
        .checked_add(1)
        .ok_or_else(|| invalid("source topology depth overflow"))?;
    match part.node() {
        PartNode::Capture(tracks) => {
            budget.charge(tracks.len() as u64 + 1)?;
            Ok(cursor == owner.placement.0.len()
                && offset == owner.offset
                && part.revision() == owner.revision
                && part.duration() == owner.duration
                && tracks.get(&owner.track).is_some_and(|p| p.id == owner.root))
        }
        PartNode::Sequence(children) => {
            let Some(&[1, ordinal]) = owner.placement.0.get(cursor..cursor + 2) else {
                return Ok(false);
            };
            let mut begin = offset;
            budget.charge(u64::from(ordinal) + 1)?;
            for (index, child) in children.iter().enumerate() {
                if index == ordinal as usize {
                    return owns_source_entry(child, owner, cursor + 2, begin, next_depth, budget);
                }
                begin = begin.checked_add(child.duration())?;
            }
            Ok(false)
        }
        PartNode::Repeat { child, count, .. } => {
            let Some(&[2, copy]) = owner.placement.0.get(cursor..cursor + 2) else {
                return Ok(false);
            };
            if copy >= *count {
                return Ok(false);
            }
            let begin = offset.checked_add(
                child
                    .duration()
                    .checked_mul(Ratio64::from_int(i64::from(copy)))?,
            )?;
            owns_source_entry(child, owner, cursor + 2, begin, next_depth, budget)
        }
        PartNode::Edit { source, edit } => {
            let payload = match edit {
                PartEdit::ReplaceTrack { track, pattern } if *track == owner.track => {
                    Some((3, pattern, part.duration(), offset))
                }
                PartEdit::TransformInstrument { track, pattern, .. } if *track == owner.track => {
                    Some((5, pattern, part.duration(), offset))
                }
                PartEdit::OverwriteRegion {
                    track,
                    pattern,
                    region,
                } if *track == owner.track => Some((
                    4,
                    pattern,
                    region.duration()?,
                    offset.checked_add(region.begin)?,
                )),
                _ => None,
            };
            if let Some((tag, pattern, duration, begin)) = payload {
                budget.charge(3)?;
                if owner.placement.0.get(cursor..) == Some(&[tag, part.node_count()][..])
                    && begin == owner.offset
                    && duration == owner.duration
                    && part.revision() == owner.revision
                    && pattern.id == owner.root
                {
                    return Ok(true);
                }
            }
            owns_source_entry(source, owner, cursor, offset, next_depth, budget)
        }
    }
}
