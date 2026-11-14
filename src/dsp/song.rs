//! Private finite-song runtime and exact retained bank ownership.
use crate::dsp::arena::SongResourceStager;
use crate::dsp::cells::{CellId, CellRead};
use crate::dsp::engine::SongFrameRegion;
use crate::song::routing::{SongLeaseKey, SongRejectCode, SongResourceKind};

impl SongResourceStager {
    pub(crate) fn bind_graph(
        &mut self,
        binding: crate::song::routing::SongGraphBanks,
    ) -> Result<(), SongRejectCode> {
        self.open(binding.graph.epoch)?;
        self.lease(binding.graph)?;
        if !crate::song::routing::SongCommand::BindGraphBanks(binding).valid()
            || self.bindings.iter().any(|b| b.graph == binding.graph)
        {
            return Err(SongRejectCode::Malformed);
        }
        for bank in [binding.controls, binding.analysis].into_iter().flatten() {
            self.lease(bank)?;
        }
        if self.bindings.len() == self.bindings.capacity() {
            return Err(SongRejectCode::Capacity);
        }
        self.bindings.push(binding);
        Ok(())
    }
    pub(crate) fn bound_cell(
        &self,
        graph: SongLeaseKey,
        logical: CellId,
    ) -> Result<CellId, SongRejectCode> {
        let bank = self
            .bindings
            .iter()
            .find(|b| b.graph == graph)
            .and_then(|b| b.controls)
            .ok_or(SongRejectCode::NotReady)?;
        self.physical_cell(bank, logical)
    }
    pub(crate) fn analysis_bank(
        &self,
        bank: SongLeaseKey,
    ) -> Result<SongFrameRegion, SongRejectCode> {
        self.lease(bank)?;
        let b = self
            .banks
            .iter()
            .find(|b| b.owner == bank)
            .ok_or(SongRejectCode::NotReady)?;
        Ok(SongFrameRegion {
            slot: bank.resource.id,
            offset: u64::try_from(b.offset).map_err(|_| SongRejectCode::Capacity)?,
            frames: u64::try_from(b.len).map_err(|_| SongRejectCode::Capacity)?,
        })
    }
}

impl SongResourceStager {
    pub(crate) fn finish_return(&mut self, key: SongLeaseKey) {
        self.leases.retain(|l| l.key != key);
        self.branches.retain(|b| {
            b.epoch != key.epoch
                || ![
                    Some((SongResourceKind::Instrument, b.instrument)),
                    b.private_fx.map(|r| (SongResourceKind::PrivateFx, r)),
                    b.track_template.map(|r| (SongResourceKind::Track, r)),
                    b.master.map(|r| (SongResourceKind::Master, r)),
                ]
                .into_iter()
                .flatten()
                .any(|(kind, resource)| kind == key.kind && resource == key.resource)
        });
        self.reusable.retain(|r| {
            self.branches
                .iter()
                .any(|b| b.epoch == r.epoch && b.branch == r.branch)
        });
        for cell in &mut self.cells {
            if cell.owner == Some(key) {
                cell.owner = None;
                cell.value = 0.0;
            }
        }
        for owner in &mut self.analysis_owners {
            if *owner == Some(key) {
                *owner = None;
            }
        }
        self.banks.retain(|b| b.owner != key);
        self.bindings
            .retain(|b| b.graph != key && b.controls != Some(key) && b.analysis != Some(key));
    }
}

