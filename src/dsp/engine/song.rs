//! Construction-preallocated, epoch-owned silent song resource staging.
use super::Engine;

use crate::song::routing::{
    SongAnalysisCapacity, SongBranchConfig, SongLeaseKey, SongRejectCode, SongResourceKind,
    SongStagePreparation,
};
use crate::song::SnapshotEpoch;

/// One actual per-slot contiguous float region; PCM byte conversion is separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongFrameRegion {
    pub slot: u32,
    pub offset: u64,
    pub frames: u64,
}
/// Ownership survives staging, retirement and delayed acknowledgments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongLeaseState {
    pub key: SongLeaseKey,
    pub staged: bool,
    pub retiring: bool,
}
/// Physical pools and bounded protocol tables allocated before processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongStagingConfig {
    pub preparations: u32,
    pub leases: u32,
    pub branches: u32,
    pub control_slots: u32,
    pub analysis_slots: u32,
    pub native_pcm_bytes: u64,
    pub critical_receipts: u32,
}
pub use crate::dsp::arena::SongResourceStager;
impl Engine {
    /// Configure only before processing, with a measured transport provider already installed.
    /// # Errors
    /// Missing provider or late construction.
    pub fn configure_song_staging(
        &mut self,
        config: SongStagingConfig,
    ) -> Result<(), SongRejectCode> {
        let measured = self
            .song_stager
            .as_ref()
            .map(|s| s.actual.ack_slots)
            .ok_or(SongRejectCode::NotReady)?;
        self.configure_song_staging_with_transport(config, measured)
    }
    /// Remaining private analysis cells, independent of legacy telemetry storage.
    /// # Errors
    /// Staging is unconfigured.
    pub fn song_analysis_capacity(&self) -> Result<SongAnalysisCapacity, SongRejectCode> {
        Ok(self
            .song_stager
            .as_ref()
            .ok_or(SongRejectCode::NotReady)?
            .remaining_analysis_capacity())
    }
    #[must_use]
    pub fn song_voice_regions(&self) -> &[SongFrameRegion] {
        self.song_stager
            .as_ref()
            .map_or(&[], SongResourceStager::voice_regions)
    }
}

use crate::dsp::arena::{decode_graph, FaultCode, GraphKind, InstallRequest, ResourceKind};
use crate::dsp::bus::SlotState;
use crate::dsp::ring::{AckProducer, Garbage, NativeInstall, NativeSongInstall, Producer, Record};
use crate::host::wire::{CtlMsg, HostMsg};
use crate::song::routing::{SongCapacityReport, SongCommand, SongHostAck};

