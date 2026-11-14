//! Full-key silent sample reservations. Storage is allocated only at construction.
use super::{
    Entry, Fault, FaultCode, InstallRequest, ResState, ResourceKind, SampleStore, StoreKind,
};
use crate::dsp::engine::SongFrameRegion;
use crate::host::caps::SampleData;
use crate::song::routing::{SongLeaseKey, SongResourceKind};
use std::sync::Arc;

impl SampleStore {
    pub(super) fn refresh_song_extents(&mut self) {
        for (index, (region, &(offset, frames))) in self
            .song_extents
            .iter_mut()
            .zip(self.free.iter())
            .enumerate()
        {
            *region = SongFrameRegion {
                slot: u32::try_from(index).unwrap_or(u32::MAX),
                offset: u64::try_from(offset).unwrap_or(u64::MAX),
                frames: if index < self.n_free {
                    u64::try_from(frames).unwrap_or(u64::MAX)
                } else {
                    0
                },
            };
        }
    }

    /// Actual current contiguous float extents, maintained by every allocator path.
    #[must_use]
    pub fn song_free_extents(&self) -> &[SongFrameRegion] {
        &self.song_extents[..self.n_free]
    }

    /// Slots not occupied by legacy or song resources.
    #[must_use]
    pub fn song_free_sample_slots(&self) -> u32 {
        u32::try_from(self.entries.iter().filter(|e| e.is_none()).count()).unwrap_or(0)
    }

    pub(crate) fn song_configure_native_limit(&mut self, bytes: u64) {
        self.native_pcm_limit = Some(bytes);
    }

    pub(crate) fn song_remaining_pcm_bytes(&self) -> Option<u64> {
        let limit = match self.kind {
            StoreKind::NativeArc => self.native_pcm_limit?,
            StoreKind::Arena { .. } => 0,
        };
        self.song_remaining_pcm_with_limit(limit)
    }

    pub(crate) fn song_remaining_pcm_with_limit(&self, bytes: u64) -> Option<u64> {
        match self.kind {
            StoreKind::Arena { .. } => u64::try_from(self.free_floats()).ok()?.checked_mul(4),
            StoreKind::NativeArc => {
                let used = self.entries.iter().flatten().try_fold(0_u64, |total, e| {
                    total.checked_add(u64::try_from(e.len).ok()?.checked_mul(4)?)
                })?;
                bytes.checked_sub(used)
            }
        }
    }

    pub(crate) fn song_contains(&self, key: SongLeaseKey) -> bool {
        self.song_slot(key).is_some()
    }

