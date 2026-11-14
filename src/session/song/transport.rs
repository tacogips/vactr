//! Consuming isolated song ownership and correlated asynchronous publication.
use super::*;
use crate::host::caps::{SongPreparationLimits, SongPreparationRefusal};
use crate::sched::runtime::SongNotice;
use crate::sched::song::SongTransportState;
use crate::session::protocol::{
    ServerMsg, SongCandidateFailedBody, SongCandidateReadyBody, SongInstrumentMuteBody,
    SongInstrumentMutedBody, SongTransportStateBody, WireInstrumentSelector, WireSongSound,
    WireSongTransportState,
};
use crate::session::session::{Outgoing, Session};
use crate::song::prepare_song;
use crate::song::snapshot::FrozenSound;

/// Immutable original definitions and resource metadata published after Applied.
pub type FrozenAppliedSongCatalog = (
    Vec<crate::song::snapshot::FrozenInstrument>,
    crate::song::snapshot::FrozenCellInventory,
    crate::song::snapshot::FrozenGraphResources,
);
type RetainedAppliedSongCatalog = (SnapshotEpoch, Rc<FrozenAppliedSongCatalog>);

#[derive(Default)]
pub(in crate::session) struct SongRequests {
    limits: Option<SongPreparationLimits>,
    mute_nonce: u64,
    mutes: BTreeMap<u64, (u32, u64, WireInstrumentSelector)>,
    requests: BTreeMap<SnapshotEpoch, SongRequest>,
    applied: Option<RetainedAppliedSongCatalog>,
}
struct SongRequest {
    conn: u32,
    seq: u64,
    file: String,
    revision: u64,
    edit_epoch: u64,
}
impl Session {
    pub fn applied_song_catalog(&self) -> Option<(SnapshotEpoch, &FrozenAppliedSongCatalog)> {
        self.song_requests
            .applied
            .as_ref()
            .map(|(epoch, catalog)| (*epoch, catalog.as_ref()))
    }
    /// Host-supplied actual remaining asset capacities, including retirements.
    pub fn set_song_asset_limits(&mut self, limits: SongAssetLimits) -> Result<(), Failure> {
        self.song_asset_limits = Some(limits.validate()?);
        Ok(())
    }
    pub(in crate::session) fn build_song_request(
        &mut self,
        body: &super::super::protocol::ApplySongBody,
    ) -> Result<SnapshotEpoch, Failure> {
        if self
            .pending_song
            .as_ref()
            .is_some_and(|p| !p.resources().is_empty())
        {
            return Err(Failure::new(
                FailCode::HostUnavailable,
                "previous candidate reservations require owner cleanup before replacement",
            ));
        }
        let factory = self.song_assets.as_ref().ok_or_else(|| {
            Failure::new(
                FailCode::HostUnavailable,
                "isolated song asset capability is unavailable",
            )
        })?;
        let limits = self.song_asset_limits.ok_or_else(|| {
            Failure::new(
                FailCode::HostUnavailable,
                "song host has not supplied remaining asset capacities",
            )
        })?;
        self.song_epoch = self
            .song_epoch
            .checked_add(1)
            .ok_or_else(|| failure("song snapshot epoch overflow"))?;
        let epoch = SnapshotEpoch(self.song_epoch);
        let cx = CandidateBuildCtx {
            assets: factory.as_ref(),
            asset_limits: limits,
            lock: self.lock.as_ref(),
            cache: self.cache.as_deref(),
        };
        let candidate = evaluate(
            &body.code,
            &body.file,
            body.doc_revision,
            body.edit_epoch,
            epoch,
            &cx,
        )?;
        self.pending_song = Some(prepare_song(candidate)?);
        Ok(epoch)
    }
    pub fn cancel_pending_song(&mut self, epoch: SnapshotEpoch) -> Result<(), Failure> {
        let pending = self
            .pending_song
            .as_mut()
            .ok_or_else(|| failure("unknown pending song epoch"))?;
        cancel_song_candidate(pending, epoch)
    }
    pub fn pending_song(&self) -> Option<&PreparedSong> {
        self.pending_song.as_ref()
    }
}

