//! Exact original-owner finite playback, separate from legacy slot scheduling.
use super::*;
use crate::host::caps::{
    AudioHost, SongHostPreparation, SongPreparationLimits, SongPreparationProgress,
    SongPreparationRefusal, SongReadyBundle, SongSubmitError,
};
use crate::sched::song::{SongTransport, SongTransportRefusal, SongTransportState};
use crate::song::routing::{SongClockRequest, SongCommand, SongHostAck, SongHostClock, SongMute};
use crate::song::snapshot::FrozenSound;
use crate::song::snapshot::{FrozenCellInventory, FrozenGraphResources, FrozenInstrument};
use crate::song::{
    InstrumentSelector, PreparedSong, SnapshotEpoch, SongApplyAck, SongLimits, SongSettings,
};

#[cfg(test)]
#[path = "song/clock_tests.rs"]
mod clock_tests;

mod apply;
mod overlay;

const OWNER_LIMIT: usize = 4;
const NOTICE_LIMIT: usize = 256;
pub enum SongNotice {
    Ready {
        epoch: SnapshotEpoch,
        revision: u64,
    },
    Applied {
        ack: SongApplyAck,
        catalog: Rc<AppliedCatalog>,
    },
    CarriedMute {
        epoch: SnapshotEpoch,
        sound: FrozenSound,
        muted: bool,
        frame: u64,
    },
    Failed {
        epoch: SnapshotEpoch,
        revision: u64,
        failure: Failure,
    },
    Muted {
        request: u64,
        epoch: SnapshotEpoch,
        muted: bool,
        frame: u64,
    },
    MuteFailed {
        request: u64,
        epoch: SnapshotEpoch,
    },
    State {
        epoch: SnapshotEpoch,
        state: SongTransportState,
        families: Vec<FrozenSound>,
    },
}
type AppliedCatalog = (
    Vec<FrozenInstrument>,
    FrozenCellInventory,
    FrozenGraphResources,
);
struct FamilyRecord {
    sound: FrozenSound,
    instrument: u32,
    certificate: Option<overlay::FrozenFamilyCertificate>,
    muted: bool,
}
struct Preparing {
    owner: SongHostPreparation,
    previous: Option<SnapshotEpoch>,
    work: u32,
    epoch: SnapshotEpoch,
    revision: u64,
    reported: bool,
    invalidation: Option<Failure>,
}
struct Running {
    transport: SongTransport,
    revision: u64,
    activation: u64,
    families: Vec<FamilyRecord>,
    catalog: Rc<AppliedCatalog>,
    offered: Ratio64,
    cutoff_cycle: Option<Ratio64>,
    replacement: Option<apply::PendingReplacement>,
    activation_posted: bool,
    applied_reported: bool,
    failure_reported: bool,
    published_state: Option<SongTransportState>,
}
struct MuteFamily {
    instrument: u32,
    posted: bool,
    acknowledged: bool,
}
struct PendingMute {
    failed: bool,
    request: u64,
    epoch: SnapshotEpoch,
    muted: bool,
    frame: u64,
    application_frame: u64,
    families: Vec<MuteFamily>,
}
#[derive(Default)]
pub(super) struct SongRuntime {
    preparing: Option<Preparing>,
    ready: Option<SongReadyBundle>,
    ready_previous: Option<SnapshotEpoch>,
    ready_work: u32,
    owners: Vec<Running>,
    clock: Option<SongHostClock>,
    request: Option<SongClockRequest>,
    nonce: u64,
    clock_failure: Option<ClockProgressFailure>,
    notices: VecDeque<SongNotice>,
    deferred: VecDeque<HostMsg>,
    last_state: Option<SongTransportState>,
    mutes: Vec<PendingMute>,
}
#[derive(Debug)]
struct ClockProgressFailure {
    epoch: Option<SnapshotEpoch>,
    failure: Failure,
}
impl SongRuntime {
    fn receive_clock(&mut self, ack: SongHostAck) -> bool {
        match ack {
            SongHostAck::ClockReport(report) if self.request == Some(report.request) => {
                if !(8000..=192000).contains(&report.clock.sample_rate)
                    || self
                        .clock
                        .is_some_and(|old| old.sample_rate != report.clock.sample_rate)
                {
                    self.request = None;
                    self.clock_failure = Some(ClockProgressFailure {
                        epoch: Some(report.request.epoch),
                        failure: song_failure("invalid exact song clock report"),
                    });
                    return true;
                }
                self.request = None;
                // A native direct read can be newer than this queued real report.
                if self.clock.is_none_or(|old| old.frame <= report.clock.frame) {
                    self.clock = Some(report.clock);
                }
                true
            }
            SongHostAck::ClockRejected(rejected) if self.request == Some(rejected.request) => {
                self.request = None;
                self.clock_failure = Some(ClockProgressFailure {
                    epoch: Some(rejected.request.epoch),
                    failure: song_failure("exact song clock refresh rejected"),
                });
                true
            }
            _ => false,
        }
    }
    fn refresh_clock(
        &mut self,
        host: &mut dyn AudioHost,
        epoch: Option<SnapshotEpoch>,
    ) -> Result<(), ClockProgressFailure> {
        if let Ok(clock) = host.song_clock() {
            if !(8000..=192000).contains(&clock.sample_rate)
                || self
                    .clock
                    .is_some_and(|old| old.sample_rate != clock.sample_rate)
            {
                return Err(ClockProgressFailure {
                    epoch: None,
                    failure: song_failure("invalid actual song clock observation"),
                });
            }
            if self.clock.is_none_or(|old| old.frame <= clock.frame) {
                self.clock = Some(clock);
            }
        }
        if let Some(failure) = self.clock_failure.take() {
            return Err(failure);
        }
        let Some(epoch) = epoch.filter(|_| self.request.is_none()) else {
            return Ok(());
        };
        let nonce = self
            .nonce
            .checked_add(1)
            .ok_or_else(|| ClockProgressFailure {
                epoch: Some(epoch),
                failure: song_failure("song clock nonce exhausted"),
            })?;
        let request = SongClockRequest {
            epoch,
            request: nonce,
        };
        match host.try_song_command(SongCommand::RequestClock(request)) {
            Ok(()) => {
                self.request = Some(request);
                self.nonce = nonce;
                Ok(())
            }
            Err(refusal) => match refusal.error {
                SongSubmitError::Backpressure => Ok(()),
                SongSubmitError::Invalid(failure) => Err(ClockProgressFailure {
                    epoch: Some(epoch),
                    failure,
                }),
                SongSubmitError::Unavailable => Err(ClockProgressFailure {
                    epoch: Some(epoch),
                    failure: song_failure("song clock refresh unavailable"),
                }),
            },
        }
    }
}
impl Runtime {
    /// Sender ownership must select polling before the first song command and
    /// retain it through final resource retirement, not merely transport Ended.
    pub fn expect_song_receipts(&mut self, epoch: SnapshotEpoch) -> Result<(), Failure> {
        if self.pending_song_ack.is_some() {
            return Err(song_failure("unconsumed pending song receipt"));
        }
        self.song_receipts.expect(epoch)
    }
    pub fn song_receipts(&self) -> &crate::song::routing::SongReceipts {
        &self.song_receipts
    }
    pub fn pop_song_receipt(&mut self) -> Option<SongHostAck> {
        self.song_receipts.pop()
    }
    fn retain_song_ack(&mut self, ack: SongHostAck, rep: &mut TickReport) -> bool {
        match self.song_receipts.record(ack) {
            Ok(true) => true,
            Ok(false) => {
                rep.faults.push(Failure::new(
                    FailCode::Type,
                    "stale song acknowledgment retained for ownership processing",
                ));
                true
            }
            Err(ack) => {
                self.pending_song_ack = Some(ack);
                false
            }
        }
    }
    pub(super) fn poll_song_messages(&mut self, rep: &mut TickReport, msgs: &mut Vec<HostMsg>) {
        if let Some(ack) = self.pending_song_ack.take() {
            if !self.retain_song_ack(ack, rep) {
                return;
            }
        }
        for _ in 0..256 {
            match self.hosts.audio.poll_msg() {
                Ok(Some(HostMsg::Song(ack))) => {
                    if !self.retain_song_ack(ack, rep) {
                        break;
                    }
                }
                Ok(Some(message)) => msgs.push(message),
                Ok(None) => break,
                Err(failure) => {
                    rep.faults.push(failure);
                    break;
                }
            }
        }
    }
    pub(crate) fn retains_song_epoch(&self, epoch: SnapshotEpoch) -> bool {
        self.song
            .preparing
            .as_ref()
            .is_some_and(|p| p.epoch == epoch)
            || self
                .song
                .ready
                .as_ref()
                .is_some_and(|p| p.prepared().epoch() == epoch)
            || self
                .song
                .owners
                .iter()
                .any(|o| o.transport.epoch() == epoch)
    }
    /// Invalidates pending owners while retaining authentic replacement cleanup.
    /// Activated or uncertain owners require normal musical retirement instead.
    pub(crate) fn invalidate_pending_song(
        &mut self,
        epoch: SnapshotEpoch,
    ) -> Result<bool, Failure> {
        let failure = song_failure("song candidate invalidated by document revision/edit epoch");
        if let Some(p) = self.song.preparing.as_mut().filter(|p| p.epoch == epoch) {
            if matches!(
                p.owner.progress(),
                SongPreparationProgress::Cancelling
                    | SongPreparationProgress::Cancelled
                    | SongPreparationProgress::Failed
            ) {
                return Ok(true);
            }
            p.owner.cancel()?;
            p.invalidation.get_or_insert(failure);
            return Ok(true);
        }
        if self
            .song
            .ready
            .as_ref()
            .is_some_and(|r| r.prepared().epoch() == epoch)
        {
            if let Some(ready) = self.song.ready.take() {
                let revision = ready.prepared().revision();
                self.song.preparing = Some(Preparing {
                    owner: SongHostPreparation::retire(ready),
                    previous: self.song.ready_previous.take(),
                    work: self.song.ready_work,
                    epoch,
                    revision,
                    reported: false,
                    invalidation: Some(failure),
                });
                return Ok(true);
            }
        }
        if let Some(owner) = self
            .song
            .owners
            .iter_mut()
            .find(|o| o.transport.epoch() == epoch)
        {
            if let Some(pending) = &mut owner.replacement {
                if !pending.committed {
                    pending.invalidated = true;
                    owner.transport.invalidate_replacement()?;
                    return Ok(true);
                }
            }
            if !owner.activation_posted {
                owner.transport.abort(failure);
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// Transfers the original isolated authority; refusal returns that same owner.
    #[allow(clippy::result_large_err)]
    pub fn prepare_song(
        &mut self,
        prepared: PreparedSong,
        limits: SongPreparationLimits,
    ) -> Result<(), SongPreparationRefusal> {
        if self.song.preparing.is_some()
            || self.song.ready.is_some()
            || self.song.owners.len() >= OWNER_LIMIT
            || self
                .slots
                .iter()
                .any(|s| s.bound.is_some() || s.pending.is_some())
        {
            return Err(SongPreparationRefusal {
                prepared,
                failure: song_failure("song owner busy or legacy slots active"),
            });
        }
        let epoch = prepared.epoch();
        let revision = prepared.revision();
        let previous = self
            .song
            .owners
            .iter()
            .rev()
            .find(|o| {
                o.transport.applied_activation().is_some()
                    && o.transport.state() != SongTransportState::Failed
            })
            .map(|o| o.transport.epoch());
        if self
            .song
            .owners
            .iter()
            .any(|o| o.replacement.as_ref().is_some_and(|p| !p.definitive()))
        {
            return Err(SongPreparationRefusal {
                prepared,
                failure: song_failure("replacement already pending"),
            });
        }
        let work = limits.max_work;
        let owner = SongHostPreparation::begin(prepared, limits)?;
        self.song.preparing = Some(Preparing {
            owner,
            previous,
            work,
            epoch,
            revision,
            reported: false,
            invalidation: None,
        });
        Ok(())
    }
    /// Consumes genuine Ready without publishing an enqueue as Applied.
    #[allow(clippy::result_large_err)]
    pub fn start_song(
        &mut self,
        ready: SongReadyBundle,
        activation_frame: u64,
    ) -> Result<(), SongTransportRefusal> {
        if self.song.owners.len() >= OWNER_LIMIT
            || self
                .slots
                .iter()
                .any(|s| s.bound.is_some() || s.pending.is_some())
        {
            return Err(SongTransportRefusal {
                ready,
                failure: song_failure("song owner capacity or legacy slot conflict"),
            });
        }
        let revision = ready.prepared().revision();
        let mut work = if self.song.ready_work == 0 {
            SongLimits::default().max_nodes
        } else {
            self.song.ready_work
        };
        let families = match overlay::build_family_records(&ready, &mut work) {
            Ok(families) => families,
            Err(failure) => return Err(SongTransportRefusal { ready, failure }),
        };
        let catalog = match apply::capture_catalog(&ready, &mut work) {
            Ok(catalog) => catalog,
            Err(failure) => return Err(SongTransportRefusal { ready, failure }),
        };
        self.song.ready_work = work;
        let nonce_floor = ready.clock_request_floor();
        let transport = SongTransport::new(ready, activation_frame, SongLimits::default())?;
        self.song.nonce = self.song.nonce.max(nonce_floor);
        self.song.owners.push(Running {
            transport,
            revision,
            activation: activation_frame,
            families,
            catalog,
            offered: Ratio64::ZERO,
            cutoff_cycle: None,
            replacement: None,
            activation_posted: false,
            applied_reported: false,
            failure_reported: false,
            published_state: None,
        });
        Ok(())
    }
    pub fn song_state(&self) -> Option<SongTransportState> {
        self.song
            .owners
            .last()
            .map(|o| o.transport.state())
            .or(self.song.last_state)
    }
    pub fn pop_song_notice(&mut self) -> Option<SongNotice> {
        self.song.notices.pop_front()
    }
    fn song_notice(&mut self, notice: SongNotice) -> Result<(), Failure> {
        if self.song.notices.len() >= NOTICE_LIMIT {
            return Err(song_failure("song publication queue full"));
        }
        self.song.notices.push_back(notice);
        Ok(())
    }
    fn dispatch_owned_song_ack(&mut self, ack: SongHostAck) -> Result<(), SongHostAck> {
        if self.song.receive_clock(ack) {
            return Ok(());
        }
        if let Some(p) = &mut self.song.preparing {
            if p.epoch == ack.epoch()
                && (p.owner.receive(ack).is_ok()
                    || matches!(ack, SongHostAck::SliceAccepted { .. }))
            {
                return Ok(());
            }
        }
        for owner in &mut self.song.owners {
            if owner.transport.epoch() == ack.epoch() {
                if let SongHostAck::Muted(mute) = ack {
                    if let Some(pending) = self.song.mutes.iter_mut().find(|p| {
                        p.epoch == mute.epoch && p.muted == mute.muted && mute.frame >= p.frame
                    }) {
                        if let Some(family) = pending.families.iter_mut().find(|f| {
                            f.instrument == mute.instrument && f.posted && !f.acknowledged
                        }) {
                            for record in &mut owner.families {
                                if record.instrument == mute.instrument {
                                    record.muted = mute.muted;
                                }
                            }
                            family.acknowledged = true;
                            pending.application_frame = pending.application_frame.max(mute.frame);
                            return Ok(());
                        }
                    }
                    return Err(ack);
                }
                return owner.transport.receive(ack);
            }
        }
        Err(ack)
    }
    pub(crate) fn owns_song_resources(&self) -> bool {
        self.song.preparing.is_some() || self.song.ready.is_some() || !self.song.owners.is_empty()
    }
    pub(super) fn song_enabled(&self) -> bool {
        self.song.preparing.is_some()
            || self.song.ready.is_some()
            || !self.song.owners.is_empty()
            || self.song.request.is_some()
    }
    pub(super) fn drain_owned_song_messages(
        &mut self,
        rep: &mut TickReport,
        messages: &mut Vec<HostMsg>,
    ) {
        if let Some(ack) = self.pending_song_ack.take() {
            if !self.retain_song_ack(ack, rep) {
                return;
            }
        }
        if self.song.deferred.is_empty() {
            let mut incoming = Vec::new();
            self.hosts.audio.drain(&mut incoming);
            self.song.deferred.extend(incoming);
        }
        // Retain at most one actual host drain batch until every POD is dispatched.
        while let Some(message) = self.song.deferred.pop_front() {
            match message {
                HostMsg::Song(ack) => {
                    if let Err(ack) = self.dispatch_owned_song_ack(ack) {
                        if !self.retain_song_ack(ack, rep) {
                            break;
                        }
                    }
                }
                other => messages.push(other),
            }
        }
    }

    fn fail_song_clock(&mut self, error: ClockProgressFailure, rep: &mut TickReport) {
        let ClockProgressFailure {
            epoch: target,
            failure,
        } = error;
        if self.song.owners.is_empty() {
            self.song.last_state = Some(SongTransportState::Failed);
        }
        if self
            .song
            .ready
            .as_ref()
            .is_some_and(|ready| target.is_none_or(|epoch| epoch == ready.prepared().epoch()))
        {
            let ready = self.song.ready.take().expect("matching Ready was checked");
            let epoch = ready.prepared().epoch();
            let revision = ready.prepared().revision();
            let _ = self.song_notice(SongNotice::Failed {
                epoch,
                revision,
                failure: failure.clone(),
            });
            self.song.preparing = Some(Preparing {
                owner: SongHostPreparation::retire(ready),
                previous: self.song.ready_previous.take(),
                work: self.song.ready_work,
                epoch,
                revision,
                reported: true,
                invalidation: None,
            });
        }
        for owner in &mut self.song.owners {
            if target.is_none_or(|epoch| epoch == owner.transport.epoch()) {
                owner.transport.abort(failure.clone());
            }
        }
        rep.faults.push(failure);
    }
    pub(super) fn tick_song(&mut self, rep: &mut TickReport) {
        if !self.song_enabled() {
            return;
        }
        if let Some(mut p) = self.song.preparing.take() {
            if let Some(failure) = p.invalidation.as_ref() {
                if self
                    .song_notice(SongNotice::Failed {
                        epoch: p.epoch,
                        revision: p.revision,
                        failure: failure.clone(),
                    })
                    .is_err()
                {
                    self.song.preparing = Some(p);
                    return;
                }
                p.invalidation = None;
                p.reported = true;
            }
            if let Err(failure) = p.owner.submit(self.hosts.audio.as_mut()) {
                if self.song.owners.is_empty() {
                    self.song.last_state = Some(SongTransportState::Failed);
                }
                if !p.reported {
                    let _ = self.song_notice(SongNotice::Failed {
                        epoch: p.epoch,
                        revision: p.revision,
                        failure,
                    });
                    p.reported = true;
                }
            }
            match p.owner.progress() {
                SongPreparationProgress::Ready => match p.owner.take_ready() {
                    Ok(ready) => {
                        let _ = self.song_notice(SongNotice::Ready {
                            epoch: p.epoch,
                            revision: p.revision,
                        });
                        self.song.ready_previous = p.previous;
                        self.song.ready_work = p.work;
                        self.song.ready = Some(ready);
                    }
                    Err(failure) => {
                        rep.faults.push(failure);
                        self.song.preparing = Some(p);
                    }
                },
                SongPreparationProgress::Cancelled | SongPreparationProgress::Failed => {
                    if self.song.owners.is_empty() {
                        self.song.last_state = Some(SongTransportState::Failed);
                    }
                    if p.owner.take_cancelled().is_err() {
                        self.song.preparing = Some(p);
                    }
                }
                _ => self.song.preparing = Some(p),
            }
        }
        // Preparation owns its barrier stream. Continue consuming any existing
        // runtime request, but start no competing refresh until it completes.
        let refresh_epoch = if self.song.preparing.is_some() {
            None
        } else {
            self.song
                .ready
                .as_ref()
                .map(|r| r.prepared().epoch())
                .or_else(|| self.song.owners.last().map(|o| o.transport.epoch()))
        };
        if let Some(ready) = &self.song.ready {
            self.song.nonce = self.song.nonce.max(ready.clock_request_floor());
        }
        if let Err(failure) = self
            .song
            .refresh_clock(self.hosts.audio.as_mut(), refresh_epoch)
        {
            self.fail_song_clock(failure, rep);
        }
        let Some(clock) = self.song.clock else {
            return;
        };
        if self
            .song
            .ready_previous
            .is_none_or(|previous| !self.song.mutes.iter().any(|m| m.epoch == previous))
        {
            if let Some(ready) = self.song.ready.take() {
                if let Err(refusal) = self.start_ready(ready, clock) {
                    self.retire_failed_ready(refusal);
                }
            }
        }
        self.drive_replacements(rep);
        self.drive_song_mutes();
        for owner in &mut self.song.owners {
            if owner.replacement.is_some()
                && !owner.activation_posted
                && owner.transport.failure().is_none()
            {
                continue;
            }
            if !owner.activation_posted && owner.replacement.is_none() {
                match owner.transport.submit_activation(self.hosts.audio.as_mut()) {
                    Ok(()) => owner.activation_posted = true,
                    Err(refusal) => {
                        match refusal.error {
                            SongSubmitError::Backpressure => {}
                            SongSubmitError::Invalid(failure) => owner.transport.abort(failure),
                            SongSubmitError::Unavailable => {
                                owner.transport.abort(song_failure("song host unavailable"))
                            }
                        }
                        if owner.transport.state() != SongTransportState::Failed {
                            continue;
                        }
                    }
                }
            }
            let settings = owner.transport.settings();
            let horizon = song_horizon(clock, owner.activation, self.cfg.lookahead, settings);
            match horizon.and_then(|end| {
                let end = owner.cutoff_cycle.map_or(end, |cutoff| end.min(cutoff));
                owner.offered = owner.offered.max(end);
                owner.transport.advance(
                    self.hosts.audio.as_mut(),
                    clock,
                    TimeSpan::new(Ratio64::ZERO, end)?,
                )
            }) {
                Ok(progress) => rep.committed += progress.committed,
                Err(failure) => {
                    owner.transport.abort(failure.clone());
                    rep.faults.push(failure);
                }
            }
            if !owner.applied_reported && owner.replacement.as_ref().is_none_or(|p| p.committed) {
                if let Some(actual) = owner.transport.applied_activation() {
                    let carried = owner.replacement.as_ref().map_or(0, |p| p.carried.len());
                    if owner.replacement.as_ref().is_some_and(|p| p.invalidated) {
                        owner.applied_reported = true;
                    } else if self
                        .song
                        .notices
                        .len()
                        .checked_add(carried + 1)
                        .is_some_and(|n| n <= NOTICE_LIMIT)
                    {
                        self.song.notices.push_back(SongNotice::Applied {
                            ack: SongApplyAck {
                                epoch: actual.epoch,
                                doc_revision: owner.revision,
                                application_frame: actual.frame,
                            },
                            catalog: Rc::clone(&owner.catalog),
                        });
                        if let Some(pending) = &owner.replacement {
                            for sound in &pending.carried {
                                self.song.notices.push_back(SongNotice::CarriedMute {
                                    epoch: actual.epoch,
                                    sound: sound.clone(),
                                    muted: true,
                                    frame: actual.frame,
                                });
                            }
                        }
                        owner.applied_reported = true;
                    }
                }
            }
            if !owner.failure_reported {
                if let Some(failure) = owner.transport.failure() {
                    if self.song.notices.len() < NOTICE_LIMIT {
                        self.song.notices.push_back(SongNotice::Failed {
                            epoch: owner.transport.epoch(),
                            revision: owner.revision,
                            failure: failure.clone(),
                        });
                        owner.failure_reported = true;
                    }
                }
            }
        }
        let active_epoch = self
            .song
            .owners
            .iter()
            .rev()
            .find(|o| {
                o.transport.applied_activation().is_some()
                    && matches!(
                        o.transport.state(),
                        SongTransportState::Playing | SongTransportState::Draining
                    )
            })
            .map(|o| o.transport.epoch());
        for pending in &self.song.mutes {
            if self.song.notices.len() >= NOTICE_LIMIT {
                break;
            }
            if active_epoch != Some(pending.epoch)
                || (pending.failed && pending.families.iter().all(|f| !f.posted || f.acknowledged))
            {
                self.song.notices.push_back(SongNotice::MuteFailed {
                    request: pending.request,
                    epoch: pending.epoch,
                });
            } else if pending.families.iter().all(|f| f.acknowledged) {
                self.song.notices.push_back(SongNotice::Muted {
                    request: pending.request,
                    epoch: pending.epoch,
                    muted: pending.muted,
                    frame: pending.application_frame,
                });
            }
        }
        self.song.mutes.retain(|p| {
            !self.song.notices.iter().any(|n| {
                matches!(n,
            SongNotice::Muted { request, epoch, .. } | SongNotice::MuteFailed { request, epoch }
                if *request == p.request && *epoch == p.epoch)
            })
        });
        for owner in &mut self.song.owners {
            let state = owner.transport.state();
            if owner.published_state != Some(state)
                && owner.applied_reported
                && self.song.notices.len() < NOTICE_LIMIT
            {
                let mut families = Vec::new();
                for record in &owner.families {
                    if !families.contains(&record.sound) {
                        families.push(record.sound.clone());
                    }
                }
                self.song.notices.push_back(SongNotice::State {
                    epoch: owner.transport.epoch(),
                    state,
                    families,
                });
                owner.published_state = Some(state);
            }
        }
        for owner in &self.song.owners {
            self.song.last_state = Some(owner.transport.state());
        }
        let promised: Vec<_> = self
            .song
            .owners
            .iter()
            .filter(|o| self.retain_promised_old(o.transport.epoch()))
            .map(|o| o.transport.epoch())
            .collect();
        self.song.owners.retain_mut(|o| {
            if promised.contains(&o.transport.epoch())
                || o.replacement.as_ref().is_some_and(|p| !p.definitive())
            {
                true
            } else {
                match o.transport.state() {
                    SongTransportState::Ended => {
                        o.published_state != Some(SongTransportState::Ended)
                            || o.transport.take_retired().is_err()
                    }
                    SongTransportState::Failed => {
                        o.transport.take_retired().is_err() && o.transport.take_cancelled().is_err()
                    }
                    _ => true,
                }
            }
        });
    }
}
fn song_failure(message: &str) -> Failure {
    Failure::new(FailCode::BeyondCapability, message)
}
fn future_frame(clock: SongHostClock, seconds: f64) -> Result<u64, Failure> {
    if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {
        return Err(song_failure("invalid finite song lead"));
    }
    let microseconds = (seconds * 1_000_000.).ceil();
    #[allow(clippy::cast_possible_truncation)]
    // Bounded nonnegative control duration, never clock recovery.
    let duration = Ratio64::new(microseconds as i64, 1_000_000)?;
    clock
        .frame
        .checked_add(SongLimits::default().frames_at(duration, clock.sample_rate)?)
        .ok_or_else(|| song_failure("song lead frame overflow"))
}
fn song_horizon(
    clock: SongHostClock,
    activation: u64,
    lookahead: f64,
    settings: crate::song::SongSettings,
) -> Result<Ratio64, Failure> {
    let frame = future_frame(clock, lookahead)?;
    let relative = i64::try_from(frame.saturating_sub(activation))
        .map_err(|_| song_failure("song relative frame overflow"))?;
    Ratio64::new(relative, i64::from(clock.sample_rate))?.checked_div(settings.seconds_per_cycle()?)
}
