//! Immutable issuing authority and bounded provenance validation.
use super::{
    resolve_origin, use_invalid, use_overflow, FrozenSourceUseCover, FrozenSourceUseIdentity,
    OriginView,
};
use crate::pattern::occ::ProducerStep;
use crate::pattern::TimeSpan;
use crate::reader::span::NodeId;
use crate::song::{EventHandle, SongLimits};
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};
/// Query-issued index timing, in the issuing Slice's local clock.
/// ```compile_fail
/// use vactr::song::source_uses::FrozenSliceTiming;
/// fn forge(timing: &mut FrozenSliceTiming) { timing.sample_start = vactr::value::Ratio64::ZERO; }
/// ```
/// ```compile_fail
/// use vactr::song::source_uses::FrozenSliceTiming;
/// fn forge(timing: FrozenSliceTiming) { let FrozenSliceTiming { issuer, .. } = timing; }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSliceTiming {
    issuer: NodeId,
    issuer_trace: Vec<ProducerStep>,
    index_trace: Vec<ProducerStep>,
    index_whole: TimeSpan,
    sample_start: Ratio64,
    subject_handle: EventHandle,
    admission_depth: u32,
}
impl FrozenSliceTiming {
    pub(in crate::song) fn from_issued(
        issuer: NodeId,
        issuer_trace: Vec<ProducerStep>,
        index_trace: Vec<ProducerStep>,
        index_whole: TimeSpan,
        sample_start: Ratio64,
        subject_handle: EventHandle,
        admission_depth: u32,
    ) -> Self {
        Self {
            issuer,
            issuer_trace,
            index_trace,
            index_whole,
            sample_start,
            subject_handle,
            admission_depth,
        }
    }
    pub(in crate::song) const fn admission_depth(&self) -> u32 {
        self.admission_depth
    }
    #[must_use]
    pub const fn issuer(&self) -> NodeId {
        self.issuer
    }
    #[must_use]
    pub fn issuer_trace(&self) -> &[ProducerStep] {
        &self.issuer_trace
    }
    #[must_use]
    pub fn index_trace(&self) -> &[ProducerStep] {
        &self.index_trace
    }
    #[must_use]
    pub const fn index_whole(&self) -> TimeSpan {
        self.index_whole
    }
    #[must_use]
    pub const fn sample_start(&self) -> Ratio64 {
        self.sample_start
    }
    #[must_use]
    pub fn subject_handle(&self) -> &EventHandle {
        &self.subject_handle
    }
}
/// Original selected-source provenance, separate from the transformed handle.
///
/// ```compile_fail
/// use vactr::song::source_uses::FrozenSourceOrigin;
/// fn forge(origin: &mut FrozenSourceOrigin) { origin.source_whole = None; }
/// ```
/// ```compile_fail
/// use vactr::song::source_uses::FrozenSourceOriginFrame;
/// fn forge(frame: &mut FrozenSourceOriginFrame) { frame.issued_handle = frame.handle.clone(); }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSourceOrigin {
    pub source_part: TimeSpan,
    pub original_instrument: super::super::snapshot::FrozenSound,
    pub handle: EventHandle,
    pub entry_trace: Vec<ProducerStep>,
    pub(crate) issued_handle: EventHandle,
    pub(crate) source_whole: Option<TimeSpan>,
    pub(in crate::song) slice_timings: Vec<FrozenSliceTiming>,
    pub inherited: Vec<FrozenSourceOriginFrame>,
}
/// One inner selected-source origin, in outer-to-inner order.
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSourceOriginFrame {
    pub source_part: TimeSpan,
    pub original_instrument: super::super::snapshot::FrozenSound,
    pub handle: EventHandle,
    pub entry_trace: Vec<ProducerStep>,
    pub(crate) issued_handle: EventHandle,
    pub(crate) source_whole: Option<TimeSpan>,
    pub(in crate::song) slice_timings: Vec<FrozenSliceTiming>,
}
impl FrozenSourceOrigin {
    /// Ordered immutable timing records, without changing original source whole.
    #[must_use]
    pub fn slice_timings(&self) -> &[FrozenSliceTiming] {
        &self.slice_timings
    }
    /// Authentic source-local whole before later timing transformations.
    #[must_use]
    pub const fn source_whole(&self) -> Option<TimeSpan> {
        self.source_whole
    }
    pub(crate) fn borrowed_view(&self) -> OriginView<'_> {
        OriginView {
            handle: &self.handle,
            issued_handle: &self.issued_handle,
            original_instrument: &self.original_instrument,
            entry_trace: &self.entry_trace,
            source_part: self.source_part,
            source_whole: self.source_whole,
            slice_timings: &self.slice_timings,
        }
    }
}
impl FrozenSourceOriginFrame {
    /// Ordered immutable timing records, without changing original source whole.
    #[must_use]
    pub fn slice_timings(&self) -> &[FrozenSliceTiming] {
        &self.slice_timings
    }
    /// Authentic source-local whole before later timing transformations.
    #[must_use]
    pub const fn source_whole(&self) -> Option<TimeSpan> {
        self.source_whole
    }
    pub(crate) fn borrowed_view(&self) -> OriginView<'_> {
        OriginView {
            handle: &self.handle,
            issued_handle: &self.issued_handle,
            original_instrument: &self.original_instrument,
            entry_trace: &self.entry_trace,
            source_part: self.source_part,
            source_whole: self.source_whole,
            slice_timings: &self.slice_timings,
        }
    }
}
/// Matches typed source-entry terms with full issued authority.
pub fn resolve_source_use(
    cover: &FrozenSourceUseCover,
    origin: &FrozenSourceOrigin,
    limits: SongLimits,
) -> Result<FrozenSourceUseIdentity, Failure> {
    resolve_origin(cover, origin.borrowed_view(), limits)
}
/// Resolves an inherited source-local origin without copying it.
pub fn resolve_source_use_frame(
    cover: &FrozenSourceUseCover,
    origin: &FrozenSourceOriginFrame,
    limits: SongLimits,
) -> Result<FrozenSourceUseIdentity, Failure> {
    resolve_origin(cover, origin.borrowed_view(), limits)
}
pub(crate) const SOURCE_AUTHORITY_INSPECTION_WORK: u32 = 16;
impl OriginView<'_> {
    pub(crate) fn handle(&self) -> &EventHandle {
        self.handle
    }
    pub(crate) fn authority_work(&self) -> Result<u32, Failure> {
        source_authority_validation_work(self.handle, self.issued_handle, self.source_whole)
    }
}
pub(crate) fn source_whole_work(
    handle: &EventHandle,
    whole: Option<TimeSpan>,
) -> Result<u32, Failure> {
    let count = handle
        .placement()
        .0
        .len()
        .checked_add(handle.occurrence().producer_ordinals.len())
        .and_then(|n| n.checked_add(8 + if whole.is_some() { 5 } else { 1 }))
        .ok_or_else(use_overflow)?;
    u32::try_from(count).map_err(|_| use_overflow())
}
pub(crate) fn source_authority_validation_work(
    handle: &EventHandle,
    issued: &EventHandle,
    whole: Option<TimeSpan>,
) -> Result<u32, Failure> {
    let count = handle
        .placement()
        .0
        .len()
        .checked_add(handle.occurrence().producer_ordinals.len())
        .and_then(|n| n.checked_add(issued.placement().0.len()))
        .and_then(|n| n.checked_add(issued.occurrence().producer_ordinals.len()))
        .and_then(|n| n.checked_add(24 + if whole.is_some() { 12 } else { 0 }))
        .ok_or_else(use_overflow)?;
    u32::try_from(count).map_err(|_| use_overflow())
}
pub(crate) fn validate_source_whole(
    handle: &EventHandle,
    issued: &EventHandle,
    part: TimeSpan,
    whole: Option<TimeSpan>,
    remaining: &mut u32,
) -> Result<(), Failure> {
    let cost = source_authority_validation_work(handle, issued, whole)?;
    *remaining = remaining.checked_sub(cost).ok_or_else(|| {
        Failure::new(
            FailCode::FuelExhausted,
            "source authority validation work exhausted",
        )
    })?;
    if handle != issued {
        return Err(use_invalid(
            "source handle differs from issued whole authority",
        ));
    }
    if part.end < part.begin {
        return Err(use_invalid("source part is reversed"));
    }
    if let Some(whole) = whole {
        if whole.end < whole.begin
            || whole.begin != issued.occurrence().onset
            || !crate::song::source::support_intersects(whole, part)
        {
            return Err(use_invalid("invalid issued source whole support"));
        }
    }
    Ok(())
}

