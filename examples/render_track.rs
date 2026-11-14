//! Production-path headless stereo WAV export without an audio device.
//! Build: `CARGO_TERM_QUIET=true cargo build --release --example render_track`.
//! Run: `render_track SCORE OUTPUT --cycles 64 --tail-seconds 8`.
//! A zero-frame install pass starts cycle zero without adding scheduling latency.
//! Declared effect delays remain part of the music; no normalization or clipping.

#[path = "render_track/audit.rs"]
mod audit;
#[cfg(test)]
#[path = "render_track/samples.rs"]
mod samples;
#[cfg(test)]
#[path = "render_track/tests.rs"]
mod tests;
#[path = "render_track/wav.rs"]
mod wav;

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use vactr::clock::ClockSource;
use vactr::dsp::caps::CapabilitySet;
use vactr::host::caps::{Hosts, InstResolver};
use vactr::host::native::{AudioSide, NativeAudioHost, NativeSampleLoader};
use vactr::ns::evaluator::Evaluator;
use vactr::ns::insts::InstRegistry;
use vactr::ns::namespace::Prelude;
use vactr::ns::stage::{EffectSink, StagedEffect};
use vactr::reader::span::FileId;
use vactr::sched::cells::Tier;
use vactr::sched::runtime::{Runtime, RuntimeConfig};
use vactr::types::diag::{Diagnostic, Severity};

use audit::{AudioAudit, CheckedHost};

const BLOCK: usize = 256;
const CELLS: usize = 1024;
type Result<T> = std::result::Result<T, String>;

#[derive(Debug)]
struct Options {
    source: PathBuf,
    output: PathBuf,
    cycles: u32,
    seconds: Option<f64>,
    tail_seconds: f64,
    bpm: Option<f64>,
    rate: u32,
}
impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let source = args.next().ok_or_else(usage)?;
        let output = args.next().ok_or_else(usage)?;
        let mut options = Self {
            source: source.into(),
            output: output.into(),
            cycles: 64,
            seconds: None,
            tail_seconds: 8.0,
            bpm: None,
            rate: 48000,
        };
        let mut explicit_cycles = false;
        while let Some(flag) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--cycles" => {
                    options.cycles = value
                        .parse()
                        .map_err(|_| "cycles must be a positive integer")?;
                    explicit_cycles = true;
                }
                "--seconds" => options.seconds = Some(positive(&value, "seconds")?),
                "--tail-seconds" => {
                    options.tail_seconds = value.parse().map_err(|_| "invalid tail seconds")?;
                    if !options.tail_seconds.is_finite() || options.tail_seconds < 0.0 {
                        return Err("tail seconds must be finite and nonnegative".into());
                    }
                }
                "--bpm" => options.bpm = Some(positive(&value, "BPM")?),
                "--sample-rate" => {
                    options.rate = value.parse().map_err(|_| "invalid sample rate")?
                }
                _ => return Err(format!("unknown option {flag}\n{}", usage())),
            }
        }
        if options.cycles == 0 {
            return Err("cycles must be positive".into());
        }
        if explicit_cycles && options.seconds.is_some() {
            return Err("choose --cycles or --seconds".into());
        }
        if !(8000..=192000).contains(&options.rate) {
            return Err("sample rate must be 8000..192000 Hz".into());
        }
        if options.source == options.output {
            return Err("source and output must differ".into());
        }
        Ok(options)
    }
}
fn usage() -> String {
    "usage: render_track SCORE OUTPUT [--cycles 64 | --seconds N] [--tail-seconds 8] [--bpm N] [--sample-rate 48000]".into()
}
fn positive(value: &str, label: &str) -> Result<f64> {
    let number: f64 = value.parse().map_err(|_| format!("invalid {label}"))?;
    if number.is_finite() && number > 0.0 {
        Ok(number)
    } else {
        Err(format!("{label} must be positive and finite"))
    }
}
fn diagnostics(diags: &[Diagnostic]) -> Result<()> {
    if let Some(diag) = diags.iter().find(|diag| diag.severity == Severity::Error) {
        return Err(format!("{}: {}", diag.code, diag.message));
    }
    Ok(())
}
fn evaluate(ev: &mut Evaluator, source: &str) -> Result<()> {
    let outcomes = ev
        .eval_str(source, FileId::new(1))
        .map_err(|diag| diag.to_string())?;
    for outcome in outcomes {
        diagnostics(&outcome.diags)?;
        outcome
            .value
            .map_err(|failure| format!("{}: {}", failure.code, failure.message))?;
    }
    Ok(())
}
fn drain(rt: &mut Runtime, ev: &mut Evaluator) -> Result<()> {
    let report = rt.drain(ev);
    diagnostics(&report.diags)?;
    if let Some(failure) = report.faults.first() {
        return Err(format!("{}: {}", failure.code, failure.message));
    }
    Ok(())
}
fn tick(rt: &mut Runtime, ev: &mut Evaluator, now: f64) -> Result<()> {
    let report = rt.tick(ev, now);
    diagnostics(&report.diags)?;
    if let Some(failure) = report.faults.first() {
        return Err(format!("{}: {}", failure.code, failure.message));
    }
    Ok(())
}
fn frame_count(seconds: f64, rate: u32) -> Result<u64> {
    let frames = (seconds * f64::from(rate)).round();
    if !frames.is_finite() || frames < 0.0 || frames > f64::from(u32::MAX) / 4.0 {
        return Err("requested duration exceeds stereo PCM RIFF size limit".into());
    }
    Ok(frames as u64)
}