use crate::dsp::arena::song::Cell;
use crate::song::routing::SongBranchId;
use crate::song::routing::{SongAudioEvent, SongBranchConfig, SongCommand, SongHostAck};
use crate::song::SnapshotEpoch;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SongVoiceOwner {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) branch: SongBranchId,
    pub(crate) generation: u32,
    pub(crate) instrument: SongLeaseKey,
}
#[derive(Clone, Copy)]
pub(crate) struct SongGainGate {
    pub(crate) from: f32,
    pub(crate) to: f32,
    pub(crate) start: u64,
    pub(crate) end: u64,
}
impl SongGainGate {
    pub(crate) const fn open() -> Self {
        Self {
            from: 1.0,
            to: 1.0,
            start: 0,
            end: 0,
        }
    }
    pub(crate) fn gain(self, frame: u64) -> f32 {
        if frame >= self.end {
            return self.to;
        }
        if frame <= self.start {
            return self.from;
        }
        #[allow(clippy::cast_precision_loss)]
        let t = (frame - self.start) as f32 / (self.end - self.start) as f32;
        self.from + (self.to - self.from) * t
    }
    pub(crate) fn retarget(&mut self, frame: u64, target: f32) -> Result<(), SongRejectCode> {
        let end = frame.checked_add(64).ok_or(SongRejectCode::Malformed)?;
        *self = Self {
            from: self.gain(frame),
            to: target,
            start: frame,
            end,
        };
        Ok(())
    }
    pub(crate) fn tail(frame: u64, start: u64, deadline: u64) -> f32 {
        let fade = deadline.saturating_sub(64).max(start);
        Self {
            from: 1.0,
            to: 0.0,
            start: fade,
            end: deadline,
        }
        .gain(frame)
    }
}
#[derive(Clone, Copy)]
pub(crate) struct SongEpochEnd {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) arrangement: u64,
    pub(crate) deadline: u64,
    pub(crate) closed: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct RuntimeBranch {
    pub(crate) config: SongBranchConfig,
    pub(crate) template: usize,
    pub(crate) fx: Option<usize>,
    pub(crate) track: usize,
    pub(crate) muted: bool,
    pub(crate) ended: bool,
    pub(crate) gate: SongGainGate,
    pub(crate) release_frame: Option<u64>,
    pub(crate) closed: bool,
}
impl RuntimeBranch {
    pub(crate) fn matches(self, event: SongAudioEvent) -> bool {
        self.config.epoch == event.epoch
            && self.config.branch == event.branch
            && self.config.generation == event.generation
    }
    pub(crate) fn fx_key(self) -> Result<SongLeaseKey, SongRejectCode> {
        Ok(SongLeaseKey {
            epoch: self.config.epoch,
            resource: self.config.private_fx.ok_or(SongRejectCode::NotReady)?,
            kind: SongResourceKind::PrivateFx,
        })
    }
    pub(crate) fn owner(self) -> SongVoiceOwner {
        SongVoiceOwner {
            epoch: self.config.epoch,
            branch: self.config.branch,
            generation: self.config.generation,
            instrument: SongLeaseKey {
                epoch: self.config.epoch,
                resource: self.config.instrument,
                kind: SongResourceKind::Instrument,
            },
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct SongTimedCommand {
    pub(crate) command: SongCommand,
    pub(crate) frame: u64,
    pub(crate) failure: Option<SongRejectCode>,
}
#[derive(Clone, Copy)]
pub(crate) struct RuntimeTrack {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) track: usize,
    pub(crate) master: usize,
    pub(crate) track_key: SongLeaseKey,
    pub(crate) master_key: SongLeaseKey,
}
impl RuntimeTrack {
    pub(crate) fn owned_track(self, buses: &crate::dsp::bus::BusGraph) -> bool {
        buses.song_is_live(self.track_key, self.track)
    }
    pub(crate) fn owned_master(self, buses: &crate::dsp::bus::BusGraph) -> bool {
        buses.song_is_live(self.master_key, self.master)
    }
    pub(crate) fn renderable(
        self,
        buses: &crate::dsp::bus::BusGraph,
        frame: u64,
        endpoint: Option<&SongEpochEnd>,
    ) -> Result<bool, SongRejectCode> {
        if endpoint.is_some_and(|e| frame >= e.deadline) {
            return Ok(false);
        }
        if !self.owned_track(buses) || !self.owned_master(buses) {
            return Err(SongRejectCode::StaleEpoch);
        }
        Ok(true)
    }
}
#[derive(Clone, Copy)]
pub(crate) struct SongReusableSlot {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) branch: SongBranchId,
    pub(crate) last_generation: u32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SongReplacementPhase {
    Queued,
    Armed,
    Applied,
    FailedAfterArm,
    Rejected,
}
#[derive(Clone, Copy)]
pub(crate) struct SongAppliedOwner {
    pub(crate) activation: crate::song::routing::SongActivation,
    pub(crate) endpoint: Option<crate::song::routing::SongEndpoints>,
    pub(crate) musically_closed: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct SongReplacementOutcome {
    pub(crate) acknowledgment: SongHostAck,
    pub(crate) frame: u64,
    pub(crate) before_boundary_commit: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct SongReplacementRecord {
    pub(crate) command: crate::song::routing::SongReplacement,
    pub(crate) phase: SongReplacementPhase,
    pub(crate) previous: SongAppliedOwner,
    pub(crate) outcomes: [Option<SongReplacementOutcome>; 2],
}
/// All capacity is allocated before processing with the real staging tables.
pub(crate) struct SongRuntime {
    pub(crate) primed_mutes: Vec<crate::song::routing::SongInitialMute>,
    pub(crate) replacements: Vec<SongReplacementRecord>,
    pub(crate) last_applied_owner: Option<SongAppliedOwner>,
    pub(crate) reusable: Vec<SongReusableSlot>,
    pub(crate) active: Vec<SnapshotEpoch>,
    pub(crate) endpoints: Vec<SongEpochEnd>,
    pub(crate) endpoint_reservations: Vec<SnapshotEpoch>,
    pub(crate) branches: Vec<RuntimeBranch>,
    pub(crate) tracks: Vec<RuntimeTrack>,
    pub(crate) leases: Vec<SongLeaseKey>,
    pub(crate) voices: Box<[Option<SongVoiceOwner>]>,
    pub(crate) commands: Vec<SongTimedCommand>,
    pub(crate) receipts: Vec<SongHostAck>,
}
impl SongRuntime {
    pub(crate) fn new(
        preparations: usize,
        branches: usize,
        leases: usize,
        voices: usize,
        commands: usize,
        receipts: usize,
    ) -> Result<Self, SongRejectCode> {
        if preparations == 0
            || branches == 0
            || leases == 0
            || voices == 0
            || commands == 0
            || receipts == 0
        {
            return Err(SongRejectCode::Capacity);
        }
        Ok(Self {
            primed_mutes: Vec::with_capacity(branches),
            replacements: Vec::with_capacity(preparations),
            last_applied_owner: None,
            reusable: Vec::with_capacity(branches),
            active: Vec::with_capacity(preparations),
            endpoints: Vec::with_capacity(preparations),
            endpoint_reservations: Vec::with_capacity(preparations),
            branches: Vec::with_capacity(branches),
            tracks: Vec::with_capacity(branches),
            leases: Vec::with_capacity(leases),
            voices: vec![None; voices].into_boxed_slice(),
            commands: Vec::with_capacity(commands),
            receipts: Vec::with_capacity(receipts),
        })
    }
    pub(crate) fn has_pending_receipts(&self) -> bool {
        !self.receipts.is_empty()
            || self
                .replacements
                .iter()
                .any(|x| x.outcomes.iter().any(Option::is_some))
    }
    pub(crate) fn has_endpoint_obligation(&self, epoch: SnapshotEpoch) -> bool {
        self.endpoints.iter().any(|e| e.epoch == epoch)
            || self.endpoint_reservations.contains(&epoch)
    }
    pub(crate) fn reserve_endpoint(&mut self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode> {
        if self.has_endpoint_obligation(epoch) {
            return Ok(());
        }
        if self
            .endpoints
            .len()
            .checked_add(self.endpoint_reservations.len())
            .is_none_or(|used| used >= self.endpoints.capacity())
        {
            return Err(SongRejectCode::Capacity);
        }
        self.endpoint_reservations.push(epoch);
        Ok(())
    }
    /// Existing/reserved ownership is authenticated by admission/F preflight.
    /// Transfers its single debit; callers cannot perform a fresh A allocation.
    pub(crate) fn commit_endpoint(&mut self, endpoint: SongEpochEnd) {
        if let Some(existing) = self
            .endpoints
            .iter_mut()
            .find(|e| e.epoch == endpoint.epoch)
        {
            *existing = endpoint;
        } else {
            self.release_endpoint_reservation(endpoint.epoch);
            self.endpoints.push(endpoint);
        }
    }
    pub(crate) fn release_endpoint_reservation(&mut self, epoch: SnapshotEpoch) {
        self.endpoint_reservations
            .retain(|reserved| *reserved != epoch);
    }
    pub(crate) fn replacement_gain(&self, epoch: SnapshotEpoch, frame: u64) -> f32 {
        let mut gain = 1.0;
        for record in &self.replacements {
            if matches!(
                record.phase,
                SongReplacementPhase::Queued | SongReplacementPhase::Rejected
            ) {
                continue;
            }
            let a = record.command.activation;
            if epoch == a.epoch {
                gain *= SongGainGate {
                    from: 0.0,
                    to: 1.0,
                    start: a.frame,
                    end: a.frame + 64,
                }
                .gain(frame);
            } else if epoch == record.command.previous && !record.previous.musically_closed {
                gain *= SongGainGate {
                    from: 1.0,
                    to: 0.0,
                    start: a.frame - 64,
                    end: a.frame,
                }
                .gain(frame);
            }
        }
        gain
    }
    pub(crate) fn reusable_slot(
        &self,
        epoch: SnapshotEpoch,
        branch: SongBranchId,
    ) -> Option<SongReusableSlot> {
        self.reusable
            .iter()
            .find(|s| s.epoch == epoch && s.branch == branch)
            .copied()
    }
    pub(crate) fn command_rank(command: SongCommand) -> u8 {
        match command {
            SongCommand::Activate(_) => 0,
            SongCommand::Release(_) | SongCommand::Endpoints(_) => 1,
            SongCommand::RebindBranch(_) => 2,
            SongCommand::Event(_) => 4,
            _ => 3,
        }
    }
    pub(crate) fn owns_epoch(&self, epoch: SnapshotEpoch) -> bool {
        self.active.contains(&epoch)
    }
    pub(crate) fn owns_lease(&self, key: SongLeaseKey) -> bool {
        self.leases.contains(&key)
    }
    pub(crate) fn queue(&mut self, command: SongCommand, now: u64) -> Result<(), SongRejectCode> {
        let frame = match command {
            SongCommand::RebindBranch(r) => r.transition_frame,
            SongCommand::Activate(a) => a.frame,
            SongCommand::Event(e) => e.frame,
            SongCommand::Mute(m) => m.frame.max(now),
            SongCommand::Release(r) => r.frame,
            SongCommand::Endpoints(e) => e.arrangement,
            _ => return Err(SongRejectCode::Malformed),
        };
        if frame < now || !command.valid() {
            return Err(SongRejectCode::Malformed);
        }
        if self.commands.len() == self.commands.capacity() {
            return Err(SongRejectCode::Capacity);
        }
        // Stable order within each rank; transitions precede events at the same frame.
        let rank = Self::command_rank(command);
        let index = self
            .commands
            .iter()
            .position(|c| (c.frame, Self::command_rank(c.command)) > (frame, rank))
            .unwrap_or(self.commands.len());
        self.commands.insert(
            index,
            SongTimedCommand {
                command,
                frame,
                failure: None,
            },
        );
        Ok(())
    }
}

pub(crate) struct SongPrivateCells<'a> {
    cells: &'a [Cell],
    bank: Option<SongLeaseKey>,
}
impl CellRead for SongPrivateCells<'_> {
    fn get(&self, cell: CellId) -> f32 {
        self.cells
            .get(cell.index())
            .filter(|c| c.owner.is_some() && c.owner == self.bank)
            .map_or(0.0, |c| c.value)
    }
}
pub(crate) struct SongRenderCells<'a, C: CellRead + ?Sized> {
    pub(crate) legacy: &'a C,
    pub(crate) song: Option<SongPrivateCells<'a>>,
}
impl<C: CellRead + ?Sized> CellRead for SongRenderCells<'_, C> {
    fn get(&self, cell: CellId) -> f32 {
        self.song
            .as_ref()
            .map_or_else(|| self.legacy.get(cell), |s| s.get(cell))
    }
}
impl SongResourceStager {
    pub(crate) fn private_cells(&self, graph: SongLeaseKey) -> SongPrivateCells<'_> {
        SongPrivateCells {
            cells: &self.cells,
            bank: self
                .bindings
                .iter()
                .find(|b| b.graph == graph)
                .and_then(|b| b.controls),
        }
    }
    pub(crate) fn graph_views(
        &mut self,
        graph: SongLeaseKey,
    ) -> Result<(SongPrivateCells<'_>, &mut [f32]), SongRejectCode> {
        let binding = self.bindings.iter().find(|b| b.graph == graph).copied();
        let (offset, len) = match binding.and_then(|b| b.analysis) {
            Some(owner) => {
                let b = self
                    .banks
                    .iter()
                    .find(|b| b.owner == owner)
                    .ok_or(SongRejectCode::NotReady)?;
                (b.offset, b.len)
            }
            None => (0, 0),
        };
        let cells = SongPrivateCells {
            cells: &self.cells,
            bank: binding.and_then(|b| b.controls),
        };
        let end = offset.checked_add(len).ok_or(SongRejectCode::Capacity)?;
        let analysis = self
            .analysis
            .get_mut(offset..end)
            .ok_or(SongRejectCode::Capacity)?;
        Ok((cells, analysis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::arena::StoreKind;
    use crate::dsp::bus::BusTemplate;
    use crate::dsp::caps::CapabilitySet;
    use crate::dsp::cells::AtomicCells;
    use crate::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
    use crate::dsp::graph::{
        AnalyzerKind, BusId, Edge, EffectKind, EffectSpec, InstDef, InstId, UGenSpec,
    };
    use crate::dsp::ring::{
        EngineIo, EventRing, NativeInstall, NativeRecord, NativeSongInstall, SpscRing,
    };
    use crate::dsp::ugen::{Template, AMP};
    use crate::host::wire::{AudioEvent, Ctl, CtlMsg};
    use crate::sched::slots::SlotId;
    use crate::song::routing::*;

    #[test]
    fn endpoint_debits_are_unique_transfer_without_growth_and_refuse_one_short() {
        let a = SnapshotEpoch(11);
        let b = SnapshotEpoch(12);
        let c = SnapshotEpoch(13);
        let mut exact = SongRuntime::new(2, 1, 1, 1, 4, 1).unwrap();
        let capacity = exact.endpoints.capacity();
        exact.reserve_endpoint(a).unwrap();
        exact.reserve_endpoint(a).unwrap();
        exact.reserve_endpoint(b).unwrap();
        assert_eq!(exact.endpoint_reservations, vec![a, b]);
        assert_eq!(exact.reserve_endpoint(c), Err(SongRejectCode::Capacity));
        exact.commit_endpoint(SongEpochEnd {
            epoch: a,
            arrangement: 0,
            deadline: 0,
            closed: false,
        });
        assert_eq!(
            exact.endpoints.len() + exact.endpoint_reservations.len(),
            capacity
        );
        assert_eq!(exact.endpoint_reservations, vec![b]);
        assert_eq!(exact.reserve_endpoint(c), Err(SongRejectCode::Capacity));
        exact.release_endpoint_reservation(b);
        exact.reserve_endpoint(c).unwrap();
        exact.commit_endpoint(SongEpochEnd {
            epoch: c,
            arrangement: 1,
            deadline: 1,
            closed: false,
        });
        assert_eq!(exact.endpoints.capacity(), capacity);
        assert!(exact.endpoint_reservations.is_empty());
        let mut short = SongRuntime::new(1, 1, 1, 1, 4, 1).unwrap();
        short.reserve_endpoint(a).unwrap();
        assert_eq!(short.reserve_endpoint(b), Err(SongRejectCode::Capacity));
        assert_eq!(short.endpoint_reservations, vec![a]);
    }

    #[test]
    fn late_mute_preserves_original_command_and_other_timed_commands_stay_strict() {
        let mut runtime = SongRuntime::new(1, 1, 1, 1, 8, 1).unwrap();
        let epoch = SnapshotEpoch(1);
        let mute = SongCommand::Mute(SongMute {
            epoch,
            instrument: 1,
            muted: true,
            frame: 10,
        });
        runtime.queue(mute, 20).unwrap();
        assert_eq!(runtime.commands[0].command, mute);
        assert_eq!(runtime.commands[0].frame, 20);
        let late = [
            SongCommand::Activate(SongActivation { epoch, frame: 10 }),
            SongCommand::Endpoints(SongEndpoints {
                epoch,
                arrangement: 10,
                tail_deadline: 30,
            }),
            SongCommand::Release(SongBranchRelease {
                epoch,
                branch: SongBranchId(0),
                generation: 1,
                frame: 10,
                tail_deadline: 30,
            }),
            SongCommand::RebindBranch(SongBranchRebind {
                epoch,
                branch: SongBranchId(0),
                expected_generation: 1,
                generation: 2,
                transition_frame: 10,
                tail_deadline: 30,
            }),
            SongCommand::Event(SongAudioEvent {
                epoch,
                branch: SongBranchId(0),
                generation: 1,
                frame: 10,
                event: AudioEvent::new(0., SlotId::new(0), 1, InstId::new(1)),
            }),
        ];
        for command in late {
            assert!(command.valid());
            assert_eq!(runtime.queue(command, 20), Err(SongRejectCode::Malformed));
        }
        let mut invalid = AudioEvent::new(0., SlotId::new(0), 1, InstId::new(1));
        invalid.push_ctl(AMP, Ctl::Const(f32::NAN)).unwrap();
        assert_eq!(
            runtime.queue(
                SongCommand::Event(SongAudioEvent {
                    epoch,
                    branch: SongBranchId(0),
                    generation: 1,
                    frame: 30,
                    event: invalid,
                }),
                20
            ),
            Err(SongRejectCode::Malformed)
        );
        assert_eq!(runtime.commands.len(), 1);
        assert_eq!(runtime.commands[0].command, mute);
    }

    #[test]
    fn actual_private_analyzer_writes_owner_bank_without_legacy_alias() {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 2;
        let mut config = EngineConfig::new(&caps, 48000., 16, StoreKind::NativeArc);
        config.template_slots = 2;
        config.bus_slots = 4;
        config.voice_seconds = 0.1;
        let rate = 48000u32;
        let room_frames = crate::dsp::effects::mem_len(EffectKind::Room, 48000., &caps);
        let delay_frames = usize::try_from(rate)
            .unwrap()
            .checked_mul(4)
            .unwrap()
            .checked_add(4)
            .unwrap();
        let private_frames = room_frames
            .checked_add(delay_frames.checked_mul(2).unwrap())
            .unwrap();
        let constructor_frames = crate::dsp::granular::effect_mem_len(48000., &caps);
        let additional_frames = private_frames.saturating_sub(constructor_frames);
        // Engine adds this real granular pool to secs(bus_seconds), per slot.
        // One frame bounds rounding; the maintained region check is authoritative.
        config.bus_seconds = additional_frames.checked_add(1).unwrap() as f32 / rate as f32;
        let mut engine = Engine::with_config(config);
        let (mut acks, mut received) = SpscRing::split(8);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 1,
                    leases: 8,
                    branches: 1,
                    control_slots: 2,
                    analysis_slots: 64,
                    native_pcm_bytes: 0,
                    critical_receipts: 2,
                },
                &acks,
            )
            .unwrap();
        let key = |id, kind| SongLeaseKey {
            epoch: SnapshotEpoch(2),
            resource: SongResourceRef { id, generation: 3 },
            kind,
        };
        let owners = [
            key(1, SongResourceKind::Instrument),
            key(2, SongResourceKind::PrivateFx),
            key(3, SongResourceKind::Track),
            key(4, SongResourceKind::Master),
            key(5, SongResourceKind::ControlCells),
            key(6, SongResourceKind::AnalysisBank),
        ];
        engine
            .begin_song_preparation(SongStagePreparation {
                preparation: SongPreparation {
                    epoch: SnapshotEpoch(2),
                    branches: 1,
                    resources: 6,
                    required: SongHostCapacities {
                        sample_rate: 48000,
                        ..SongHostCapacities::default()
                    },
                },
                analysis_required: SongAnalysisCapacity { slots: 64 },
            })
            .unwrap();
        for owner in owners {
            engine
                .reserve_song_resource(SongResourceReservation {
                    epoch: owner.epoch,
                    resource: owner.resource,
                    kind: owner.kind,
                })
                .unwrap();
        }
        let (mut tx, mut controls) = SpscRing::split(8);
        let (_event_tx, mut events) = EventRing::split(2);
        let mut cells = AtomicCells::new(64);
        assert!(cells.set(CellId::new(0), 99.));
        for command in [
            SongCommand::InitCells(SongCellInit {
                lease: owners[4],
                cell: CellId::new(42),
                value: 0.6,
            }),
            SongCommand::ReserveAnalysis(SongAnalysisReservation {
                lease: owners[5],
                slots: 64,
            }),
            SongCommand::BindGraphBanks(SongGraphBanks {
                graph: owners[0],
                controls: Some(owners[4]),
                analysis: Some(owners[5]),
            }),
        ] {
            tx.push(NativeRecord::Msg(CtlMsg::Song(command)))
                .ok()
                .unwrap();
        }
        let mut output = [0.; 32];
        engine.process(
            &mut EngineIo {
                events: &mut events,
                controls: &mut controls,
                acks: &mut acks,
                cells: &mut cells,
                garbage: None,
            },
            &mut output,
            16,
        );
        while received.pop().is_some() {}
        let kind = EffectKind::Analyzer(AnalyzerKind::Level);
        let definition = InstDef {
            id: InstId::new(55),
            params: vec![(AMP, Ctl::Cell(CellId::new(42)))].into_boxed_slice(),
            nodes: vec![
                UGenSpec::SinOsc,
                UGenSpec::Effect(EffectSpec {
                    kind,
                    params: vec![(
                        crate::dsp::effects::param_ctl(kind, "id").unwrap(),
                        Ctl::Const(0.),
                    )]
                    .into_boxed_slice(),
                }),
            ]
            .into_boxed_slice(),
            edges: vec![Edge {
                from: 0,
                to: 1,
                port: 0,
                output: 0,
            }]
            .into_boxed_slice(),
            node_params: Box::new([]),
        };
        engine
            .stage_song_native(NativeSongInstall {
                lease: owners[0],
                payload: NativeInstall::Inst {
                    resource: 1,
                    gen: 3,
                    template: Template::from_inst(&definition, &engine.build_env()).unwrap(),
                },
            })
            .ok()
            .unwrap();
        let available_regions = engine.song_bus_regions().to_vec();
        assert!(
            available_regions
                .iter()
                .any(|region| region.frames >= private_frames as u64),
            "actual free individual region fits full Room and stereo delay"
        );
        for owner in owners[1..4].iter().copied() {
            let mut template = Box::new(BusTemplate::new());
            template.bus = BusId::new(owner.resource.id);
            engine
                .stage_song_native(NativeSongInstall {
                    lease: owner,
                    payload: NativeInstall::Bus {
                        resource: owner.resource.id,
                        gen: 3,
                        master: owner.kind == SongResourceKind::Master,
                        template,
                    },
                })
                .ok()
                .unwrap();
        }
        let layout = engine.song_private_delay_layout(owners[1]).unwrap();
        let actual_slot = engine
            .buses()
            .slots
            .iter()
            .position(|slot| {
                slot.resource == owners[1].resource.id
                    && slot.gen == owners[1].resource.generation
                    && slot.state == crate::dsp::bus::SlotState::Staged
            })
            .unwrap();
        assert_eq!(layout.owner, owners[1]);
        assert_eq!(layout.left.slot as usize, actual_slot);
        assert_eq!(layout.right.slot, layout.left.slot);
        assert!(actual_slot > 0);
        assert_eq!(layout.room.offset, 0);
        assert_eq!(layout.room.frames, room_frames as u64);
        assert_eq!(layout.chain_frames, 0);
        assert_eq!(layout.left.offset, layout.room.frames);
        assert_eq!(layout.left.frames, delay_frames as u64);
        assert_eq!(layout.right.offset, layout.left.offset + layout.left.frames);
        assert_eq!(layout.right.frames, delay_frames as u64);
        assert_eq!(
            layout.storage_frames,
            available_regions
                .iter()
                .find(|region| region.slot == layout.left.slot)
                .unwrap()
                .frames
        );
        assert!(layout.right.offset + layout.right.frames <= layout.storage_frames);
        assert_eq!(layout.send_frames_per_channel, 16);
        engine
            .stage_song_branch(SongBranchConfig {
                epoch: SnapshotEpoch(2),
                branch: SongBranchId(0),
                generation: 4,
                family: 0,
                track: 3,
                instrument: owners[0].resource,
                private_fx: Some(owners[1].resource),
                track_template: Some(owners[2].resource),
                master: Some(owners[3].resource),
                transition_frame: 0,
                tail_deadline: u64::MAX,
            })
            .unwrap();
        engine.seal_song_preparation(SnapshotEpoch(2)).unwrap();
        engine
            .activate_song(SongActivation {
                epoch: SnapshotEpoch(2),
                frame: 16,
            })
            .unwrap();
        tx.push(NativeRecord::Msg(CtlMsg::Song(SongCommand::Event(
            SongAudioEvent {
                epoch: SnapshotEpoch(2),
                branch: SongBranchId(0),
                generation: 4,
                frame: 16,
                event: AudioEvent::new(0., SlotId::new(0), 1, InstId::new(55)),
            },
        ))))
        .ok()
        .unwrap();
        engine.process(
            &mut EngineIo {
                events: &mut events,
                controls: &mut controls,
                acks: &mut acks,
                cells: &mut cells,
                garbage: None,
            },
            &mut output,
            16,
        );
        assert!(output.iter().any(|v| v.abs() > 0.001));
        assert!(engine.song_analysis_test_value(owners[5], 0).unwrap() > 0.);
        assert!(engine.analysis().iter().all(|value| *value == 0.));
        assert!(engine
            .song_analysis_test_value(key(6, SongResourceKind::ControlCells), 0)
            .is_err());
    }
}
