//! Checked musical boundaries and retained original replacement ownership.
use super::*;
use crate::host::caps::{SongReplacementCommit, SongReplacementProgress, SongReplacementSource};
use crate::song::routing::SongInitialMute;

pub(super) struct PendingReplacement {
    pub(super) previous: SnapshotEpoch,
    cycle: Ratio64,
    submitted: bool,
    pub(super) invalidated: bool,
    pub(super) committed: bool,
    recovered: bool,
    returned_source: Option<SongReplacementSource>,
    commit: Option<SongReplacementCommit>,
    pub(super) carried: Vec<FrozenSound>,
}
impl PendingReplacement {
    pub(super) fn definitive(&self) -> bool {
        self.committed || self.recovered
    }
}
pub(super) fn capture_catalog(
    ready: &SongReadyBundle,
    remaining: &mut u32,
) -> Result<Rc<AppliedCatalog>, Failure> {
    let r = ready.prepared().snapshot().routing();
    overlay::charge(
        remaining,
        r.instruments.len()
            + r.cells.values().len()
            + r.cells.references().len()
            + r.cells.analysis_ranges().len()
            + r.cells.analysis_banks().len()
            + r.resources.entries().len(),
    )?;
    for inst in &r.instruments {
        overlay::charge(remaining, inst.parameters.len() + inst.defaults.len())?;
    }
    Ok(Rc::new((
        r.instruments.clone(),
        r.cells.clone(),
        r.resources.clone(),
    )))
}
fn replacement_boundary(
    clock: SongHostClock,
    old_activation: u64,
    settings: SongSettings,
    minimum_safe_frame: u64,
) -> Result<(u64, Ratio64), Failure> {
    let cycle = settings.seconds_per_cycle()?;
    let numerator = u128::try_from(cycle.num())
        .ok()
        .and_then(|n| n.checked_mul(u128::from(clock.sample_rate)))
        .filter(|n| *n > 0)
        .ok_or_else(|| song_failure("invalid cycle frame numerator"))?;
    let denominator =
        u128::try_from(cycle.den()).map_err(|_| song_failure("invalid cycle frame denominator"))?;
    let relative = u128::from(minimum_safe_frame.saturating_sub(old_activation));
    let k = if relative == 0 {
        0
    } else {
        let threshold = relative
            .checked_mul(2)
            .and_then(|r| r.checked_sub(1))
            .and_then(|r| r.checked_mul(denominator))
            .ok_or_else(|| song_failure("replacement cycle overflow"))?;
        let doubled = numerator
            .checked_mul(2)
            .ok_or_else(|| song_failure("replacement cycle overflow"))?;
        (threshold / doubled)
            .checked_add(u128::from(threshold % doubled != 0))
            .ok_or_else(|| song_failure("replacement cycle overflow"))?
    };
    let product = k
        .checked_mul(numerator)
        .ok_or_else(|| song_failure("replacement frame overflow"))?;
    let remainder = product % denominator;
    let frames = (product / denominator)
        .checked_add(u128::from(remainder >= denominator - remainder))
        .ok_or_else(|| song_failure("replacement frame overflow"))?;
    let frame = old_activation
        .checked_add(u64::try_from(frames).map_err(|_| song_failure("replacement frame overflow"))?)
        .ok_or_else(|| song_failure("replacement frame overflow"))?;
    Ok((
        frame,
        Ratio64::from_int(
            i64::try_from(k).map_err(|_| song_failure("replacement cycle overflow"))?,
        ),
    ))
}
impl Runtime {
    pub(super) fn retire_failed_ready(&mut self, refusal: SongTransportRefusal) {
        let epoch = refusal.ready.prepared().epoch();
        let revision = refusal.ready.prepared().revision();
        if self.song.owners.is_empty() {
            self.song.last_state = Some(SongTransportState::Failed);
        }
        self.song.preparing = Some(Preparing {
            owner: SongHostPreparation::retire(refusal.ready),
            previous: self.song.ready_previous.take(),
            work: self.song.ready_work,
            epoch,
            revision,
            reported: false,
            invalidation: Some(refusal.failure),
        });
    }
    pub(super) fn retain_promised_old(&self, epoch: SnapshotEpoch) -> bool {
        self.song
            .preparing
            .as_ref()
            .is_some_and(|p| p.previous == Some(epoch))
            || self.song.ready_previous == Some(epoch)
            || self.song.owners.iter().any(|o| {
                o.replacement
                    .as_ref()
                    .is_some_and(|p| p.previous == epoch && !p.definitive())
            })
    }
    pub(super) fn start_ready(
        &mut self,
        ready: SongReadyBundle,
        clock: SongHostClock,
    ) -> Result<(), SongTransportRefusal> {
        let Some(previous) = self.song.ready_previous else {
            let frame = match future_frame(clock, self.cfg.lookahead) {
                Ok(frame) => frame,
                Err(failure) => return Err(SongTransportRefusal { ready, failure }),
            };
            return self.start_song(ready, frame);
        };
        let selected = (|| {
            let old = self
                .song
                .owners
                .iter()
                .find(|o| o.transport.epoch() == previous)
                .ok_or_else(|| song_failure("promised old owner missing"))?;
            let activation = old
                .transport
                .applied_activation()
                .ok_or_else(|| song_failure("old owner not acknowledged"))?;
            let offered = old
                .offered
                .checked_mul(old.transport.settings().seconds_per_cycle()?)?;
            let offered_frame = activation
                .frame
                .checked_add(SongLimits::default().frames_at(offered, clock.sample_rate)?)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| song_failure("offered horizon overflow"))?;
            let safe = future_frame(clock, self.cfg.lookahead)?
                .checked_add(64)
                .ok_or_else(|| song_failure("replacement lead overflow"))?;
            replacement_boundary(
                clock,
                activation.frame,
                old.transport.settings(),
                offered_frame.max(safe),
            )
        })();
        let (frame, cycle) = match selected {
            Ok(selected) => selected,
            Err(failure) => return Err(SongTransportRefusal { ready, failure }),
        };
        self.start_song(ready, frame)?;
        let mut new = self
            .song
            .owners
            .pop()
            .expect("start_song retained new owner");
        let mut work = self.song.ready_work;
        let result = (|| {
            let old = self
                .song
                .owners
                .iter_mut()
                .find(|o| o.transport.epoch() == previous)
                .ok_or_else(|| song_failure("promised old owner missing"))?;
            let nonce = self
                .song
                .nonce
                .checked_add(1)
                .ok_or_else(|| song_failure("overlay nonce overflow"))?;
            let mut overlay = Vec::new();
            let mut carried = Vec::new();
            for family in &mut new.families {
                for prior in old.families.iter().filter(|f| f.muted) {
                    overlay::charge(&mut work, 1)?;
                    if let (Some(a), Some(b)) = (&prior.certificate, &family.certificate) {
                        if overlay::equivalent_frozen_family(a, b, &mut work)? {
                            family.muted = true;
                            break;
                        }
                    }
                }
                if family.muted {
                    overlay::charge(&mut work, overlay.len() + carried.len() + 1)?;
                    if !overlay
                        .iter()
                        .any(|m: &SongInitialMute| m.instrument == family.instrument)
                    {
                        overlay.push(SongInitialMute {
                            epoch: new.transport.epoch(),
                            overlay_nonce: nonce,
                            instrument: family.instrument,
                            muted: true,
                        });
                    }
                    if !carried.contains(&family.sound) {
                        carried.push(family.sound.clone());
                    }
                }
            }
            let source = old.transport.replacement_source()?;
            new.replacement = Some(PendingReplacement {
                previous,
                cycle,
                submitted: false,
                invalidated: false,
                committed: false,
                recovered: false,
                returned_source: None,
                commit: None,
                carried,
            });
            self.song.nonce = nonce;
            if let Err(refusal) = new.transport.prepare_replacement(source, nonce, overlay) {
                new.replacement
                    .as_mut()
                    .expect("intent retained")
                    .returned_source = Some(refusal.source);
                return Err(refusal.failure);
            }
            Ok(())
        })();
        self.song.ready_work = work;
        self.song.ready_previous = None;
        if let Err(failure) = result {
            new.transport.abort(failure);
        }
        self.song.owners.push(new);
        Ok(())
    }
    pub(super) fn drive_replacements(&mut self, rep: &mut TickReport) {
        for index in 0..self.song.owners.len() {
            if self.song.owners[index].replacement.is_none() {
                continue;
            }
            let mut new = self.song.owners.remove(index);
            let pending = new.replacement.as_mut().expect("replacement checked");
            if !pending.submitted && !pending.invalidated && new.transport.failure().is_none() {
                // Priming records and Replace share one bounded controller turn.
                for _ in 0..=NOTICE_LIMIT {
                    match new.transport.submit_replacement(self.hosts.audio.as_mut()) {
                        Ok(SongReplacementProgress::Priming) => {}
                        Ok(SongReplacementProgress::Submitted) => {
                            pending.submitted = true;
                            new.activation_posted = true;
                            if let Some(old) = self
                                .song
                                .owners
                                .iter_mut()
                                .find(|o| o.transport.epoch() == pending.previous)
                            {
                                old.cutoff_cycle = Some(pending.cycle);
                            }
                            break;
                        }
                        Err(refusal) => {
                            match refusal.error {
                                SongSubmitError::Backpressure => {}
                                SongSubmitError::Invalid(failure) => new.transport.abort(failure),
                                SongSubmitError::Unavailable => new
                                    .transport
                                    .abort(song_failure("replacement host unavailable")),
                            }
                            break;
                        }
                    }
                }
            }
            if pending.commit.is_none() && !pending.committed {
                pending.commit = new.transport.take_replacement_commit();
            }
            if let Some(commit) = pending.commit.take() {
                if let Some(old) = self
                    .song
                    .owners
                    .iter_mut()
                    .find(|o| o.transport.epoch() == pending.previous)
                {
                    match old.transport.observe_replacement(commit) {
                        Ok(()) => pending.committed = true,
                        Err(refusal) => {
                            pending.commit = Some(refusal.commit);
                            rep.faults.push(refusal.failure);
                        }
                    }
                } else {
                    pending.commit = Some(commit);
                }
            }
            if !pending.committed && new.transport.state() == SongTransportState::Failed {
                if pending.returned_source.is_none() {
                    match new.transport.take_cancelled_replacement_source() {
                        Ok(source) => pending.returned_source = source,
                        Err(failure) => rep.faults.push(failure),
                    }
                }
                if let Some(source) = pending.returned_source.take() {
                    if let Some(old) = self
                        .song
                        .owners
                        .iter_mut()
                        .find(|o| o.transport.epoch() == pending.previous)
                    {
                        match old.transport.reclaim_replacement_source(source) {
                            Ok(()) => {
                                pending.recovered = true;
                                old.cutoff_cycle = None;
                            }
                            Err(refusal) => {
                                pending.returned_source = Some(refusal.source);
                                rep.faults.push(refusal.failure);
                            }
                        }
                    } else {
                        pending.returned_source = Some(source);
                    }
                }
            }
            self.song.owners.insert(index, new);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_cycle_uses_earliest_rounded_boundary() {
        let clock = SongHostClock {
            frame: 100,
            sample_rate: 8000,
        };
        let settings = SongSettings {
            bpm: Ratio64::new(320000, 11).unwrap(),
            cycle_beats: Ratio64::ONE,
            ..SongSettings::default()
        };
        assert_eq!(
            replacement_boundary(clock, 100, settings, 117).unwrap(),
            (117, Ratio64::ONE)
        );
        assert_eq!(
            replacement_boundary(clock, 100, settings, 118).unwrap(),
            (133, Ratio64::from_int(2))
        );
        assert_eq!(
            replacement_boundary(clock, 100, settings, 100).unwrap(),
            (100, Ratio64::ZERO)
        );
    }

    #[test]
    fn non_half_fractional_boundary_matches_original_frame_contract() {
        let clock = SongHostClock {
            frame: 100,
            sample_rate: 8000,
        };
        let settings = SongSettings {
            bpm: Ratio64::new(800000, 27).unwrap(),
            cycle_beats: Ratio64::ONE,
            ..SongSettings::default()
        };
        assert_eq!(
            replacement_boundary(clock, 100, settings, 116).unwrap(),
            (116, Ratio64::ONE)
        );
        assert_eq!(
            replacement_boundary(clock, 100, settings, 117).unwrap(),
            (132, Ratio64::from_int(2))
        );
        let settings = SongSettings {
            bpm: Ratio64::from_int(123),
            ..SongSettings::default()
        };
        let expected = SongLimits::default()
            .frames_at(
                settings
                    .seconds_per_cycle()
                    .unwrap()
                    .checked_mul(Ratio64::from_int(3))
                    .unwrap(),
                8000,
            )
            .unwrap();
        assert_eq!(expected, 46829);
        assert_eq!(
            replacement_boundary(clock, 0, settings, expected).unwrap(),
            (expected, Ratio64::from_int(3))
        );
    }

    #[test]
    fn replacement_boundary_rejects_unrepresentable_frame() {
        let clock = SongHostClock {
            frame: u64::MAX - 1,
            sample_rate: 8000,
        };
        assert!(
            replacement_boundary(clock, u64::MAX - 1, SongSettings::default(), u64::MAX).is_err()
        );
        let tiny_cycle = SongSettings {
            bpm: Ratio64::from_int(i64::MAX),
            cycle_beats: Ratio64::ONE,
            ..SongSettings::default()
        };
        assert!(replacement_boundary(clock, 0, tiny_cycle, u64::MAX).is_err());
    }
    #[cfg(feature = "host-native")]
    #[test]
    fn permanent_startup_failure_retires_original_ready_once() {
        use crate::dsp::{arena::StoreKind, engine::EngineConfig};
        use crate::host::{caps::Hosts, native::audio::NativeAudioHost, noop::NoopHost};
        use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
        let mut caps = CapabilitySet::native();
        caps.max_voices = 2;
        let mut config = EngineConfig::new(
            &caps,
            8000.,
            crate::host::native::audio::MAX_BLOCK,
            StoreKind::NativeArc,
        );
        config.bus_slots = 12;
        let (mut audio, mut side) = NativeAudioHost::headless_with_config(config, 64).unwrap();
        let factory = DecodedSongAssetFactory::new(
            Default::default(),
            Default::default(),
            Default::default(),
        );
        let cx = crate::session::song::CandidateBuildCtx {
            assets: &factory,
            asset_limits: SongAssetLimits {
                max_resources: 64,
                max_pcm_bytes: 1_000_000,
                max_source_files: 16,
                max_source_bytes: 100_000,
                max_banks: 16,
                max_walk_nodes: 100_000,
                max_walk_depth: 256,
            },
            lock: None,
            cache: None,
        };
        let epoch = SnapshotEpoch(411);
        let candidate=crate::session::song::evaluate_song_candidate(
            "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone}] duration: 4} tail-seconds: 0 > play-song",
            "startup.vact",1,epoch,&cx).unwrap();
        let mut owner = SongHostPreparation::begin(
            crate::song::prepare_song(candidate).unwrap(),
            SongPreparationLimits {
                capabilities: caps,
                song: SongLimits::default(),
                max_resources: 128,
                max_pending_records: 4096,
                max_graph_bytes: 65536,
                max_work: 8_000_000,
            },
        )
        .map_err(|r| r.failure)
        .unwrap();
        for _ in 0..512 {
            owner.submit(&mut audio).unwrap();
            side.render(&mut [0.; 32], 2);
            let mut messages = Vec::new();
            audio.drain(&mut messages);
            for m in messages {
                if let HostMsg::Song(ack) = m {
                    owner.receive(ack).unwrap();
                }
            }
            if owner.progress() == SongPreparationProgress::Ready {
                break;
            }
        }
        let ready = owner.take_ready().unwrap();
        let keys = ready.resources().to_vec();
        let mut hosts = Hosts::noop();
        hosts.audio = Box::new(audio);
        let (mut runtime, _) =
            Runtime::new(hosts, Rc::new(NoopHost), caps, RuntimeConfig::default());
        runtime.song.ready = Some(ready);
        // Exercise the real bounded certificate counter after genuine host adoption.
        runtime.song.ready_work = 1;
        let mut receipts = Vec::new();
        let mut failures = 0;
        for _ in 0..512 {
            runtime.tick_song(&mut TickReport::default());
            while let Some(notice) = runtime.pop_song_notice() {
                if let SongNotice::Failed {
                    epoch: failed,
                    failure,
                    ..
                } = notice
                {
                    assert_eq!(failed, epoch);
                    assert_eq!(failure.code, FailCode::FuelExhausted);
                    failures += 1;
                }
            }
            side.render(&mut [0.; 32], 2);
            let mut messages = Vec::new();
            runtime.hosts.audio.drain(&mut messages);
            for message in messages {
                if let HostMsg::Song(ack) = message {
                    receipts.push(ack);
                    runtime.dispatch_owned_song_ack(ack).unwrap();
                }
            }
            if failures > 0 && runtime.song.preparing.is_none() {
                break;
            }
        }
        assert_eq!(failures, 1);
        assert!(runtime.song.ready.is_none());
        assert!(runtime.song.preparing.is_none());
        assert!(runtime.song.owners.is_empty());
        assert_eq!(runtime.song_state(), Some(SongTransportState::Failed));
        assert!(!receipts
            .iter()
            .any(|ack| matches!(ack, SongHostAck::Applied(_))));
        assert!(receipts.contains(&SongHostAck::PreparationCancelled(epoch)));
        for key in keys {
            assert_eq!(
                receipts
                    .iter()
                    .filter(|ack| **ack == SongHostAck::LeaseReturned(key))
                    .count(),
                1
            );
        }
    }
}
