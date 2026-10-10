//! Immutable selected-source use recipes. Shared nodes retain independent edges.
mod certification;
use super::source::{mapped_edge_support, support_intersects};
mod conditional;
pub use conditional::FrozenStaticCondition;
pub mod layout;
pub(crate) mod origin;
pub(crate) mod sampling;
mod source_free;
pub mod timing;
use super::{EventHandle, PartRevision, PlacementPath};
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::query::TimeSpan;
use crate::value::ratio::Ratio64;
pub use origin::{
    resolve_source_use, resolve_source_use_frame, FrozenSliceTiming, FrozenSourceOrigin,
    FrozenSourceOriginFrame,
};
pub use sampling::FrozenStaticSampling;

/// Actual first-structure mode of the issued Slice recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenSliceStructure {
    Subject { issuer: crate::reader::span::NodeId },
    Index { issuer: crate::reader::span::NodeId },
}
/// A copied graph; incoming edges, rather than allocation identities, name uses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrozenSourceUseGraph {
    pub root: u32,
    pub nodes: Vec<FrozenSourceUseNode>,
}
/// A typed source traversal edge. Duplicate children remain duplicate edges.
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSourceUseEdge {
    pub trace: Vec<FrozenUseTraceTerm>,
    /// Ordered normalized slot transforms; silent slots remain in their weights.
    pub layout: Vec<layout::FrozenUseSlotLayout>,
    pub child: u32,
}
/// A symbolic edge term; repeated ordinals remain checked ranges, not expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenUseTraceTerm {
    Exact(ProducerStep),
    Copies { kind: ProducerKind, count: u32 },
}
/// Original operation plus its configuration-support mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSourceUseNode {
    pub operation: FrozenUseOperation,
    pub edges: Vec<FrozenSourceUseEdge>,
    pub mapping: FrozenUseMapping,
}
/// Every pattern operation has an explicit semantic category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenUseOperation {
    Steps,
    Pure,
    Sound,
    Signal,
    Source,
    Fast,
    Slow,
    Hurry,
    Rev,
    Every,
    WhenMod,
    SometimesBy,
    DegradeBy,
    Maybe,
    Choose,
    Hold,
    Repeat,
    Stack,
    Cat,
    FastCat,
    Superimpose,
    Off,
    Jux,
    Iter,
    Chop,
    Ply,
    Striate,
    Slice,
    Splice,
    LoopAt,
    Fit,
    Chunk,
    Grid,
    Euclid,
    Control,
    ScaleNotes,
    Chord,
    Voicing,
    Arp,
    Segment,
    Range,
    MidiNotes,
    Tune,
    Strum,
    Harp,
    Inversion,
}
/// Checked timing/structural recipe, without an evaluated event stream.
#[derive(Clone, Debug, PartialEq)]
pub enum FrozenUseMapping {
    Empty,
    Source {
        policy: u32,
    },
    Preserve,
    SelectContent {
        content: u32,
    },
    Parallel,
    CycleSelect,
    CycleConcat,
    SampleCycles {
        content: u32,
    },
    SampleGrid {
        content: u32,
        sampling: FrozenStaticSampling,
    },
    Rate {
        factor: Ratio64,
    },
    ReflectCycles,
    Iterate {
        count: u32,
    },
    Shift {
        amount: Ratio64,
    },
    Subdivide {
        count: u32,
    },
    Restructure {
        timing: u32,
        content: u32,
    },
    Slices {
        starts: Vec<Ratio64>,
        splice: bool,
        structure: FrozenSliceStructure,
    },
    Conditional,
    ConditionalStatic(FrozenStaticCondition),
    Uncertifiable(FrozenUseReason),
}
/// A specific reason why a finite resource cover has not been certified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenUseReason {
    DynamicRate,
    DynamicCount,
    DynamicSource,
    DynamicTiming,
    UncertifiedCallback,
    UnclosedResource,
    UnsupportedLiveInput,
}
/// An operation-specific diagnostic retains its addressed graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenUseDiagnostic {
    pub node: u32,
    pub operation: FrozenUseOperation,
    pub reason: FrozenUseReason,
}
/// Checked support recipe and scalar reservation; constructors stay private.
#[derive(Clone, Debug)]
pub struct FrozenSourceUseCover {
    graph: FrozenSourceUseGraph,
    window: TimeSpan,
    configuration_bound: u64,
    consumed_work: u32,
    policies: Vec<(
        PartRevision,
        crate::value::intern::KwId,
        Vec<super::snapshot::FrozenSound>,
    )>,
}
impl FrozenSourceUseCover {
    /// Actual admitted certification work, including final graph/family copying.
    /// Clones retain this value; callers charge storage and matcher work separately.
    #[must_use]
    pub const fn consumed_work(&self) -> u32 {
        self.consumed_work
    }

