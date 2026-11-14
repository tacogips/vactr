//! Checked song command refusals and the existing wire codec.
use crate::song::routing::SongCommand;
use crate::vm::fail::Failure;

#[derive(Debug)]
pub enum SongSubmitError {
    Backpressure,
    Unavailable,
    Invalid(Failure),
}
#[derive(Debug)]
pub struct SongCommandRefusal {
    pub command: SongCommand,
    pub error: SongSubmitError,
}

/// Refusal preserves the caller's complete sample lease and shared PCM allocation.
#[derive(Debug)]
pub struct SongSampleRefusal {
    pub lease: crate::song::routing::SongLeaseKey,
    pub data: std::sync::Arc<super::SampleData>,
    pub error: SongSubmitError,
}
/// Sender-local retained ownership limits, independent of raw DSP capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongSampleSenderCapacity {
    Unbounded,
    Bounded { resources: u32, pcm_bytes: u64 },
}

pub(crate) mod song_codec {
    use crate::host::wire::{AudioEvent, WireError};
    use crate::song::routing::*;
    use crate::song::SnapshotEpoch;
    struct Codec<'a> {
        bytes: &'a mut [u8],
        pos: usize,
        failed: bool,
    }
    impl Codec<'_> {
        fn put(&mut self, b: &[u8]) {
            let Some(end) = self.pos.checked_add(b.len()) else {
                self.failed = true;
                return;
            };
            if let Some(dst) = self.bytes.get_mut(self.pos..end) {
                dst.copy_from_slice(b);
                self.pos = end;
            } else {
                self.failed = true;
            }
        }
        fn u8(&mut self, x: u8) {
            self.put(&[x]);
        }
        fn u32(&mut self, x: u32) {
            self.put(&x.to_le_bytes());
        }
        fn u64(&mut self, x: u64) {
            self.put(&x.to_le_bytes());
        }
        fn finish(self) -> usize {
            if self.failed {
                0
            } else {
                self.pos
            }
        }
    }
    struct Decode<'a> {
        bytes: &'a [u8],
        pos: usize,
    }
    impl Decode<'_> {
        fn get<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
            let end = self.pos.checked_add(N).ok_or(WireError::Truncated)?;
            let src = self.bytes.get(self.pos..end).ok_or(WireError::Truncated)?;
            let mut out = [0; N];
            out.copy_from_slice(src);
            self.pos = end;
            Ok(out)
        }
        fn u8(&mut self) -> Result<u8, WireError> {
            Ok(self.get::<1>()?[0])
        }
        fn u32(&mut self) -> Result<u32, WireError> {
            Ok(u32::from_le_bytes(self.get()?))
        }
        fn u64(&mut self) -> Result<u64, WireError> {
            Ok(u64::from_le_bytes(self.get()?))
        }
        fn boolean(&mut self) -> Result<bool, WireError> {
            match self.u8()? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(WireError::BadValue),
            }
        }
    }
    pub(crate) fn encode_command(c: SongCommand, out: &mut [u8]) -> usize {
        if !c.valid() {
            return 0;
        }
        let mut w = Codec {
            bytes: out,
            pos: 0,
            failed: false,
        };
        w.u8(SONG_COMMAND_TAG);
        w.u8(match c {
            SongCommand::Replace(_) => 20,
            SongCommand::PrimeMute(_) => 21,
            SongCommand::ConfigureReusableBranch(_) => 18,
            SongCommand::RebindBranch(_) => 19,
            SongCommand::RequestClock(_) => 17,
            SongCommand::RequestCapacity(_) => 15,
            SongCommand::BindGraphBanks(_) => 16,
            SongCommand::BeginStaging(_) => 10,
            SongCommand::InitCells(_) => 11,
            SongCommand::ReserveAnalysis(_) => 12,
            SongCommand::CancelPreparation(_) => 13,
            SongCommand::CancelLease(_) => 14,
            SongCommand::BeginPreparation(_) => 6,
            SongCommand::ReserveResource(_) => 7,
            SongCommand::ConfigureBranch(_) => 8,
            SongCommand::SealPreparation(_) => 9,
            SongCommand::Prepare(_) => 0,
            SongCommand::Activate(_) => 1,
            SongCommand::Mute(_) => 2,
            SongCommand::Endpoints(_) => 3,
            SongCommand::Release(_) => 4,
            SongCommand::Event(_) => 5,
        });
        w.u64(c.epoch().0);
        match c {
            SongCommand::Replace(r) => {
                w.u64(r.activation.frame);
                w.u64(r.previous.0);
                w.u64(r.overlay_nonce);
                w.u32(r.overlay_count);
            }
            SongCommand::PrimeMute(m) => {
                w.u64(m.overlay_nonce);
                w.u32(m.instrument);
                w.u8(u8::from(m.muted));
            }
            SongCommand::RequestClock(r) => w.u64(r.request),
            SongCommand::BindGraphBanks(b) => {
                put_key_tail(&mut w, b.graph);
                put_optional_key(&mut w, b.controls);
                put_optional_key(&mut w, b.analysis);
            }
            SongCommand::BeginStaging(s) => {
                w.u32(s.preparation.branches);
                w.u32(s.preparation.resources);
                put_capacity(&mut w, s.preparation.required);
                w.u32(s.analysis_required.slots);
            }
            SongCommand::InitCells(c) => {
                put_key_tail(&mut w, c.lease);
                w.u32(c.cell.get());
                w.u32(c.value.to_bits());
            }
            SongCommand::ReserveAnalysis(a) => {
                put_key_tail(&mut w, a.lease);
                w.u32(a.slots);
            }
            SongCommand::CancelLease(k) => put_key_tail(&mut w, k),
            SongCommand::CancelPreparation(_) | SongCommand::RequestCapacity(_) => {}
            SongCommand::BeginPreparation(p) => {
                w.u32(p.branches);
                w.u32(p.resources);
                put_capacity(&mut w, p.required);
            }
            SongCommand::ReserveResource(r) => {
                put_resource(&mut w, r.resource);
                w.u8(r.kind as u8);
            }
            SongCommand::ConfigureBranch(b) => {
                w.u32(b.branch.0);
                w.u32(b.generation);
                w.u32(b.family);
                w.u32(b.track);
                put_resource(&mut w, b.instrument);
                put_optional(&mut w, b.private_fx);
                put_optional(&mut w, b.track_template);
                put_optional(&mut w, b.master);
                w.u64(b.transition_frame);
                w.u64(b.tail_deadline);
            }
            SongCommand::ConfigureReusableBranch(reusable) => {
                let b = reusable.config;
                w.u32(b.branch.0);
                w.u32(b.generation);
                w.u32(b.family);
                w.u32(b.track);
                put_resource(&mut w, b.instrument);
                put_optional(&mut w, b.private_fx);
                put_optional(&mut w, b.track_template);
                put_optional(&mut w, b.master);
                w.u64(b.transition_frame);
                w.u64(b.tail_deadline);
                w.u32(reusable.last_generation);
            }
            SongCommand::RebindBranch(r) => {
                w.u32(r.branch.0);
                w.u32(r.expected_generation);
                w.u32(r.generation);
                w.u64(r.transition_frame);
                w.u64(r.tail_deadline);
            }
            SongCommand::SealPreparation(_) => {}
            SongCommand::Prepare(_) => {}
            SongCommand::Activate(a) => w.u64(a.frame),
            SongCommand::Mute(m) => {
                w.u32(m.instrument);
                w.u8(u8::from(m.muted));
                w.u64(m.frame);
            }
            SongCommand::Endpoints(e) => {
                if e.tail_deadline < e.arrangement {
                    return 0;
                }
                w.u64(e.arrangement);
                w.u64(e.tail_deadline);
            }
            SongCommand::Release(r) => {
                if r.tail_deadline < r.frame {
                    return 0;
                }
                w.u32(r.branch.0);
                w.u32(r.generation);
                w.u64(r.frame);
                w.u64(r.tail_deadline);
            }
            SongCommand::Event(e) => {
                w.u32(e.branch.0);
                w.u32(e.generation);
                w.u64(e.frame);
                w.u8(1);
                if w.failed {
                    return 0;
                }
                let n = e.event.encode(&mut w.bytes[w.pos..]);
                if n == 0 {
                    return 0;
                }
                w.pos += n;
            }
        }
        w.finish()
    }
    pub(crate) fn decode_command(bytes: &[u8]) -> Result<(SongCommand, usize), WireError> {
        let mut r = Decode { bytes, pos: 0 };
        if r.u8()? != SONG_COMMAND_TAG {
            return Err(WireError::BadTag(bytes[0]));
        }
        let kind = r.u8()?;
        let epoch = SnapshotEpoch(r.u64()?);
        let c = match kind {
            20 => SongCommand::Replace(SongReplacement {
                activation: SongActivation {
                    epoch,
                    frame: r.u64()?,
                },
                previous: SnapshotEpoch(r.u64()?),
                overlay_nonce: r.u64()?,
                overlay_count: r.u32()?,
            }),
            21 => SongCommand::PrimeMute(SongInitialMute {
                epoch,
                overlay_nonce: r.u64()?,
                instrument: r.u32()?,
                muted: r.boolean()?,
            }),
            0 => SongCommand::Prepare(epoch),
            1 => SongCommand::Activate(SongActivation {
                epoch,
                frame: r.u64()?,
            }),
            2 => SongCommand::Mute(SongMute {
                epoch,
                instrument: r.u32()?,
                muted: r.boolean()?,
                frame: r.u64()?,
            }),
            3 => {
                let arrangement = r.u64()?;
                let tail_deadline = r.u64()?;
                if tail_deadline < arrangement {
                    return Err(WireError::BadValue);
                }
                SongCommand::Endpoints(SongEndpoints {
                    epoch,
                    arrangement,
                    tail_deadline,
                })
            }
            4 => {
                let branch = SongBranchId(r.u32()?);
                let generation = r.u32()?;
                let frame = r.u64()?;
                let tail_deadline = r.u64()?;
                if tail_deadline < frame {
                    return Err(WireError::BadValue);
                }
                SongCommand::Release(SongBranchRelease {
                    epoch,
                    branch,
                    generation,
                    frame,
                    tail_deadline,
                })
            }
            5 => {
                let branch = SongBranchId(r.u32()?);
                let generation = r.u32()?;
                let frame = r.u64()?;
                if r.u8()? != 1 {
                    return Err(WireError::BadValue);
                }
                let (event, n) = AudioEvent::decode(&bytes[r.pos..])?;
                if !event.time.is_finite() {
                    return Err(WireError::BadValue);
                }
                r.pos += n;
                SongCommand::Event(SongAudioEvent {
                    epoch,
                    branch,
                    generation,
                    frame,
                    event,
                })
            }
            6 => SongCommand::BeginPreparation(SongPreparation {
                epoch,
                branches: r.u32()?,
                resources: r.u32()?,
                required: get_capacity(&mut r)?,
            }),
            7 => SongCommand::ReserveResource(SongResourceReservation {
                epoch,
                resource: get_resource(&mut r)?,
                kind: get_kind(&mut r)?,
            }),
            8 => SongCommand::ConfigureBranch(SongBranchConfig {
                epoch,
                branch: SongBranchId(r.u32()?),
                generation: r.u32()?,
                family: r.u32()?,
                track: r.u32()?,
                instrument: get_resource(&mut r)?,
                private_fx: get_optional(&mut r)?,
                track_template: get_optional(&mut r)?,
                master: get_optional(&mut r)?,
                transition_frame: r.u64()?,
                tail_deadline: r.u64()?,
            }),
            18 => SongCommand::ConfigureReusableBranch(SongReusableBranch {
                config: SongBranchConfig {
                    epoch,
                    branch: SongBranchId(r.u32()?),
                    generation: r.u32()?,
                    family: r.u32()?,
                    track: r.u32()?,
                    instrument: get_resource(&mut r)?,
                    private_fx: get_optional(&mut r)?,
                    track_template: get_optional(&mut r)?,
                    master: get_optional(&mut r)?,
                    transition_frame: r.u64()?,
                    tail_deadline: r.u64()?,
                },
                last_generation: r.u32()?,
            }),
            19 => SongCommand::RebindBranch(SongBranchRebind {
                epoch,
                branch: SongBranchId(r.u32()?),
                expected_generation: r.u32()?,
                generation: r.u32()?,
                transition_frame: r.u64()?,
                tail_deadline: r.u64()?,
            }),
            9 => SongCommand::SealPreparation(epoch),
            10 => SongCommand::BeginStaging(SongStagePreparation {
                preparation: SongPreparation {
                    epoch,
                    branches: r.u32()?,
                    resources: r.u32()?,
                    required: get_capacity(&mut r)?,
                },
                analysis_required: SongAnalysisCapacity { slots: r.u32()? },
            }),
            11 => SongCommand::InitCells(SongCellInit {
                lease: get_key_tail(&mut r, epoch)?,
                cell: crate::dsp::cells::CellId::new(r.u32()?),
                value: f32::from_bits(r.u32()?),
            }),
            12 => SongCommand::ReserveAnalysis(SongAnalysisReservation {
                lease: get_key_tail(&mut r, epoch)?,
                slots: r.u32()?,
            }),
            13 => SongCommand::CancelPreparation(epoch),
            14 => SongCommand::CancelLease(get_key_tail(&mut r, epoch)?),
            17 => SongCommand::RequestClock(SongClockRequest {
                epoch,
                request: r.u64()?,
            }),
            15 => SongCommand::RequestCapacity(epoch),
            16 => SongCommand::BindGraphBanks(SongGraphBanks {
                graph: get_key_tail(&mut r, epoch)?,
                controls: get_optional_key(&mut r, epoch)?,
                analysis: get_optional_key(&mut r, epoch)?,
            }),
            _ => return Err(WireError::BadValue),
        };
        if !c.valid() {
            return Err(WireError::BadValue);
        }
        Ok((c, r.pos))
    }
    pub(crate) fn encode_ack(a: SongHostAck, out: &mut [u8]) -> usize {
        let mut w = Codec {
            bytes: out,
            pos: 0,
            failed: false,
        };
        w.u8(SONG_ACK_TAG);
        w.u8(match a {
            SongHostAck::ActivationRejected { .. } => 14,
            SongHostAck::BranchRebound(_) => 13,
            SongHostAck::ClockReport(_) => 11,
            SongHostAck::ClockRejected(_) => 12,
            SongHostAck::CapacityReport(_) => 9,
            SongHostAck::CapacityRejected { .. } => 10,
            SongHostAck::LeaseReturned(_) => 6,
            SongHostAck::PreparationCancelled(_) => 7,
            SongHostAck::SliceAccepted { .. } => 8,
            SongHostAck::ResourceReady { .. } => 4,
            SongHostAck::ResourceRetired { .. } => 5,
            SongHostAck::Ready(_) => 0,
            SongHostAck::Applied(_) => 1,
            SongHostAck::Muted(_) => 2,
            SongHostAck::Rejected { .. } => 3,
        });
        w.u64(a.epoch().0);
        match a {
            SongHostAck::ActivationRejected { activation, reason } => {
                w.u64(activation.frame);
                w.u8(reason as u8);
            }
            SongHostAck::BranchRebound(r) => {
                w.u32(r.branch.0);
                w.u32(r.generation);
                w.u64(r.frame);
            }
            SongHostAck::ClockReport(r) => {
                w.u64(r.request.request);
                w.u64(r.clock.frame);
                w.u32(r.clock.sample_rate);
            }
            SongHostAck::ClockRejected(r) => {
                w.u64(r.request.request);
                w.u8(r.reason as u8);
            }
            SongHostAck::CapacityReport(r) => {
                w.u64(r.serial);
                put_capacity(&mut w, r.available);
                w.u32(r.analysis.slots);
            }
            SongHostAck::LeaseReturned(k) => put_key_tail(&mut w, k),
            SongHostAck::PreparationCancelled(_) => {}
            SongHostAck::SliceAccepted { lease, offset } => {
                put_key_tail(&mut w, lease);
                w.u32(offset);
            }
            SongHostAck::ResourceReady { resource, .. }
            | SongHostAck::ResourceRetired { resource, .. } => put_resource(&mut w, resource),
            SongHostAck::Ready(_) => {}
            SongHostAck::Applied(a) => w.u64(a.frame),
            SongHostAck::Muted(m) => {
                w.u32(m.instrument);
                w.u8(u8::from(m.muted));
                w.u64(m.frame)
            }
            SongHostAck::Rejected { reason, .. } | SongHostAck::CapacityRejected { reason, .. } => {
                w.u8(reason as u8)
            }
        }
        w.finish()
    }
    pub(crate) fn decode_ack(bytes: &[u8]) -> Result<(SongHostAck, usize), WireError> {
        let mut r = Decode { bytes, pos: 0 };
        if r.u8()? != SONG_ACK_TAG {
            return Err(WireError::BadTag(bytes[0]));
        }
        let kind = r.u8()?;
        let epoch = SnapshotEpoch(r.u64()?);
        let ack = match kind {
            0 => SongHostAck::Ready(epoch),
            1 => SongHostAck::Applied(SongActivation {
                epoch,
                frame: r.u64()?,
            }),
            2 => SongHostAck::Muted(SongMute {
                epoch,
                instrument: r.u32()?,
                muted: r.boolean()?,
                frame: r.u64()?,
            }),
            3 => SongHostAck::Rejected {
                epoch,
                reason: get_reject(&mut r)?,
            },
            4 => SongHostAck::ResourceReady {
                epoch,
                resource: get_resource(&mut r)?,
            },
            5 => SongHostAck::ResourceRetired {
                epoch,
                resource: get_resource(&mut r)?,
            },
            6 => SongHostAck::LeaseReturned(get_key_tail(&mut r, epoch)?),
            7 => SongHostAck::PreparationCancelled(epoch),
            8 => SongHostAck::SliceAccepted {
                lease: get_key_tail(&mut r, epoch)?,
                offset: r.u32()?,
            },
            9 => SongHostAck::CapacityReport(SongCapacityReport {
                epoch,
                serial: r.u64()?,
                available: get_capacity(&mut r)?,
                analysis: SongAnalysisCapacity { slots: r.u32()? },
            }),
            11 => SongHostAck::ClockReport(SongClockReport {
                request: SongClockRequest {
                    epoch,
                    request: r.u64()?,
                },
                clock: SongHostClock {
                    frame: r.u64()?,
                    sample_rate: r.u32()?,
                },
            }),
            12 => SongHostAck::ClockRejected(SongClockFailure {
                request: SongClockRequest {
                    epoch,
                    request: r.u64()?,
                },
                reason: get_reject(&mut r)?,
            }),
            10 => SongHostAck::CapacityRejected {
                epoch,
                reason: get_reject(&mut r)?,
            },
            13 => SongHostAck::BranchRebound(SongBranchRebound {
                epoch,
                branch: SongBranchId(r.u32()?),
                generation: r.u32()?,
                frame: r.u64()?,
            }),
            14 => SongHostAck::ActivationRejected {
                activation: SongActivation {
                    epoch,
                    frame: r.u64()?,
                },
                reason: get_reject(&mut r)?,
            },
            _ => return Err(WireError::BadValue),
        };
        Ok((ack, r.pos))
    }
    fn get_reject(r: &mut Decode<'_>) -> Result<SongRejectCode, WireError> {
        match r.u8()? {
            0 => Ok(SongRejectCode::StaleEpoch),
            1 => Ok(SongRejectCode::NotReady),
            2 => Ok(SongRejectCode::Capacity),
            3 => Ok(SongRejectCode::Malformed),
            4 => Ok(SongRejectCode::HostFault),
            _ => Err(WireError::BadValue),
        }
    }
    fn put_key_tail(w: &mut Codec<'_>, k: SongLeaseKey) {
        put_resource(w, k.resource);
        w.u8(k.kind as u8);
    }
    fn get_key_tail(r: &mut Decode<'_>, epoch: SnapshotEpoch) -> Result<SongLeaseKey, WireError> {
        Ok(SongLeaseKey {
            epoch,
            resource: get_resource(r)?,
            kind: get_kind(r)?,
        })
    }
    fn put_optional_key(w: &mut Codec<'_>, key: Option<SongLeaseKey>) {
        w.u8(u8::from(key.is_some()));
        if let Some(key) = key {
            put_key_tail(w, key);
        }
    }
    fn get_optional_key(
        r: &mut Decode<'_>,
        epoch: SnapshotEpoch,
    ) -> Result<Option<SongLeaseKey>, WireError> {
        if r.boolean()? {
            Ok(Some(get_key_tail(r, epoch)?))
        } else {
            Ok(None)
        }
    }
    fn get_kind(r: &mut Decode<'_>) -> Result<SongResourceKind, WireError> {
        match r.u8()? {
            0 => Ok(SongResourceKind::Instrument),
            1 => Ok(SongResourceKind::PrivateFx),
            2 => Ok(SongResourceKind::Track),
            3 => Ok(SongResourceKind::Master),
            4 => Ok(SongResourceKind::Sample),
            7 => Ok(SongResourceKind::ControlCells),
            8 => Ok(SongResourceKind::AnalysisBank),
            _ => Err(WireError::BadValue),
        }
    }
    fn put_resource(w: &mut Codec<'_>, r: SongResourceRef) {
        w.u32(r.id);
        w.u32(r.generation);
    }
    fn get_resource(r: &mut Decode<'_>) -> Result<SongResourceRef, WireError> {
        Ok(SongResourceRef {
            id: r.u32()?,
            generation: r.u32()?,
        })
    }
    fn put_optional(w: &mut Codec<'_>, r: Option<SongResourceRef>) {
        w.u8(u8::from(r.is_some()));
        if let Some(r) = r {
            put_resource(w, r);
        }
    }
    fn get_optional(r: &mut Decode<'_>) -> Result<Option<SongResourceRef>, WireError> {
        if r.boolean()? {
            Ok(Some(get_resource(r)?))
        } else {
            Ok(None)
        }
    }
    fn put_capacity(w: &mut Codec<'_>, c: SongHostCapacities) {
        for v in [
            c.sample_rate,
            c.cell_slots,
            c.voice_slots,
            c.template_slots,
            c.bus_slots,
            c.sample_resources,
            c.ack_slots,
        ] {
            w.u32(v);
        }
        for v in [c.pcm_bytes, c.voice_frames, c.bus_frames] {
            w.u64(v);
        }
    }
    fn get_capacity(r: &mut Decode<'_>) -> Result<SongHostCapacities, WireError> {
        Ok(SongHostCapacities {
            sample_rate: r.u32()?,
            cell_slots: r.u32()?,
            voice_slots: r.u32()?,
            template_slots: r.u32()?,
            bus_slots: r.u32()?,
            sample_resources: r.u32()?,
            ack_slots: r.u32()?,
            pcm_bytes: r.u64()?,
            voice_frames: r.u64()?,
            bus_frames: r.u64()?,
        })
    }
}
