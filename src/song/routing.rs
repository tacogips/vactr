//! Immutable route admission and POD song transport. No callback allocation.
use super::SnapshotEpoch;
use crate::host::wire::AudioEvent;

/// Actual remaining allocations after current and retiring reservations.
/// Frame arenas count f32 state samples, not seconds or nominal voices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SongHostCapacities {
    pub sample_rate: u32,
    pub cell_slots: u32,
    pub voice_slots: u32,
    pub template_slots: u32,
    pub bus_slots: u32,
    pub sample_resources: u32,
    pub pcm_bytes: u64,
    pub voice_frames: u64,
    pub bus_frames: u64,
    pub ack_slots: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongResourceRef {
    pub id: u32,
    pub generation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SongResourceKind {
    Instrument,
    PrivateFx,
    Track,
    Master,
    Sample,
    ControlCells = 7,
    AnalysisBank = 8,
}
/// Full ownership identity retained across uploads and delayed receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongLeaseKey {
    pub epoch: SnapshotEpoch,
    pub resource: SongResourceRef,
    pub kind: SongResourceKind,
}
/// Explicit owner-local control and analyzer bank association for a graph lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongGraphBanks {
    pub graph: SongLeaseKey,
    pub controls: Option<SongLeaseKey>,
    pub analysis: Option<SongLeaseKey>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongAnalysisCapacity {
    pub slots: u32,
}
/// Measured staging capacity report; publication and serial ownership belong to the stager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongCapacityReport {
    pub epoch: SnapshotEpoch,
    pub serial: u64,
    pub available: SongHostCapacities,
    pub analysis: SongAnalysisCapacity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongStagePreparation {
    pub preparation: SongPreparation,
    pub analysis_required: SongAnalysisCapacity,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SongCellInit {
    pub lease: SongLeaseKey,
    pub cell: crate::dsp::cells::CellId,
    pub value: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongAnalysisReservation {
    pub lease: SongLeaseKey,
    pub slots: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongResourceReservation {
    pub epoch: SnapshotEpoch,
    pub resource: SongResourceRef,
    pub kind: SongResourceKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongPreparation {
    pub epoch: SnapshotEpoch,
    pub branches: u32,
    pub resources: u32,
    pub required: SongHostCapacities,
}
/// References host-remapped leased resources; candidate IDs are never host IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongBranchConfig {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub generation: u32,
    pub family: u32,
    pub track: u32,
    pub instrument: SongResourceRef,
    pub private_fx: Option<SongResourceRef>,
    pub track_template: Option<SongResourceRef>,
    pub master: Option<SongResourceRef>,
    pub transition_frame: u64,
    pub tail_deadline: u64,
}
/// An admitted reusable physical branch and its checked logical generation ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongReusableBranch {
    pub config: SongBranchConfig,
    pub last_generation: u32,
}
impl SongReusableBranch {
    #[must_use]
    pub fn valid(self) -> bool {
        self.config.private_fx.is_some()
            && self.config.generation != 0
            && self.config.generation <= self.last_generation
            && self.config.transition_frame <= self.config.tail_deadline
    }
}
/// Timed atomic reuse of one admitted immutable private graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongBranchRebind {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub expected_generation: u32,
    pub generation: u32,
    pub transition_frame: u64,
    pub tail_deadline: u64,
}
impl SongBranchRebind {
    #[must_use]
    pub fn valid(self) -> bool {
        self.expected_generation.checked_add(1) == Some(self.generation)
            && self.expected_generation != 0
            && self.transition_frame <= self.tail_deadline
    }
}
/// Receipt of an actual audio-side generation transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongBranchRebound {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub generation: u32,
    pub frame: u64,
}
/// One private instrument/effect branch in a prepared song generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SongBranchId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongActivation {
    pub epoch: SnapshotEpoch,
    pub frame: u64,
}
/// Future atomic replacement intent; portable validation does not adopt resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongReplacement {
    pub activation: SongActivation,
    pub previous: SnapshotEpoch,
    pub overlay_nonce: u64,
    pub overlay_count: u32,
}
/// One staged initial family gate, authenticated by the future replacement arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongInitialMute {
    pub epoch: SnapshotEpoch,
    pub overlay_nonce: u64,
    pub instrument: u32,
    pub muted: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongMute {
    pub epoch: SnapshotEpoch,
    pub instrument: u32,
    pub muted: bool,
    pub frame: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SongRejectCode {
    StaleEpoch,
    NotReady,
    Capacity,
    Malformed,
    HostFault,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongHostAck {
    ActivationRejected {
        activation: SongActivation,
        reason: SongRejectCode,
    },
    BranchRebound(SongBranchRebound),
    ClockReport(SongClockReport),
    ClockRejected(SongClockFailure),
    CapacityReport(SongCapacityReport),
    LeaseReturned(SongLeaseKey),
    PreparationCancelled(SnapshotEpoch),
    SliceAccepted {
        lease: SongLeaseKey,
        offset: u32,
    },
    ResourceReady {
        epoch: SnapshotEpoch,
        resource: SongResourceRef,
    },
    ResourceRetired {
        epoch: SnapshotEpoch,
        resource: SongResourceRef,
    },
    Ready(SnapshotEpoch),
    Applied(SongActivation),
    Muted(SongMute),
    Rejected {
        epoch: SnapshotEpoch,
        reason: SongRejectCode,
    },
    CapacityRejected {
        epoch: SnapshotEpoch,
        reason: SongRejectCode,
    },
}
impl SongHostAck {
    #[must_use]
    pub const fn epoch(self) -> SnapshotEpoch {
        match self {
            Self::Ready(e)
            | Self::Rejected { epoch: e, .. }
            | Self::CapacityRejected { epoch: e, .. }
            | Self::ResourceReady { epoch: e, .. }
            | Self::ResourceRetired { epoch: e, .. } => e,
            Self::LeaseReturned(k) | Self::SliceAccepted { lease: k, .. } => k.epoch,
            Self::PreparationCancelled(e) => e,
            Self::CapacityReport(r) => r.epoch,
            Self::ClockReport(r) => r.request.epoch,
            Self::ClockRejected(r) => r.request.epoch,
            Self::Applied(a) => a.epoch,
            Self::ActivationRejected { activation, .. } => activation.epoch,
            Self::Muted(m) => m.epoch,
            Self::BranchRebound(r) => r.epoch,
        }
    }
}
/// Actual integer rendered timebase; never reconstructed from floating seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongHostClock {
    pub frame: u64,
    pub sample_rate: u32,
}
/// Caller-owned correlation identity for one read-only observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongClockRequest {
    pub epoch: SnapshotEpoch,
    pub request: u64,
}
/// Actual clock captured when the audio callback consumes the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongClockReport {
    pub request: SongClockRequest,
    pub clock: SongHostClock,
}
/// Clock-specific failure preserves correlation without consuming other receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongClockFailure {
    pub request: SongClockRequest,
    pub reason: SongRejectCode,
}
/// Explicit mono input: song realization has already expanded each chord tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SongAudioEvent {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub generation: u32,
    pub frame: u64,
    pub event: AudioEvent,
}
/// Absolute end-exclusive arrangement endpoint and explicit private tail deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongEndpoints {
    pub epoch: SnapshotEpoch,
    pub arrangement: u64,
    pub tail_deadline: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongBranchRelease {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub generation: u32,
    pub frame: u64,
    pub tail_deadline: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
// Inline event ownership avoids allocations on either transport or callback.
#[allow(clippy::large_enum_variant)]
pub enum SongCommand {
    Replace(SongReplacement),
    PrimeMute(SongInitialMute),
    RequestClock(SongClockRequest),
    RequestCapacity(SnapshotEpoch),
    BeginStaging(SongStagePreparation),
    InitCells(SongCellInit),
    ReserveAnalysis(SongAnalysisReservation),
    CancelPreparation(SnapshotEpoch),
    CancelLease(SongLeaseKey),
    BeginPreparation(SongPreparation),
    ReserveResource(SongResourceReservation),
    ConfigureBranch(SongBranchConfig),
    ConfigureReusableBranch(SongReusableBranch),
    RebindBranch(SongBranchRebind),
    SealPreparation(SnapshotEpoch),
    Prepare(SnapshotEpoch),
    Activate(SongActivation),
    Mute(SongMute),
    Endpoints(SongEndpoints),
    Release(SongBranchRelease),
    Event(SongAudioEvent),
    BindGraphBanks(SongGraphBanks),
}
impl SongCommand {
    #[must_use]
    pub const fn epoch(self) -> SnapshotEpoch {
        match self {
            Self::Prepare(e)
            | Self::SealPreparation(e)
            | Self::CancelPreparation(e)
            | Self::RequestCapacity(e) => e,
            Self::Replace(r) => r.activation.epoch,
            Self::PrimeMute(m) => m.epoch,
            Self::RequestClock(r) => r.epoch,
            Self::BeginStaging(s) => s.preparation.epoch,
            Self::InitCells(c) => c.lease.epoch,
            Self::ReserveAnalysis(a) => a.lease.epoch,
            Self::CancelLease(k) => k.epoch,
            Self::BindGraphBanks(b) => b.graph.epoch,
            Self::BeginPreparation(p) => p.epoch,
            Self::ReserveResource(r) => r.epoch,
            Self::ConfigureBranch(b) => b.epoch,
            Self::ConfigureReusableBranch(b) => b.config.epoch,
            Self::RebindBranch(r) => r.epoch,
            Self::Activate(a) => a.epoch,
            Self::Mute(m) => m.epoch,
            Self::Endpoints(e) => e.epoch,
            Self::Release(r) => r.epoch,
            Self::Event(e) => e.epoch,
        }
    }
}
///0x1A remains the existing sample-begin tag. New records append stable tags.
pub(crate) const SONG_COMMAND_TAG: u8 = 0x1B;
pub(crate) const SONG_ACK_TAG: u8 = 0x49;
pub const SONG_COMMAND_MAX_LEN: usize = 35 + AudioEvent::ENCODED_LEN;
pub const SONG_ACK_MAX_LEN: usize = 74;

/// Bounded receipt ledger. An expected epoch must be selected before accepting
/// acknowledgments; receipts never themselves activate transport.
#[derive(Default)]
pub struct SongReceipts {
    epoch: Option<SnapshotEpoch>,
    entries: std::collections::VecDeque<SongHostAck>,
}
impl SongReceipts {
    pub const CAPACITY: usize = 64;
    /// Changing ownership expectations never discards unprocessed acknowledgments.
    pub fn expect(&mut self, epoch: SnapshotEpoch) -> Result<(), crate::vm::fail::Failure> {
        if !self.entries.is_empty() {
            return Err(capacity_failure("unconsumed song receipts"));
        }
        self.epoch = Some(epoch);
        Ok(())
    }
    pub fn expected_epoch(&self) -> Option<SnapshotEpoch> {
        self.epoch
    }
    /// All ownership identities are retained, including stale retirement replies.
    /// Success classifies the expected epoch; full storage returns the exact POD.
    pub fn record(&mut self, ack: SongHostAck) -> Result<bool, SongHostAck> {
        if self.entries.len() == Self::CAPACITY {
            return Err(ack);
        }
        let expected = self.epoch == Some(ack.epoch());
        self.entries.push_back(ack);
        Ok(expected)
    }
    pub fn pop(&mut self) -> Option<SongHostAck> {
        self.entries.pop_front()
    }
    pub fn iter(&self) -> impl Iterator<Item = &SongHostAck> {
        self.entries.iter()
    }
}

impl SongCommand {
    /// Validates native POD identically to the checked byte decoder.
    #[must_use]
    pub fn valid(self) -> bool {
        match self {
            Self::Replace(r) => {
                r.previous != r.activation.epoch
                    && r.overlay_nonce != 0
                    && r.activation.frame >= 64
                    && r.activation.frame.checked_add(64).is_some()
            }
            Self::PrimeMute(m) => m.overlay_nonce != 0,
            Self::BindGraphBanks(b) => {
                matches!(
                    b.graph.kind,
                    SongResourceKind::Instrument
                        | SongResourceKind::PrivateFx
                        | SongResourceKind::Track
                        | SongResourceKind::Master
                ) && b.controls.is_none_or(|k| {
                    k.epoch == b.graph.epoch && k.kind == SongResourceKind::ControlCells
                }) && b.analysis.is_none_or(|k| {
                    k.epoch == b.graph.epoch && k.kind == SongResourceKind::AnalysisBank
                })
            }
            Self::BeginStaging(s) => Self::BeginPreparation(s.preparation).valid(),
            Self::InitCells(c) => {
                c.lease.kind == SongResourceKind::ControlCells && c.value.is_finite()
            }
            Self::ReserveAnalysis(a) => a.lease.kind == SongResourceKind::AnalysisBank,
            Self::Endpoints(e) => e.arrangement <= e.tail_deadline,
            Self::Release(r) => r.frame <= r.tail_deadline,
            Self::ConfigureBranch(b) => b.transition_frame <= b.tail_deadline,
            Self::ConfigureReusableBranch(b) => b.valid(),
            Self::RebindBranch(r) => r.valid(),
            Self::BeginPreparation(p) => (8000..=192000).contains(&p.required.sample_rate),
            Self::Event(e) => {
                e.event.time.is_finite()
                    && usize::from(e.event.n_ctl) <= crate::host::wire::MAX_CTLS
                    && e.event.controls().iter().all(|(_, c)| match c {
                        crate::host::wire::Ctl::Const(v) => v.is_finite(),
                        _ => true,
                    })
            }
            _ => true,
        }
    }
}

impl SongHostCapacities {
    /// Subtract true current/retiring leases; fixed voice pool/arena remain shared.
    /// No nominal engine total is treated as free capacity.
    pub fn after_reservations(
        self,
        current: Self,
        retiring: Self,
    ) -> Result<Self, crate::vm::fail::Failure> {
        self.validate_rate()?;
        if self.sample_rate != current.sample_rate || self.sample_rate != retiring.sample_rate {
            return Err(capacity_failure("reservation sample rates disagree"));
        }
        let mut out = self;
        macro_rules! sub {($($field:ident),*)=>{$(out.$field=self.$field.checked_sub(current.$field.checked_add(retiring.$field).ok_or_else(capacity_overflow)?).ok_or_else(||capacity_failure(stringify!($field)))?;)*};}
        sub!(
            cell_slots,
            template_slots,
            bus_slots,
            sample_resources,
            pcm_bytes,
            bus_frames,
            ack_slots
        );
        Ok(out)
    }
    /// Validate the complete fixed requirement before sending any preparation.
    pub fn admit(self, required: Self) -> Result<(), crate::vm::fail::Failure> {
        self.validate_rate()?;
        if self.sample_rate != required.sample_rate {
            return Err(capacity_failure("preparation sample rate disagrees"));
        }
        macro_rules! bound {($($field:ident),*)=>{$(if required.$field>self.$field{return Err(capacity_failure(stringify!($field)));})*};}
        bound!(
            cell_slots,
            voice_slots,
            template_slots,
            bus_slots,
            sample_resources,
            pcm_bytes,
            voice_frames,
            bus_frames,
            ack_slots
        );
        Ok(())
    }
    fn validate_rate(self) -> Result<(), crate::vm::fail::Failure> {
        if !(8000..=192000).contains(&self.sample_rate) {
            return Err(crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::Type,
                "unsupported song sample rate",
            ));
        }
        Ok(())
    }
}
fn capacity_failure(field: &str) -> crate::vm::fail::Failure {
    crate::vm::fail::Failure::new(
        crate::vm::fail::FailCode::BeyondCapability,
        format!("insufficient measured song capacity: {field}"),
    )
}
fn capacity_overflow() -> crate::vm::fail::Failure {
    crate::vm::fail::Failure::new(
        crate::vm::fail::FailCode::Overflow,
        "song resource requirement overflow",
    )
}

