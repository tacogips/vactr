//! Streaming finite-song export through the actual native host and owned transport.
use super::wav::WavOutput;
use crate::dsp::{arena::StoreKind, caps::CapabilitySet};
use crate::host::caps::{
    AudioHost, SampleSrc, SongHostPreparation, SongPreparationLimits, SongPreparationProgress,
};
use crate::host::native::audio::{AudioSide, NativeAudioHost, MAX_BLOCK};
use crate::host::wire::HostMsg;
use crate::pattern::TimeSpan;
use crate::sched::song::{SongTransport, SongTransportState};
use crate::song::routing::SongEndpoints;
use crate::song::{PreparedSong, SnapshotEpoch, SongLimits, SongSettings};
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};
use std::path::{Path, PathBuf};

/// Native stereo PCM16 output; no normalization or manual cycle limit.
#[derive(Clone, Debug)]
pub struct SongExportOptions {
    pub output: PathBuf,
    pub sample_rate: u32,
}
/// Actual immutable score metadata and completed production transport result.
#[derive(Clone, Debug)]
pub struct SongExportReport {
    pub arrangement_frames: u64,
    pub tail_frames: u64,
    pub total_frames: u64,
    pub epoch: SnapshotEpoch,
    pub revision: u64,
    pub seed: u64,
    pub seed_policy: &'static str,
    pub settings: SongSettings,
    pub duration: Ratio64,
    pub sample_rate: u32,
    pub warnings: Vec<String>,
    pub final_state: SongTransportState,
}
fn fail(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}
fn canonical_output(path: &Path) -> Result<PathBuf, Failure> {
    if path.exists() {
        return std::fs::canonicalize(path).map_err(|e| fail(e.to_string()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| fail("output must name a WAV file"))?;
    let mut ancestor = parent.to_owned();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        let name = ancestor
            .file_name()
            .ok_or_else(|| fail("output has no existing parent"))?
            .to_owned();
        missing.push(name);
        ancestor = ancestor
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
    }
    let mut resolved = std::fs::canonicalize(ancestor).map_err(|e| fail(e.to_string()))?;
    for name in missing.into_iter().rev() {
        resolved.push(name);
    }
    resolved.push(name);
    Ok(resolved)
}
fn alias_check(song: &PreparedSong, output: &Path) -> Result<(), Failure> {
    let destination = canonical_output(output)?;
    let snapshot = song.snapshot();
    let mut sources = vec![PathBuf::from(snapshot.file())];
    sources.extend(
        snapshot
            .routing()
            .sources
            .iter()
            .map(|source| PathBuf::from(source.path.as_ref())),
    );
    for resource in snapshot.routing().resources.entries() {
        if let SampleSrc::Path(path) = resource.source() {
            sources.push(PathBuf::from(path.text.as_ref()));
        }
    }
    for source in sources {
        if source.exists()
            && std::fs::canonicalize(source).map_err(|e| fail(e.to_string()))? == destination
        {
            return Err(fail("song output aliases a source file"));
        }
    }
    Ok(())
}
fn drain(host: &mut NativeAudioHost, transport: &mut SongTransport) -> Result<(), Failure> {
    let mut messages = Vec::new();
    host.drain(&mut messages);
    for message in messages {
        if let HostMsg::Song(ack) = message {
            transport
                .receive(ack)
                .map_err(|ack| fail(format!("unowned export receipt: {ack:?}")))?;
        }
    }
    Ok(())
}
fn pump_owner(
    host: &mut NativeAudioHost,
    side: &mut AudioSide,
    owner: &mut SongHostPreparation,
) -> Result<(), Failure> {
    let mut silence = [0.; MAX_BLOCK * 2];
    owner.submit(host)?;
    side.render(&mut silence, 2);
    let mut messages = Vec::new();
    host.drain(&mut messages);
    for message in messages {
        if let HostMsg::Song(ack) = message {
            owner
                .receive(ack)
                .map_err(|ack| fail(format!("unowned preparation receipt: {ack:?}")))?;
        }
    }
    Ok(())
}
fn cleanup(
    host: &mut NativeAudioHost,
    side: &mut AudioSide,
    core: &mut SongTransport,
    duration: Ratio64,
    limits: SongLimits,
) -> Result<(), Failure> {
    let mut silence = [0.; MAX_BLOCK * 2];
    for _ in 0..limits.max_nodes {
        let clock = host.song_clock()?;
        core.advance(host, clock, TimeSpan::new(Ratio64::ZERO, duration)?)?;
        if core.take_retired().is_ok() || core.take_cancelled().is_ok() {
            return Ok(());
        }
        side.render(&mut silence, 2);
        drain(host, core)?;
    }
    Err(fail("export resource retirement work exhausted"))
}
/// Consumes a fresh original candidate, publishing only a complete checked WAV.
/// # Errors
/// Admission, realization, callback rejection, PCM/RIFF, alias or filesystem failure.
pub fn export_song(
    song: PreparedSong,
    options: &SongExportOptions,
) -> Result<SongExportReport, Failure> {
    alias_check(&song, &options.output)?;
    let limits = SongLimits::default();
    let duration = song.snapshot().duration();
    let settings = song.snapshot().settings();
    let seconds = duration.checked_mul(settings.seconds_per_cycle()?)?;
    let total = limits.frames_at(
        seconds.checked_add(settings.tail_seconds)?,
        options.sample_rate,
    )?;
    super::wav::header(total, options.sample_rate)?;
    let mut output = WavOutput::create(&options.output, total, options.sample_rate)?;
    let report = render(song, options, &mut output)?;
    output.finish()?;
    Ok(report)
}
fn render(
    song: PreparedSong,
    options: &SongExportOptions,
    output: &mut WavOutput,
) -> Result<SongExportReport, Failure> {
    let limits = SongLimits::default();
    let settings = song.snapshot().settings();
    let duration = song.snapshot().duration();
    let revision = song.revision();
    let epoch = song.epoch();
    let warnings = song.snapshot().warnings().to_vec();
    let seconds = duration.checked_mul(settings.seconds_per_cycle()?)?;
    let arrangement_frames = limits.frames_at(seconds, options.sample_rate)?;
    let total_frames = limits.frames_at(
        seconds.checked_add(settings.tail_seconds)?,
        options.sample_rate,
    )?;
    let caps = CapabilitySet::native();
    let config = crate::host::song_profile::song_engine_config(
        options.sample_rate as f32,
        MAX_BLOCK,
        caps,
        StoreKind::NativeArc,
        2,
    )
    .map_err(|error| fail(format!("invalid song export engine profile: {error:?}")))?;
    let (mut host, mut side) = NativeAudioHost::headless_with_config(config, 4096)?;
    let mut owner = SongHostPreparation::begin(
        song,
        SongPreparationLimits {
            capabilities: caps,
            song: limits,
            max_resources: 256,
            max_pending_records: 4096,
            max_graph_bytes: 1_000_000,
            max_work: 8_000_000,
        },
    )
    .map_err(|refusal| refusal.failure)?;
    let mut ready = None;
    let preparation = (|| {
        for _ in 0..limits.max_nodes {
            pump_owner(&mut host, &mut side, &mut owner)?;
            if owner.progress() == SongPreparationProgress::Ready {
                ready = Some(owner.take_ready()?);
                return Ok(());
            }
        }
        Err(fail("export preparation work exhausted"))
    })();
    if let Err(error) = preparation {
        owner.cancel()?;
        for _ in 0..limits.max_nodes {
            pump_owner(&mut host, &mut side, &mut owner)?;
            if owner.take_cancelled().is_ok() {
                return Err(error);
            }
        }
        return Err(fail(format!(
            "{error}; export preparation cleanup exhausted"
        )));
    }
    let clock = host.song_clock()?;
    let activation = clock
        .frame
        .checked_add((MAX_BLOCK as u64) * 4)
        .ok_or_else(|| fail("export activation frame overflow"))?;
    let mut core = SongTransport::new(
        ready.ok_or_else(|| fail("export Ready missing"))?,
        activation,
        limits,
    )
    .map_err(|refusal| refusal.failure)?;
    let deadline = core.endpoints().tail_deadline;
    let mut block = [0.; MAX_BLOCK * 2];
    let outcome = (|| {
        core.submit_activation(&mut host)
            .map_err(|refusal| fail(format!("export activation refused: {:?}", refusal.error)))?;
        loop {
            let clock = host.song_clock()?;
            core.advance(&mut host, clock, TimeSpan::new(Ratio64::ZERO, duration)?)?;
            if clock.frame >= deadline {
                break;
            }
            let boundary = if clock.frame < activation {
                activation
            } else {
                deadline
            };
            let frames = usize::try_from((boundary - clock.frame).min(MAX_BLOCK as u64))
                .map_err(|_| fail("export block frame overflow"))?;
            side.render(&mut block[..frames * 2], 2);
            drain(&mut host, &mut core)?;
            if let Some(error) = core.failure() {
                return Err(error.clone());
            }
            if clock.frame >= activation {
                output.write(&block[..frames * 2])?;
            }
        }
        cleanup(&mut host, &mut side, &mut core, duration, limits)?;
        if core.state() != SongTransportState::Ended {
            return Err(fail("export transport did not complete successfully"));
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        let clock = host.song_clock()?;
        // An active failure uses a real earlier endpoint, never cancellation.
        if core.applied_activation().is_some() {
            let frame = clock.frame.max(activation);
            let tail = limits.frames_at(settings.tail_seconds, options.sample_rate)?;
            let _ = core.cutoff(
                &mut host,
                SongEndpoints {
                    epoch,
                    arrangement: frame.min(deadline),
                    tail_deadline: frame.checked_add(tail).unwrap_or(deadline).min(deadline),
                },
            );
        }
        core.abort(error.clone());
        if let Err(cleanup_error) = cleanup(&mut host, &mut side, &mut core, duration, limits) {
            return Err(fail(format!("{error}; cleanup: {cleanup_error}")));
        }
        return Err(error);
    }
    Ok(SongExportReport {
        arrangement_frames,
        tail_frames: total_frames - arrangement_frames,
        total_frames,
        epoch,
        revision,
        seed: settings.seed,
        seed_policy: "frozen root seed",
        settings,
        duration,
        sample_rate: options.sample_rate,
        warnings,
        final_state: core.state(),
    })
}