fn render(options: &Options, source: &str) -> Result<serde_json::Value> {
    let caps = CapabilitySet::native();
    let (host, mut side) = NativeAudioHost::headless(options.rate, caps, CELLS);
    let clock = host.clock();
    let cells = host.cells();
    let audio = Rc::new(RefCell::new(AudioAudit::new(host, options.rate)));
    let base = std::env::current_dir()
        .map_err(|error| format!("cannot read current directory: {error}"))?;
    // Match the CLI's cwd root and also allow this project's bundled assets
    // when the compiled exporter is invoked from another working directory.
    let loader = NativeSampleLoader::new(
        &[base.clone(), PathBuf::from(env!("CARGO_MANIFEST_DIR"))],
        &base,
    );
    let score_path = if options.source.is_absolute() {
        options.source.clone()
    } else {
        base.join(&options.source)
    };
    loader.register_file(FileId::new(1), &score_path);
    let hosts = Hosts {
        audio: Box::new(CheckedHost(Rc::clone(&audio))),
        samples: Box::new(loader.clone()),
        ..Hosts::noop()
    };
    let registry = InstRegistry::shared();
    let resolver: Rc<dyn InstResolver> = Rc::new(Rc::clone(&registry));
    let cfg = RuntimeConfig {
        cell_pool: cells.capacity(),
        tier: Tier::Native(cells),
        sample_rate: options.rate,
        ..RuntimeConfig::default()
    };
    let (mut rt, mut sink) = Runtime::new(hosts, resolver, caps, cfg);
    let mut ev = Evaluator::with_insts(
        Prelude::core(),
        Box::new(loader),
        Box::new(sink.clone()),
        Rc::clone(&registry),
    );
    if !registry.borrow().template_errors().is_empty() {
        return Err(format!(
            "prelude template errors: {:?}",
            registry.borrow().template_errors()
        ));
    }
    drain(&mut rt, &mut ev)?;
    evaluate(&mut ev, source)?;
    if let Some(bpm) = options.bpm {
        evaluate(&mut ev, &format!("use-bpm {bpm}"))?;
    }
    drain(&mut rt, &mut ev)?;
    if rt.clock().source() != ClockSource::Internal {
        return Err("offline export requires internal clock".into());
    }
    let tempo = rt.clock().tempo();
    let cycle_seconds = tempo
        .cycle_seconds()
        .map_err(|failure| failure.message.to_string())?;
    let arrangement_seconds = options
        .seconds
        .unwrap_or(f64::from(options.cycles) * cycle_seconds);
    let arrangement_frames = frame_count(arrangement_seconds, options.rate)?;
    let tail_frames = frame_count(options.tail_seconds, options.rate)?;
    let frames = arrangement_frames
        .checked_add(tail_frames)
        .ok_or("frame count overflow")?;
    wav::header(frames, options.rate)?;
    audio.borrow_mut().cutoff = Some(arrangement_frames as f64 / f64::from(options.rate));
    // Install without advancing the frame clock or losing the first musical beat.
    let install_passes = install_initial_graphs(&mut side, &audio)?;
    let mut output = wav::WavOutput::create(&options.output, frames, options.rate)?;
    let mut metrics = Metrics::new(cycle_seconds, options.rate, arrangement_frames);
    let mut buffer = [0.0f32; BLOCK * 2];
    render_until(
        &mut rt,
        &mut ev,
        &mut side,
        &audio,
        &clock,
        arrangement_frames,
        tempo,
        &mut buffer,
        &mut output,
        &mut metrics,
    )?;
    // Natural slot releases cancel queued later events and let bus/voice tails ring.
    for key in rt.slots().keys() {
        sink.apply(StagedEffect::Revoke(key));
    }
    drain(&mut rt, &mut ev)?;
    render_until(
        &mut rt,
        &mut ev,
        &mut side,
        &audio,
        &clock,
        frames,
        tempo,
        &mut buffer,
        &mut output,
        &mut metrics,
    )?;
    // Consume final callback acknowledgments without scheduling new work.
    audio.borrow_mut().check_installs()?;
    if metrics.left.energy + metrics.right.energy <= 0.0 {
        return Err("render contains no audible audio".into());
    }
    output.finish()?;
    Ok(serde_json::json!({
        "score": options.source, "output": options.output, "sample_rate": options.rate,
        "channels": 2, "bits_per_sample": 16, "frames": frames,
        "duration_seconds": frames as f64 / f64::from(options.rate),
        "arrangement_frames": arrangement_frames, "tail_frames": tail_frames,
        "bpm": tempo.bpm.to_f64(), "beats_per_cycle": tempo.beats_per_cycle.to_f64(),
        "cycles": arrangement_seconds / cycle_seconds, "events": audio.borrow().events,
        "peak": metrics.left.peak.max(metrics.right.peak),
        "rms": ((metrics.left.energy+metrics.right.energy)/(2.0*frames as f64)).sqrt(),
        "left": metrics.left.json(), "right": metrics.right.json(),
        "sections": metrics.sections.iter().enumerate().map(|(index, metric)| serde_json::json!({"begin_cycle": index*8,"end_cycle": (((index+1)*8) as f64).min(arrangement_seconds/cycle_seconds),"rms": metric.rms(),"peak":metric.peak,"frames":metric.frames/2})).collect::<Vec<_>>(),
        "tail": metrics.tail.json(), "scheduling_preroll_frames": 0,
        "installation_passes": install_passes
    }))
}
fn install_initial_graphs(side: &mut AudioSide, audio: &Rc<RefCell<AudioAudit>>) -> Result<usize> {
    // Drain install work through zero-frame callbacks without advancing music.
    // Native pointer handovers currently fit one pass; repeating also supports
    // hosts that defer install work across control quanta.
    // A rejected graph never acknowledges; bound attempts rather than spinning.
    for pass in 1..=128 {
        side.render(&mut [], 2);
        if audio.borrow_mut().installs_ready()? {
            return Ok(pass);
        }
    }
    audio.borrow_mut().check_installs()?;
    Err("graph installation exceeded the 128-pass startup bound".into())
}
#[allow(clippy::too_many_arguments)]
fn render_until(
    rt: &mut Runtime,
    ev: &mut Evaluator,
    side: &mut AudioSide,
    audio: &Rc<RefCell<AudioAudit>>,
    clock: &vactr::host::native::FrameClock,
    end: u64,
    tempo: vactr::clock::Tempo,
    buffer: &mut [f32],
    output: &mut wav::WavOutput,
    metrics: &mut Metrics,
) -> Result<()> {
    while clock.frames() < end {
        tick(rt, ev, clock.now())?;
        if rt.clock().tempo() != tempo {
            return Err("tempo changed during export; use a constant-tempo score".into());
        }
        audio.borrow_mut().check()?;
        // Tick at least every 10 ms even at low sample rates, within the
        // production runtime's 30 ms commit lead.
        let quantum = BLOCK.min((metrics.rate / 100) as usize);
        let n = usize::try_from((end - clock.frames()).min(quantum as u64))
            .map_err(|_| "block size overflow")?;
        let first = clock.frames();
        side.render(&mut buffer[..n * 2], 2);
        metrics.add(first, &buffer[..n * 2])?;
        output.write(&buffer[..n * 2])?;
    }
    Ok(())
}
#[derive(Clone, Default)]
struct Measure {
    energy: f64,
    peak: f32,
    frames: u64,
}
impl Measure {
    fn add(&mut self, sample: f32) {
        self.energy += f64::from(sample).powi(2);
        self.peak = self.peak.max(sample.abs());
        self.frames += 1;
    }
    fn rms(&self) -> f64 {
        if self.frames == 0 {
            0.0
        } else {
            (self.energy / self.frames as f64).sqrt()
        }
    }
    fn json(&self) -> serde_json::Value {
        serde_json::json!({"rms":self.rms(),"peak":self.peak,"samples":self.frames})
    }
}
struct Metrics {
    left: Measure,
    right: Measure,
    sections: Vec<Measure>,
    tail: Measure,
    section_frames: f64,
    arrangement: u64,
    rate: u32,
}
impl Metrics {
    fn new(cycle_seconds: f64, rate: u32, arrangement: u64) -> Self {
        Self {
            left: Measure::default(),
            right: Measure::default(),
            sections: Vec::new(),
            tail: Measure::default(),
            section_frames: cycle_seconds * 8.0 * f64::from(rate),
            arrangement,
            rate,
        }
    }
    fn add(&mut self, first: u64, samples: &[f32]) -> Result<()> {
        for (index, frame) in samples.chunks_exact(2).enumerate() {
            let at = first + index as u64;
            for sample in frame {
                if !sample.is_finite() || sample.abs() > 1.0 {
                    return Err(format!("invalid or clipped output at frame {at}: {sample}"));
                }
            }
            self.left.add(frame[0]);
            self.right.add(frame[1]);
            if at >= self.arrangement {
                self.tail.add(frame[0]);
                self.tail.add(frame[1]);
            } else {
                let section = (at as f64 / self.section_frames).floor() as usize;
                if self.sections.len() <= section {
                    self.sections.resize(section + 1, Measure::default());
                }
                self.sections[section].add(frame[0]);
                self.sections[section].add(frame[1]);
            }
        }
        Ok(())
    }
}
fn run() -> Result<()> {
    let options = Options::parse(std::env::args().skip(1))?;
    let source = std::fs::read_to_string(&options.source)
        .map_err(|error| format!("cannot read {}: {error}", options.source.display()))?;
    // Canonical equality also catches a path alias that would replace its score.
    if options.output.exists()
        && std::fs::canonicalize(&options.source).ok()
            == std::fs::canonicalize(&options.output).ok()
    {
        return Err("output resolves to the source score".into());
    }
    let summary = render(&options, &source)?;
    eprintln!("{summary}");
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", serde_json::json!({"error":error}));
            ExitCode::FAILURE
        }
    }
}
