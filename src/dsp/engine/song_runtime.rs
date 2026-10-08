//! Exact finite-song lifecycle and private rendering.
use super::*;
mod delay;
mod replacement;
mod reuse;
use crate::dsp::effects::FxCtx;
use crate::dsp::song::{RuntimeBranch, RuntimeTrack};
use crate::dsp::song::{SongEpochEnd, SongGainGate};
use crate::dsp::voice::event_ctl;
use crate::song::routing::{SongCommand, SongHostAck, SongLeaseKey, SongResourceKind};
use crate::song::routing::{SongEndpoints, SongMute};
use crate::song::SnapshotEpoch;

impl Engine {
    pub(super) fn apply_song_runtime_frame(
        &mut self,
        frame: u64,
        acks: &mut AckProducer,
    ) -> Result<(), SongRejectCode> {
        self.advance_song_replacements(frame);
        self.finish_song_deadlines(frame);
        self.flush_runtime_receipts(acks);
        self.apply_song_ends_under_pressure(frame);
        loop {
            let runtime = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
            let Some(command) = runtime
                .commands
                .first()
                .copied()
                .filter(|c| c.frame <= frame)
            else {
                break;
            };
            if runtime.receipts.len() == runtime.receipts.capacity() {
                if command.failure.is_none() {
                    let result = match command.command {
                        SongCommand::Event(event) => Some(self.start_song_event(event)),
                        SongCommand::Release(r) => Some(self.end_song_branches(
                            r.epoch,
                            Some((r.branch, r.generation)),
                            r.tail_deadline,
                        )),
                        SongCommand::Endpoints(e) => Some(self.apply_song_end(e)),
                        _ => None,
                    };
                    if let Some(result) = result {
                        match result {
                            Ok(()) => {
                                if let Some(runtime) = &mut self.song_runtime {
                                    runtime.commands.remove(0);
                                }
                                self.finish_song_deadlines(frame);
                                continue;
                            }
                            Err(reason) => {
                                if let Some(runtime) = &mut self.song_runtime {
                                    runtime.commands[0].failure = Some(reason);
                                }
                            }
                        }
                    }
                }
                break;
            }
            let receipt = if let Some(reason) = command.failure {
                Some(match command.command {
                    SongCommand::Activate(activation) => {
                        SongHostAck::ActivationRejected { activation, reason }
                    }
                    _ => SongHostAck::Rejected {
                        epoch: command.command.epoch(),
                        reason,
                    },
                })
            } else {
                match command.command {
                    SongCommand::RebindBranch(rebind) => {
                        Some(match self.rebind_song_branch(rebind, frame) {
                            Ok(receipt) => SongHostAck::BranchRebound(receipt),
                            Err(reason) => SongHostAck::Rejected {
                                epoch: rebind.epoch,
                                reason,
                            },
                        })
                    }
                    SongCommand::Activate(mut a) => match self.activate_song_epoch(a.epoch) {
                        Ok(()) => {
                            a.frame = frame;
                            if let Some(r) = &mut self.song_runtime {
                                let endpoint = r.commands.iter().find_map(|c| match c.command {
                                    SongCommand::Endpoints(e) if e.epoch == a.epoch => Some(e),
                                    _ => None,
                                });
                                r.last_applied_owner = Some(crate::dsp::song::SongAppliedOwner {
                                    activation: a,
                                    endpoint,
                                    musically_closed: false,
                                });
                            }
                            Some(SongHostAck::Applied(a))
                        }
                        Err(reason) => Some(SongHostAck::ActivationRejected {
                            activation: a,
                            reason,
                        }),
                    },
                    SongCommand::Event(event) => {
                        self.start_song_event(event)
                            .err()
                            .map(|reason| SongHostAck::Rejected {
                                epoch: event.epoch,
                                reason,
                            })
                    }
                    SongCommand::Release(release) => self
                        .end_song_branches(
                            release.epoch,
                            Some((release.branch, release.generation)),
                            release.tail_deadline,
                        )
                        .err()
                        .map(|reason| SongHostAck::Rejected {
                            epoch: release.epoch,
                            reason,
                        }),
                    SongCommand::Endpoints(end) => {
                        self.apply_song_end(end)
                            .err()
                            .map(|reason| SongHostAck::Rejected {
                                epoch: end.epoch,
                                reason,
                            })
                    }
                    SongCommand::Mute(m) => Some(match self.mute_song_family(m, frame) {
                        Ok(m) => SongHostAck::Muted(m),
                        Err(reason) => SongHostAck::Rejected {
                            epoch: m.epoch,
                            reason,
                        },
                    }),
                    _ => None,
                }
            };
            if let Some(runtime) = &mut self.song_runtime {
                runtime.commands.remove(0);
                if let Some(receipt) = receipt {
                    runtime.receipts.push(receipt);
                }
            }
            self.finish_song_deadlines(frame);
            self.flush_runtime_receipts(acks);
        }
        Ok(())
    }
    pub(in crate::dsp::engine) fn flush_runtime_receipts(&mut self, acks: &mut AckProducer) {
        if self.song_ack.is_some() || self.song_followup.is_some() {
            return;
        }
        if let Some(runtime) = &mut self.song_runtime {
            while runtime.receipts.len() < runtime.receipts.capacity() {
                let next = runtime
                    .replacements
                    .iter()
                    .enumerate()
                    .flat_map(|(index, record)| {
                        record
                            .outcomes
                            .iter()
                            .enumerate()
                            .filter_map(move |(slot, outcome)| {
                                outcome.map(|outcome| (index, slot, outcome))
                            })
                    })
                    .min_by_key(|(index, slot, outcome)| {
                        (
                            outcome.frame,
                            !outcome.before_boundary_commit,
                            *index,
                            *slot,
                        )
                    });
                let Some((index, slot, outcome)) = next else {
                    break;
                };
                if runtime
                    .commands
                    .iter()
                    .any(|c| c.frame < outcome.frame && c.frame <= self.frame)
                {
                    break;
                }
                runtime.replacements[index].outcomes[slot] = None;
                runtime.receipts.push(outcome.acknowledgment);
            }
            while let Some(receipt) = runtime.receipts.first().copied() {
                if acks.push_critical(HostMsg::Song(receipt)).is_err() {
                    break;
                }
                runtime.receipts.remove(0);
            }
        }
    }
    fn start_song_event(
        &mut self,
        event: crate::song::routing::SongAudioEvent,
    ) -> Result<(), SongRejectCode> {
        let runtime = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        let branch = runtime
            .branches
            .iter()
            .copied()
            .find(|b| b.matches(event))
            .ok_or(SongRejectCode::StaleEpoch)?;
        if event.frame < self.frame {
            return Err(SongRejectCode::Malformed);
        }
        if !runtime.owns_lease(branch.owner().instrument)
            || event.frame < branch.config.transition_frame
            || (!branch.ended && event.frame >= branch.config.tail_deadline)
        {
            return Err(SongRejectCode::NotReady);
        }
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let key = branch.owner().instrument;
        let mut audio = event.event;
        for (_, ctl) in &mut audio.ctl[..usize::from(audio.n_ctl)] {
            if let Ctl::Cell(logical) = *ctl {
                *ctl = Ctl::Cell(stager.bound_cell(key, logical)?);
            }
        }
        let cells = stager.private_cells(key);
        if let Some(bank) = event_ctl(&audio, BANK) {
            let id = resolve(bank, &cells);
            if !id.is_finite() || id < 0.0 || id.fract() != 0.0 {
                return Err(SongRejectCode::Malformed);
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let id = id as u32;
            let physical = stager.sample_id(&self.store, event.epoch, id)?;
            for (ctl, value) in &mut audio.ctl[..usize::from(audio.n_ctl)] {
                if *ctl == BANK {
                    #[allow(clippy::cast_precision_loss)]
                    {
                        *value = Ctl::Const(physical as f32);
                    }
                }
            }
        }
        let (delay_time, delay_feedback) = Self::validate_private_delay_controls(&audio, &cells)?;
        let destination = branch.fx.ok_or(SongRejectCode::NotReady)?;
        let delay_key = branch.fx_key()?;
        self.buses.slots[destination].song_private_layout(
            delay_key,
            u32::try_from(destination).map_err(|_| SongRejectCode::Capacity)?,
        )?;
        let template = self
            .templates
            .get(branch.template)
            .ok_or(SongRejectCode::NotReady)?;
        if audio.inst != template.inst {
            return Err(SongRejectCode::Malformed);
        }
        #[allow(clippy::cast_precision_loss)]
        let time = self.frame as f64 / f64::from(self.sr);
        // Suppressed occurrences are consumed after ownership/resource validation.
        if branch.muted || branch.ended {
            return Ok(());
        }
        let index = self
            .pool
            .voices
            .iter()
            .position(|v| {
                !v.active
                    && self
                        .templates
                        .get(branch.template)
                        .is_some_and(|t| v.mem_len() >= t.mem_total)
            })
            .ok_or(SongRejectCode::Capacity)?;
        self.buses.slots[destination].set_song_delay(delay_key, delay_time, delay_feedback)?;
        let seed = self.pool.seed();
        self.pool.voices[index].start(
            template,
            branch.template,
            &audio,
            None,
            0,
            time,
            destination,
            &cells,
            self.sr,
            &self.cfg.caps,
            seed,
        );
        self.buses.slots[destination].users += 1;
        self.output.admit(self.frame);
        self.buses.slots[destination].set_room(
            event_ctl(&audio, ROOM).map(|c| resolve(c, &cells)),
            event_ctl(&audio, SIZE).map(|c| resolve(c, &cells)),
        );
        if let Some(runtime) = &mut self.song_runtime {
            runtime.voices[index] = Some(branch.owner());
        }
        Ok(())
    }
}

impl Engine {
    pub(super) fn render_song_branches<C: CellRead + ?Sized>(
        &mut self,
        _legacy: &C,
        frames: usize,
    ) -> Result<(), SongRejectCode> {
        let Self {
            song_runtime,
            song_stager,
            buses,
            store,
            sr,
            cfg,
            fft,
            dry,
            fx_scratch,
            counters,
            mix_l,
            mix_r,
            frame,
            ..
        } = self;
        let Some(runtime) = song_runtime.as_ref() else {
            return Ok(());
        };
        let stager = song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
        // Private branch effects precede track sums. No shared orbit is touched.
        for branch in &runtime.branches {
            if branch.closed || (branch.ended && *frame >= branch.config.tail_deadline) {
                continue;
            }
            let index = branch.fx.ok_or(SongRejectCode::NotReady)?;
            let key = buses.slots[index]
                .song_key
                .ok_or(SongRejectCode::StaleEpoch)?;
            if key != branch.fx_key()? || !buses.song_is_live(key, index) {
                return Err(SongRejectCode::StaleEpoch);
            }
            let (cells, analysis) = stager.graph_views(key)?;
            let scope = store.song_read_scope(Some(key.epoch));
            let mut ctx = FxCtx {
                sr: *sr,
                store: &scope,
                fft,
                caps: &cfg.caps,
                scratch: fx_scratch,
                analysis,
                stats: &mut counters.fx,
            };
            buses.run_song(index, frames, &cells, dry, &mut ctx);
            buses.slots[index].run_song_delay(key, 0, frames, *sr)?;
            if let Some(start) = branch.release_frame {
                for k in 0..frames {
                    let gain = SongGainGate::tail(
                        frame.saturating_add(k as u64),
                        start,
                        branch.config.tail_deadline,
                    );
                    buses.slots[index].l[k] *= gain;
                    buses.slots[index].r[k] *= gain;
                }
            }
            buses.add_song_output_gated(index, branch.track, frames, *frame, branch.gate);
        }
        for (i, branch) in runtime.tracks.iter().enumerate() {
            if !branch.renderable(
                buses,
                *frame,
                runtime.endpoints.iter().find(|e| e.epoch == branch.epoch),
            )? {
                continue;
            }
            if runtime.tracks[..i].iter().any(|b| b.track == branch.track) {
                continue;
            }
            let key = buses.slots[branch.track]
                .song_key
                .ok_or(SongRejectCode::StaleEpoch)?;
            if key != branch.track_key || !buses.song_is_live(key, branch.track) {
                return Err(SongRejectCode::StaleEpoch);
            }
            let (cells, analysis) = stager.graph_views(key)?;
            let scope = store.song_read_scope(Some(key.epoch));
            let mut ctx = FxCtx {
                sr: *sr,
                store: &scope,
                fft,
                caps: &cfg.caps,
                scratch: fx_scratch,
                analysis,
                stats: &mut counters.fx,
            };
            buses.run_song(branch.track, frames, &cells, dry, &mut ctx);
            buses.add_song_output(branch.track, branch.master, frames);
        }
        for (i, branch) in runtime.tracks.iter().enumerate() {
            if !branch.renderable(
                buses,
                *frame,
                runtime.endpoints.iter().find(|e| e.epoch == branch.epoch),
            )? {
                continue;
            }
            if runtime.tracks[..i]
                .iter()
                .any(|b| b.master == branch.master)
            {
                continue;
            }
            let key = buses.slots[branch.master]
                .song_key
                .ok_or(SongRejectCode::StaleEpoch)?;
            if key != branch.master_key || !buses.song_is_live(key, branch.master) {
                return Err(SongRejectCode::StaleEpoch);
            }
            let (cells, analysis) = stager.graph_views(key)?;
            let scope = store.song_read_scope(Some(key.epoch));
            let mut ctx = FxCtx {
                sr: *sr,
                store: &scope,
                fft,
                caps: &cfg.caps,
                scratch: fx_scratch,
                analysis,
                stats: &mut counters.fx,
            };
            buses.run_song(branch.master, frames, &cells, dry, &mut ctx);
            let (left, right) = buses.frames(branch.master, frames);
            for k in 0..frames {
                let gain = runtime
                    .endpoints
                    .iter()
                    .find(|e| e.epoch == key.epoch)
                    .map_or(1.0, |e| {
                        SongGainGate::tail(
                            frame.saturating_add(k as u64),
                            e.arrangement,
                            e.deadline,
                        )
                    });
                let gain =
                    gain * runtime.replacement_gain(key.epoch, frame.saturating_add(k as u64));
                mix_l[k] += left[k] * gain;
                mix_r[k] += right[k] * gain;
            }
        }
        Ok(())
    }
}

impl Engine {
    fn end_song_branches(
        &mut self,
        epoch: SnapshotEpoch,
        selected: Option<(crate::song::routing::SongBranchId, u32)>,
        deadline: u64,
    ) -> Result<(), SongRejectCode> {
        let runtime = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        let mut found = false;
        for branch in &mut runtime.branches {
            if branch.config.epoch != epoch
                || selected.is_some_and(|(id, generation)| {
                    id != branch.config.branch || generation != branch.config.generation
                })
            {
                continue;
            }
            found = true;
            branch.ended = true;
            branch.config.tail_deadline = deadline;
            branch.release_frame = selected.map(|_| self.frame);
            for (voice, owner) in self.pool.voices.iter_mut().zip(runtime.voices.iter()) {
                if owner.is_some_and(|owner| owner == branch.owner()) && voice.active {
                    voice.release();
                }
            }
        }
        if found {
            Ok(())
        } else {
            Err(SongRejectCode::StaleEpoch)
        }
    }
}

impl Engine {
    fn mute_song_family(
        &mut self,
        mut m: SongMute,
        frame: u64,
    ) -> Result<SongMute, SongRejectCode> {
        frame.checked_add(64).ok_or(SongRejectCode::Malformed)?;
        let runtime = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        if !runtime.owns_epoch(m.epoch) {
            return Err(SongRejectCode::StaleEpoch);
        }
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let mut found = false;
        for b in runtime
            .branches
            .iter()
            .filter(|b| b.config.epoch == m.epoch && b.config.family == m.instrument)
        {
            found = true;
            let fx = b.fx.ok_or(SongRejectCode::NotReady)?;
            let key = self.buses.slots[fx]
                .song_key
                .ok_or(SongRejectCode::StaleEpoch)?;
            if key != b.fx_key()?
                || !self.buses.song_is_live(key, fx)
                || !runtime.owns_lease(key)
                || !self.buses.can_reset_song_bus(key)
                || self.templates.get(b.template).is_none()
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        if !found {
            return Err(SongRejectCode::StaleEpoch);
        }
        let runtime = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        for branch in &mut runtime.branches {
            if branch.config.epoch != m.epoch
                || branch.config.family != m.instrument
                || branch.muted == m.muted
            {
                continue;
            }
            let key = self.buses.slots[branch.fx.ok_or(SongRejectCode::NotReady)?]
                .song_key
                .ok_or(SongRejectCode::StaleEpoch)?;
            self.buses
                .reset_song_bus(key, &stager.private_cells(key), self.sr, &self.cfg.caps)?;
            for (v, owner) in self.pool.voices.iter_mut().zip(runtime.voices.iter_mut()) {
                if *owner != Some(branch.owner()) {
                    continue;
                }
                if m.muted {
                    if let Some(t) = self.templates.get(branch.template) {
                        v.reset_song_voice_state(
                            t,
                            &stager.private_cells(branch.owner().instrument),
                            self.sr,
                            &self.cfg.caps,
                        );
                    }
                } else {
                    if v.active {
                        self.buses.slots[v.bus].users =
                            self.buses.slots[v.bus].users.saturating_sub(1);
                    }
                    v.stop_song_voice();
                    *owner = None;
                }
            }
            branch.muted = m.muted;
            branch.closed = false;
            branch
                .gate
                .retarget(frame, if m.muted { 0.0 } else { 1.0 })?;
        }
        if m.muted {
            let branches = &runtime.branches;
            runtime.commands.retain(|c| c.failure.is_some() || !matches!(c.command, SongCommand::Event(e)
                if branches.iter().any(|b| b.matches(e) && b.config.family == m.instrument && b.muted)));
        }
        m.frame = frame;
        Ok(m)
    }
    fn apply_song_end(&mut self, e: SongEndpoints) -> Result<(), SongRejectCode> {
        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        if !r.owns_epoch(e.epoch) {
            return Err(SongRejectCode::StaleEpoch);
        }
        let existing = r.endpoints.iter().position(|p| p.epoch == e.epoch);
        if e.arrangement > e.tail_deadline
            || existing.is_some_and(|i| {
                let old = r.endpoints[i];
                old.closed || e.arrangement > old.arrangement || e.tail_deadline > old.deadline
            })
        {
            return Err(SongRejectCode::Malformed);
        }
        if !r.has_endpoint_obligation(e.epoch) {
            return Err(SongRejectCode::Capacity);
        }
        if r.branches.iter().any(|b| b.config.epoch == e.epoch) {
            self.end_song_branches(e.epoch, None, e.tail_deadline)?;
        }
        let r = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        let endpoint = SongEpochEnd {
            epoch: e.epoch,
            arrangement: e.arrangement,
            deadline: e.tail_deadline,
            closed: false,
        };
        r.commit_endpoint(endpoint);
        r.commands.retain(|c| {
            if c.failure.is_some() || c.command.epoch() != e.epoch {
                return true;
            }
            match c.command {
                SongCommand::Event(_) => false,
                SongCommand::Endpoints(later) => later == e || later.arrangement < e.arrangement,
                SongCommand::Release(_) | SongCommand::RebindBranch(_) => c.frame < e.arrangement,
                _ => true,
            }
        });
        Ok(())
    }
}

impl Engine {
    fn apply_song_ends_under_pressure(&mut self, frame: u64) {
        if self
            .song_runtime
            .as_ref()
            .is_none_or(|r| r.receipts.len() != r.receipts.capacity())
        {
            return;
        }
        let mut index = 0;
        loop {
            let Some(c) = self
                .song_runtime
                .as_ref()
                .and_then(|r| r.commands.get(index))
                .copied()
            else {
                break;
            };
            if c.frame > frame {
                break;
            }
            if c.failure.is_some()
                || !self
                    .song_runtime
                    .as_ref()
                    .is_some_and(|r| r.owns_epoch(c.command.epoch()))
            {
                index += 1;
                continue;
            }
            let result = match c.command {
                SongCommand::Release(r) => {
                    self.end_song_branches(r.epoch, Some((r.branch, r.generation)), r.tail_deadline)
                }
                SongCommand::Endpoints(e) => self.apply_song_end(e),
                _ => {
                    index += 1;
                    continue;
                }
            };
            if let Some(r) = &mut self.song_runtime {
                if let Some(at) = r
                    .commands
                    .iter()
                    .position(|pending| pending.command == c.command && pending.frame == c.frame)
                {
                    match result {
                        Ok(()) => {
                            r.commands.remove(at);
                        }
                        Err(reason) => {
                            r.commands[at].failure = Some(reason);
                            index = at + 1;
                        }
                    }
                }
            }
        }
        self.finish_song_deadlines(frame);
    }
}

impl crate::dsp::song::SongRuntime {
    pub(crate) fn has_live_branches(&self) -> bool {
        self.branches
            .iter()
            .any(|branch| !branch.ended && !branch.closed)
            || self.voices.iter().any(Option::is_some)
    }
}