    fn song_slot(&self, key: SongLeaseKey) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.is_some_and(|e| e.song == Some(key)))
    }

    /// Reserve an exact extent and full identity before upload.
    /// # Errors
    /// Malformed geometry, reused identity, or insufficient real storage.
    pub fn song_reserve(
        &mut self,
        key: SongLeaseKey,
        request: InstallRequest,
    ) -> Result<(), Fault> {
        self.reserve_song_sample(key, request)
            .map_err(|code| Fault {
                code,
                resource: key.resource.id,
            })
    }

    fn reserve_song_sample(
        &mut self,
        key: SongLeaseKey,
        req: InstallRequest,
    ) -> Result<(), FaultCode> {
        if key.kind != SongResourceKind::Sample
            || key.resource.id != req.resource
            || req.kind != ResourceKind::Sample
            || !matches!(req.channels, 1 | 2)
            || !(8_000..=192_000).contains(&req.rate)
            || self.song_slot(key).is_some()
        {
            return Err(FaultCode::BadResource);
        }
        let len = req
            .frames
            .checked_mul(usize::from(req.channels))
            .ok_or(FaultCode::BadRecord)?;
        let bytes = u64::try_from(len)
            .ok()
            .and_then(|n| n.checked_mul(4))
            .ok_or(FaultCode::BadRecord)?;
        if self
            .song_remaining_pcm_bytes()
            .is_none_or(|free| bytes > free)
        {
            return Err(FaultCode::ArenaExhausted);
        }
        let slot = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(FaultCode::ArenaExhausted)?;
        let offset = match self.kind {
            StoreKind::Arena { .. } => self.alloc(len).ok_or(FaultCode::ArenaExhausted)?,
            StoreKind::NativeArc => 0,
        };
        let physical = (0..super::MAX_RESOURCES)
            .filter_map(|i| u32::try_from(i).ok())
            .map(|i| 0x0008_0000 + i)
            .find(|id| self.slot(*id).is_none())
            .ok_or(FaultCode::ArenaExhausted)?;
        self.entries[slot] = Some(Entry {
            song: Some(key),
            id: physical,
            gen: key.resource.generation,
            state: ResState::Reserved,
            channels: req.channels,
            rate: req.rate,
            off: offset,
            len,
            received: 0,
        });
        Ok(())
    }

    /// Begin once, consuming the already reserved extent rather than allocating twice.
    /// # Errors
    /// Duplicate starts, changed geometry, or exhausted extent.
    pub fn song_begin(&mut self, key: SongLeaseKey, req: InstallRequest) -> Result<(), FaultCode> {
        if req.kind != ResourceKind::Sample
            || key.resource.id != req.resource
            || !matches!(self.kind, StoreKind::Arena { .. })
        {
            return Err(FaultCode::BadResource);
        }
        if self.song_slot(key).is_none() {
            self.reserve_song_sample(key, req)?;
        }
        let index = self.song_slot(key).ok_or(FaultCode::BadResource)?;
        let entry = self.entries[index].as_mut().ok_or(FaultCode::BadResource)?;
        let len = req
            .frames
            .checked_mul(usize::from(req.channels))
            .ok_or(FaultCode::BadRecord)?;
        if entry.state != ResState::Reserved
            || entry.len != len
            || entry.channels != req.channels
            || entry.rate != req.rate
        {
            return Err(FaultCode::BadResource);
        }
        entry.state = if len == 0 {
            ResState::Staged
        } else {
            ResState::Installing
        };
        Ok(())
    }

    /// Copy exactly the next finite slice. Completion remains silent.
    /// # Errors
    /// Foreign leases, gaps/overlap, truncation, nonfinite PCM or duplicate completion.
    pub fn song_write_slice(
        &mut self,
        key: SongLeaseKey,
        offset: usize,
        bytes: &[u8],
    ) -> Result<bool, FaultCode> {
        let index = self.song_slot(key).ok_or(FaultCode::BadResource)?;
        let mut entry = self.entries[index].ok_or(FaultCode::BadResource)?;
        let n = bytes.len() / 4;
        let end = offset.checked_add(n).ok_or(FaultCode::BadRecord)?;
        if entry.state != ResState::Installing
            || offset != entry.received
            || bytes.is_empty()
            || bytes.len() % 4 != 0
            || bytes.len() > super::SLICE_BYTES
            || end > entry.len
            || bytes
                .chunks_exact(4)
                .any(|b| !f32::from_le_bytes([b[0], b[1], b[2], b[3]]).is_finite())
        {
            return Err(FaultCode::BadRecord);
        }
        let start = entry.off.checked_add(offset).ok_or(FaultCode::BadRecord)?;
        let dst = self
            .arena
            .get_mut(start..start + n)
            .ok_or(FaultCode::BadResource)?;
        for (value, b) in dst.iter_mut().zip(bytes.chunks_exact(4)) {
            *value = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        entry.received = end;
        let done = end == entry.len;
        if done {
            entry.state = ResState::Staged;
        }
        self.entries[index] = Some(entry);
        Ok(done)
    }

    pub(crate) fn song_install_arc(
        &mut self,
        key: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        if !matches!(self.kind, StoreKind::NativeArc)
            || !matches!(data.channels, 1 | 2)
            || !(8_000..=192_000).contains(&data.rate)
            || data.frames.len() % usize::from(data.channels) != 0
        {
            return Err(data);
        }
        let request = InstallRequest {
            resource: key.resource.id,
            kind: ResourceKind::Sample,
            frames: data.frames.len() / usize::from(data.channels),
            channels: data.channels,
            rate: data.rate,
            bytes: 0,
            origin: None,
        };
        if self.song_slot(key).is_none() && self.reserve_song_sample(key, request).is_err() {
            return Err(data);
        }
        let Some(index) = self.song_slot(key) else {
            return Err(data);
        };
        let Some(entry) = self.entries[index].as_mut() else {
            return Err(data);
        };
        if entry.state != ResState::Reserved
            || entry.len != data.frames.len()
            || entry.rate != data.rate
            || entry.channels != data.channels
        {
            return Err(data);
        }
        entry.state = ResState::Staged;
        entry.received = entry.len;
        self.native[index] = Some(data);
        Ok(())
    }

    pub(crate) fn song_physical_id(&self, key: SongLeaseKey) -> Option<u32> {
        self.entries[self.song_slot(key)?].map(|entry| entry.id)
    }

    pub(crate) fn song_take_arc(&mut self, key: SongLeaseKey) -> Option<Arc<SampleData>> {
        let index = self.song_slot(key)?;
        self.native[index].take()
    }

    /// Release only after owned native storage has been handed back off-thread.
    /// # Errors
    /// Foreign lease or native ownership still awaiting garbage handoff.
    pub fn song_cancel(&mut self, key: SongLeaseKey) -> Result<(), Fault> {
        let Some(index) = self.song_slot(key) else {
            return Err(Fault {
                code: FaultCode::BadResource,
                resource: key.resource.id,
            });
        };
        if self.native[index].is_some() {
            return Err(Fault {
                code: FaultCode::BadResource,
                resource: key.resource.id,
            });
        }
        if let Some(entry) = self.entries[index].take() {
            if matches!(self.kind, StoreKind::Arena { .. }) {
                self.dealloc(entry.off, entry.len);
            }
        }
        Ok(())
    }
}