    /// The immutable mapping recipe; it does not contain a realized score.
    #[must_use]
    pub fn graph(&self) -> &FrozenSourceUseGraph {
        &self.graph
    }
    /// Addressed finite support window.
    #[must_use]
    pub const fn window(&self) -> TimeSpan {
        self.window
    }
    /// Conservative checked count of placement/configuration components.
    #[must_use]
    pub const fn configuration_bound(&self) -> u64 {
        self.configuration_bound
    }
}
/// A use/placement identity. Notes and chord tone ordinals are excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenSourceUseIdentity {
    pub edges: Vec<u32>,
    pub copies: Vec<u32>,
    pub policy: u32,
    pub placement: PlacementPath,
    pub revision: PartRevision,
}

pub(crate) use super::source::copy_origin;

/// A selected immutable source policy root, including its full pre-pick family.
#[derive(Clone, Debug)]
pub struct FrozenSelectedSource {
    pub root_part: usize,
    pub track: crate::value::intern::KwId,
    pub family: Vec<super::snapshot::FrozenSound>,
}
#[derive(Clone, Debug)]
pub struct FrozenPattern {
    pub id: crate::reader::span::NodeId,
    pub families: Vec<(
        super::snapshot::FrozenSound,
        super::snapshot::FrozenAudioRoute,
    )>,
    pub named_buses: Vec<crate::value::intern::KwId>,
    pub sources: Vec<FrozenSelectedSource>,
    pub source_uses: FrozenSourceUseGraph,
    pub(crate) index_timing: Option<std::rc::Rc<timing::FrozenIndexTiming>>,
}

impl FrozenPattern {
    /// Actual copied operands, not a finite geometry or host-readiness certificate.
    #[must_use]
    pub fn index_timing(&self) -> Option<&timing::FrozenIndexTiming> {
        self.index_timing.as_deref()
    }
}

#[cfg(test)]
pub(crate) use certification::certify_extraction_reference;
/// Certifies a finite conservative reservation without expanding any placement.
/// Unknown dynamic mappings are errors rather than successful guessed bounds.
pub use certification::certify_source_uses;
pub(crate) use certification::certify_source_uses_metered;

