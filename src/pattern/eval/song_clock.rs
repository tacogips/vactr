//! Authentic pre-child issuer clocks and scoped structural/selected-source relations.
use super::song_observation::CanonicalOwnerFrame;
use crate::pattern::occ::ProducerTrace;
use crate::pattern::query::Event;
use crate::pattern::query::{sect, TimeSpan};
use crate::value::Ratio64;
use crate::vm::fail::{FailCode, Failure};
use std::cell::OnceCell;
use std::rc::Rc;
mod dispatch;
#[path = "song_clock_projection.rs"]
mod projection;
pub(crate) use projection::{ProjectedInvocation, ProjectionBudget, SourceBoundaryRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClockOrientation {
    At,
    Before,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum ClockBoundary {
    Fast(Ratio64),
    Squeeze {
        width: Ratio64,
        base: Ratio64,
        step: TimeSpan,
    },
    Iter(Ratio64),
    Rev(Ratio64),
    Sample(Rc<CanonicalSamplingRelation>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalClockFrame {
    boundary: ClockBoundary,
    child_piece: TimeSpan,
    parent_piece: TimeSpan,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalClockContext {
    owner: CanonicalOwnerFrame,
    frames: Vec<CanonicalClockFrame>,
    sources: Vec<Rc<SelectedSourceBoundary>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalClockProjection {
    Known(Rc<CanonicalClockContext>),
    Unknown,
    Permit(Rc<SamplingPermit>),
    Source(Rc<SelectedSourceBoundary>),
}
/// Actual structural timing, distinct from the sampled child's whole.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CanonicalSamplingRelation {
    whole: Option<TimeSpan>,
    part: TimeSpan,
    point: Ratio64,
    producer: Option<ProducerTrace>,
}
/// Ephemeral scoped permit. The pointer is compared only, never dereferenced
/// or retained in a published observation; the caller keeps the child borrowed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SamplingPermit {
    child: *const crate::pattern::pat::Pat,
    point: Ratio64,
    producer: Option<ProducerTrace>,
    relation: Rc<CanonicalSamplingRelation>,
    parent: CanonicalClockProjection,
}
/// Query-issued selected source boundary, completed only by genuine returned rows.
#[derive(Debug)]
pub(crate) struct SelectedSourceBoundary {
    source: crate::song::source::SongSource,
    request: TimeSpan,
    producer: ProducerTrace,
    parent: CanonicalClockProjection,
    returned: OnceCell<Vec<Rc<crate::song::source::SongEventOrigin>>>,
}
impl SelectedSourceBoundary {
    pub(super) fn validate_issued_child(
        &self,
        owner: &CanonicalOwnerFrame,
        entry: &[crate::pattern::occ::ProducerStep],
        original_sound: &crate::value::value::Sound,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
    ) -> Result<bool, Failure> {
        work.charge(
            entry.len() as u64
                + self.producer.steps.len() as u64
                + self.source.selector().family().len() as u64
                + 1,
        )?;
        Ok(entry == self.producer.steps
            && self.source.selector().contains(original_sound)
            && source_owns_owner(self.source.part(), self.source.track(), owner, depth, work)?)
    }
    pub(super) fn actual_returned(&self) -> Option<&[Rc<crate::song::source::SongEventOrigin>]> {
        self.returned.get().map(Vec::as_slice)
    }
}
impl PartialEq for SelectedSourceBoundary {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl Eq for SelectedSourceBoundary {}
impl CanonicalClockFrame {
    pub(crate) fn fast(
        factor: Ratio64,
        child: TimeSpan,
        parent: TimeSpan,
    ) -> Result<Self, Failure> {
        if factor <= Ratio64::ZERO {
            return Err(invalid("clock factor must be positive"));
        }
        Ok(Self {
            boundary: ClockBoundary::Fast(factor),
            child_piece: child,
            parent_piece: parent,
        })
    }
    pub(crate) fn squeeze(
        width: Ratio64,
        base: Ratio64,
        step: TimeSpan,
        child: TimeSpan,
        parent: TimeSpan,
    ) -> Result<Self, Failure> {
        if width <= Ratio64::ZERO {
            return Err(invalid("clock squeeze width must be positive"));
        }
        Ok(Self {
            boundary: ClockBoundary::Squeeze { width, base, step },
            child_piece: child,
            parent_piece: parent,
        })
    }
    pub(crate) fn iter(shift: Ratio64, child: TimeSpan, parent: TimeSpan) -> Result<Self, Failure> {
        Ok(Self {
            boundary: ClockBoundary::Iter(shift),
            child_piece: child,
            parent_piece: parent,
        })
    }
    pub(crate) fn rev(mirror: Ratio64, child: TimeSpan, parent: TimeSpan) -> Self {
        Self {
            boundary: ClockBoundary::Rev(mirror),
            child_piece: child,
            parent_piece: parent,
        }
    }
    fn point(&self, t: Ratio64) -> Result<Ratio64, Failure> {
        match &self.boundary {
            ClockBoundary::Fast(factor) => t.checked_div(*factor),
            ClockBoundary::Squeeze { width, base, step } => step
                .begin
                .checked_add(t.checked_sub(*base)?.checked_mul(*width)?),
            ClockBoundary::Iter(shift) => t.checked_sub(*shift),
            ClockBoundary::Rev(mirror) => mirror.checked_sub(t),
            ClockBoundary::Sample(_) => {
                Err(invalid("sampling relation is not an affine point map"))
            }
        }
    }
    fn whole(&self, whole: TimeSpan) -> Result<TimeSpan, Failure> {
        let begin = self.point(whole.begin)?;
        let end = self.point(whole.end)?;
        TimeSpan::new(begin.min(end), begin.max(end))
    }
}
/// Owner-associated output; root arrangement projection is a separate step.
#[derive(Debug)]
#[allow(dead_code)] // Fields consumed by the following immutable geometry phase.
pub(crate) struct CanonicalClockFootprint<'a> {
    pub owner: &'a CanonicalOwnerFrame,
    pub whole: TimeSpan,
    pub sample_start: Ratio64,
    pub orientation: ClockOrientation,
    /// Only captured query-piece/Rev predicates, not structural/source
    /// eligibility or proof that a pre-subject observation emitted a note.
    pub applicable: bool,
}
#[cfg(test)]
type SourceEvidence = (
    TimeSpan,
    ProducerTrace,
    CanonicalClockProjection,
    Vec<Rc<crate::song::source::SongEventOrigin>>,
    Rc<SelectedSourceBoundary>,
);
/// Authenticates the exact original payload owner through the immutable selected Part.
fn source_owns_owner(
    part: &crate::song::Part,
    track: crate::value::intern::KwId,
    owner: &CanonicalOwnerFrame,
    depth: u32,
    work: &mut super::song_observation::CanonicalIndexCollector,
) -> Result<bool, Failure> {
    use crate::song::{PartEdit, PartNode};
    work.charge(1)?;
    if depth > work.limits.max_depth {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "source owner topology depth exceeded",
        ));
    }
    if track != owner.track {
        return Ok(false);
    }
    let mut payload = None;
    let owns_child = match part.node() {
        PartNode::Capture(tracks) => {
            payload = tracks.get(&track);
            false
        }
        PartNode::Sequence(children) => {
            let mut found = false;
            for child in children {
                if source_owns_owner(child, track, owner, depth + 1, work)? {
                    found = true;
                    break;
                }
            }
            found
        }
        PartNode::Repeat { child, count, .. } => {
            *count != 0 && source_owns_owner(child, track, owner, depth + 1, work)?
        }
        PartNode::Edit { source, edit } => {
            match edit {
                PartEdit::ReplaceTrack {
                    track: selected,
                    pattern,
                }
                | PartEdit::TransformInstrument {
                    track: selected,
                    pattern,
                    ..
                }
                | PartEdit::OverwriteRegion {
                    track: selected,
                    pattern,
                    ..
                } if *selected == track => payload = Some(pattern),
                _ => {}
            }
            source_owns_owner(source, track, owner, depth + 1, work)?
        }
    };
    Ok(owns_child
        || (part.revision() == owner.revision
            && part.duration() == owner.duration
            && payload.is_some_and(|pattern| pattern.id == owner.root)))
}
impl CanonicalClockProjection {
    /// Immutable owner/frame payloads are shared; copying an observation never
    /// allocates another frame sequence or duplicates original authority.
    pub(crate) fn copy_work(&self) -> u64 {
        1
    }
    fn frame_work(&self) -> u64 {
        match self {
            Self::Known(context) => {
                context.owner.placement.0.len() as u64 + context.frames.len() as u64 + 2
            }
            Self::Unknown | Self::Permit(_) | Self::Source(_) => 1,
        }
    }
    pub(crate) fn rebind_observation(
        &self,
        original: &Self,
        observation: &Self,
        owner: &CanonicalOwnerFrame,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
    ) -> Result<Self, Failure> {
        self.rebind_observation_mapped(original, observation, owner, depth, work, None)
    }
    pub(super) fn rebind_observation_mapped(
        &self,
        original: &Self,
        observation: &Self,
        owner: &CanonicalOwnerFrame,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mapper: Option<&mut super::song_provenance::IssuedOriginRebinder>,
    ) -> Result<Self, Failure> {
        work.charge(owner.placement.0.len() as u64 * 2 + 1)?;
        let (Self::Known(current), Self::Known(old)) = (self, original) else {
            return Ok(observation.clone());
        };
        if &current.owner != owner || &old.owner != owner {
            return Err(invalid("foreign replay clock entry owner"));
        }
        match (old.sources.first(), current.sources.first()) {
            (Some(previous), Some(next)) => {
                let paths = previous
                    .source
                    .selector()
                    .family()
                    .iter()
                    .chain(next.source.selector().family())
                    .map(|sound| match sound {
                        crate::value::value::Sound::Sample(path) => path.text.len() as u64,
                        _ => 0,
                    })
                    .sum::<u64>();
                work.charge(
                    paths
                        + (previous.source.selector().family().len()
                            + next.source.selector().family().len())
                            as u64
                        + 1,
                )?;
                if !Rc::ptr_eq(previous.source.part(), next.source.part())
                    || previous.source.track() != next.source.track()
                    || previous.source.selector() != next.source.selector()
                {
                    return Err(invalid("foreign replay selected source boundary"));
                }
                observation.replace_source(previous, next, depth, work, mapper)
            }
            (None, Some(next)) => {
                if !source_owns_owner(next.source.part(), next.source.track(), owner, depth, work)?
                {
                    return Err(invalid("sampled source does not own retained raw owner"));
                }
                observation.insert_source(owner, next, depth, work, mapper)
            }
            (None, None) => match mapper {
                Some(mapper) => observation.copy_issued_clock(depth, work, mapper),
                None => Ok(observation.clone()),
            },
            _ => Err(invalid("missing replay selected source boundary")),
        }
    }
    fn copy_issued_clock(
        &self,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mapper: &mut super::song_provenance::IssuedOriginRebinder,
    ) -> Result<Self, Failure> {
        if depth >= work.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "issued clock inherited depth",
            ));
        }
        work.charge(1)?;
        let Self::Known(context) = self else {
            return Ok(self.clone());
        };
        work.charge(
            context.frames.len() as u64
                + context.owner.placement.0.len() as u64
                + context.sources.len() as u64
                + 1,
        )?;
        let mut sources = Vec::with_capacity(context.sources.len());
        for boundary in &context.sources {
            if let Some(current) = mapper.copied_boundary(boundary, work)? {
                sources.push(current);
                continue;
            }
            let parent = boundary.parent.copy_issued_clock(depth + 1, work, mapper)?;
            work.charge(
                boundary.producer.steps.len() as u64
                    + boundary.source.selector().family().len() as u64
                    + 1,
            )?;
            let returned = OnceCell::new();
            returned
                .set(
                    mapper.copy_members(
                        boundary
                            .actual_returned()
                            .ok_or_else(|| invalid("unsealed clock members"))?,
                        depth + 1,
                        work,
                    )?,
                )
                .map_err(|_| invalid("clock member copy"))?;
            let current = Rc::new(SelectedSourceBoundary {
                source: boundary.source.clone(),
                request: boundary.request,
                producer: boundary.producer.clone(),
                parent,
                returned,
            });
            mapper.boundary_pair(boundary, &current, work)?;
            sources.push(current);
        }
        Ok(Self::Known(Rc::new(CanonicalClockContext {
            owner: context.owner.clone(),
            frames: context.frames.clone(),
            sources,
        })))
    }
    fn insert_source(
        &self,
        owner: &CanonicalOwnerFrame,
        next: &Rc<SelectedSourceBoundary>,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mut mapper: Option<&mut super::song_provenance::IssuedOriginRebinder>,
    ) -> Result<Self, Failure> {
        work.charge(1)?;
        if depth > work.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source insertion depth exceeded",
            ));
        }
        let Self::Known(context) = self else {
            return Ok(self.clone());
        };
        work.charge(context.owner.placement.0.len() as u64 + owner.placement.0.len() as u64 + 1)?;
        if context.owner == *owner {
            if !context.sources.is_empty() {
                return Err(invalid("independent source context already linked"));
            }
            work.charge(context.frames.len() as u64 + context.owner.placement.0.len() as u64 + 2)?;
            return Ok(Self::Known(Rc::new(CanonicalClockContext {
                owner: context.owner.clone(),
                frames: context.frames.clone(),
                sources: vec![next.clone()],
            })));
        }
        work.charge(
            context.sources.len() as u64
                + context.frames.len() as u64
                + context.owner.placement.0.len() as u64
                + 1,
        )?;
        let mut sources = Vec::with_capacity(context.sources.len());
        let mut changed = false;
        for boundary in &context.sources {
            if let Some(mapper) = mapper.as_deref_mut() {
                if let Some(current) = mapper.copied_boundary(boundary, work)? {
                    changed = true;
                    sources.push(current);
                    continue;
                }
            }
            let parent = boundary.parent.insert_source(
                owner,
                next,
                depth + 1,
                work,
                mapper.as_deref_mut(),
            )?;
            if parent.same_storage(&boundary.parent) && mapper.is_none() {
                sources.push(boundary.clone());
                continue;
            }
            changed = true;
            let members = boundary
                .returned
                .get()
                .ok_or_else(|| invalid("unsealed descendant source boundary"))?;
            work.charge(
                boundary.producer.steps.len() as u64 * 2
                    + boundary.source.selector().family().len() as u64
                    + members.len() as u64
                    + 2,
            )?;
            let returned = OnceCell::new();
            returned
                .set(match mapper.as_deref_mut() {
                    Some(mapper) => mapper.copy_members(members, depth + 1, work)?,
                    None => members.clone(),
                })
                .map_err(|_| invalid("source membership already sealed"))?;
            let copied = Rc::new(SelectedSourceBoundary {
                source: boundary.source.clone(),
                request: boundary.request,
                producer: boundary.producer.clone(),
                parent,
                returned,
            });
            if let Some(mapper) = mapper.as_deref_mut() {
                mapper.boundary_pair(boundary, &copied, work)?;
            }
            sources.push(copied);
        }
        if !changed {
            return Ok(self.clone());
        }
        Ok(Self::Known(Rc::new(CanonicalClockContext {
            owner: context.owner.clone(),
            frames: context.frames.clone(),
            sources,
        })))
    }
    fn same_storage(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown, Self::Unknown) => true,
            (Self::Known(a), Self::Known(b)) => Rc::ptr_eq(a, b),
            (Self::Permit(a), Self::Permit(b)) => Rc::ptr_eq(a, b),
            (Self::Source(a), Self::Source(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
    fn replace_source(
        &self,
        old: &Rc<SelectedSourceBoundary>,
        next: &Rc<SelectedSourceBoundary>,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mut mapper: Option<&mut super::song_provenance::IssuedOriginRebinder>,
    ) -> Result<Self, Failure> {
        work.charge(1)?;
        if depth > work.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source binding depth exhausted",
            ));
        }
        let Self::Known(context) = self else {
            return Ok(self.clone());
        };
        work.charge(
            context.sources.len() as u64
                + context.frames.len() as u64
                + context.owner.placement.0.len() as u64
                + 1,
        )?;
        let mut changed = false;
        let mut sources = Vec::with_capacity(context.sources.len());
        for boundary in &context.sources {
            if Rc::ptr_eq(boundary, old) {
                changed = true;
                sources.push(next.clone());
            } else {
                if let Some(mapper) = mapper.as_deref_mut() {
                    if let Some(current) = mapper.copied_boundary(boundary, work)? {
                        changed = true;
                        sources.push(current);
                        continue;
                    }
                }
                let parent = boundary.parent.replace_source(
                    old,
                    next,
                    depth + 1,
                    work,
                    mapper.as_deref_mut(),
                )?;
                if parent.same_storage(&boundary.parent) && mapper.is_none() {
                    sources.push(boundary.clone());
                } else {
                    changed = true;
                    let members = boundary
                        .returned
                        .get()
                        .ok_or_else(|| invalid("unsealed cached source boundary"))?;
                    work.charge(
                        boundary.producer.steps.len() as u64 * 2
                            + boundary.source.selector().family().len() as u64
                            + members.len() as u64
                            + 2,
                    )?;
                    let returned = OnceCell::new();
                    returned
                        .set(match mapper.as_deref_mut() {
                            Some(mapper) => mapper.copy_members(members, depth + 1, work)?,
                            None => members.clone(),
                        })
                        .map_err(|_| invalid("source binding already completed"))?;
                    let copied = Rc::new(SelectedSourceBoundary {
                        source: boundary.source.clone(),
                        request: boundary.request,
                        producer: boundary.producer.clone(),
                        parent,
                        returned,
                    });
                    if let Some(mapper) = mapper.as_deref_mut() {
                        mapper.boundary_pair(boundary, &copied, work)?;
                    }
                    sources.push(copied);
                }
            }
        }
        if !changed {
            return Ok(self.clone());
        }
        Ok(Self::Known(Rc::new(CanonicalClockContext {
            owner: context.owner.clone(),
            frames: context.frames.clone(),
            sources,
        })))
    }
    pub(super) fn rebind_completed_boundary(
        &self,
        original: &Self,
        boundary: &Rc<SelectedSourceBoundary>,
        owner: &CanonicalOwnerFrame,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mapper: &mut super::song_provenance::IssuedOriginRebinder,
    ) -> Result<Rc<SelectedSourceBoundary>, Failure> {
        if let Some(current) = mapper.copied_boundary(boundary, work)? {
            return Ok(current);
        }
        let parent = self.rebind_observation_mapped(
            original,
            &boundary.parent,
            owner,
            depth,
            work,
            Some(mapper),
        )?;
        work.charge(
            boundary.producer.steps.len() as u64 * 2
                + boundary.source.selector().family().len() as u64
                + 2,
        )?;
        let members = boundary
            .returned
            .get()
            .ok_or_else(|| invalid("unsealed issued replay boundary"))?;
        let returned = OnceCell::new();
        returned
            .set(mapper.copy_members(members, depth + 1, work)?)
            .map_err(|_| invalid("issued member sealing"))?;
        let current = Rc::new(SelectedSourceBoundary {
            source: boundary.source.clone(),
            request: boundary.request,
            producer: boundary.producer.clone(),
            parent,
            returned,
        });
        mapper.boundary_pair(boundary, &current, work)?;
        Ok(current)
    }
    /// Project only this authenticated owner basis; source hops are separate.
    #[allow(dead_code)] // Existing capture tests and pending immutable consumer.
    pub(crate) fn project_for<'a>(
        &'a self,
        expected_owner: &CanonicalOwnerFrame,
        whole: TimeSpan,
        issuer_start: Ratio64,
        work: &super::song_observation::SharedIndexWork,
    ) -> Result<Option<CanonicalClockFootprint<'a>>, Failure> {
        self.project_charged(
            expected_owner,
            whole,
            issuer_start,
            ClockOrientation::At,
            &mut |cost| work.borrow_mut().charge(cost),
        )
    }
}
impl super::QState<'_, '_> {
    // Computational native queries still share and debit the original ledger.
    // Only queries without that ledger bypass instrumentation altogether.
    pub(crate) fn has_clock_observation(&self) -> bool {
        self.observation.is_some()
    }
    fn charge_clock(&self, cost: u64) -> Result<(), Failure> {
        if let Some(work) = &self.observation {
            work.borrow_mut().charge(cost)?;
        }
        Ok(())
    }
    pub(crate) fn with_clock_owner<T>(
        &mut self,
        owner: &CanonicalOwnerFrame,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let source = match &self.song_clock {
            CanonicalClockProjection::Source(boundary) => Some(boundary.clone()),
            _ => None,
        };
        self.charge_clock(owner.placement.0.len() as u64 + 1 + u64::from(source.is_some()))?;
        let saved = std::mem::replace(
            &mut self.song_clock,
            CanonicalClockProjection::Known(Rc::new(CanonicalClockContext {
                owner: owner.clone(),
                frames: Vec::new(),
                sources: source.into_iter().collect(),
            })),
        );
        let result = f(self);
        self.song_clock = saved;
        result
    }
    pub(crate) fn with_clock_frame<T>(
        &mut self,
        frame: impl FnOnce() -> Result<CanonicalClockFrame, Failure>,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if self.observation.is_none() {
            return f(self);
        }
        if !matches!(self.song_clock, CanonicalClockProjection::Known(_)) {
            self.charge_clock(1)?;
            return f(self);
        }
        // Finish allocation/copy temporaries before recursively querying the child.
        let saved = self.install_clock_frame(frame)?;
        let result = f(self);
        self.song_clock = saved;
        result
    }
    #[inline(never)]
    fn install_clock_frame(
        &mut self,
        frame: impl FnOnce() -> Result<CanonicalClockFrame, Failure>,
    ) -> Result<CanonicalClockProjection, Failure> {
        let source_count = match &self.song_clock {
            CanonicalClockProjection::Known(context) => context.sources.len() as u64,
            _ => 0,
        };
        self.charge_clock(self.song_clock.frame_work() + source_count + 1)?;
        let frame = frame()?;
        let CanonicalClockProjection::Known(context) = &self.song_clock else {
            return Err(invalid("clock basis changed before child query"));
        };
        if context.frames.len()
            >= self
                .song_limits()
                .map_or(0, |limits| limits.max_depth as usize)
        {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "clock frame depth exhausted",
            ));
        }
        let capacity = context
            .frames
            .len()
            .checked_add(1)
            .ok_or_else(|| invalid("clock frame storage overflow"))?;
        // Allocate final capacity once: no uncharged second Vec growth after
        // cloning the original immutable frame sequence.
        let mut frames = Vec::with_capacity(capacity);
        frames.extend_from_slice(&context.frames);
        frames.push(frame);
        let child = CanonicalClockProjection::Known(Rc::new(CanonicalClockContext {
            owner: context.owner.clone(),
            frames,
            sources: context.sources.clone(),
        }));
        Ok(std::mem::replace(&mut self.song_clock, child))
    }
    /// Activate only at a resolved structural child's real sampling edge.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_structural_sample<T>(
        &mut self,
        child: &crate::pattern::pat::Pat,
        at: Ratio64,
        ordinal: u32,
        whole: Option<TimeSpan>,
        part: TimeSpan,
        timing: Option<&Event>,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if !self.has_clock_observation() {
            return f(self);
        }
        let length = self.producer_len().unwrap_or(0);
        let timing_length = timing
            .and_then(|event| event.producer.as_ref())
            .map_or(0, |trace| trace.steps.len());
        self.charge_clock((length + timing_length) as u64 * 2 + 5)?;
        let mut producer = self.producer();
        if let Some(trace) = &mut producer {
            trace.push(crate::pattern::occ::ProducerKind::Child, ordinal);
        }
        let relation = Rc::new(CanonicalSamplingRelation {
            whole,
            part,
            point: at,
            producer: timing.and_then(|event| event.producer.clone()),
        });
        let permit = SamplingPermit {
            child: std::ptr::from_ref(child),
            point: at,
            producer,
            relation,
            parent: self.song_clock.clone(),
        };
        let saved = std::mem::replace(
            &mut self.song_clock,
            CanonicalClockProjection::Permit(Rc::new(permit)),
        );
        let result = f(self);
        self.song_clock = saved;
        result
    }
    /// Consume a matching permit before recursion; nested/unrelated samples
    /// cannot reuse or consume the pending caller authority.
    pub(crate) fn with_clock_sampling<T>(
        &mut self,
        pattern: &crate::pattern::pat::Pat,
        point: Ratio64,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let permit = match &self.song_clock {
            CanonicalClockProjection::Permit(permit) => permit.clone(),
            _ => return self.with_clock_unknown(f),
        };
        self.charge_clock(self.producer_len().unwrap_or(0) as u64 * 2 + 1)?;
        if !std::ptr::eq(permit.child, pattern)
            || permit.point != point
            || permit.producer != self.producer()
        {
            return self.with_clock_unknown(f);
        }
        let saved = std::mem::replace(&mut self.song_clock, permit.parent.clone());
        let result = self.with_clock_frame(
            || {
                Ok(CanonicalClockFrame {
                    boundary: ClockBoundary::Sample(permit.relation.clone()),
                    child_piece: TimeSpan::point(point),
                    parent_piece: permit.relation.part,
                })
            },
            f,
        );
        self.song_clock = saved;
        result
    }
    pub(crate) fn with_source_boundary<T>(
        &mut self,
        source: &crate::song::source::SongSource,
        request: TimeSpan,
        producer: &ProducerTrace,
        f: impl FnOnce(&mut Self, Option<&Rc<SelectedSourceBoundary>>) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if !self.has_clock_observation() {
            return f(self, None);
        }
        self.charge_clock(
            producer.steps.len() as u64 * 2 + source.selector().family().len() as u64 + 2,
        )?;
        let boundary = Rc::new(SelectedSourceBoundary {
            source: source.clone(),
            request,
            producer: producer.clone(),
            parent: self.song_clock.clone(),
            returned: OnceCell::new(),
        });
        let saved = std::mem::replace(
            &mut self.song_clock,
            CanonicalClockProjection::Source(boundary.clone()),
        );
        let result = f(self, Some(&boundary));
        self.song_clock = saved;
        result
    }
    pub(crate) fn complete_source_boundary(
        &mut self,
        boundary: &Rc<SelectedSourceBoundary>,
        events: &[Event],
    ) -> Result<(), Failure> {
        self.charge_clock(events.len() as u64 + 1)?;
        let mut returned = Vec::with_capacity(events.len());
        for event in events {
            returned.push(
                event
                    .song_source
                    .clone()
                    .ok_or_else(|| invalid("returned source authority missing"))?,
            );
        }
        boundary
            .returned
            .set(returned)
            .map_err(|_| invalid("selected source boundary completed twice"))
    }
    pub(crate) fn with_clock_unknown<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.charge_clock(1)?;
        let saved = std::mem::replace(&mut self.song_clock, CanonicalClockProjection::Unknown);
        let result = f(self);
        self.song_clock = saved;
        result
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod structural_tests;

#[cfg(test)]
impl CanonicalClockProjection {
    pub(crate) fn same_frame_basis(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Known(a), Self::Known(b)) => a.owner == b.owner && a.frames == b.frames,
            _ => false,
        }
    }
    pub(crate) fn sampling_evidence(&self) -> Vec<(Option<TimeSpan>, TimeSpan, Ratio64)> {
        match self {
            Self::Known(context) => context
                .frames
                .iter()
                .filter_map(|frame| match &frame.boundary {
                    ClockBoundary::Sample(relation) => {
                        Some((relation.whole, relation.part, relation.point))
                    }
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
    pub(crate) fn source_evidence(&self) -> Vec<SourceEvidence> {
        match self {
            Self::Known(context) => context
                .sources
                .iter()
                .map(|boundary| {
                    (
                        boundary.request,
                        boundary.producer.clone(),
                        boundary.parent.clone(),
                        boundary
                            .returned
                            .get()
                            .expect("genuine source query completed")
                            .clone(),
                        boundary.clone(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}
