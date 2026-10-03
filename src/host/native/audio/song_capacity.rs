//! Host-thread capacity refresh provenance; no callbacks access this ledger.
use super::NativeAudioHost;
use crate::dsp::ring::{NativeRecord, Producer};
use crate::host::caps::AudioHost;
use crate::host::wire::{CtlMsg, HostMsg};
use crate::song::routing::{
    SongAnalysisCapacity, SongCapacityReport, SongCommand, SongHostAck, SongHostCapacities,
    SongRejectCode,
};
use crate::song::SnapshotEpoch;
use crate::vm::fail::Failure;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapacityQueryOrigin {
    Internal { epoch: SnapshotEpoch },
    Public { epoch: SnapshotEpoch },
}
impl CapacityQueryOrigin {
    fn epoch(self) -> SnapshotEpoch {
        match self {
            Self::Internal { epoch } | Self::Public { epoch } => epoch,
        }
    }
}

pub(super) struct SongCapacityCache {
    report: Option<SongCapacityReport>,
    origins: VecDeque<CapacityQueryOrigin>,
    origin_limit: usize,
    enabled: bool,
    next: u64,
    unsent: Option<SnapshotEpoch>,
    internal_inflight: bool,
    dirty: bool,
}
impl SongCapacityCache {
    pub(super) fn new(
        control_slots: usize,
        ack_slots: usize,
        pending_results: usize,
        configured: bool,
    ) -> Self {
        let bound = control_slots
            .checked_add(ack_slots)
            .and_then(|n| n.checked_add(pending_results));
        Self {
            report: None,
            origins: VecDeque::with_capacity(bound.unwrap_or(0)),
            origin_limit: bound.unwrap_or(0),
            enabled: configured && bound.is_some(),
            next: 0,
            unsent: None,
            internal_inflight: false,
            dirty: false,
        }
    }
    pub(super) fn request(&mut self, controls: &mut Producer<NativeRecord>) {
        self.invalidate();
        self.retry(controls);
    }
    pub(super) fn invalidate(&mut self) {
        self.report = None;
        self.dirty = true;
    }
    pub(super) fn retry(&mut self, controls: &mut Producer<NativeRecord>) {
        if !self.enabled
            || self.internal_inflight
            || !self.dirty
            || self.origins.len() >= self.origin_limit
        {
            return;
        }
        let epoch = if let Some(epoch) = self.unsent {
            epoch
        } else {
            let Some(next) = self.next.checked_add(1) else {
                self.enabled = false;
                return;
            };
            self.next = next;
            let epoch = SnapshotEpoch(next);
            self.unsent = Some(epoch);
            epoch
        };
        if controls
            .push(NativeRecord::Msg(CtlMsg::Song(
                SongCommand::RequestCapacity(epoch),
            )))
            .is_ok()
        {
            self.origins
                .push_back(CapacityQueryOrigin::Internal { epoch });
            self.unsent = None;
            self.internal_inflight = true;
            self.dirty = false;
        }
    }
    // Return the original inline owned record without allocating an error Box.
    #[allow(clippy::result_large_err)]
    pub(super) fn post(
        &mut self,
        controls: &mut Producer<NativeRecord>,
        record: NativeRecord,
    ) -> Result<(), NativeRecord> {
        let public = match &record {
            NativeRecord::Msg(CtlMsg::Song(SongCommand::RequestCapacity(epoch))) => Some(*epoch),
            _ => None,
        };
        if public.is_some() && self.origins.len() >= self.origin_limit {
            return Err(record);
        }
        let mutates = match &record {
            NativeRecord::Msg(CtlMsg::Song(SongCommand::RequestCapacity(_))) => false,
            NativeRecord::Msg(
                CtlMsg::Song(_)
                | CtlMsg::GraphInstall { .. }
                | CtlMsg::GraphRetire { .. }
                | CtlMsg::SampleRetire { .. },
            ) => true,
            NativeRecord::Install(_) | NativeRecord::SongInstall(_) => true,
            _ => false,
        };
        controls.push(record)?;
        if let Some(epoch) = public {
            self.origins
                .push_back(CapacityQueryOrigin::Public { epoch });
        }
        if mutates {
            self.request(controls);
        }
        Ok(())
    }
    pub(super) fn observe(&mut self, message: HostMsg) -> Option<HostMsg> {
        let result_epoch = match message {
            HostMsg::Song(SongHostAck::CapacityReport(report)) => Some(report.epoch),
            HostMsg::Song(SongHostAck::CapacityRejected { epoch, .. }) => Some(epoch),
            _ => None,
        };
        if let Some(epoch) = result_epoch {
            let Some(origin) = self.origins.front().copied() else {
                return Some(message);
            };
            if origin.epoch() != epoch {
                self.report = None;
                return Some(message);
            }
            self.origins.pop_front();
            if matches!(origin, CapacityQueryOrigin::Internal { .. }) {
                self.internal_inflight = false;
                match message {
                    HostMsg::Song(SongHostAck::CapacityReport(report)) if !self.dirty => {
                        self.report = Some(report);
                    }
                    HostMsg::Song(SongHostAck::CapacityRejected { .. }) => {
                        self.report = None;
                    }
                    _ => {
                        self.report = None;
                    }
                }
                return None;
            }
        }
        if matches!(
            message,
            HostMsg::Retired { .. }
                | HostMsg::Installed { .. }
                | HostMsg::Song(
                    SongHostAck::LeaseReturned(_)
                        | SongHostAck::PreparationCancelled(_)
                        | SongHostAck::ResourceReady { .. }
                        | SongHostAck::ResourceRetired { .. }
                )
        ) {
            self.invalidate();
        }
        Some(message)
    }
    pub(super) fn capacities(&self) -> Result<SongHostCapacities, SongRejectCode> {
        if self.dirty || self.internal_inflight || self.unsent.is_some() {
            return Err(SongRejectCode::NotReady);
        }
        self.report
            .map(|r| r.available)
            .ok_or(SongRejectCode::NotReady)
    }
    pub(super) fn analysis(&self) -> Result<SongAnalysisCapacity, SongRejectCode> {
        self.capacities()?;
        self.report
            .map(|r| r.analysis)
            .ok_or(SongRejectCode::NotReady)
    }
}