type CoverCacheKey = (
    usize,
    u32,
    Option<crate::value::intern::KwId>,
    Ratio64,
    Ratio64,
    u32,
);
struct Certification<'a> {
    inventory: &'a super::snapshot::FrozenRoutingInventory,
    limits: super::SongLimits,
    remaining: &'a mut u32,
    visiting: std::collections::BTreeSet<(usize, u32)>,
    cache: std::collections::BTreeMap<CoverCacheKey, u64>,
    source_free_cache: std::collections::BTreeMap<(usize, u32, bool), u32>,
}
fn supported_window(
    window: TimeSpan,
    duration: Ratio64,
) -> Result<Option<TimeSpan>, crate::vm::fail::Failure> {
    if window.is_point() {
        return Ok((window.begin >= Ratio64::ZERO && window.begin < duration).then_some(window));
    }
    let begin = window.begin.max(Ratio64::ZERO);
    let end = window.end.min(duration);
    if begin >= end {
        Ok(None)
    } else {
        Ok(Some(TimeSpan::new(begin, end)?))
    }
}
fn use_invalid(message: &str) -> crate::vm::fail::Failure {
    crate::vm::fail::Failure::new(crate::vm::fail::FailCode::Type, message)
}
pub(crate) fn use_overflow() -> crate::vm::fail::Failure {
    crate::vm::fail::Failure::new(
        crate::vm::fail::FailCode::Overflow,
        "source-use bound overflow",
    )
}
fn cycles(window: TimeSpan) -> Result<u64, crate::vm::fail::Failure> {
    let count = window
        .end
        .floor()
        .checked_sub(window.begin.floor())
        .and_then(|n| n.checked_add(1))
        .ok_or_else(use_overflow)?;
    u64::try_from(count).map_err(|_| use_overflow())
}

