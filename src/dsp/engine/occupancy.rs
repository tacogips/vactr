//! Current physical ownership and retained reservation admission.
use super::*;
use crate::dsp::bus::SlotState;
use crate::song::routing::{
    SongHostCapacities, SongRejectCode, SongResourceKind, SongResourceReservation,
    SongStagePreparation,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SongUnadoptedCounts {
    pub(crate) templates: u32,
    pub(crate) buses: u32,
    pub(crate) samples: u32,
}
impl Engine {
    pub(super) fn song_unadopted_counts(&self) -> Result<SongUnadoptedCounts, SongRejectCode> {
        let mut counts = SongUnadoptedCounts::default();
        if let Some(stager) = &self.song_stager {
            for lease in &stager.leases {
                let counter = match lease.key.kind {
                    SongResourceKind::Instrument if !self.templates.song_contains(lease.key) => {
                        &mut counts.templates
                    }
                    SongResourceKind::PrivateFx
                    | SongResourceKind::Track
                    | SongResourceKind::Master
                        if !self.buses.song_contains(lease.key) =>
                    {
                        &mut counts.buses
                    }
                    SongResourceKind::Sample if !self.store.song_contains(lease.key) => {
                        &mut counts.samples
                    }
                    _ => continue,
                };
                *counter = counter.checked_add(1).ok_or(SongRejectCode::Capacity)?;
            }
        }
        Ok(counts)
    }
    pub(super) fn song_legacy_claims(&self) -> Result<SongUnadoptedCounts, FaultCode> {
        self.song_unadopted_counts()
            .map_err(|_| FaultCode::BadResource)
    }
    /// Current physical capacity after outstanding unadopted claims.
    /// # Errors
    /// Unconfigured provider or inconsistent/exhausted physical geometry.
    pub fn song_remaining_capacities(&self) -> Result<SongHostCapacities, SongRejectCode> {
        let mut result = self
            .song_stager
            .as_ref()
            .ok_or(SongRejectCode::NotReady)?
            .remaining_capacities();
        let claims = self.song_unadopted_counts()?;
        let free = |count: usize, claimed: u32| {
            u32::try_from(count)
                .ok()
                .and_then(|n| n.checked_sub(claimed))
                .ok_or(SongRejectCode::Capacity)
        };
        result.template_slots = free(
            self.templates
                .slots
                .iter()
                .filter(|s| {
                    s.state == SlotState::Free
                        && s.t.is_some() == matches!(self.cfg.store, StoreKind::Arena { .. })
                })
                .count(),
            claims.templates,
        )?;
        result.bus_slots = free(
            self.buses
                .slots
                .iter()
                .filter(|s| s.state == SlotState::Free)
                .count(),
            claims.buses,
        )?;
        result.sample_resources = self
            .store
            .song_free_sample_slots()
            .checked_sub(claims.samples)
            .ok_or(SongRejectCode::Capacity)?;
        result.pcm_bytes = self
            .store
            .song_remaining_pcm_bytes()
            .ok_or(SongRejectCode::Capacity)?;
        result.bus_frames = self.buses.song_available_frames(claims.buses)?;
        Ok(result)
    }
    /// Physical free regions; admission also uses outstanding claim counts.
    #[must_use]
    pub fn song_bus_regions(&self) -> &[SongFrameRegion] {
        self.buses.song_regions()
    }
    /// Admit a preparation against current actual storage.
    /// # Errors
    /// Stale identity, exhausted tables or insufficient physical/private storage.
    pub fn begin_song_preparation(
        &mut self,
        command: SongStagePreparation,
    ) -> Result<(), SongRejectCode> {
        let available = self.song_remaining_capacities()?;
        if !crate::dsp::arena::song::fits(command.preparation.required, available) {
            return Err(SongRejectCode::Capacity);
        }
        self.song_stager
            .as_mut()
            .ok_or(SongRejectCode::NotReady)?
            .begin_with_available(command, available)
    }
    /// Reserve one current physical-role claim without fabricated geometry.
    /// # Errors
    /// Foreign, duplicate or over-capacity reservation.
    pub fn reserve_song_resource(
        &mut self,
        command: SongResourceReservation,
    ) -> Result<(), SongRejectCode> {
        let available = self.song_remaining_capacities()?;
        self.song_stager
            .as_mut()
            .ok_or(SongRejectCode::NotReady)?
            .reserve_with_available(command, available)
    }
    pub(super) fn begin_legacy_sample(
        &mut self,
        resource: u32,
        gen: u32,
        frames: usize,
        channels: u8,
        rate: u32,
    ) -> Result<(), FaultCode> {
        let claims = self.song_legacy_claims()?;
        self.store
            .begin_with_song_claims(resource, gen, frames, channels, rate, claims.samples)
    }
    pub(super) fn return_legacy_install(
        &mut self,
        garbage: Option<&mut Producer<Garbage>>,
    ) -> bool {
        let Some(owner) = self.pending_legacy_install.take() else {
            return true;
        };
        self.retain_legacy_owner(owner, garbage);
        self.pending_legacy_install.is_none()
    }
    pub(super) fn retain_legacy_owner(
        &mut self,
        owner: Garbage,
        garbage: Option<&mut Producer<Garbage>>,
    ) {
        debug_assert!(self.pending_legacy_install.is_none());
        self.pending_legacy_install = match garbage {
            Some(queue) => queue.push(owner).err(),
            None => Some(owner),
        };
    }
    pub(super) fn install_bytes<C: CellStore>(
        &mut self,
        id: u32,
        gen: u32,
        bytes: &[u8],
        cells: &C,
    ) -> Result<(), FaultCode> {
        let claims = self.song_legacy_claims()?;
        match decode_graph(bytes, &mut self.raw, &mut self.bus_tmp)? {
            GraphKind::Inst => {
                if self.cfg.output_channels == 2
                    && self.raw.nodes[..self.raw.n_nodes].iter().any(|n| {
                        matches!(
                            n,
                            crate::dsp::ugen::Node::Out3 | crate::dsp::ugen::Node::Out4
                        )
                    })
                {
                    return Err(FaultCode::OutputChannels);
                }
                let env = self.build_env();
                self.templates
                    .build_with_song_claims(&self.raw, &env, id, gen, claims.templates)
            }
            kind => {
                let t = *self.bus_tmp;
                let master = kind == GraphKind::Master;
                self.buses
                    .install_with_song_claims(
                        &t,
                        master,
                        id,
                        gen,
                        cells,
                        self.sr,
                        &self.cfg.caps,
                        claims.buses,
                    )
                    .then_some(())
                    .ok_or(FaultCode::BadResource)
            }
        }
    }
    pub(super) fn install_native<C: CellStore>(
        &mut self,
        n: NativeInstall,
        cells: &C,
        garbage: Option<&mut Producer<Garbage>>,
    ) -> Option<HostMsg> {
        let claims = self.song_unadopted_counts();
        let claims = claims.unwrap_or(SongUnadoptedCounts {
            templates: u32::MAX,
            buses: u32::MAX,
            samples: u32::MAX,
        });
        let (resource, gen, ok) = match n {
            NativeInstall::Inst {
                resource,
                gen,
                template,
            } => match self.cfg.output_channels == 4 || !template.has_quad {
                true => match self.templates.adopt_with_song_claims(
                    template,
                    resource,
                    gen,
                    claims.templates,
                ) {
                    Ok(()) => (resource, gen, true),
                    Err(t) => {
                        self.retain_legacy_owner(Garbage::Template(t), garbage);
                        (resource, gen, false)
                    }
                },
                false => {
                    self.retain_legacy_owner(Garbage::Template(template), garbage);
                    self.fault(FaultCode::OutputChannels, resource);
                    (resource, gen, false)
                }
            },
            NativeInstall::Bus {
                resource,
                gen,
                master,
                template,
            } => {
                let caps = self.cfg.caps;
                let ok = self.buses.install_with_song_claims(
                    &template,
                    master,
                    resource,
                    gen,
                    cells,
                    self.sr,
                    &caps,
                    claims.buses,
                );
                self.retain_legacy_owner(Garbage::Bus(template), garbage);
                (resource, gen, ok)
            }
            NativeInstall::Sample {
                resource,
                gen,
                data,
            } => match self
                .store
                .install_arc_with_song_claims(resource, gen, data, claims.samples)
            {
                Ok(()) => (resource, gen, true),
                Err(data) => {
                    self.retain_legacy_owner(Garbage::Sample(data), garbage);
                    (resource, gen, false)
                }
            },
        };
        if ok {
            Some(HostMsg::Installed { resource, gen })
        } else {
            self.fault(FaultCode::BadResource, resource);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::cells::AtomicCells;
    use crate::dsp::graph::{InstDef, UGenSpec};
    use crate::dsp::ring::{EventRing, NativeRecord, SpscRing};
    use crate::song::routing::{SongAnalysisCapacity, SongPreparation};
    use crate::song::SnapshotEpoch;
    #[test]
    fn legacy_release_restores_current_admission_after_provider_construction() {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 1;
        let mut cfg = EngineConfig::new(&caps, 48_000., 16, StoreKind::NativeArc);
        cfg.template_slots = 2;
        let mut engine = Engine::with_config(cfg);
        let (mut tx, mut controls) = SpscRing::split(8);
        let (_, mut events) = EventRing::split(2);
        let (mut acks, mut received) = SpscRing::split(8);
        let (mut garbage, mut returned) = SpscRing::split(8);
        let mut cells = AtomicCells::new(8);
        let def = InstDef {
            id: InstId::new(40),
            params: Box::new([]),
            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
            edges: Box::new([]),
            node_params: Box::new([]),
        };
        let t = Template::from_inst(&def, &engine.build_env()).unwrap();
        assert!(engine
            .install_native(
                NativeInstall::Inst {
                    resource: 40,
                    gen: 1,
                    template: t
                },
                &cells,
                Some(&mut garbage)
            )
            .is_some());
        assert_eq!(engine.blocks, 0);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 2,
                    leases: 4,
                    branches: 1,
                    control_slots: 8,
                    analysis_slots: 64,
                    native_pcm_bytes: 4096,
                    critical_receipts: 2,
                },
                &acks,
            )
            .unwrap();
        assert_eq!(
            engine.song_remaining_capacities().unwrap().template_slots,
            1
        );
        tx.push(NativeRecord::Msg(CtlMsg::GraphRetire { id: 40 }))
            .ok()
            .unwrap();
        let mut output = [0.; 32];
        engine.process(
            &mut EngineIo {
                events: &mut events,
                controls: &mut controls,
                acks: &mut acks,
                cells: &mut cells,
                garbage: Some(&mut garbage),
            },
            &mut output,
            16,
        );
        assert!(returned.pop().is_some());
        while received.pop().is_some() {}
        let available = engine.song_remaining_capacities().unwrap();
        assert_eq!(available.template_slots, 2);
        let required = SongHostCapacities {
            sample_rate: available.sample_rate,
            template_slots: 2,
            ..SongHostCapacities::default()
        };
        let p = SongStagePreparation {
            preparation: SongPreparation {
                epoch: SnapshotEpoch(1),
                resources: 2,
                branches: 0,
                required,
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        };
        assert_eq!(engine.begin_song_preparation(p), Ok(()));
    }
}