/// Immutable frozen descriptor plus genuine live source leaves, kept outside public equality.
#[cfg_attr(not(test), allow(dead_code))] // The next authenticated routing consumer reads this private sidecar.
pub(crate) struct FrozenIssuedSourceContribution {
    leaves: std::rc::Rc<crate::pattern::eval::song_provenance::IssuedSourceLeaves>,
    augmented_origin: FrozenSourceOrigin,
    members: Vec<FrozenIssuedMemberCopy>,
}
#[cfg_attr(not(test), allow(dead_code))] // The next authenticated routing consumer checks actual member correspondence.
pub(crate) struct FrozenIssuedMemberCopy {
    member_slot: usize,
    origin: FrozenSourceOrigin,
}
impl FrozenIssuedMemberCopy {
    pub(in crate::song) fn copied(member_slot: usize, origin: FrozenSourceOrigin) -> Self {
        Self {
            member_slot,
            origin,
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn member_slot(&self) -> usize {
        self.member_slot
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn origin(&self) -> &FrozenSourceOrigin {
        &self.origin
    }
}
impl FrozenIssuedSourceContribution {
    pub(in crate::song) fn copied(
        original: &crate::song::query::issued::IssuedSourceContribution,
        augmented_origin: FrozenSourceOrigin,
        members: Vec<FrozenIssuedMemberCopy>,
    ) -> Self {
        Self {
            leaves: original.leaves.clone(),
            augmented_origin,
            members,
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn leaves(
        &self,
    ) -> &std::rc::Rc<crate::pattern::eval::song_provenance::IssuedSourceLeaves> {
        &self.leaves
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn augmented_origin(&self) -> &FrozenSourceOrigin {
        &self.augmented_origin
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn members(&self) -> &[FrozenIssuedMemberCopy] {
        &self.members
    }
}
impl FrozenSourceOrigin {
    fn preserves_timing_prefix(augmented: &[FrozenSliceTiming], raw: &[FrozenSliceTiming]) -> bool {
        augmented.len() >= raw.len()
            && augmented.iter().zip(raw).all(|(a, b)| {
                a.issuer == b.issuer && a.issuer_trace == b.issuer_trace
                && a.index_trace == b.index_trace && a.index_whole == b.index_whole
                && a.sample_start == b.sample_start && a.subject_handle == b.subject_handle
                // Both complete origin copies independently admitted their actual depths.
                // Adding original timing records can only increase the derived whole-chain certificate.
                && a.admission_depth >= b.admission_depth
            })
    }
    pub(in crate::song) fn admit_raw_comparison(
        &self,
        raw: &Self,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        fn debit(remaining: &mut u32, count: usize) -> Result<(), Failure> {
            let count = u32::try_from(count).map_err(|_| use_overflow())?;
            *remaining = remaining.checked_sub(count).ok_or_else(|| {
                Failure::new(FailCode::FuelExhausted, "frozen origin comparison work")
            })?;
            Ok(())
        }
        fn handle(handle: &EventHandle, remaining: &mut u32) -> Result<(), Failure> {
            debit(remaining, 8)?;
            debit(remaining, handle.placement().0.len())?;
            debit(remaining, handle.occurrence().producer_ordinals.len())
        }
        fn fields(view: OriginView<'_>, remaining: &mut u32) -> Result<(), Failure> {
            debit(remaining, 8)?;
            handle(view.handle, remaining)?;
            handle(view.issued_handle, remaining)?;
            debit(remaining, view.entry_trace.len())?;
            if let crate::song::snapshot::FrozenSound::Sample { path, .. } =
                view.original_instrument
            {
                debit(remaining, path.len())?;
            }
            debit(remaining, view.slice_timings.len())?;
            for timing in view.slice_timings {
                debit(remaining, 8)?;
                debit(remaining, timing.issuer_trace.len())?;
                debit(remaining, timing.index_trace.len())?;
                handle(&timing.subject_handle, remaining)?;
            }
            Ok(())
        }
        for origin in [self, raw] {
            fields(origin.borrowed_view(), remaining)?;
            debit(remaining, origin.inherited.len())?;
            for frame in &origin.inherited {
                fields(frame.borrowed_view(), remaining)?;
            }
        }
        Ok(())
    }
    pub(in crate::song) fn preserves_raw_member(&self, raw: &Self) -> bool {
        self.handle == raw.handle
            && self.issued_handle == raw.issued_handle
            && self.source_part == raw.source_part
            && self.source_whole == raw.source_whole
            && self.original_instrument == raw.original_instrument
            && self.entry_trace == raw.entry_trace
            && self.inherited.len() == raw.inherited.len()
            && self.inherited.iter().zip(&raw.inherited).all(|(a, b)| {
                a.handle == b.handle
                    && a.issued_handle == b.issued_handle
                    && a.source_part == b.source_part
                    && a.source_whole == b.source_whole
                    && a.original_instrument == b.original_instrument
                    && a.entry_trace == b.entry_trace
                    && Self::preserves_timing_prefix(&a.slice_timings, &b.slice_timings)
            })
            && Self::preserves_timing_prefix(&self.slice_timings, &raw.slice_timings)
    }
}