#[derive(Clone, Copy)]
pub(crate) struct OriginView<'a> {
    pub(super) handle: &'a EventHandle,
    pub(super) issued_handle: &'a EventHandle,
    pub(super) original_instrument: &'a super::snapshot::FrozenSound,
    pub(super) entry_trace: &'a [ProducerStep],
    pub(super) source_part: TimeSpan,
    pub(super) source_whole: Option<TimeSpan>,
    pub(super) slice_timings: &'a [FrozenSliceTiming],
}
pub(super) fn resolve_origin(
    cover: &FrozenSourceUseCover,
    origin: OriginView<'_>,
    limits: super::SongLimits,
) -> Result<FrozenSourceUseIdentity, crate::vm::fail::Failure> {
    limits.validate()?;
    let mut remaining = limits.max_nodes;
    origin::validate_source_whole(
        origin.handle,
        origin.issued_handle,
        origin.source_part,
        origin.source_whole,
        &mut remaining,
    )?;
    super::source::slices::validate_frozen_slice_timings(
        origin.slice_timings,
        origin.issued_handle,
        &mut remaining,
        limits.max_depth,
    )?;
    let mut search = UseSearch {
        cover,
        origin,
        remaining,
        max_depth: limits.max_depth,
        visiting: std::collections::BTreeSet::new(),
        edges: Vec::new(),
        copies: Vec::new(),
        found: None,
    };
    search.walk(cover.graph.root, 0, 0, cover.window)?;
    search
        .found
        .ok_or_else(|| use_invalid("source origin does not match admitted use"))
}
struct UseSearch<'a> {
    cover: &'a FrozenSourceUseCover,
    origin: OriginView<'a>,
    remaining: u32,
    max_depth: u32,
    visiting: std::collections::BTreeSet<u32>,
    edges: Vec<u32>,
    copies: Vec<u32>,
    found: Option<FrozenSourceUseIdentity>,
}
impl UseSearch<'_> {
    fn admit(&mut self, n: usize) -> Result<(), crate::vm::fail::Failure> {
        let n = u32::try_from(n).map_err(|_| use_overflow())?;
        self.remaining = self.remaining.checked_sub(n).ok_or_else(|| {
            crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::FuelExhausted,
                "source-use resolution work exhausted",
            )
        })?;
        Ok(())
    }
    fn walk(
        &mut self,
        index: u32,
        offset: usize,
        depth: u32,
        window: TimeSpan,
    ) -> Result<(), crate::vm::fail::Failure> {
        if depth >= self.max_depth {
            return Err(crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::DepthExceeded,
                "source-use resolution depth exhausted",
            ));
        }
        self.admit(1)?;
        if !self.visiting.insert(index) {
            return Err(use_invalid("cyclic source-use resolution"));
        }
        let result = self.inner(index, offset, depth, window);
        self.visiting.remove(&index);
        result
    }
    #[inline(never)]
    fn inner(
        &mut self,
        index: u32,
        offset: usize,
        depth: u32,
        window: TimeSpan,
    ) -> Result<(), crate::vm::fail::Failure> {
        let node = self
            .cover
            .graph
            .nodes
            .get(index as usize)
            .ok_or_else(|| use_invalid("source-use resolution index out of range"))?;
        if let FrozenUseMapping::Source { policy } = node.mapping {
            if offset != self.origin.entry_trace.len() {
                return Ok(());
            }
            let (revision, track, family) = self
                .cover
                .policies
                .get(policy as usize)
                .ok_or_else(|| use_invalid("source-use resolution policy out of range"))?;
            self.admit(family.len())?;
            if *revision != self.origin.handle.revision()
                || *track != self.origin.handle.track()
                || !family.contains(self.origin.original_instrument)
            {
                return Ok(());
            }
            if !support_intersects(window, self.origin.source_part) {
                return Ok(());
            }
            if self.found.is_some() {
                return Err(use_invalid("ambiguous source-use origin"));
            }
            self.admit(
                self.edges
                    .len()
                    .checked_add(self.copies.len())
                    .and_then(|n| n.checked_add(self.origin.handle.placement().0.len()))
                    .ok_or_else(use_overflow)?,
            )?;
            self.found = Some(FrozenSourceUseIdentity {
                edges: self.edges.clone(),
                copies: self.copies.clone(),
                policy,
                placement: self.origin.handle.placement().clone(),
                revision: *revision,
            });
            return Ok(());
        }
        for (ordinal, edge) in node.edges.iter().enumerate() {
            if matches!(node.mapping, FrozenUseMapping::Slices { .. }) && ordinal != 0 {
                continue;
            }
            if let FrozenUseMapping::SelectContent { content }
            | FrozenUseMapping::Restructure { content, .. }
            | FrozenUseMapping::SampleGrid { content, .. }
            | FrozenUseMapping::SampleCycles { content } = node.mapping
            {
                if ordinal != content as usize {
                    continue;
                }
            }
            self.admit(edge.trace.len().checked_add(1).ok_or_else(use_overflow)?)?;
            let end = offset
                .checked_add(edge.trace.len())
                .ok_or_else(use_overflow)?;
            let Some(actual) = self.origin.entry_trace.get(offset..end) else {
                continue;
            };
            let copies_before = self.copies.len();
            let mut matches = true;
            for (term, step) in edge.trace.iter().zip(actual) {
                match term {
                    FrozenUseTraceTerm::Exact(expected) => {
                        if expected != step {
                            matches = false;
                            break;
                        }
                    }
                    FrozenUseTraceTerm::Copies { kind, count } => {
                        if *kind != step.kind || step.ordinal >= *count {
                            matches = false;
                            break;
                        }
                        self.copies.push(step.ordinal);
                    }
                }
            }
            if matches {
                if let FrozenUseMapping::SampleGrid { sampling, .. } = node.mapping {
                    let direct_source = self
                        .cover
                        .graph
                        .nodes
                        .get(edge.child as usize)
                        .is_some_and(|child| {
                            matches!(child.mapping, FrozenUseMapping::Source { .. })
                        });
                    if direct_source
                        && (!self.origin.source_part.is_point()
                            || !sampling::contains_sample(
                                sampling,
                                window,
                                self.origin.source_part.begin,
                                &mut self.remaining,
                            )?)
                    {
                        self.copies.truncate(copies_before);
                        continue;
                    }
                }
                self.admit(edge.layout.len())?;
                let Some(edge_window) =
                    mapped_edge_support(node, edge, window, Some(actual), &mut self.remaining)?
                else {
                    self.copies.truncate(copies_before);
                    continue;
                };
                self.edges
                    .push(u32::try_from(ordinal).map_err(|_| use_overflow())?);
                self.walk(edge.child, end, depth + 1, edge_window)?;
                self.edges.pop();
            }
            self.copies.truncate(copies_before);
        }
        Ok(())
    }
}