impl Engine {
    fn song_key_open(&self, key: SongLeaseKey) -> Result<usize, SongRejectCode> {
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        stager.open(key.epoch)?;
        let index = stager.lease(key)?;
        if stager.leases[index].staged {
            return Err(SongRejectCode::Malformed);
        }
        Ok(index)
    }
    fn song_adopted(&mut self, key: SongLeaseKey) -> Result<(), SongRejectCode> {
        let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
        let index = stager.lease(key)?;
        stager.leases[index].staged = true;
        Ok(())
    }
    /// Stage ownership without activating any instrument, bus, master or sample.
    /// # Errors
    /// Returns the original owner when identity, initialized cells or physical fit fails.
    pub fn stage_song_native(
        &mut self,
        install: NativeSongInstall,
    ) -> Result<(), NativeSongInstall> {
        if !install.valid() || self.song_key_open(install.lease).is_err() {
            return Err(install);
        }
        let NativeSongInstall { lease, payload } = install;
        let result = match payload {
            NativeInstall::Sample {
                resource,
                gen,
                data,
            } => {
                if self.song_validation.is_some() {
                    return Err(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Sample {
                            resource,
                            gen,
                            data,
                        },
                    });
                }
                let request = InstallRequest {
                    resource,
                    kind: ResourceKind::Sample,
                    frames: data.frames.len() / usize::from(data.channels.max(1)),
                    channels: data.channels,
                    rate: data.rate,
                    bytes: 0,
                    origin: None,
                };
                if data.frames.len() % usize::from(data.channels.max(1)) != 0
                    || self.store.song_reserve(lease, request).is_err()
                {
                    return Err(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Sample {
                            resource,
                            gen,
                            data,
                        },
                    });
                }
                self.song_validation = Some((
                    NativeSongInstall {
                        lease,
                        payload: NativeInstall::Sample {
                            resource,
                            gen,
                            data,
                        },
                    },
                    0,
                ));
                return Ok(());
            }
            NativeInstall::Inst {
                resource,
                gen,
                mut template,
            } => {
                if self.map_song_template(lease, &mut template).is_err() {
                    return Err(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Inst {
                            resource,
                            gen,
                            template,
                        },
                    });
                }
                let Some(slot) = self
                    .templates
                    .slots
                    .iter_mut()
                    .find(|s| s.state == SlotState::Free && s.t.is_none())
                else {
                    return Err(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Inst {
                            resource,
                            gen,
                            template,
                        },
                    });
                };
                template.resource = resource;
                template.gen = gen;
                slot.t = Some(template);
                slot.state = SlotState::Staged;
                slot.song = Some(lease);
                Ok(())
            }
            NativeInstall::Bus {
                resource,
                gen,
                master,
                mut template,
            } => {
                if self.map_song_bus(lease, &mut template).is_err() {
                    return Err(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Bus {
                            resource,
                            gen,
                            master,
                            template,
                        },
                    });
                }
                let Some(stager) = self.song_stager.as_ref() else {
                    return Err(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Bus {
                            resource,
                            gen,
                            master,
                            template,
                        },
                    });
                };
                self.buses
                    .stage_song_bus(lease, template, stager, self.sr, &self.cfg.caps)
                    .map_err(|template| NativeSongInstall {
                        lease,
                        payload: NativeInstall::Bus {
                            resource,
                            gen,
                            master,
                            template,
                        },
                    })
            }
        };
        result?;
        // All preceding checks authenticated this lease; retain ownership on invariant failure.
        if self.song_adopted(lease).is_err() {
            self.fault(FaultCode::BadResource, lease.resource.id);
        }
        Ok(())
    }
    fn map_song_template(
        &self,
        key: SongLeaseKey,
        template: &mut crate::dsp::ugen::Template,
    ) -> Result<(), SongRejectCode> {
        self.song_stager
            .as_ref()
            .ok_or(SongRejectCode::NotReady)?
            .map_template(key, template, &self.store, self.cfg.output_channels)
    }
    fn map_song_bus(
        &self,
        key: SongLeaseKey,
        template: &mut crate::dsp::bus::BusTemplate,
    ) -> Result<(), SongRejectCode> {
        template.remap_song_banks(
            key,
            self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?,
            &self.store,
        )
    }
    /// Exact private control address for an authenticated bank.
    /// # Errors
    /// Missing bank or uninitialized logical reference.
    pub fn song_control_cell(
        &self,
        bank: SongLeaseKey,
        logical: crate::dsp::cells::CellId,
    ) -> Result<crate::dsp::cells::CellId, SongRejectCode> {
        self.song_stager
            .as_ref()
            .ok_or(SongRejectCode::NotReady)?
            .physical_cell(bank, logical)
    }
    /// Copied initialized private value; legacy AtomicCells never participate.
    /// # Errors
    /// Foreign or uninitialized bank reference.
    pub fn song_control_value(
        &self,
        bank: SongLeaseKey,
        logical: crate::dsp::cells::CellId,
    ) -> Result<f32, SongRejectCode> {
        let s = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let cell = s.physical_cell(bank, logical)?;
        Ok(crate::dsp::cells::CellRead::get(s, cell))
    }
    /// Exact owner-local analysis allocation; analyzer IDs remain logical.
    /// # Errors
    /// Unknown or uninitialized bank.
    pub fn song_analysis_region(
        &self,
        bank: SongLeaseKey,
    ) -> Result<SongFrameRegion, SongRejectCode> {
        self.song_stager
            .as_ref()
            .ok_or(SongRejectCode::NotReady)?
            .analysis_bank(bank)
    }
    /// Actual allocator extents after every legacy/song allocation and release.
    #[must_use]
    pub fn song_pcm_extents(&self) -> &[SongFrameRegion] {
        self.store.song_free_extents()
    }
    /// Configure exact adopted branch references, without making them audible.
    /// # Errors
    /// Missing resources, foreign generations or more branches than admitted.
    pub fn stage_song_branch(&mut self, branch: SongBranchConfig) -> Result<(), SongRejectCode> {
        self.validate_song_private_exclusivity(branch)?;
        let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
        stager.open(branch.epoch)?;
        if branch.tail_deadline < branch.transition_frame
            || stager
                .branches
                .iter()
                .any(|b| b.epoch == branch.epoch && b.branch == branch.branch)
        {
            return Err(SongRejectCode::Malformed);
        }
        for (kind, reference) in [
            (SongResourceKind::Instrument, Some(branch.instrument)),
            (SongResourceKind::PrivateFx, branch.private_fx),
            (SongResourceKind::Track, branch.track_template),
            (SongResourceKind::Master, branch.master),
        ] {
            if let Some(resource) = reference {
                let index = stager.lease(SongLeaseKey {
                    epoch: branch.epoch,
                    resource,
                    kind,
                })?;
                if !stager.leases[index].staged {
                    return Err(SongRejectCode::NotReady);
                }
            }
        }
        let p = stager
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == branch.epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if stager.branches.len() == stager.branches.capacity()
            || stager
                .branches
                .iter()
                .filter(|b| b.epoch == branch.epoch)
                .count()
                >= p.command.preparation.branches as usize
        {
            return Err(SongRejectCode::Capacity);
        }
        stager.branches.push(branch);
        Ok(())
    }
    /// Ready means all admitted resources and branches were physically adopted.
    /// # Errors
    /// Any incomplete resource, branch or cell/analysis bank.
    pub fn seal_song_preparation(&mut self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode> {
        let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
        stager.open(epoch)?;
        let p = stager
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if stager
            .leases
            .iter()
            .filter(|l| l.key.epoch == epoch && l.staged && !l.retiring)
            .count()
            != p.command.preparation.resources as usize
            || stager.branches.iter().filter(|b| b.epoch == epoch).count()
                != p.command.preparation.branches as usize
        {
            return Err(SongRejectCode::NotReady);
        }
        stager.preparation_mut(epoch)?.ready = true;
        Ok(())
    }
    /// Cancellation starts an ownership/receipt cursor; no live epoch is affected.
    /// # Errors
    /// Unconfigured or foreign epoch.
    pub fn cancel_song_preparation(&mut self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode> {
        let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
        stager.preparation_mut(epoch)?.cancelling = true;
        for lease in &mut stager.leases {
            if lease.key.epoch == epoch {
                lease.retiring = true;
            }
        }
        Ok(())
    }
    fn critical_song(&mut self, acks: &mut AckProducer, ack: SongHostAck) {
        self.song_ack = acks.push_critical(HostMsg::Song(ack)).err();
    }
    fn song_command(
        &mut self,
        command: SongCommand,
    ) -> Result<Option<SongHostAck>, SongRejectCode> {
        if !command.valid() {
            return Err(SongRejectCode::Malformed);
        }
        match command {
            SongCommand::BindGraphBanks(binding) => {
                self.song_stager
                    .as_mut()
                    .ok_or(SongRejectCode::NotReady)?
                    .bind_graph(binding)?;
                Ok(None)
            }
            SongCommand::RequestClock(request) => {
                if !self.sr.is_finite()
                    || self.sr.fract() != 0.0
                    || !(8000.0..=192000.0).contains(&self.sr)
                {
                    return Err(SongRejectCode::Malformed);
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let sample_rate = self.sr as u32; // Exact integral bounded rate validated above.
                Ok(Some(SongHostAck::ClockReport(
                    crate::song::routing::SongClockReport {
                        request,
                        clock: crate::song::routing::SongHostClock {
                            frame: self.frame,
                            sample_rate,
                        },
                    },
                )))
            }
            SongCommand::RequestCapacity(epoch) => {
                let available = self.song_remaining_capacities()?;
                let analysis = self.song_analysis_capacity()?;
                let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
                stager.serial = stager
                    .serial
                    .checked_add(1)
                    .ok_or(SongRejectCode::Capacity)?;
                Ok(Some(SongHostAck::CapacityReport(SongCapacityReport {
                    epoch,
                    serial: stager.serial,
                    available,
                    analysis,
                })))
            }
            SongCommand::BeginStaging(p) => {
                self.begin_song_preparation(p)?;
                Ok(None)
            }
            SongCommand::BeginPreparation(preparation) => {
                self.begin_song_preparation(SongStagePreparation {
                    preparation,
                    analysis_required: SongAnalysisCapacity { slots: 0 },
                })?;
                Ok(None)
            }
            SongCommand::ReserveResource(r) => {
                self.reserve_song_resource(r)?;
                Ok(None)
            }
            SongCommand::InitCells(init) => {
                self.song_stager
                    .as_mut()
                    .ok_or(SongRejectCode::NotReady)?
                    .init_cell(init)?;
                Ok(None)
            }
            SongCommand::ReserveAnalysis(reservation) => {
                self.song_stager
                    .as_mut()
                    .ok_or(SongRejectCode::NotReady)?
                    .reserve_analysis(reservation)?;
                Ok(None)
            }
            SongCommand::RebindBranch(_) => {
                self.queue_song_runtime(command)?;
                Ok(None)
            }
            SongCommand::ConfigureReusableBranch(branch) => {
                self.stage_song_reusable_branch(branch)?;
                Ok(None)
            }
            SongCommand::ConfigureBranch(branch) => {
                self.stage_song_branch(branch)?;
                Ok(None)
            }
            SongCommand::SealPreparation(epoch) => {
                self.seal_song_preparation(epoch)?;
                Ok(Some(SongHostAck::Ready(epoch)))
            }
            SongCommand::CancelPreparation(epoch) => {
                self.cancel_song_preparation(epoch)?;
                Ok(None)
            }
            SongCommand::CancelLease(key) => {
                let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
                let index = stager.lease(key)?;
                stager.leases[index].retiring = true;
                Ok(None)
            }
            _ => Err(SongRejectCode::NotReady),
        }
    }
    pub(super) fn song_record(
        &mut self,
        record: Record<'_>,
        acks: &mut AckProducer,
        garbage: Option<&mut Producer<Garbage>>,
    ) {
        match record {
            Record::SongNative(install) => {
                let key = install.lease;
                let epoch = key.epoch;
                let valid = install.valid();
                match self.stage_song_native(install) {
                    Ok(()) => {
                        if self.song_validation.is_none() {
                            self.critical_song(
                                acks,
                                SongHostAck::ResourceReady {
                                    epoch,
                                    resource: key.resource,
                                },
                            );
                        }
                    }
                    Err(install) => {
                        self.pending_song_install = Some(install);
                        self.return_song_install(garbage);
                        self.critical_song(
                            acks,
                            SongHostAck::Rejected {
                                epoch,
                                reason: if valid {
                                    SongRejectCode::NotReady
                                } else {
                                    SongRejectCode::Malformed
                                },
                            },
                        );
                    }
                }
            }
            Record::Msg(CtlMsg::Song(command)) => match self.song_command(command) {
                Ok(Some(ack)) => self.critical_song(acks, ack),
                Ok(None) => {}
                Err(reason) => self.critical_song(
                    acks,
                    if let SongCommand::RequestClock(request) = command {
                        SongHostAck::ClockRejected(crate::song::routing::SongClockFailure {
                            request,
                            reason,
                        })
                    } else if matches!(command, SongCommand::RequestCapacity(_)) {
                        SongHostAck::CapacityRejected {
                            epoch: command.epoch(),
                            reason,
                        }
                    } else {
                        SongHostAck::Rejected {
                            epoch: command.epoch(),
                            reason,
                        }
                    },
                ),
            },
            Record::SongSampleBegin {
                lease,
                frames,
                channels,
                rate,
            } => {
                let request = InstallRequest {
                    resource: lease.resource.id,
                    kind: ResourceKind::Sample,
                    frames: frames as usize,
                    channels,
                    rate,
                    bytes: 0,
                    origin: None,
                };
                let result = self.song_key_open(lease).and_then(|_| {
                    self.store
                        .song_begin(lease, request)
                        .map_err(|_| SongRejectCode::Capacity)
                });
                if let Err(reason) = result {
                    self.critical_song(
                        acks,
                        SongHostAck::Rejected {
                            epoch: lease.epoch,
                            reason,
                        },
                    );
                } else if frames == 0 {
                    let _ = self.song_adopted(lease);
                    self.critical_song(
                        acks,
                        SongHostAck::ResourceReady {
                            epoch: lease.epoch,
                            resource: lease.resource,
                        },
                    );
                }
            }
            Record::SongSlice {
                lease,
                offset,
                data,
            } => {
                let result = self.song_key_open(lease).and_then(|_| {
                    self.store
                        .song_write_slice(lease, offset as usize, data)
                        .map_err(|_| SongRejectCode::Malformed)
                });
                match result {
                    Ok(done) => {
                        if done {
                            let _ = self.song_adopted(lease);
                            self.song_followup = Some(HostMsg::Song(SongHostAck::ResourceReady {
                                epoch: lease.epoch,
                                resource: lease.resource,
                            }));
                        }
                        self.critical_song(acks, SongHostAck::SliceAccepted { lease, offset });
                    }
                    Err(reason) => self.critical_song(
                        acks,
                        SongHostAck::Rejected {
                            epoch: lease.epoch,
                            reason,
                        },
                    ),
                }
            }
            Record::SongGraph { lease, bytes } => {
                let result = self.stage_song_graph(lease, bytes);
                match result {
                    Ok(()) => self.critical_song(
                        acks,
                        SongHostAck::ResourceReady {
                            epoch: lease.epoch,
                            resource: lease.resource,
                        },
                    ),
                    Err(reason) => self.critical_song(
                        acks,
                        SongHostAck::Rejected {
                            epoch: lease.epoch,
                            reason,
                        },
                    ),
                }
            }
            _ => {}
        }
    }
    fn stage_song_graph(
        &mut self,
        lease: SongLeaseKey,
        bytes: &[u8],
    ) -> Result<(), SongRejectCode> {
        self.song_key_open(lease)?;
        let kind = decode_graph(bytes, &mut self.raw, &mut self.bus_tmp)
            .map_err(|_| SongRejectCode::Malformed)?;
        match kind {
            GraphKind::Inst if lease.kind == SongResourceKind::Instrument => {
                let env = self.build_env();
                let slot = self
                    .templates
                    .slots
                    .iter()
                    .position(|s| s.state == SlotState::Free && s.t.is_some())
                    .ok_or(SongRejectCode::Capacity)?;
                let mut template = self.templates.slots[slot]
                    .t
                    .take()
                    .ok_or(SongRejectCode::Capacity)?;
                let result = template
                    .build(&self.raw, &env)
                    .map_err(|_| SongRejectCode::Malformed)
                    .and_then(|()| self.map_song_template(lease, &mut template));
                template.resource = lease.resource.id;
                template.gen = lease.resource.generation;
                self.templates.slots[slot].t = Some(template);
                result?;
                self.templates.slots[slot].state = SlotState::Staged;
                self.templates.slots[slot].song = Some(lease);
            }
            kind @ (GraphKind::Bus(_) | GraphKind::Master) => {
                if (kind == GraphKind::Master) != (lease.kind == SongResourceKind::Master) {
                    return Err(SongRejectCode::Malformed);
                }
                let mut template = *self.bus_tmp;
                self.map_song_bus(lease, &mut template)?;
                // Arena bus decoding uses the preallocated scratch; adopt into a free slot without moving a Box.
                let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
                self.buses
                    .stage_song_arena(lease, &template, stager, self.sr, &self.cfg.caps)?;
            }
            _ => return Err(SongRejectCode::Malformed),
        }
        self.song_adopted(lease)
    }
    pub(super) fn pump_song_staging(
        &mut self,
        acks: &mut AckProducer,
        mut garbage: Option<&mut Producer<Garbage>>,
    ) -> bool {
        if let Some((key, queued)) = self.song_return_key.take() {
            if queued {
                self.finish_song_return(key);
            } else {
                self.song_return_key = Some((key, true));
                self.critical_song(acks, SongHostAck::LeaseReturned(key));
                return false;
            }
        }
        if let Some((install, checked)) = self.song_validation.take() {
            let lease = install.lease;
            let NativeInstall::Sample { ref data, .. } = install.payload else {
                self.pending_song_install = Some(install);
                return false;
            };
            let end = checked
                .saturating_add(super::INSTALL_BYTES_PER_QUANTUM / 4)
                .min(data.frames.len());
            let invalid = data.frames[checked..end].iter().any(|x| !x.is_finite());
            if invalid {
                self.pending_song_install = Some(install);
                self.return_song_install(garbage.as_deref_mut());
                self.critical_song(
                    acks,
                    SongHostAck::Rejected {
                        epoch: lease.epoch,
                        reason: SongRejectCode::Malformed,
                    },
                );
                return false;
            }
            if end < data.frames.len() {
                self.song_validation = Some((install, end));
                return false;
            }
            let NativeInstall::Sample {
                resource,
                gen,
                data,
            } = install.payload
            else {
                return false;
            };
            match self.store.song_install_arc(lease, data) {
                Ok(()) => {
                    let _ = self.song_adopted(lease);
                    self.critical_song(
                        acks,
                        SongHostAck::ResourceReady {
                            epoch: lease.epoch,
                            resource: lease.resource,
                        },
                    );
                }
                Err(data) => {
                    self.pending_song_install = Some(NativeSongInstall {
                        lease,
                        payload: NativeInstall::Sample {
                            resource,
                            gen,
                            data,
                        },
                    });
                    self.return_song_install(garbage.as_deref_mut());
                    self.critical_song(
                        acks,
                        SongHostAck::Rejected {
                            epoch: lease.epoch,
                            reason: SongRejectCode::Capacity,
                        },
                    );
                }
            }
            return false;
        }
        let retiring = self
            .song_stager
            .as_ref()
            .and_then(|s| {
                s.leases
                    .iter()
                    .filter(|l| l.retiring)
                    .min_by_key(|l| match l.key.kind {
                        SongResourceKind::Instrument
                        | SongResourceKind::PrivateFx
                        | SongResourceKind::Track
                        | SongResourceKind::Master => 0,
                        _ => 1,
                    })
            })
            .map(|l| l.key);
        if let Some(key) = retiring {
            let owner = match key.kind {
                SongResourceKind::Sample => {
                    self.store
                        .song_take_arc(key)
                        .map(|data| NativeInstall::Sample {
                            resource: key.resource.id,
                            gen: key.resource.generation,
                            data,
                        })
                }
                SongResourceKind::Instrument => self
                    .templates
                    .slots
                    .iter_mut()
                    .find(|slot| slot.state == SlotState::Staged && slot.song == Some(key))
                    .and_then(|slot| {
                        if matches!(self.cfg.store, crate::dsp::arena::StoreKind::NativeArc) {
                            slot.t.take()
                        } else {
                            None
                        }
                    })
                    .map(|template| NativeInstall::Inst {
                        resource: key.resource.id,
                        gen: key.resource.generation,
                        template,
                    }),
                SongResourceKind::PrivateFx
                | SongResourceKind::Track
                | SongResourceKind::Master => {
                    self.buses
                        .song_take_template(key)
                        .map(|template| NativeInstall::Bus {
                            resource: key.resource.id,
                            gen: key.resource.generation,
                            master: key.kind == SongResourceKind::Master,
                            template,
                        })
                }
                SongResourceKind::ControlCells | SongResourceKind::AnalysisBank => None,
            };
            if let Some(payload) = owner {
                self.pending_song_install = Some(NativeSongInstall {
                    lease: key,
                    payload,
                });
            }
            self.song_return_key = Some((key, false));
            if !self.return_song_install(garbage) {
                return false;
            }
            self.song_return_key = Some((key, true));
            self.critical_song(acks, SongHostAck::LeaseReturned(key));
            return false;
        }
        if let Some(epoch) = self
            .song_stager
            .as_ref()
            .and_then(|s| {
                s.preparations.iter().find(|p| {
                    p.cancelling
                        && !s
                            .leases
                            .iter()
                            .any(|l| l.key.epoch == p.command.preparation.epoch)
                })
            })
            .map(|p| p.command.preparation.epoch)
        {
            if let Some(s) = self.song_stager.as_mut() {
                s.preparations
                    .retain(|p| p.command.preparation.epoch != epoch);
            }
            self.critical_song(acks, SongHostAck::PreparationCancelled(epoch));
            return false;
        }
        true
    }
    fn finish_song_return(&mut self, key: SongLeaseKey) {
        let _ = self.store.song_cancel(key);
        self.buses.song_finish_cancel(key);
        for slot in &mut self.templates.slots {
            if slot.state == SlotState::Staged && slot.song == Some(key) {
                slot.state = SlotState::Free;
                slot.song = None;
            }
        }
        if let Some(s) = self.song_stager.as_mut() {
            s.finish_return(key);
        }
    }
}

#[cfg(test)]
mod capacity_result_tests {
    use super::*;
    #[test]
    fn actual_serial_exhaustion_is_capacity_rejected_not_generic_rejection() {
        let mut engine = Engine::new(
            &crate::dsp::caps::CapabilitySet::browser(),
            48000.,
            16,
            crate::dsp::arena::StoreKind::NativeArc,
        );
        let (mut acks, mut rx) = crate::dsp::ring::SpscRing::split(2);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 1,
                    leases: 1,
                    branches: 1,
                    control_slots: 0,
                    analysis_slots: 0,
                    native_pcm_bytes: 64,
                    critical_receipts: 1,
                },
                &acks,
            )
            .unwrap();
        engine.song_stager.as_mut().unwrap().serial = u64::MAX;
        let epoch = SnapshotEpoch(u64::MAX);
        engine.song_record(
            Record::Msg(CtlMsg::Song(SongCommand::RequestCapacity(epoch))),
            &mut acks,
            None,
        );
        assert_eq!(
            rx.pop(),
            Some(HostMsg::Song(SongHostAck::CapacityRejected {
                epoch,
                reason: SongRejectCode::Capacity
            }))
        );
        engine.song_record(
            Record::Msg(CtlMsg::Song(SongCommand::Activate(
                crate::song::routing::SongActivation { epoch, frame: 0 },
            ))),
            &mut acks,
            None,
        );
        assert_eq!(
            rx.pop(),
            Some(HostMsg::Song(SongHostAck::Rejected {
                epoch,
                reason: SongRejectCode::NotReady
            }))
        );
    }
}
