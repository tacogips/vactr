//! Song-specific checked admission and upload ownership.
use super::*;

impl WasmAudioHost {
    pub(super) fn song_clock_observation(
        &self,
    ) -> Result<crate::song::routing::SongHostClock, Failure> {
        let state = self.0.borrow();
        if let Some(message) = state.song_clock_error {
            return Err(Failure::new(FailCode::HostUnavailable, message));
        }
        if state.song_clock_pending.is_some() {
            return Err(Failure::new(
                FailCode::HostUnavailable,
                "exact song clock pending",
            ));
        }
        state
            .song_clock_report
            .ok_or_else(|| Failure::new(FailCode::HostUnavailable, "exact song clock unavailable"))
    }

    #[allow(clippy::result_large_err)] // Preserve the original complete command on refusal.
    pub(super) fn try_song_command_checked(
        &mut self,
        command: crate::song::routing::SongCommand,
    ) -> Result<(), crate::host::caps::SongCommandRefusal> {
        use crate::host::caps::{SongCommandRefusal, SongSubmitError};
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        if n == 0 {
            return Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Invalid(crate::vm::fail::Failure::new(
                    crate::vm::fail::FailCode::Type,
                    "invalid song command",
                )),
            });
        }
        if let crate::song::routing::SongCommand::RequestClock(request) = command {
            if let Some(last) = self.0.borrow().song_clock_last {
                if last.epoch == request.epoch && request.request <= last.request {
                    return Err(SongCommandRefusal {
                        command,
                        error: SongSubmitError::Invalid(Failure::new(
                            if last.request == u64::MAX {
                                FailCode::Overflow
                            } else {
                                FailCode::Type
                            },
                            "song clock nonce must advance",
                        )),
                    });
                }
            }
        }
        if push_record(&bytes[..n]) {
            if let crate::song::routing::SongCommand::RequestClock(request) = command {
                let mut state = self.0.borrow_mut();
                state.song_clock_last = Some(request);
                state.song_clock_pending = Some(request);
                state.song_clock_error = None;
            }
            Ok(())
        } else {
            Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Backpressure,
            })
        }
    }

    pub(super) fn try_song_graph_checked(
        &mut self,
        lease: SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<(), crate::host::caps::SongSubmitError> {
        use crate::dsp::arena::{decode_graph, GraphKind};
        use crate::dsp::bus::BusTemplate;
        use crate::dsp::ugen::{Node, RawGraph};
        use crate::host::caps::SongSubmitError;
        let invalid =
            |message: &str| SongSubmitError::Invalid(Failure::new(FailCode::Type, message));
        let mut bytes = Vec::new();
        let result = match graph {
            GraphHandle::Inst { id, def }
                if lease.kind == SongResourceKind::Instrument && *id == def.id =>
            {
                encode_inst(def, &mut bytes)
            }
            GraphHandle::Bus { id, def }
                if matches!(
                    lease.kind,
                    SongResourceKind::PrivateFx | SongResourceKind::Track
                ) && *id == def.id =>
            {
                encode_bus(def, false, &mut bytes)
            }
            GraphHandle::Master(def) if lease.kind == SongResourceKind::Master => {
                encode_bus(def, true, &mut bytes)
            }
            _ => return Err(invalid("song graph kind or identity mismatch")),
        };
        result.map_err(|error| invalid(error.message()))?;
        if bytes.len() > SLICE_BYTES {
            return Err(invalid("song graph payload too large"));
        }
        // Validate using the same bounded structural decoder as the worklet.
        // Capability, bank and actual memory admission remain Engine authority.
        let mut raw = RawGraph::boxed();
        let mut bus = BusTemplate::new();
        let kind = decode_graph(&bytes, &mut raw, &mut bus)
            .map_err(|_| invalid("malformed song graph encoding"))?;
        let finite = |control: Ctl| match control {
            Ctl::Const(value) => value.is_finite(),
            Ctl::Cell(_) => true,
        };
        let valid = match kind {
            GraphKind::Inst => {
                raw.n_nodes > 0
                    && raw.params[..raw.n_params].iter().all(|(_, c)| finite(*c))
                    && raw.node_params[..raw.n_node_params]
                        .iter()
                        .all(|(node, _, c)| usize::from(*node) < raw.n_nodes && finite(*c))
                    && raw.nodes[..raw.n_nodes]
                        .iter()
                        .all(|node| !matches!(node, Node::Const(value) if !value.is_finite()))
            }
            GraphKind::Bus(_) | GraphKind::Master => (0..bus.n).all(|effect| {
                bus.params[effect][..usize::from(bus.n_params[effect])]
                    .iter()
                    .all(|(_, c)| finite(*c))
            }),
        };
        if !valid {
            return Err(invalid("invalid song graph controls or nodes"));
        }
        let length = 22usize
            .checked_add(bytes.len())
            .ok_or_else(|| invalid("song graph record too large"))?;
        if length > crate::dsp::ring::INBOX_SLOT_BYTES
            || !crate::host::wasm::abi::record_fits_empty_outbox(length)
        {
            return Err(invalid("song graph cannot fit empty transport"));
        }
        let mut record = vec![0; length];
        let n = encode_song_graph_record(lease, &bytes, &mut record);
        if n == 0 {
            return Err(invalid("invalid song graph record"));
        }
        if !push_record(&record[..n]) {
            return Err(SongSubmitError::Backpressure);
        }
        Ok(())
    }

    pub(super) fn song_graph(
        &mut self,
        lease: SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<(), Failure> {
        let mut bytes = Vec::new();
        let result = match graph {
            GraphHandle::Inst { def, .. } if lease.kind == SongResourceKind::Instrument => {
                encode_inst(def, &mut bytes)
            }
            GraphHandle::Bus { def, .. }
                if matches!(
                    lease.kind,
                    SongResourceKind::PrivateFx | SongResourceKind::Track
                ) =>
            {
                encode_bus(def, false, &mut bytes)
            }
            GraphHandle::Master(def) if lease.kind == SongResourceKind::Master => {
                encode_bus(def, true, &mut bytes)
            }
            _ => {
                return Err(Failure::new(
                    FailCode::Type,
                    "song graph lease kind mismatch",
                ))
            }
        };
        result.map_err(|e| Failure::new(FailCode::HostUnavailable, e.message()))?;
        let mut record = vec![0; 22 + bytes.len()];
        let n = encode_song_graph_record(lease, &bytes, &mut record);
        if n == 0 || !push_record(&record[..n]) {
            return Err(Failure::new(
                FailCode::HostUnavailable,
                "song graph upload refused",
            ));
        }
        Ok(())
    }
    pub(super) fn song_sample_checked(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), crate::host::caps::SongSampleRefusal> {
        use crate::host::caps::{SongSampleRefusal, SongSubmitError};
        let invalid = |data, message| SongSampleRefusal {
            lease,
            data,
            error: SongSubmitError::Invalid(Failure::new(FailCode::HostUnavailable, message)),
        };
        let mut s = self.0.borrow_mut();
        let Some(bytes) = data.frames.len().checked_mul(4) else {
            return Err(invalid(data, "song sample byte count overflow"));
        };
        let fits = s
            .arena_used
            .checked_add(s.song_bytes)
            .and_then(|v| v.checked_add(bytes))
            .is_some_and(|n| n <= s.arena_bytes);
        if lease.kind != SongResourceKind::Sample
            || !(1..=2).contains(&data.channels)
            || !(8000..=192000).contains(&data.rate)
            || data.frames.len() % usize::from(data.channels) != 0
            || u32::try_from(data.frames.len()).is_err()
            || data.frames.iter().any(|v| !v.is_finite())
            || !fits
            || s.song_reservations.len() >= crate::dsp::ring::INBOX_SLOTS
            || s.song_reservations
                .iter()
                .any(|(key, _)| key.epoch == lease.epoch && key.resource == lease.resource)
        {
            return Err(invalid(
                data,
                "song sample geometry or sender capacity refused",
            ));
        }
        s.song_bytes += bytes;
        s.song_reservations.push((lease, bytes));
        s.song_queue.push_back(SongPending {
            lease,
            data,
            next: 0,
            begun: false,
        });
        s.pump_song();
        Ok(())
    }
    pub(super) fn song_sample(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.song_sample_checked(lease, data)
            .map_err(|refusal| refusal.data)
    }
    pub(super) fn sample_sender_capacity(
        &self,
    ) -> Result<crate::host::caps::SongSampleSenderCapacity, Failure> {
        let s = self.0.borrow();
        let resources = crate::dsp::ring::INBOX_SLOTS
            .checked_sub(s.song_reservations.len())
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| {
                Failure::new(FailCode::Overflow, "sample sender metadata count overflow")
            })?;
        let pcm_bytes = s
            .arena_used
            .checked_add(s.song_bytes)
            .and_then(|used| s.arena_bytes.checked_sub(used))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| Failure::new(FailCode::Overflow, "sample sender byte count overflow"))?;
        Ok(crate::host::caps::SongSampleSenderCapacity::Bounded {
            resources,
            pcm_bytes,
        })
    }
}