impl NativeAudioHost {
    pub(super) fn request_song_capacity(&mut self) {
        self.song_capacity.request(&mut self.controls);
    }
    /// Last matching measured callback report; no engine access or fabricated defaults.
    /// # Errors
    /// No current report after a staging submission.
    pub fn song_remaining_capacities(
        &self,
    ) -> Result<crate::song::routing::SongHostCapacities, Failure> {
        self.song_capacity.capacities().map_err(|_| {
            Failure::new(
                crate::vm::fail::FailCode::HostUnavailable,
                "song capacity report is pending",
            )
        })
    }
    /// Actual private analyzer storage, separate from control cells.
    /// # Errors
    /// Missing current report.
    pub fn song_analysis_capacity(
        &self,
    ) -> Result<crate::song::routing::SongAnalysisCapacity, Failure> {
        self.song_capacity.analysis().map_err(|_| {
            Failure::new(
                crate::vm::fail::FailCode::HostUnavailable,
                "song analysis capacity report is pending",
            )
        })
    }
    /// Submit preparation without treating enqueue success as Ready.
    /// # Errors
    /// Bounded control transport is full.
    pub fn submit_song_preparation(
        &mut self,
        p: crate::song::routing::SongStagePreparation,
    ) -> Result<(), Failure> {
        self.song_capacity
            .post(
                &mut self.controls,
                NativeRecord::Msg(CtlMsg::Song(
                    crate::song::routing::SongCommand::BeginStaging(p),
                )),
            )
            .map_err(|_| {
                Failure::new(
                    crate::vm::fail::FailCode::HostUnavailable,
                    "song preparation control queue is full",
                )
            })?;
        Ok(())
    }
    /// Native resource ownership remains recoverable on enqueue failure.
    /// # Errors
    /// Returns the original native payload when the bounded queue is full.
    pub fn submit_song_native(
        &mut self,
        install: crate::dsp::ring::NativeSongInstall,
    ) -> Result<(), crate::dsp::ring::NativeSongInstall> {
        <Self as AudioHost>::submit_song_native(self, install)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::ring::SpscRing;
    fn report(epoch: u64) -> HostMsg {
        HostMsg::Song(SongHostAck::CapacityReport(SongCapacityReport {
            epoch: SnapshotEpoch(epoch),
            serial: epoch,
            available: SongHostCapacities {
                sample_rate: 48000,
                ..SongHostCapacities::default()
            },
            analysis: SongAnalysisCapacity { slots: 4 },
        }))
    }
    fn public(epoch: u64) -> NativeRecord {
        NativeRecord::Msg(CtlMsg::Song(SongCommand::RequestCapacity(SnapshotEpoch(
            epoch,
        ))))
    }
    #[test]
    fn exact_unsent_epoch_and_public_owner_survive_full_control_ring() {
        let (mut tx, mut rx) = SpscRing::split(1);
        let mut cache = SongCapacityCache::new(tx.capacity(), 1, 2, true);
        assert!(tx
            .push(NativeRecord::Msg(CtlMsg::SampleRetire { resource: 7 }))
            .is_ok());
        cache.request(&mut tx);
        assert_eq!(cache.unsent, Some(SnapshotEpoch(1)));
        assert!(cache.origins.is_empty());
        let rejected = cache.post(&mut tx, public(u64::MAX)).err().unwrap();
        assert!(matches!(
            rejected,
            NativeRecord::Msg(CtlMsg::Song(SongCommand::RequestCapacity(SnapshotEpoch(
                u64::MAX
            ))))
        ));
        rx.pop();
        cache.retry(&mut tx);
        assert!(matches!(
            rx.pop(),
            Some(NativeRecord::Msg(CtlMsg::Song(
                SongCommand::RequestCapacity(SnapshotEpoch(1))
            )))
        ));
        assert_eq!(cache.next, 1);
        assert_eq!(cache.origins.len(), 1);
        assert!(cache.unsent.is_none());
    }
    #[test]
    fn fifo_same_epoch_public_failure_and_unrelated_rejection_are_lossless() {
        let (mut tx, mut rx) = SpscRing::split(8);
        let mut cache = SongCapacityCache::new(tx.capacity(), 8, 2, true);
        assert!(cache.post(&mut tx, public(1)).is_ok());
        cache.request(&mut tx);
        assert!(cache.post(&mut tx, public(1)).is_ok());
        let unrelated = HostMsg::Song(SongHostAck::Rejected {
            epoch: SnapshotEpoch(1),
            reason: SongRejectCode::NotReady,
        });
        assert_eq!(cache.observe(unrelated), Some(unrelated));
        assert_eq!(cache.origins.len(), 3);
        let failure = HostMsg::Song(SongHostAck::CapacityRejected {
            epoch: SnapshotEpoch(1),
            reason: SongRejectCode::Capacity,
        });
        assert_eq!(cache.observe(failure), Some(failure));
        assert_eq!(cache.observe(failure), None);
        assert_eq!(cache.observe(failure), Some(failure));
        assert!(cache.origins.is_empty());
        while rx.pop().is_some() {}
        cache.retry(&mut tx);
        assert!(rx.pop().is_none());
        assert!(cache.capacities().is_err());
    }
    #[test]
    fn dirty_mutations_coalesce_and_stale_reply_cannot_restore_freshness() {
        let (mut tx, mut rx) = SpscRing::split(32);
        let mut cache = SongCapacityCache::new(tx.capacity(), 8, 2, true);
        let allocated = cache.origins.capacity();
        cache.request(&mut tx);
        for id in 0..12 {
            assert!(cache
                .post(&mut tx, NativeRecord::Msg(CtlMsg::GraphRetire { id }))
                .is_ok());
        }
        assert_eq!(cache.origins.len(), 1);
        let mut queries = 0;
        while let Some(record) = rx.pop() {
            if matches!(
                record,
                NativeRecord::Msg(CtlMsg::Song(SongCommand::RequestCapacity(_)))
            ) {
                queries += 1;
            }
        }
        assert_eq!(queries, 1);
        assert_eq!(cache.observe(report(1)), None);
        assert!(cache.capacities().is_err());
        cache.retry(&mut tx);
        assert!(matches!(
            rx.pop(),
            Some(NativeRecord::Msg(CtlMsg::Song(
                SongCommand::RequestCapacity(SnapshotEpoch(2))
            )))
        ));
        assert_eq!(cache.observe(report(2)), None);
        assert_eq!(cache.capacities().unwrap().sample_rate, 48000);
        cache.retry(&mut tx);
        assert!(rx.pop().is_none());
        assert_eq!(cache.origins.capacity(), allocated);
    }
    #[test]
    fn ledger_admission_precedes_push_and_checked_exhaustion_is_honest() {
        let (mut tx, mut rx) = SpscRing::split(1);
        let mut cache = SongCapacityCache::new(1, 1, 1, true);
        for epoch in 1..=3 {
            assert!(cache.post(&mut tx, public(epoch)).is_ok());
            rx.pop();
        }
        assert_eq!(cache.origins.len(), 3);
        assert!(cache.post(&mut tx, public(4)).is_err());
        assert!(rx.pop().is_none());
        let rejected = HostMsg::Song(SongHostAck::CapacityRejected {
            epoch: SnapshotEpoch(1),
            reason: SongRejectCode::NotReady,
        });
        assert_eq!(cache.observe(rejected), Some(rejected));
        assert!(cache.post(&mut tx, public(4)).is_ok());
        let mut exhausted = SongCapacityCache::new(1, 1, 1, true);
        exhausted.next = u64::MAX;
        exhausted.request(&mut tx);
        assert!(!exhausted.enabled);
        assert!(exhausted.capacities().is_err());
        let mut overflow = SongCapacityCache::new(usize::MAX, 1, 2, true);
        overflow.request(&mut tx);
        assert!(!overflow.enabled);
        assert!(overflow.capacities().is_err());
    }
    #[test]
    fn failure_retains_one_post_probe_mutation_refresh_without_retry_storm() {
        let (mut tx, mut rx) = SpscRing::split(8);
        let mut cache = SongCapacityCache::new(tx.capacity(), 8, 2, true);
        cache.request(&mut tx);
        assert!(cache
            .post(&mut tx, NativeRecord::Msg(CtlMsg::GraphRetire { id: 1 }))
            .is_ok());
        while rx.pop().is_some() {}
        assert_eq!(
            cache.observe(HostMsg::Song(SongHostAck::CapacityRejected {
                epoch: SnapshotEpoch(1),
                reason: SongRejectCode::Capacity
            })),
            None
        );
        cache.retry(&mut tx);
        assert!(matches!(
            rx.pop(),
            Some(NativeRecord::Msg(CtlMsg::Song(
                SongCommand::RequestCapacity(SnapshotEpoch(2))
            )))
        ));
        assert!(rx.pop().is_none());
        assert_eq!(
            cache.observe(HostMsg::Song(SongHostAck::CapacityRejected {
                epoch: SnapshotEpoch(2),
                reason: SongRejectCode::Capacity
            })),
            None
        );
        cache.retry(&mut tx);
        assert!(rx.pop().is_none());
        assert!(cache.capacities().is_err());
    }
    fn drive_capacity_engine(
        engine: &mut crate::dsp::engine::Engine,
        controls: &mut crate::dsp::ring::Consumer<NativeRecord>,
        events: &mut crate::dsp::ring::EventConsumer,
        acks: &mut crate::dsp::ring::AckProducer,
        cells: &mut crate::dsp::cells::AtomicCells,
    ) {
        engine.process(
            &mut crate::dsp::engine::EngineIo {
                events,
                controls,
                acks,
                cells,
                garbage: None,
            },
            &mut [0.; 32],
            16,
        );
    }
    #[test]
    fn actual_engine_control_and_ack_pressure_retains_complete_query_origins() {
        use crate::dsp::arena::StoreKind;
        use crate::dsp::caps::CapabilitySet;
        use crate::dsp::cells::AtomicCells;
        use crate::dsp::engine::{Engine, SongStagingConfig};
        use crate::dsp::ring::EventRing;
        for configured in [true, false] {
            let mut caps = CapabilitySet::browser();
            caps.max_voices = 1;
            let mut engine = Engine::new(&caps, 48000., 16, StoreKind::NativeArc);
            let (mut tx, mut controls) = SpscRing::split(1);
            let (mut acks, mut replies) = SpscRing::split(1);
            let (_events_tx, mut events) = EventRing::split(1);
            let mut cells = AtomicCells::new(1);
            if configured {
                engine
                    .configure_song_staging_for_transport(
                        SongStagingConfig {
                            preparations: 1,
                            leases: 4,
                            branches: 1,
                            control_slots: 1,
                            analysis_slots: 1,
                            native_pcm_bytes: 128,
                            critical_receipts: 1,
                        },
                        &acks,
                    )
                    .unwrap();
            }
            // The second pass tests a real failure result, not native startup probing.
            let mut cache = SongCapacityCache::new(tx.capacity(), acks.capacity(), 2, true);
            let allocated = cache.origins.capacity();
            assert!(cache.post(&mut tx, public(1)).is_ok());
            drive_capacity_engine(
                &mut engine,
                &mut controls,
                &mut events,
                &mut acks,
                &mut cells,
            );
            assert_eq!(acks.len(), acks.capacity());
            assert!(cache.post(&mut tx, public(1)).is_ok());
            cache.request(&mut tx);
            assert_eq!(tx.len(), tx.capacity());
            assert_eq!(cache.unsent, Some(SnapshotEpoch(1)));
            assert_eq!(cache.origins.len(), 2);
            assert!(cache.capacities().is_err());
            drive_capacity_engine(
                &mut engine,
                &mut controls,
                &mut events,
                &mut acks,
                &mut cells,
            );
            assert_eq!(acks.len(), acks.capacity());
            assert_eq!(tx.len(), 0);
            cache.retry(&mut tx);
            assert_eq!(cache.next, 1);
            assert!(cache.unsent.is_none());
            assert_eq!(cache.origins.len(), 3);
            drive_capacity_engine(
                &mut engine,
                &mut controls,
                &mut events,
                &mut acks,
                &mut cells,
            );
            // Full ACK retains the second reply and prevents internal query consumption.
            assert_eq!(tx.len(), tx.capacity());
            assert!(cache.capacities().is_err());
            let first = replies.pop().expect("first actual public result");
            assert_eq!(cache.observe(first), Some(first));
            drive_capacity_engine(
                &mut engine,
                &mut controls,
                &mut events,
                &mut acks,
                &mut cells,
            );
            assert_eq!(tx.len(), 0);
            let second = replies.pop().expect("retained actual public result");
            assert_eq!(cache.observe(second), Some(second));
            assert!(cache.capacities().is_err());
            drive_capacity_engine(
                &mut engine,
                &mut controls,
                &mut events,
                &mut acks,
                &mut cells,
            );
            let internal = replies.pop().expect("retained actual internal result");
            assert_eq!(cache.observe(internal), None);
            assert!(cache.origins.is_empty());
            assert!(replies.pop().is_none());
            cache.retry(&mut tx);
            assert_eq!(tx.len(), 0);
            assert_eq!(cache.origins.capacity(), allocated);
            if configured {
                for (message, serial) in [(first, 1), (second, 2), (internal, 3)] {
                    let HostMsg::Song(SongHostAck::CapacityReport(report)) = message else {
                        panic!("real report")
                    };
                    assert_eq!(report.epoch, SnapshotEpoch(1));
                    assert_eq!(report.serial, serial);
                }
                assert_eq!(
                    cache.capacities().unwrap(),
                    engine.song_remaining_capacities().unwrap()
                );
                assert_eq!(
                    cache.analysis().unwrap(),
                    engine.song_analysis_capacity().unwrap()
                );
            } else {
                let failure = HostMsg::Song(SongHostAck::CapacityRejected {
                    epoch: SnapshotEpoch(1),
                    reason: SongRejectCode::NotReady,
                });
                assert_eq!([first, second, internal], [failure; 3]);
                assert!(cache.capacities().is_err());
            }
        }
    }
}