use super::snapshot::{FrozenRoutingInventory, FrozenSound};
use crate::dsp::graph::{BusDef, BusId, InstId};
use crate::value::{intern::KwId, Ratio64};
use std::sync::Arc;

/// Logical placement edges. Repeat remains compressed; no ordinal is hashed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SongRoutePlacement {
    Sequence {
        part: usize,
        child: u32,
        offset: Ratio64,
    },
    Repeat {
        part: usize,
        count: u32,
    },
    Edit {
        part: usize,
    },
    /// Exact local edit cover; excluded cover has distinct before/after components.
    Region {
        part: usize,
        span: crate::pattern::TimeSpan,
        inside: bool,
    },
}
/// Distinct logical summing stage, even when its chain is neutral.
/// All IDs here are candidate-local and must be privately remapped on install.
#[derive(Clone, Debug)]
pub struct SongTrackRoute {
    pub track: KwId,
    pub destination: BusId,
    pub template: Option<Arc<BusDef>>,
}
/// A prepared configuration and its symbolic owning placement.
#[derive(Clone, Debug)]
pub struct SongBranchRoute {
    pub id: SongBranchId,
    pub track: KwId,
    pub instrument: FrozenSound,
    pub resolved_instrument: InstId,
    pub sample: Option<crate::host::caps::SampleSrc>,
    pub effect_template: Option<KwId>,
    pub destination: BusId,
    pub scope_part: usize,
    pub placement: Vec<SongRoutePlacement>,
    pub source_cover: Option<u32>,
    pub configurations_per_placement: u64,
    pub occurrences: u64,
    pub reserved_generations: u32,
}
/// Actual target instance, independent of a reused candidate bus template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SongDetectorTarget {
    Branch(SongBranchId),
    Track(KwId),
    Master,
}
/// Immutable certificate of a selected payload and its finite owning scope.
#[derive(Clone, Debug)]
pub struct SongSourceRouteCover {
    pub scope_part: usize,
    pub track: KwId,
    pub cover: crate::song::source_uses::FrozenSourceUseCover,
    /// Uniform live lifecycle bound for one owning finite placement.
    pub live_generation_bound: u64,
}