impl HostState {
    pub(super) fn pump_song(&mut self) {
        if self.song_flight.is_some() {
            return;
        }
        let Some(p) = self.song_queue.front_mut() else {
            return;
        };
        if !p.begun {
            let mut begin = [0; 27];
            let frames = u32::try_from(p.data.frames.len() / usize::from(p.data.channels))
                .unwrap_or(u32::MAX);
            let n =
                encode_song_sample_begin(p.lease, frames, p.data.channels, p.data.rate, &mut begin);
            if n == 0 || !push_record(&begin[..n]) {
                return;
            }
            p.begun = true;
        }
        let n = SLICE_FLOATS.min(p.data.frames.len().saturating_sub(p.next));
        let Ok(offset) = u32::try_from(p.next) else {
            return;
        };
        self.buf.resize(26 + n * 4, 0);
        let len = encode_song_slice(
            p.lease,
            offset,
            &p.data.frames[p.next..p.next + n],
            &mut self.buf,
        );
        if len > 0 && push_record(&self.buf[..len]) {
            self.song_flight = Some((p.lease, offset, n));
        }
    }
    fn observe_song_clock(&mut self, ack: SongHostAck) {
        match ack {
            SongHostAck::ClockReport(report) if self.song_clock_pending == Some(report.request) => {
                self.song_clock_pending = None;
                let clock = report.clock;
                self.song_clock_error = if !(8000..=192000).contains(&clock.sample_rate) {
                    Some("invalid song clock sample rate")
                } else if self
                    .song_clock_report
                    .is_some_and(|old| old.sample_rate != clock.sample_rate)
                {
                    Some("song clock sample rate mismatch")
                } else if self
                    .song_clock_report
                    .is_some_and(|old| old.frame > clock.frame)
                {
                    Some("song clock frame regressed")
                } else {
                    self.song_clock_report = Some(clock);
                    None
                };
            }
            SongHostAck::ClockRejected(failure)
                if self.song_clock_pending == Some(failure.request) =>
            {
                self.song_clock_pending = None;
                self.song_clock_error = Some("exact song clock request rejected");
            }
            _ => {}
        }
    }
    pub(super) fn song_ack(&mut self, ack: SongHostAck) {
        self.observe_song_clock(ack);
        match ack {
            SongHostAck::SliceAccepted { lease, offset }
                if self
                    .song_flight
                    .is_some_and(|f| (f.0, f.1) == (lease, offset)) =>
            {
                let n = self.song_flight.take().map_or(0, |f| f.2);
                if let Some(p) = self.song_queue.front_mut() {
                    p.next += n;
                    if p.next >= p.data.frames.len() {
                        self.song_queue.pop_front();
                    }
                }
            }
            SongHostAck::LeaseReturned(lease) => {
                if self.song_flight.is_some_and(|f| f.0 == lease) {
                    self.song_flight = None;
                }
                self.song_queue.retain(|p| p.lease != lease);
                self.song_reservations.retain(|(key, _)| *key != lease);
                self.song_bytes = self.song_reservations.iter().map(|(_, n)| *n).sum();
            }
            SongHostAck::Rejected { epoch, .. } | SongHostAck::PreparationCancelled(epoch) => {
                if self.song_flight.is_some_and(|f| f.0.epoch == epoch) {
                    self.song_flight = None;
                }
                self.song_queue.retain(|p| p.lease.epoch != epoch);
                // A generic epoch rejection stops transfer, but does not return
                // accepted physical leases. Only exact LeaseReturned releases charge.
            }
            _ => {}
        }
        self.pump_song();
    }
}
