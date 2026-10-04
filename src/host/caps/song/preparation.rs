//! Control-thread ownership of one finite candidate and its actual host leases.
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ring::NativeSongInstall;
use crate::host::caps::{
    AudioHost, GraphHandle, SampleData, SampleSrc, SongCommandRefusal, SongSampleSenderCapacity,
    SongSubmitError,
};
use crate::host::wire::HostMsg;
use crate::song::routing::*;
use crate::song::routing::{prepare_routes_issued, PreparedRoutes};
use crate::song::snapshot::{
    FrozenSongEvent, SongApplyAck, SongPreparationState, SongResourceLease,
};
use crate::song::{PreparedSong, SnapshotEpoch, SongLimits};
use crate::vm::fail::{FailCode, Failure};
use std::sync::Arc;
#[path = "preparation/activation.rs"]
mod activation;
pub use activation::{
    SongReplacementCommit, SongReplacementCommitRefusal, SongReplacementPreparationRefusal,
    SongReplacementProgress, SongReplacementSource, SongReplacementSourceRefusal,
};
#[path = "preparation/demand.rs"]
mod demand;
#[cfg(test)]
#[path = "preparation/issued.rs"]
mod issued;
#[path = "preparation/ledger.rs"]
mod ledger;
#[path = "preparation/pools.rs"]
mod pools;
#[path = "preparation/resources.rs"]
mod resources;
fn fail(message: &str) -> Failure {
    Failure::new(FailCode::HostUnavailable, message)
}
fn charge(remaining: &mut u32, count: usize) -> Result<(), Failure> {
    let count = u32::try_from(count)
        .map_err(|_| Failure::new(FailCode::Overflow, "preparation work overflow"))?;
    *remaining = remaining
        .checked_sub(count)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "host preparation work exhausted"))?;
    Ok(())
}
pub struct SongPreparationLimits {
    pub capabilities: CapabilitySet,
    pub song: SongLimits,
    pub max_resources: u32,
    pub max_pending_records: u32,
    pub max_graph_bytes: u64,
    pub max_work: u32,
}
pub struct SongPreparationRefusal {
    pub prepared: PreparedSong,
    pub failure: Failure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongPreparationProgress {
    AwaitingObservation,
    Uploading,
    AwaitingReady,
    Ready,
    AwaitingRetirement,
    Retired,
    Cancelling,
    Cancelled,
    Failed,
}
#[derive(Clone, Copy, Debug)]
pub struct SongPreparationDemand {
    pub required: SongHostCapacities,
    pub analysis: SongAnalysisCapacity,
    pub resources: u32,
    pub branches: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct SongPhysicalBranch {
    pub logical: SongBranchId,
    pub slot: u32,
    pub initial: SongReusableBranch,
    pub banks: SongGraphBanks,
}
#[derive(Clone, Copy, Debug)]
pub struct SongPhysicalStage {
    pub target: SongDetectorTarget,
    pub lease: SongLeaseKey,
    pub banks: SongGraphBanks,
}
#[derive(Clone, Copy)]
enum ActivationState {
    NeverSubmitted,
    PendingExclusive(SongActivation),
    Applied(SongActivation),
    RejectedNeverActivated(SongRejectCode),
}
struct SampleBinding {
    source: SampleSrc,
    key: SongLeaseKey,
}
enum Upload {
    Sample(Arc<SampleData>),
    Graph {
        graph: GraphHandle,
        native: Option<NativeSongInstall>,
        materialized: bool,
    },
}
struct Resource {
    key: SongLeaseKey,
    upload: Option<Upload>,
    admitted: bool,
    ready: bool,
    dispatched: bool,
    returned: bool,
    cells: Vec<crate::song::snapshot::FrozenCellValue>,
    analysis: u32,
    banks: Option<SongGraphBanks>,
}
#[derive(Default)]
struct Assembly {
    resources: Vec<Resource>,
    samples: Vec<SampleBinding>,
    pools: Vec<SongPhysicalBranch>,
    stages: Vec<SongPhysicalStage>,
}
enum Step {
    Control(Box<SongCommand>),
    Upload(usize),
}
struct ControlFlight {
    command: SongCommand,
    request: SongClockRequest,
    posted: bool,
    barrier_posted: bool,
    rejected: Option<SongRejectCode>,
}
pub struct SongPreparationCleanup {
    limits: SongPreparationLimits,
    remaining: u32,
    assembly: Assembly,
    steps: Vec<Step>,
    cursor: usize,
    flight: Option<ControlFlight>,
    upload: Option<usize>,
    nonce: u64,
    begun: bool,
    cancel_posted: bool,
    cancelled: bool,
    seal_ready: bool,
    activation: ActivationState,
    replacement: Option<activation::PendingReplacement>,
    failure: Option<Failure>,
    reported: bool,
}
pub struct SongHostPreparation {
    prepared: Option<PreparedSong>,
    routes: Option<PreparedRoutes>,
    report: Option<SongCapacityReport>,
    clock: Option<SongHostClock>,
    capacity_posted: bool,
    observation_posted: bool,
    progress: SongPreparationProgress,
    cleanup: SongPreparationCleanup,
}
pub struct SongReadyBundle {
    prepared: PreparedSong,
    routes: PreparedRoutes,
    resources: Vec<SongLeaseKey>,
    pools: Vec<SongPhysicalBranch>,
    stages: Vec<SongPhysicalStage>,
    clock: SongHostClock,
    cleanup: SongPreparationCleanup,
}
impl SongHostPreparation {
    /// Takes the original candidate; local refusal returns that same owner.
    #[allow(clippy::result_large_err)] // Refusal returns the original complete candidate owner.
    pub fn begin(
        prepared: PreparedSong,
        limits: SongPreparationLimits,
    ) -> Result<Self, SongPreparationRefusal> {
        let valid = limits.song.validate().and_then(|()| {
            if prepared.state() != SongPreparationState::Preparing
                || limits.max_resources == 0
                || limits.max_pending_records == 0
                || limits.max_work == 0
            {
                Err(fail("invalid host preparation state or bounds"))
            } else {
                Ok(())
            }
        });
        if let Err(failure) = valid {
            return Err(SongPreparationRefusal { prepared, failure });
        }
        let remaining = limits.max_work;
        Ok(Self {
            prepared: Some(prepared),
            routes: None,
            report: None,
            clock: None,
            capacity_posted: false,
            observation_posted: false,
            progress: SongPreparationProgress::AwaitingObservation,
            cleanup: SongPreparationCleanup {
                limits,
                remaining,
                assembly: Assembly::default(),
                steps: Vec::new(),
                cursor: 0,
                flight: None,
                upload: None,
                nonce: 1,
                begun: false,
                cancel_posted: false,
                cancelled: false,
                seal_ready: false,
                activation: ActivationState::NeverSubmitted,
                replacement: None,
                failure: None,
                reported: false,
            },
        })
    }
    fn epoch(&self) -> Result<SnapshotEpoch, Failure> {
        self.prepared
            .as_ref()
            .map(PreparedSong::epoch)
            .ok_or_else(|| fail("preparation already consumed"))
    }
    fn failed(&mut self, failure: Failure) {
        if self
            .cleanup
            .flight
            .as_ref()
            .is_some_and(|flight| !flight.posted)
        {
            self.cleanup.flight = None;
        }
        if self.cleanup.failure.is_none() {
            self.cleanup.failure = Some(failure);
        }
        self.progress = if self.cleanup.begun || self.cleanup.flight.is_some() {
            SongPreparationProgress::Cancelling
        } else {
            SongPreparationProgress::Failed
        };
    }
    fn command(&mut self, host: &mut dyn AudioHost, command: SongCommand) -> Result<bool, Failure> {
        match host.try_song_command(command) {
            Ok(()) => Ok(true),
            Err(refusal) => match refusal.error {
                SongSubmitError::Backpressure => Ok(false),
                SongSubmitError::Unavailable => Err(fail("song commands unavailable")),
                SongSubmitError::Invalid(failure) => Err(failure),
            },
        }
    }
    fn prepare(&mut self, host: &dyn AudioHost) -> Result<(), Failure> {
        let report = self.report.ok_or_else(|| fail("capacity not observed"))?;
        let clock = self.clock.ok_or_else(|| fail("clock not observed"))?;
        let mut local = self.cleanup.remaining.min(1_000_000);
        let initial = local;
        let song_limits = self.cleanup.limits.song;
        let issued_limits = SongLimits {
            max_nodes: initial,
            ..song_limits
        };
        let capabilities = &self.cleanup.limits.capabilities;
        let prepared = self
            .prepared
            .as_mut()
            .ok_or_else(|| fail("candidate consumed"))?;
        let routes_result = (|| {
            let authority =
                prepared.issue_retained_route_authority(issued_limits, &mut local, 0)?;
            prepare_routes_issued(
                authority,
                prepared.snapshot().settings(),
                capabilities,
                &report.available,
                issued_limits,
                &mut local,
                0,
            )
        })();
        let debit = initial
            .checked_sub(local)
            .ok_or_else(|| fail("issued preparation work counter increased"))?;
        charge(&mut self.cleanup.remaining, debit as usize)?;
        let routes = routes_result?;
        let mut assembly = resources::build(
            prepared,
            routes.plan(),
            &self.cleanup.limits,
            clock,
            &mut self.cleanup.remaining,
        )?;
        pools::finish(
            &mut assembly,
            routes.plan(),
            clock,
            &mut self.cleanup.remaining,
        )?;
        let demand = demand::from_assembly(&assembly, routes.plan(), &mut self.cleanup.remaining)?;
        demand::verify_observation(&demand, report, clock)?;
        if demand.required.sample_resources != 0 {
            if let SongSampleSenderCapacity::Bounded {
                resources,
                pcm_bytes,
            } = host.song_sample_sender_capacity()?
            {
                if demand.required.sample_resources > resources
                    || demand.required.pcm_bytes > pcm_bytes
                {
                    return Err(fail(
                        "complete song PCM union exceeds observed sample sender capacity",
                    ));
                }
            }
        }
        let steps = ledger::steps(
            &assembly,
            demand,
            prepared.epoch(),
            self.cleanup.limits.max_pending_records,
            &mut self.cleanup.remaining,
        )?;
        if steps.len() > self.cleanup.limits.max_pending_records as usize {
            return Err(fail("preparation record bound exceeded"));
        }
        charge(
            &mut self.cleanup.remaining,
            assembly
                .resources
                .len()
                .checked_mul(2)
                .ok_or_else(|| fail("lease projection overflow"))?,
        )?;
        prepared.reserve_bounded(
            assembly
                .resources
                .iter()
                .map(|r| SongResourceLease {
                    resource: r.key.resource.id,
                    generation: r.key.resource.generation,
                })
                .collect(),
            self.cleanup.limits.max_resources,
            &mut self.cleanup.remaining,
        )?;
        self.cleanup.assembly = assembly;
        self.cleanup.steps = steps;
        self.routes = Some(routes);
        self.progress = SongPreparationProgress::Uploading;
        Ok(())
    }
    fn advance_control(&mut self, host: &mut dyn AudioHost) -> Result<(), Failure> {
        let Some(flight) = self.cleanup.flight.as_ref() else {
            return Ok(());
        };
        if !flight.posted {
            let command = flight.command;
            if self.command(host, command)? {
                self.cleanup
                    .flight
                    .as_mut()
                    .ok_or_else(|| fail("control flight absent"))?
                    .posted = true;
            }
        } else if !flight.barrier_posted {
            let command = SongCommand::RequestClock(flight.request);
            if self.command(host, command)? {
                self.cleanup
                    .flight
                    .as_mut()
                    .ok_or_else(|| fail("control flight absent"))?
                    .barrier_posted = true;
            }
        }
        Ok(())
    }
    fn upload(&mut self, host: &mut dyn AudioHost, index: usize) -> Result<(), Failure> {
        let resource = self
            .cleanup
            .assembly
            .resources
            .get_mut(index)
            .ok_or_else(|| fail("upload index invalid"))?;
        if !resource.admitted {
            return Err(fail("upload lease unconfirmed"));
        }
        let Some(upload) = resource.upload.as_mut() else {
            return Err(fail("upload absent"));
        };
        let accepted = match upload {
            Upload::Sample(data) => match host.try_song_sample(resource.key, Arc::clone(data)) {
                Ok(()) => true,
                Err(refusal) => {
                    if refusal.lease != resource.key || !Arc::ptr_eq(data, &refusal.data) {
                        return Err(fail("host changed refused sample ownership"));
                    }
                    match refusal.error {
                        SongSubmitError::Backpressure => false,
                        SongSubmitError::Unavailable => {
                            return Err(fail("song sample upload unavailable"))
                        }
                        SongSubmitError::Invalid(failure) => return Err(failure),
                    }
                }
            },
            Upload::Graph {
                graph,
                native,
                materialized,
            } => {
                if !*materialized {
                    *native = host.materialize_song_native(resource.key, graph)?;
                    *materialized = true;
                }
                if let Some(install) = native.take() {
                    match host.submit_song_native(install) {
                        Ok(()) => true,
                        Err(returned) => {
                            *native = Some(returned);
                            false
                        }
                    }
                } else if resource.dispatched {
                    return Ok(());
                } else {
                    match host.try_song_graph(resource.key, graph) {
                        Ok(()) => true,
                        Err(SongSubmitError::Backpressure) => false,
                        Err(SongSubmitError::Unavailable) => {
                            return Err(fail("borrowed graphs unavailable"))
                        }
                        Err(SongSubmitError::Invalid(failure)) => return Err(failure),
                    }
                }
            }
        };
        if accepted {
            resource.dispatched = true;
            self.cleanup.upload = Some(index);
        }
        Ok(())
    }
    fn drive(&mut self, host: &mut dyn AudioHost) -> Result<(), Failure> {
        let epoch = self.epoch()?;
        if self.cleanup.flight.is_some() {
            return self.advance_control(host);
        }
        match self.progress {
            SongPreparationProgress::AwaitingObservation => {
                if !self.capacity_posted {
                    self.capacity_posted =
                        self.command(host, SongCommand::RequestCapacity(epoch))?;
                } else if !self.observation_posted {
                    self.observation_posted = self.command(
                        host,
                        SongCommand::RequestClock(SongClockRequest { epoch, request: 1 }),
                    )?;
                } else if self.report.is_some() && self.clock.is_some() {
                    self.prepare(host)?;
                }
            }
            SongPreparationProgress::Uploading | SongPreparationProgress::AwaitingReady => {
                if self.cleanup.upload.is_some() {
                    return Ok(());
                }
                if let Some(step) = self.cleanup.steps.get(self.cleanup.cursor) {
                    match step {
                        Step::Upload(index) => self.upload(host, *index)?,
                        Step::Control(command) => {
                            let command = **command;
                            self.cleanup.nonce = self
                                .cleanup
                                .nonce
                                .checked_add(1)
                                .ok_or_else(|| fail("clock nonce exhausted"))?;
                            self.cleanup.flight = Some(ControlFlight {
                                command,
                                request: SongClockRequest {
                                    epoch,
                                    request: self.cleanup.nonce,
                                },
                                posted: false,
                                barrier_posted: false,
                                rejected: None,
                            });
                            self.advance_control(host)?;
                        }
                    }
                } else if self.cleanup.seal_ready {
                    let prepared = self
                        .prepared
                        .as_mut()
                        .ok_or_else(|| fail("candidate consumed"))?;
                    charge(&mut self.cleanup.remaining, prepared.resources().len())?;
                    let resources = prepared.resources().to_vec();
                    prepared.acknowledge_ready(&resources)?;
                    self.progress = SongPreparationProgress::Ready;
                }
            }
            SongPreparationProgress::Cancelling => {
                if !self.cleanup.begun {
                    self.progress = SongPreparationProgress::Failed;
                } else if !self.cleanup.cancel_posted {
                    self.cleanup.cancel_posted =
                        self.command(host, SongCommand::CancelPreparation(epoch))?;
                } else if self.cleanup.cancelled
                    && self.cleanup.upload.is_none()
                    && self
                        .cleanup
                        .assembly
                        .resources
                        .iter()
                        .all(|r| !r.admitted || r.returned)
                {
                    self.progress = SongPreparationProgress::Cancelled;
                }
            }
            SongPreparationProgress::AwaitingRetirement => {
                self.cleanup.submit_replacement_cancel(host)?;
                if self
                    .cleanup
                    .assembly
                    .resources
                    .iter()
                    .all(|r| !r.admitted || r.returned)
                    && !matches!(
                        self.cleanup.activation,
                        ActivationState::PendingExclusive(_)
                    )
                {
                    self.progress = SongPreparationProgress::Retired;
                }
            }
            _ => {}
        }
        Ok(())
    }
    /// Executes at most one bounded cursor action; enqueue never establishes Ready.
    pub fn submit(&mut self, host: &mut dyn AudioHost) -> Result<SongPreparationProgress, Failure> {
        if let Err(failure) = self.drive(host) {
            self.failed(failure);
        }
        if !self.cleanup.reported {
            if let Some(failure) = &self.cleanup.failure {
                self.cleanup.reported = true;
                return Err(failure.clone());
            }
        }
        Ok(self.progress)
    }
    /// Consumes only exact receipts for this owner, returning all others intact.
    pub fn receive(&mut self, ack: SongHostAck) -> Result<(), SongHostAck> {
        let Ok(epoch) = self.epoch() else {
            return Err(ack);
        };
        if ack.epoch() != epoch {
            return Err(ack);
        }
        if matches!(self.progress, SongPreparationProgress::AwaitingRetirement) {
            if self
                .cleanup
                .replacement
                .as_ref()
                .is_some_and(|p| p.invalidated && p.cancel_posted)
                && matches!(
                    ack,
                    SongHostAck::Rejected {
                        reason: SongRejectCode::HostFault,
                        ..
                    }
                )
            {
                self.cleanup.failure = Some(fail("replacement invalidated after arm"));
                return Ok(());
            }
            if let ActivationState::PendingExclusive(_) = self.cleanup.activation {
                if matches!(
                    ack,
                    SongHostAck::Applied(_) | SongHostAck::ActivationRejected { .. }
                ) {
                    let prepared = self.prepared.as_mut().ok_or(ack)?;
                    activation::receive_activation(prepared, &mut self.cleanup, ack)?;
                    if matches!(
                        self.cleanup.activation,
                        ActivationState::RejectedNeverActivated(_)
                    ) {
                        self.progress = SongPreparationProgress::Cancelling;
                    }
                    return Ok(());
                }
            }
        }
        match ack {
            SongHostAck::CapacityReport(report)
                if self.capacity_posted && self.report.is_none() =>
            {
                self.report = Some(report);
                Ok(())
            }
            SongHostAck::CapacityRejected { reason, .. }
                if self.capacity_posted && self.report.is_none() =>
            {
                self.failed(fail(&format!("capacity observation rejected: {reason:?}")));
                Ok(())
            }
            SongHostAck::ClockReport(report) => {
                if let Some(flight) = self.cleanup.flight.as_ref() {
                    if !flight.barrier_posted || report.request != flight.request {
                        return Err(ack);
                    }
                    let flight = self.cleanup.flight.take().ok_or(ack)?;
                    let consistent = self.clock.is_none_or(|previous| {
                        report.clock.sample_rate == previous.sample_rate
                            && report.clock.frame >= previous.frame
                    });
                    self.clock = Some(report.clock);
                    if let Some(reason) = flight.rejected {
                        self.failed(fail(&format!("control rejected: {reason:?}")));
                    } else {
                        match flight.command {
                            SongCommand::BeginStaging(_) => self.cleanup.begun = true,
                            SongCommand::ReserveResource(reservation) => {
                                let key = SongLeaseKey {
                                    epoch,
                                    resource: reservation.resource,
                                    kind: reservation.kind,
                                };
                                let resource = self
                                    .cleanup
                                    .assembly
                                    .resources
                                    .iter_mut()
                                    .find(|r| r.key == key)
                                    .ok_or(ack)?;
                                resource.admitted = true;
                            }
                            _ => {}
                        }
                        self.cleanup.cursor += 1;
                    }
                    if !consistent {
                        self.failed(fail("clock changed during silent preparation"));
                    }
                    Ok(())
                } else if self.observation_posted
                    && self.clock.is_none()
                    && report.request == (SongClockRequest { epoch, request: 1 })
                {
                    self.clock = Some(report.clock);
                    Ok(())
                } else {
                    Err(ack)
                }
            }
            SongHostAck::ClockRejected(failure) => {
                if self
                    .cleanup
                    .flight
                    .as_ref()
                    .is_some_and(|f| f.request == failure.request)
                {
                    // A failed barrier cannot prove whether its mutation ran.
                    self.failed(fail("control barrier rejected; ownership unresolved"));
                    Ok(())
                } else if self.observation_posted
                    && self.clock.is_none()
                    && failure.request == (SongClockRequest { epoch, request: 1 })
                {
                    self.failed(fail("initial clock rejected"));
                    Ok(())
                } else {
                    Err(ack)
                }
            }
            SongHostAck::Rejected { reason, .. } => {
                if let Some(flight) = self.cleanup.flight.as_mut() {
                    if !flight.posted || flight.rejected.is_some() {
                        return Err(ack);
                    }
                    flight.rejected = Some(reason);
                    Ok(())
                } else if self.cleanup.upload.take().is_some() {
                    self.failed(fail(&format!("upload rejected: {reason:?}")));
                    Ok(())
                } else {
                    Err(ack)
                }
            }
            SongHostAck::ResourceReady { resource, .. } => {
                let Some(index) = self.cleanup.upload else {
                    return Err(ack);
                };
                let r = self.cleanup.assembly.resources.get_mut(index).ok_or(ack)?;
                if r.key.resource != resource || r.ready || !r.dispatched {
                    return Err(ack);
                }
                r.ready = true;
                self.cleanup.upload = None;
                self.cleanup.cursor += 1;
                Ok(())
            }
            SongHostAck::Ready(_)
                if self
                    .cleanup
                    .flight
                    .as_ref()
                    .is_some_and(|f| matches!(f.command, SongCommand::SealPreparation(_)))
                    && !self.cleanup.seal_ready =>
            {
                self.cleanup.seal_ready = true;
                Ok(())
            }
            SongHostAck::LeaseReturned(key) => {
                let r = self
                    .cleanup
                    .assembly
                    .resources
                    .iter_mut()
                    .find(|r| r.key == key && r.admitted && !r.returned)
                    .ok_or(ack)?;
                if !matches!(
                    self.progress,
                    SongPreparationProgress::Cancelling
                        | SongPreparationProgress::AwaitingRetirement
                ) {
                    return Err(ack);
                }
                r.returned = true;
                Ok(())
            }
            SongHostAck::PreparationCancelled(_)
                if self.cleanup.cancel_posted && !self.cleanup.cancelled =>
            {
                self.cleanup.cancelled = true;
                Ok(())
            }
            _ => Err(ack),
        }
    }
    /// Standalone exclusive driver; a shared Runtime must instead dispatch receive.
    pub fn poll(&mut self, host: &mut dyn AudioHost) -> Result<Option<HostMsg>, Failure> {
        match host.poll_msg()? {
            Some(HostMsg::Song(ack)) => Ok(self.receive(ack).err().map(HostMsg::Song)),
            message => Ok(message),
        }
    }
    pub fn cancel(&mut self) -> Result<(), Failure> {
        self.epoch()?;
        if !matches!(
            self.cleanup.activation,
            ActivationState::NeverSubmitted | ActivationState::RejectedNeverActivated(_)
        ) {
            return Err(fail("submitted activation requires normal retirement"));
        }
        self.progress = SongPreparationProgress::Cancelling;
        Ok(())
    }
    pub fn take_ready(&mut self) -> Result<SongReadyBundle, Failure> {
        if self.progress != SongPreparationProgress::Ready
            || self.cleanup.flight.is_some()
            || self.cleanup.upload.is_some()
            || self.prepared.is_none()
            || self.routes.is_none()
            || self.clock.is_none()
        {
            return Err(fail("host preparation not ready"));
        }
        charge(
            &mut self.cleanup.remaining,
            self.cleanup.assembly.resources.len(),
        )?;
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| fail("candidate consumed"))?;
        let routes = self.routes.take().ok_or_else(|| fail("routes absent"))?;
        let clock = self.clock.ok_or_else(|| fail("clock absent"))?;
        let replacement = SongPreparationCleanup {
            limits: SongPreparationLimits {
                capabilities: self.cleanup.limits.capabilities,
                song: self.cleanup.limits.song,
                max_resources: 0,
                max_pending_records: 0,
                max_graph_bytes: 0,
                max_work: 0,
            },
            remaining: 0,
            assembly: Assembly::default(),
            steps: Vec::new(),
            cursor: 0,
            flight: None,
            upload: None,
            nonce: 0,
            begun: false,
            cancel_posted: false,
            cancelled: false,
            seal_ready: false,
            activation: ActivationState::NeverSubmitted,
            replacement: None,
            failure: None,
            reported: false,
        };
        let mut cleanup = std::mem::replace(&mut self.cleanup, replacement);
        let resources = cleanup.assembly.resources.iter().map(|r| r.key).collect();
        let pools = std::mem::take(&mut cleanup.assembly.pools);
        let stages = std::mem::take(&mut cleanup.assembly.stages);
        Ok(SongReadyBundle {
            prepared,
            routes,
            resources,
            pools,
            stages,
            clock,
            cleanup,
        })
    }
    pub fn take_cancelled(&mut self) -> Result<PreparedSong, Failure> {
        if !matches!(
            self.progress,
            SongPreparationProgress::Cancelled | SongPreparationProgress::Failed
        ) || self.cleanup.flight.is_some()
            || self.cleanup.upload.is_some()
        {
            return Err(fail("cleanup not completed"));
        }
        let mut prepared = self
            .prepared
            .take()
            .ok_or_else(|| fail("candidate consumed"))?;
        prepared.cancel(prepared.epoch())?;
        Ok(prepared)
    }
    pub fn take_retired(&mut self) -> Result<PreparedSong, Failure> {
        if self.progress != SongPreparationProgress::Retired {
            return Err(fail("normal retirement incomplete"));
        }
        self.prepared
            .take()
            .ok_or_else(|| fail("candidate consumed"))
    }
    pub const fn progress(&self) -> SongPreparationProgress {
        self.progress
    }
    pub fn retire(ready: SongReadyBundle) -> Self {
        let SongReadyBundle {
            prepared,
            routes,
            resources: _,
            pools,
            stages,
            clock,
            mut cleanup,
        } = ready;
        cleanup.assembly.pools = pools;
        cleanup.assembly.stages = stages;
        let progress = if matches!(
            cleanup.activation,
            ActivationState::NeverSubmitted | ActivationState::RejectedNeverActivated(_)
        ) {
            SongPreparationProgress::Cancelling
        } else {
            SongPreparationProgress::AwaitingRetirement
        };
        Self {
            prepared: Some(prepared),
            routes: Some(routes),
            report: None,
            clock: Some(clock),
            capacity_posted: false,
            observation_posted: false,
            progress,
            cleanup,
        }
    }
}
impl SongReadyBundle {
    pub fn prepared(&self) -> &PreparedSong {
        &self.prepared
    }
    pub fn routes(&self) -> &SongRoutePlan {
        self.routes.plan()
    }
    pub fn resources(&self) -> &[SongLeaseKey] {
        &self.resources
    }
    pub fn pools(&self) -> &[SongPhysicalBranch] {
        &self.pools
    }
    pub fn stages(&self) -> &[SongPhysicalStage] {
        &self.stages
    }
    pub fn sample_resource(&self, source: &SampleSrc) -> Option<SongLeaseKey> {
        self.cleanup
            .assembly
            .samples
            .iter()
            .find(|s| s.source == *source)
            .map(|s| s.key)
    }
    /// Original admitted PCM geometry; no loading, copying or snapshot mutation.
    pub fn sample_data(&self, source: &SampleSrc) -> Option<&SampleData> {
        let key = self.sample_resource(source)?;
        let resource = self
            .cleanup
            .assembly
            .resources
            .iter()
            .find(|r| r.key == key)?;
        match &resource.upload {
            Some(Upload::Sample(data)) => Some(data.as_ref()),
            _ => None,
        }
    }
    pub const fn clock(&self) -> SongHostClock {
        self.clock
    }
    pub fn query(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        limits: &SongLimits,
    ) -> Result<Vec<FrozenSongEvent>, Failure> {
        self.prepared.query(span, limits)
    }
    pub(crate) fn query_issued_with_work(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        work: &crate::pattern::eval::song_observation::SharedIndexWork,
        depth: u32,
    ) -> Result<crate::song::snapshot::issued::FrozenIssuedBatch, Failure> {
        self.prepared.query_issued_with_work(span, work, depth)
    }
    pub(crate) fn resolve_issued(
        &self,
        batch: &crate::song::snapshot::issued::FrozenIssuedBatch,
        event_index: usize,
        work: &crate::pattern::eval::song_observation::SharedIndexWork,
        depth: u32,
    ) -> Result<crate::song::routing::SongResolvedRoute, Failure> {
        self.routes
            .resolve_issued_event(batch, event_index, work, depth)
    }
}