impl Session {
    /// Configures explicit host construction ceilings; physical admission still uses reports.
    /// # Errors
    /// Invalid structural or zero work bounds are rejected.
    pub fn set_song_preparation_limits(
        &mut self,
        limits: SongPreparationLimits,
    ) -> Result<(), Failure> {
        limits.song.validate()?;
        if limits.max_resources == 0 || limits.max_pending_records == 0 || limits.max_work == 0 {
            return Err(failure("invalid song preparation limits"));
        }
        self.song_requests.limits = Some(limits);
        Ok(())
    }
    /// Transfers the original prepared candidate into the actual host owner.
    /// # Errors
    /// Refusal returns the same candidate without discarding reservations.
    #[allow(clippy::result_large_err)]
    pub fn submit_prepared_song(
        &mut self,
        prepared: PreparedSong,
    ) -> Result<(), SongPreparationRefusal> {
        let Some(limits) = self.song_requests.limits.as_ref() else {
            return Err(SongPreparationRefusal {
                prepared,
                failure: Failure::new(
                    FailCode::HostUnavailable,
                    "song preparation limits have not been configured",
                ),
            });
        };
        self.rt.prepare_song(
            prepared,
            SongPreparationLimits {
                capabilities: limits.capabilities,
                song: limits.song,
                max_resources: limits.max_resources,
                max_pending_records: limits.max_pending_records,
                max_graph_bytes: limits.max_graph_bytes,
                max_work: limits.max_work,
            },
        )
    }
    pub(in crate::session) fn submit_song_request(
        &mut self,
        body: &super::super::protocol::ApplySongBody,
        conn: u32,
        seq: u64,
    ) -> Result<SnapshotEpoch, Failure> {
        if self.song_requests.requests.len() >= 8 {
            return Err(Failure::new(
                FailCode::HostUnavailable,
                "song request ownership is full",
            ));
        }
        let epoch = self.build_song_request(body)?;
        let Some(prepared) = self.pending_song.take() else {
            return Err(failure("missing prepared song"));
        };
        if let Err(refusal) = self.submit_prepared_song(prepared) {
            self.pending_song = Some(refusal.prepared);
            return Err(refusal.failure);
        }
        self.song_requests.requests.insert(
            epoch,
            SongRequest {
                conn,
                seq,
                file: body.file.clone(),
                revision: body.doc_revision,
                edit_epoch: body.edit_epoch,
            },
        );
        Ok(epoch)
    }
    pub(in crate::session) fn invalidate_song_requests(
        &mut self,
        change: &super::super::protocol::DocChangedBody,
    ) -> Result<(), Failure> {
        for (&epoch, request) in &self.song_requests.requests {
            if request.file == change.file
                && (request.revision != change.doc_revision
                    || request.edit_epoch != change.edit_epoch)
            {
                self.rt.invalidate_pending_song(epoch)?;
            }
        }
        Ok(())
    }
    pub(in crate::session) fn submit_song_mute(
        &mut self,
        body: SongInstrumentMuteBody,
        conn: u32,
        seq: u64,
    ) -> Result<(), Failure> {
        if self.song_requests.mutes.len() >= 32 {
            return Err(failure("mute requester ownership is full"));
        }
        let active = self
            .rt
            .active_song_families(body.epoch)
            .ok_or_else(|| failure("unknown acknowledged active song epoch"))?;
        let mut sounds = Vec::new();
        for selected in body.selector.family() {
            let sound = active
                .iter()
                .find(|sound| wire_sound(sound) == *selected)
                .ok_or_else(|| failure("mute selector is not a certified active family"))?;
            sounds.push(sound.clone());
        }
        let request = self
            .song_requests
            .mute_nonce
            .checked_add(1)
            .ok_or_else(|| failure("mute request nonce exhausted"))?;
        self.rt
            .request_song_mute(body.epoch, &sounds, body.muted, request)?;
        self.song_requests.mute_nonce = request;
        self.song_requests
            .mutes
            .insert(request, (conn, seq, body.selector));
        Ok(())
    }
    pub(in crate::session) fn publish_song_notices(&mut self) -> Vec<Outgoing> {
        let mut out = Vec::new();
        while let Some(notice) = self.rt.pop_song_notice() {
            let (epoch, message) = match notice {
                SongNotice::Ready { epoch, revision } => (
                    epoch,
                    ServerMsg::SongCandidateReady(SongCandidateReadyBody {
                        epoch,
                        doc_revision: revision,
                    }),
                ),
                SongNotice::Applied { ack, catalog } => {
                    self.song_requests.applied = Some((ack.epoch, catalog));
                    (ack.epoch, ServerMsg::SongCandidateApplied(ack))
                }
                SongNotice::CarriedMute {
                    epoch,
                    sound,
                    muted,
                    frame,
                } => {
                    if let Ok(selector) = WireInstrumentSelector::new(vec![wire_sound(&sound)]) {
                        let message = ServerMsg::SongInstrumentMuted(SongInstrumentMutedBody {
                            epoch,
                            selector,
                            muted,
                            application_frame: frame,
                        });
                        out.push(Outgoing {
                            to: crate::session::session::Dest::Topic(
                                crate::session::protocol::Topic::Telemetry,
                            ),
                            env: self.envelope(message, None),
                        });
                    }
                    continue;
                }
                SongNotice::Failed {
                    epoch,
                    revision,
                    failure,
                } => (
                    epoch,
                    ServerMsg::SongCandidateFailed(SongCandidateFailedBody {
                        epoch: Some(epoch),
                        doc_revision: Some(revision),
                        code: failure.code.as_str().into(),
                        message: failure.message.to_string(),
                    }),
                ),
                SongNotice::Muted {
                    request,
                    epoch,
                    muted,
                    frame,
                } => {
                    if let Some((conn, seq, selector)) = self.song_requests.mutes.remove(&request) {
                        out.extend(self.route(
                            conn,
                            Some(seq),
                            vec![ServerMsg::SongInstrumentMuted(SongInstrumentMutedBody {
                                epoch,
                                selector,
                                muted,
                                application_frame: frame,
                            })],
                        ));
                    }
                    continue;
                }
                SongNotice::MuteFailed { request, epoch } => {
                    if let Some((conn, seq, _)) = self.song_requests.mutes.remove(&request) {
                        out.extend(self.route(conn, Some(seq), vec![ServerMsg::SongCandidateFailed(
                            SongCandidateFailedBody { epoch: Some(epoch), doc_revision: None,
                                code: "mute-not-applied".into(), message: "song ended, failed or was replaced before complete mute acknowledgement".into() })]));
                    }
                    continue;
                }
                SongNotice::State {
                    epoch,
                    state,
                    families,
                } => {
                    let state = match state {
                        SongTransportState::Prepared => WireSongTransportState::Prepared,
                        SongTransportState::Playing => WireSongTransportState::Playing,
                        SongTransportState::Draining => WireSongTransportState::Draining,
                        SongTransportState::Ended => WireSongTransportState::Ended,
                        SongTransportState::Failed => WireSongTransportState::Failed,
                    };
                    let instruments = families
                        .iter()
                        .filter_map(|sound| {
                            WireInstrumentSelector::new(vec![wire_sound(sound)]).ok()
                        })
                        .collect();
                    out.extend(self.route(
                        0,
                        None,
                        vec![ServerMsg::SongTransportState(SongTransportStateBody {
                            epoch,
                            state,
                            instruments,
                        })],
                    ));
                    continue;
                }
            };
            if let Some(request) = self.song_requests.requests.get(&epoch) {
                let (conn, seq) = (request.conn, request.seq);
                out.extend(self.route(conn, Some(seq), vec![message]));
            }
        }
        self.song_requests
            .requests
            .retain(|epoch, _| self.rt.retains_song_epoch(*epoch));
        out
    }
}

fn wire_sound(sound: &FrozenSound) -> WireSongSound {
    match sound {
        FrozenSound::Builtin(name) => WireSongSound::Builtin {
            name: crate::value::intern::name_of_kw(*name).to_string(),
        },
        FrozenSound::Instrument(id) => WireSongSound::Instrument { id: id.get() },
        FrozenSound::Sample { path, file } => WireSongSound::Sample {
            path: path.to_string(),
            file: file.map(|f| f.get()),
        },
        FrozenSound::Buffer(id) => WireSongSound::Buffer { id: *id },
    }
}