/// Pre-certified inner payload for a selected finite source root.
#[derive(Clone, Debug)]
pub struct SongNestedSourceRouteCover {
    pub root_part: usize,
    pub scope_part: usize,
    pub track: KwId,
    pub cover: crate::song::source_uses::FrozenSourceUseCover,
}

/// Detector binding only; it never adds an audio sum edge. Candidate IDs must
/// be remapped to private epoch-owned stage IDs before installing any template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongSidechainRoute {
    pub template: BusId,
    pub target: SongDetectorTarget,
    pub unit: u32,
    pub source_track: KwId,
    pub source_destination: BusId,
}
/// Admission proof for copied topology, never installation/Ready proof.
#[derive(Clone, Debug)]
pub struct SongRoutePlan {
    pub source_covers: Vec<SongSourceRouteCover>,
    pub nested_source_covers: Vec<SongNestedSourceRouteCover>,
    pub branches: Vec<SongBranchRoute>,
    pub tracks: Vec<SongTrackRoute>,
    pub master: Option<Arc<BusDef>>,
    pub sidechains: Vec<SongSidechainRoute>,
    pub max_live_generations: u32,
    pub required_bytes: u64,
    /// Independent private stereo delay allocation, included in bus_frames admission.
    pub branch_delay_frames: u64,
    pub required: SongHostCapacities,
    /// Authoritative immutable FX scopes/cutoffs for realized route selection.
    pub topology: FrozenRoutingInventory,
}

mod components;
mod configuration;
mod density;
mod index;
mod nested;
mod prepare;
mod prepared;
pub(crate) use prepared::{
    prepare_routes_issued, PreparedPolicyRef, PreparedRoutes, PreparedSiteRef,
};
mod source;
mod stages;
pub use prepare::prepare_routes;
pub use stages::{materialize_route_buses, SongRouteBus};

/// One admitted configuration, shared by its notes and chord tones.
#[derive(Clone, Debug, PartialEq)]
pub struct SongResolvedRoute {
    pub branch: SongBranchId,
    pub placement: crate::song::PlacementPath,
    pub sources: Vec<SongSourceConfiguration>,
    pub configuration: crate::pattern::TimeSpan,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SongSourceConfiguration {
    pub identity: crate::song::source_uses::FrozenSourceUseIdentity,
    pub original_instrument: FrozenSound,
    pub original_configuration: crate::pattern::TimeSpan,
    pub configuration: crate::pattern::TimeSpan,
}
pub use source::resolve_route;

#[cfg(test)]
pub(crate) use index::{inspect_static_family_program, StaticFamilyRows};