use crate::dsp::cells::{CellId, CellRead};
use crate::dsp::engine::{SongLeaseState, SongStagingConfig};
use crate::song::routing::{
    SongAnalysisCapacity, SongAnalysisReservation, SongBranchConfig, SongCellInit,
    SongHostCapacities, SongRejectCode, SongResourceReservation, SongStagePreparation,
};
use crate::song::SnapshotEpoch;

#[derive(Clone, Copy)]
pub(crate) struct Preparation {
    pub(crate) command: SongStagePreparation,
    pub(crate) ready: bool,
    pub(crate) cancelling: bool,
    pub(crate) active: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct Cell {
    pub(crate) owner: Option<SongLeaseKey>,
    pub(crate) logical: CellId,
    pub(crate) value: f32,
}
#[derive(Clone, Copy)]
pub(crate) struct AnalysisBank {
    pub(crate) owner: SongLeaseKey,
    pub(crate) offset: usize,
    pub(crate) len: usize,
}
/// Private storage contains no shared AtomicCells or mutable snapshot aliases.
pub struct SongResourceStager {
    pub(crate) actual: SongHostCapacities,
    pub(crate) preparations: Vec<Preparation>,
    pub(crate) leases: Vec<SongLeaseState>,
    pub(crate) branches: Vec<SongBranchConfig>,
    pub(crate) reusable: Vec<crate::dsp::song::SongReusableSlot>,
    pub(crate) cells: Box<[Cell]>,
    pub(crate) analysis: Box<[f32]>,
    pub(crate) analysis_owners: Box<[Option<SongLeaseKey>]>,
    pub(crate) banks: Vec<AnalysisBank>,
    pub(crate) bindings: Vec<crate::song::routing::SongGraphBanks>,
    pub(crate) bus_regions: Box<[SongFrameRegion]>,
    pub(crate) voice_regions: Box<[SongFrameRegion]>,
    pub(crate) pcm_extents: Box<[SongFrameRegion]>,
    pub(crate) serial: u64,
}
impl SongResourceStager {
    pub(crate) fn new(
        config: SongStagingConfig,
        actual: SongHostCapacities,
        bus_regions: &[SongFrameRegion],
        voice_regions: &[SongFrameRegion],
        pcm_extents: &[SongFrameRegion],
    ) -> Result<Self, SongRejectCode> {
        if config.preparations == 0
            || config.leases == 0
            || config.branches == 0
            || config.critical_receipts == 0
            || config.critical_receipts > actual.ack_slots
            || config.control_slots != actual.cell_slots
        {
            return Err(SongRejectCode::Capacity);
        }
        let cells = vec![
            Cell {
                owner: None,
                logical: CellId::new(0),
                value: 0.0
            };
            config.control_slots as usize
        ]
        .into_boxed_slice();
        Ok(Self {
            actual,
            preparations: Vec::with_capacity(config.preparations as usize),
            leases: Vec::with_capacity(config.leases as usize),
            branches: Vec::with_capacity(config.branches as usize),
            reusable: Vec::with_capacity(config.branches as usize),
            cells,
            analysis: vec![0.0; config.analysis_slots as usize].into_boxed_slice(),
            analysis_owners: vec![None; config.analysis_slots as usize].into_boxed_slice(),
            banks: Vec::with_capacity(config.leases as usize),
            bindings: Vec::with_capacity(config.leases as usize),
            bus_regions: bus_regions.into(),
            voice_regions: voice_regions.into(),
            pcm_extents: pcm_extents.into(),
            serial: 0,
        })
    }
    pub(crate) fn reusable_slot(
        &self,
        epoch: SnapshotEpoch,
        branch: crate::song::routing::SongBranchId,
    ) -> Option<crate::dsp::song::SongReusableSlot> {
        self.reusable
            .iter()
            .find(|s| s.epoch == epoch && s.branch == branch)
            .copied()
    }
    #[must_use]
    pub fn remaining_capacities(&self) -> SongHostCapacities {
        let mut remaining = self.actual;
        remaining.cell_slots =
            u32::try_from(self.cells.iter().filter(|c| c.owner.is_none()).count()).unwrap_or(0);
        for lease in &self.leases {
            match lease.key.kind {
                SongResourceKind::Instrument => {
                    remaining.template_slots = remaining.template_slots.saturating_sub(1)
                }
                SongResourceKind::PrivateFx
                | SongResourceKind::Track
                | SongResourceKind::Master => {
                    remaining.bus_slots = remaining.bus_slots.saturating_sub(1);
                }
                SongResourceKind::Sample => {
                    remaining.sample_resources = remaining.sample_resources.saturating_sub(1)
                }
                SongResourceKind::ControlCells | SongResourceKind::AnalysisBank => {}
            }
        }
        remaining
    }
    #[must_use]
    pub fn remaining_analysis_capacity(&self) -> SongAnalysisCapacity {
        SongAnalysisCapacity {
            slots: u32::try_from(self.analysis_owners.iter().filter(|o| o.is_none()).count())
                .unwrap_or(0),
        }
    }
    #[must_use]
    pub fn bus_regions(&self) -> &[SongFrameRegion] {
        &self.bus_regions
    }
    #[must_use]
    pub fn voice_regions(&self) -> &[SongFrameRegion] {
        &self.voice_regions
    }
    #[must_use]
    pub fn pcm_extents(&self) -> &[SongFrameRegion] {
        &self.pcm_extents
    }
    #[must_use]
    pub fn lease_states(&self) -> &[SongLeaseState] {
        &self.leases
    }
    pub(crate) fn preparation_mut(
        &mut self,
        epoch: SnapshotEpoch,
    ) -> Result<&mut Preparation, SongRejectCode> {
        let preparation = self
            .preparations
            .iter_mut()
            .find(|p| p.command.preparation.epoch == epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if preparation.active {
            return Err(SongRejectCode::NotReady);
        }
        Ok(preparation)
    }
    pub(crate) fn open(&self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode> {
        let p = self
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if p.ready || p.cancelling {
            Err(SongRejectCode::NotReady)
        } else {
            Ok(())
        }
    }
    pub(crate) fn lease(&self, key: SongLeaseKey) -> Result<usize, SongRejectCode> {
        self.leases
            .iter()
            .position(|l| l.key == key && !l.retiring)
            .ok_or(SongRejectCode::StaleEpoch)
    }
    pub(crate) fn begin_with_available(
        &mut self,
        command: SongStagePreparation,
        available: SongHostCapacities,
    ) -> Result<(), SongRejectCode> {
        let p = command.preparation;
        if self
            .preparations
            .iter()
            .any(|old| old.command.preparation.epoch == p.epoch)
        {
            return Err(SongRejectCode::StaleEpoch);
        }
        if self.preparations.len() == self.preparations.capacity()
            || p.resources as usize > self.leases.capacity() - self.leases.len()
            || p.branches as usize > self.branches.capacity() - self.branches.len()
            || !fits(p.required, available)
            || command.analysis_required.slots > self.remaining_analysis_capacity().slots
        {
            return Err(SongRejectCode::Capacity);
        }
        self.preparations.push(Preparation {
            command,
            ready: false,
            cancelling: false,
            active: false,
        });
        Ok(())
    }
    pub(crate) fn reserve_with_available(
        &mut self,
        command: SongResourceReservation,
        available: SongHostCapacities,
    ) -> Result<(), SongRejectCode> {
        self.open(command.epoch)?;
        let key = SongLeaseKey {
            epoch: command.epoch,
            resource: command.resource,
            kind: command.kind,
        };
        if self
            .leases
            .iter()
            .any(|old| old.key.epoch == key.epoch && old.key.resource == key.resource)
        {
            return Err(SongRejectCode::Malformed);
        }
        let p = self
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == key.epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if self.leases.len() == self.leases.capacity()
            || self
                .leases
                .iter()
                .filter(|l| l.key.epoch == key.epoch)
                .count()
                >= p.command.preparation.resources as usize
        {
            return Err(SongRejectCode::Capacity);
        }
        let left = available;
        let free = match key.kind {
            SongResourceKind::Instrument => left.template_slots,
            SongResourceKind::PrivateFx | SongResourceKind::Track | SongResourceKind::Master => {
                left.bus_slots
            }
            SongResourceKind::Sample => left.sample_resources,
            SongResourceKind::ControlCells | SongResourceKind::AnalysisBank => 1,
        };
        if free == 0 {
            return Err(SongRejectCode::Capacity);
        }
        self.leases.push(SongLeaseState {
            key,
            staged: false,
            retiring: false,
        });
        Ok(())
    }
    pub(crate) fn init_cell(&mut self, init: SongCellInit) -> Result<(), SongRejectCode> {
        self.open(init.lease.epoch)?;
        let lease = self.lease(init.lease)?;
        if init.lease.kind != SongResourceKind::ControlCells || !init.value.is_finite() {
            return Err(SongRejectCode::Malformed);
        }
        if self
            .cells
            .iter()
            .any(|c| c.owner == Some(init.lease) && c.logical == init.cell)
        {
            return Err(SongRejectCode::Malformed);
        }
        let slot = self
            .cells
            .iter_mut()
            .find(|c| c.owner.is_none())
            .ok_or(SongRejectCode::Capacity)?;
        *slot = Cell {
            owner: Some(init.lease),
            logical: init.cell,
            value: init.value,
        };
        self.leases[lease].staged = true;
        Ok(())
    }
    pub(crate) fn reserve_analysis(
        &mut self,
        reservation: SongAnalysisReservation,
    ) -> Result<(), SongRejectCode> {
        self.open(reservation.lease.epoch)?;
        let lease = self.lease(reservation.lease)?;
        if reservation.lease.kind != SongResourceKind::AnalysisBank
            || self.banks.iter().any(|b| b.owner == reservation.lease)
        {
            return Err(SongRejectCode::Malformed);
        }
        let len = reservation.slots as usize;
        if len > self.analysis_owners.len() {
            return Err(SongRejectCode::Capacity);
        }
        let start = if len == 0 {
            0
        } else {
            let mut run = 0;
            self.analysis_owners
                .iter()
                .position(|owner| {
                    run = if owner.is_none() { run + 1 } else { 0 };
                    run == len
                })
                .and_then(|end| end.checked_add(1)?.checked_sub(len))
                .ok_or(SongRejectCode::Capacity)?
        };
        if self.banks.len() == self.banks.capacity() {
            return Err(SongRejectCode::Capacity);
        }
        for owner in &mut self.analysis_owners[start..start + len] {
            *owner = Some(reservation.lease);
        }
        self.analysis[start..start + len].fill(0.0);
        self.banks.push(AnalysisBank {
            owner: reservation.lease,
            offset: start,
            len,
        });
        self.leases[lease].staged = true;
        Ok(())
    }
    pub(crate) fn physical_cell(
        &self,
        bank: SongLeaseKey,
        logical: CellId,
    ) -> Result<CellId, SongRejectCode> {
        let mut matches = self
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.owner == Some(bank) && c.logical == logical);
        let (index, _) = matches.next().ok_or(SongRejectCode::Malformed)?;
        if matches.next().is_some() {
            return Err(SongRejectCode::Malformed);
        }
        Ok(CellId::new(
            u32::try_from(index).map_err(|_| SongRejectCode::Capacity)?,
        ))
    }
}
impl CellRead for SongResourceStager {
    fn get(&self, cell: CellId) -> f32 {
        self.cells.get(cell.index()).map_or(0.0, |c| c.value)
    }
}
pub(crate) fn fits(required: SongHostCapacities, available: SongHostCapacities) -> bool {
    required.sample_rate == available.sample_rate
        && required.cell_slots <= available.cell_slots
        && required.voice_slots <= available.voice_slots
        && required.template_slots <= available.template_slots
        && required.bus_slots <= available.bus_slots
        && required.sample_resources <= available.sample_resources
        && required.pcm_bytes <= available.pcm_bytes
        && required.voice_frames <= available.voice_frames
        && required.bus_frames <= available.bus_frames
        && required.ack_slots <= available.ack_slots
}

use crate::dsp::ugen::{Node, Src};
use crate::host::wire::Ctl;

impl SongResourceStager {
    pub(crate) fn map_template(
        &self,
        lease: SongLeaseKey,
        template: &mut crate::dsp::ugen::Template,
        store: &SampleStore,
        output_channels: u8,
    ) -> Result<(), SongRejectCode> {
        let stager = self;
        if template.n_nodes > template.nodes.len()
            || template.n_params > template.params.len()
            || template.n_fx > template.fx_params.len()
            || template.n_refs > template.refs.len()
            || (template.has_quad && output_channels != 4)
            || self
                .voice_regions()
                .iter()
                .all(|region| u64::try_from(template.mem_total).is_ok_and(|n| n > region.frames))
        {
            return Err(SongRejectCode::Capacity);
        }
        for (id, ctl) in template.params.iter().take(template.n_params) {
            if *id == crate::dsp::ugen::BANK {
                self.resource_ctl(lease, *ctl, store)?;
            }
        }
        for (_, ctl) in template.params.iter().take(template.n_params) {
            validate_ctl(stager, lease, *ctl)?;
        }
        for node in template.nodes.iter().take(template.n_nodes) {
            for input in &node.inputs {
                if let Src::Cell(cell) = input {
                    stager.bound_cell(lease, *cell)?;
                }
            }
        }
        for (params, n) in template
            .fx_params
            .iter()
            .zip(template.fx_n.iter())
            .take(template.n_fx)
        {
            if usize::from(*n) > params.len() {
                return Err(SongRejectCode::Malformed);
            }
            for (_, ctl) in params.iter().take(usize::from(*n)) {
                validate_ctl(stager, lease, *ctl)?;
            }
        }
        for node in template.nodes.iter().take(template.n_nodes) {
            let resource = match node.node {
                Node::SamplePlay(bank)
                | Node::Granular(crate::dsp::graph::GranSrc::Sample(bank)) => Some(bank.get()),
                Node::Wavetable(table)
                | Node::Granular(crate::dsp::graph::GranSrc::Table(table)) => Some(table.get()),
                _ => None,
            };
            if let Some(resource) = resource {
                if !template.refs[..template.n_refs].contains(&resource) {
                    return Err(SongRejectCode::Malformed);
                }
                self.sample_id(store, lease.epoch, resource)?;
            }
            if let Node::Effect { kind, fx } = node.node {
                let index = usize::from(fx);
                if index >= template.n_fx {
                    return Err(SongRejectCode::Malformed);
                }
                let params = &template.fx_params[index][..usize::from(template.fx_n[index])];
                if let crate::dsp::graph::EffectKind::Analyzer(analyzer) = kind {
                    self.validate_analyzer(lease, analyzer, params)?;
                }
                if kind == crate::dsp::graph::EffectKind::Convolution {
                    for (id, ctl) in params {
                        if Some(*id) == crate::dsp::effects::param_ctl(kind, "ir") {
                            self.resource_ctl(lease, *ctl, store)?;
                        }
                    }
                }
            }
        }
        for resource in template.refs.iter().take(template.n_refs) {
            self.sample_id(store, lease.epoch, *resource)?;
        }
        for (id, ctl) in template.params.iter_mut().take(template.n_params) {
            *ctl = if *id == crate::dsp::ugen::BANK {
                self.resource_ctl(lease, *ctl, store)?
            } else {
                mapped_ctl(stager, lease, *ctl)?
            };
        }
        for node in template.nodes.iter_mut().take(template.n_nodes) {
            for input in &mut node.inputs {
                if let Src::Cell(cell) = input {
                    *cell = stager.bound_cell(lease, *cell)?;
                }
            }
            node.node = match node.node {
                Node::SamplePlay(bank) => Node::SamplePlay(crate::dsp::graph::BankRef::new(
                    self.sample_id(store, lease.epoch, bank.get())?,
                )),
                Node::Wavetable(table) => Node::Wavetable(crate::dsp::graph::TableRef::new(
                    self.sample_id(store, lease.epoch, table.get())?,
                )),
                Node::Granular(crate::dsp::graph::GranSrc::Sample(bank)) => Node::Granular(
                    crate::dsp::graph::GranSrc::Sample(crate::dsp::graph::BankRef::new(
                        self.sample_id(store, lease.epoch, bank.get())?,
                    )),
                ),
                Node::Granular(crate::dsp::graph::GranSrc::Table(table)) => Node::Granular(
                    crate::dsp::graph::GranSrc::Table(crate::dsp::graph::TableRef::new(
                        self.sample_id(store, lease.epoch, table.get())?,
                    )),
                ),
                node => node,
            };
        }
        for (index, (params, n)) in template
            .fx_params
            .iter_mut()
            .zip(template.fx_n.iter())
            .take(template.n_fx)
            .enumerate()
        {
            let convolution = template.nodes.iter().take(template.n_nodes).any(|node| matches!(node.node, Node::Effect { kind: crate::dsp::graph::EffectKind::Convolution, fx } if usize::from(fx) == index));
            for (id, ctl) in params.iter_mut().take(usize::from(*n)) {
                *ctl = if convolution
                    && Some(*id)
                        == crate::dsp::effects::param_ctl(
                            crate::dsp::graph::EffectKind::Convolution,
                            "ir",
                        )
                {
                    self.resource_ctl(lease, *ctl, store)?
                } else {
                    mapped_ctl(stager, lease, *ctl)?
                };
            }
        }
        for resource in template.refs.iter_mut().take(template.n_refs) {
            *resource = self.sample_id(store, lease.epoch, *resource)?;
        }
        Ok(())
    }
    pub(crate) fn sample_id(
        &self,
        store: &SampleStore,
        epoch: SnapshotEpoch,
        logical: u32,
    ) -> Result<u32, SongRejectCode> {
        let stager = self;
        let mut keys = stager.leases.iter().filter(|l| {
            l.key.epoch == epoch
                && l.key.kind == SongResourceKind::Sample
                && l.key.resource.id == logical
                && l.staged
                && !l.retiring
        });
        let key = keys.next().ok_or(SongRejectCode::NotReady)?.key;
        if keys.next().is_some() {
            return Err(SongRejectCode::Malformed);
        }
        store.song_physical_id(key).ok_or(SongRejectCode::NotReady)
    }
}

pub(crate) fn validate_ctl(
    stager: &SongResourceStager,
    graph: SongLeaseKey,
    ctl: Ctl,
) -> Result<(), SongRejectCode> {
    match ctl {
        Ctl::Cell(cell) => stager.bound_cell(graph, cell).map(|_| ()),
        Ctl::Const(value) if value.is_finite() => Ok(()),
        _ => Err(SongRejectCode::Malformed),
    }
}
pub(crate) fn mapped_ctl(
    stager: &SongResourceStager,
    graph: SongLeaseKey,
    ctl: Ctl,
) -> Result<Ctl, SongRejectCode> {
    match ctl {
        Ctl::Cell(cell) => Ok(Ctl::Cell(stager.bound_cell(graph, cell)?)),
        value => {
            validate_ctl(stager, graph, value)?;
            Ok(value)
        }
    }
}

/// Exclusive read capability; the previous epoch is restored on every exit.
pub(crate) struct SongReadScope<'a> {
    store: &'a mut SampleStore,
    previous: Option<SnapshotEpoch>,
}
impl std::ops::Deref for SongReadScope<'_> {
    type Target = SampleStore;
    fn deref(&self) -> &Self::Target {
        self.store
    }
}
impl Drop for SongReadScope<'_> {
    fn drop(&mut self) {
        self.store.song_epoch = self.previous;
    }
}
impl SampleStore {
    pub(crate) fn song_read_scope(&mut self, epoch: Option<SnapshotEpoch>) -> SongReadScope<'_> {
        let previous = self.song_epoch;
        self.song_epoch = epoch;
        SongReadScope {
            store: self,
            previous,
        }
    }
    pub(crate) fn get_selected(&self, id: u32) -> Option<super::SampleView<'_>> {
        let index = self.slot(id)?;
        let entry = self.entries[index]?;
        if !matches!(entry.state, ResState::Live | ResState::Retiring)
            || entry.song.map(|key| key.epoch) != self.song_epoch
        {
            return None;
        }
        let data = match self.kind {
            StoreKind::NativeArc => &self.native[index].as_ref()?.frames[..],
            StoreKind::Arena { .. } => self.arena.get(entry.off..entry.off + entry.len)?,
        };
        Some(super::SampleView {
            data,
            channels: entry.channels,
            rate: entry.rate,
        })
    }
    pub(crate) fn song_is_staged(&self, key: SongLeaseKey) -> bool {
        self.song_slot(key)
            .is_some_and(|i| self.entries[i].is_some_and(|e| e.state == ResState::Staged))
    }
    pub(crate) fn activate_song_sample(&mut self, key: SongLeaseKey) -> Result<(), SongRejectCode> {
        let index = self.song_slot(key).ok_or(SongRejectCode::StaleEpoch)?;
        let entry = self.entries[index]
            .as_mut()
            .ok_or(SongRejectCode::StaleEpoch)?;
        if entry.state != ResState::Staged {
            return Err(SongRejectCode::NotReady);
        }
        entry.state = ResState::Live;
        Ok(())
    }
    pub(crate) fn retire_song_sample(&mut self, key: SongLeaseKey) -> Result<(), SongRejectCode> {
        let index = self.song_slot(key).ok_or(SongRejectCode::StaleEpoch)?;
        let entry = self.entries[index]
            .as_mut()
            .ok_or(SongRejectCode::StaleEpoch)?;
        entry.state = ResState::Retiring;
        Ok(())
    }
}
impl SongResourceStager {
    pub(crate) fn close_preparation_for_activation(
        &mut self,
        epoch: SnapshotEpoch,
    ) -> Result<(), SongRejectCode> {
        let preparation = self.preparation_mut(epoch)?;
        if !preparation.ready || preparation.cancelling {
            return Err(SongRejectCode::NotReady);
        }
        preparation.active = true;
        Ok(())
    }
}
