//! BE-SCHED tests (design 11.3-11.6, 10.3, 10.4; vactrol-core.md TASK-007
//! criteria 1-5, 8-11 and TASK-008 criteria 5-6 diagnostic half).
//!
//! Every test drives a real `Evaluator` + `Runtime` with a mock clock and
//! recording hosts, and asserts EMITTED MULTIPLICITY at the recording host
//! (never set equality). `Played` models the sink side of the 11.3
//! revocation contract over the recorded calls with an explicit control
//! delivery delay, so each test states the cycle duration and transport
//! delay it relies on.

mod cells;
mod control;
mod dryrun;
mod faults;
mod granular;
mod merge;
mod output;
mod rebind;

use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::caps::CapabilitySet;
use crate::dsp::graph::{InstDef, InstId};
use crate::host::caps::{
    Hosts, InstResolver, MidiEvent, OscEvent, Route, SampleLoader, SampleSrc, SignalInput,
};
use crate::host::noop::NoopHost;
use crate::host::testing::{
    AudioCall, MockClock, RecordingAudioHost, RecordingMidiHost, RecordingOscHost,
    RecordingRenderHost, SinkCall,
};
use crate::host::wire::{AudioEvent, Ctl, HostMsg, Release, SlotControl, SlotControlAck};
use crate::ns::evaluator::{Evaluator, FormOutcome};
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, SlotKey, StagedEffect};
use crate::pattern::combinators::control::control;
use crate::pattern::pat::Pat;
use crate::pattern::query::TimeSpan;
use crate::reader::span::FileId;
use crate::sched::runtime::{DrainReport, Runtime, RuntimeConfig, RuntimeSink, TickReport};
use crate::sched::slots::CtlId;
use crate::value::intern::{intern_kw, name_of_kw};
use crate::value::ratio::Ratio64;
use crate::value::value::{Sound, Value};
use crate::vm::fail::{FailCode, Failure};

/// A source file id for test forms.
const FILE: FileId = FileId::new(1);

/// The tick period of every test, seconds.
pub(super) const DT: f64 = 0.01;

/// `n/d` as a ratio.
pub(super) fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).expect("ratio")
}

/// `[b, e)`.
pub(super) fn span(b: Ratio64, e: Ratio64) -> TimeSpan {
    TimeSpan { begin: b, end: e }
}

/// A granular instrument (declares `density` and `size`).
pub(super) const GRAN: InstId = InstId::new(7);
/// The instrument of every other audio sound.
pub(super) const SYNTH: InstId = InstId::new(1);
/// The instrument that plays host banks.
pub(super) const SAMPLER: InstId = InstId::new(2);

/// Routes sounds like BE-INST's registry: `:break` is a host bank,
/// `gran` sample paths the granular instrument, MIDI and OSC sounds their
/// sinks.
#[derive(Debug, Default)]
pub(super) struct Stub;

impl InstResolver for Stub {
    fn route(&self, s: &Sound) -> Result<Route, Failure> {
        Ok(match s {
            Sound::Builtin(k) if &*name_of_kw(*k) == "break" => Route::Audio {
                inst: SAMPLER,
                sample: Some(SampleSrc::Bank { kw: *k, index: 0 }),
            },
            Sound::Builtin(k) if &*name_of_kw(*k) == "nope" => {
                return Err(Failure::new(FailCode::UnknownSound, "no such sound"))
            }
            Sound::Builtin(_) => Route::Audio {
                inst: SYNTH,
                sample: None,
            },
            Sound::Sample(p) if p.text.contains("gran") => Route::Audio {
                inst: GRAN,
                sample: None,
            },
            Sound::Sample(p) => Route::Audio {
                inst: SAMPLER,
                sample: Some(SampleSrc::Path(p.clone())),
            },
            Sound::MidiOut(ch) => Route::Midi { ch: *ch },
            Sound::Osc(a) => Route::Osc { addr: a.clone() },
            Sound::Inst(id) => Route::Audio {
                inst: *id,
                sample: None,
            },
            Sound::Buffer(buf) => Route::Audio {
                inst: SAMPLER,
                sample: Some(SampleSrc::Buffer { id: buf.id }),
            },
        })
    }

    fn inst(&self, id: InstId) -> Option<Arc<InstDef>> {
        let ctl = |n: &str| crate::dsp::controls::row(n).expect("row").ctl;
        let params: Box<[(CtlId, Ctl)]> = if id == GRAN {
            Box::new([
                (ctl("density"), Ctl::Const(24.0)),
                (ctl("size"), Ctl::Const(0.1)),
            ])
        } else {
            Box::new([(ctl("amp"), Ctl::Const(0.5))])
        };
        Some(Arc::new(InstDef {
            id,
            params,
            nodes: Box::new([]),
            edges: Box::new([]),
            node_params: Box::new([]),
        }))
    }

    fn signal_inputs(&self) -> Vec<SignalInput> {
        Vec::new()
    }
}

/// A sink the evaluator releases into, shared with the runtime.
struct Shared(RuntimeSink);

impl EffectSink for Shared {
    fn apply(&mut self, e: StagedEffect) {
        self.0.apply(e);
    }
}

/// An evaluator, a runtime and the recording hosts on one mock clock.
pub(super) struct Rig {
    pub ev: Evaluator,
    pub rt: Runtime,
    pub sink: RuntimeSink,
    pub clock: MockClock,
    pub audio: RecordingAudioHost,
    pub midi: RecordingMidiHost,
    pub osc: RecordingOscHost,
    pub render: RecordingRenderHost,
    pub ticks: Vec<TickReport>,
    /// Control delivery delay: every recorded `SlotControl` is acknowledged
    /// `ack_delay` after it was sent (`None`: never acknowledged).
    pub ack_delay: Option<f64>,
    acked: usize,
}

/// The test prelude: the core prelude plus OSC and granular sounds.
fn prelude() -> Prelude {
    let mut p = Prelude::core();
    p.register_value("osc-a", Value::Sound(Rc::new(Sound::Osc(Rc::from("/a")))));
    p
}

impl Rig {
    pub(super) fn new() -> Rig {
        Rig::with(RuntimeConfig::default(), CapabilitySet::native())
    }

    pub(super) fn with(cfg: RuntimeConfig, caps: CapabilitySet) -> Rig {
        Rig::with_loader(cfg, caps, Box::new(NoopHost))
    }

    pub(super) fn with_loader(
        cfg: RuntimeConfig,
        caps: CapabilitySet,
        samples: Box<dyn SampleLoader>,
    ) -> Rig {
        let clock = MockClock::new(0.0);
        let audio = RecordingAudioHost::new(clock.clone());
        let midi = RecordingMidiHost::new(clock.clone());
        let osc = RecordingOscHost::new(clock.clone());
        let render = RecordingRenderHost::new(clock.clone());
        let hosts = Hosts {
            audio: Box::new(audio.clone()),
            midi: Box::new(midi.clone()),
            osc: Box::new(osc.clone()),
            render: Box::new(render.clone()),
            midi_in: Box::new(NoopHost),
            samples,
        };
        let (rt, sink) = Runtime::new(hosts, Rc::new(Stub), caps, cfg);
        let ev = Evaluator::new(
            prelude(),
            Box::new(NoopHost),
            Box::new(Shared(sink.clone())),
        );
        Rig {
            ev,
            rt,
            sink,
            clock,
            audio,
            midi,
            osc,
            render,
            ticks: Vec::new(),
            ack_delay: Some(0.0),
            acked: 0,
        }
    }

    /// Evaluates `src` (every form must succeed) and drains the runtime.
    pub(super) fn run(&mut self, src: &str) -> DrainReport {
        let out = self.eval(src);
        for o in &out {
            assert!(o.value.is_ok(), "{src:?}: {:?}", o.value);
        }
        self.rt.drain(&mut self.ev)
    }

    /// Evaluates `src` without asserting success and without draining.
    pub(super) fn eval(&mut self, src: &str) -> Vec<FormOutcome> {
        self.ev
            .eval_str(src, FILE)
            .unwrap_or_else(|d| panic!("{src:?}: {d}"))
    }

    /// Releases one effect directly and drains.
    pub(super) fn apply(&mut self, e: StagedEffect) -> DrainReport {
        self.sink.apply(e);
        self.rt.drain(&mut self.ev)
    }

    /// Binds a Rust-built pattern to `slot`.
    pub(super) fn bind(&mut self, slot: SlotKey, p: Pat) -> DrainReport {
        self.apply(StagedEffect::SlotBind {
            slot,
            value: Value::Pattern(Rc::new(p)),
        })
    }

    /// Acknowledges recorded controls whose delivery time has come.
    fn deliver_acks(&mut self) {
        let Some(delay) = self.ack_delay else {
            return;
        };
        let now = self.clock.now();
        let calls = self.audio.calls();
        let controls: Vec<(f64, SlotControl)> = calls
            .iter()
            .filter_map(|(t, c)| match c {
                AudioCall::Control(c) => Some((*t, *c)),
                _ => None,
            })
            .collect();
        while self.acked < controls.len() && controls[self.acked].0 + delay <= now + 1e-9 {
            let (_, c) = controls[self.acked];
            self.audio.reply(HostMsg::SlotControlAck(SlotControlAck {
                slot: c.slot,
                gen: c.new_gen,
            }));
            self.acked += 1;
        }
    }

    /// One tick at host time `t`.
    pub(super) fn tick_at(&mut self, t: f64) -> &TickReport {
        self.clock.set(t);
        self.deliver_acks();
        let rep = self.rt.tick(&mut self.ev, t);
        self.ticks.push(rep);
        self.ticks.last().expect("a tick")
    }

    /// Ticks every `DT` from the current time up to and including `end`.
    pub(super) fn run_to(&mut self, end: f64) {
        let mut t = self.clock.now();
        if self.ticks.is_empty() {
            self.tick_at(t);
        }
        while t + DT <= end + 1e-9 {
            t += DT;
            self.tick_at(t);
        }
    }

    /// Every audio event sent, with its arrival time.
    pub(super) fn sent(&self) -> Vec<(f64, AudioEvent)> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                AudioCall::Send(e) => Some((t, e)),
                _ => None,
            })
            .collect()
    }

    /// Every slot control the audio host received, with its arrival time.
    pub(super) fn controls(&self) -> Vec<(f64, SlotControl)> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                AudioCall::Control(c) => Some((t, c)),
                _ => None,
            })
            .collect()
    }

    /// Every MIDI call.
    pub(super) fn midi_calls(&self) -> Vec<(f64, SinkCall<MidiEvent>)> {
        self.midi.calls()
    }

    /// Every OSC call.
    pub(super) fn osc_calls(&self) -> Vec<(f64, SinkCall<OscEvent>)> {
        self.osc.calls()
    }

    /// Every fault any tick reported.
    pub(super) fn faults(&self) -> Vec<Failure> {
        self.ticks.iter().flat_map(|t| t.faults.clone()).collect()
    }

    /// Every console line any tick forwarded.
    pub(super) fn console(&self) -> Vec<String> {
        self.ticks
            .iter()
            .flat_map(|t| t.console.iter().map(|s| s.to_string()))
            .collect()
    }

    /// Every diagnostic any tick reported.
    pub(super) fn diags(&self) -> Vec<crate::types::diag::Diagnostic> {
        self.ticks.iter().flat_map(|t| t.diags.clone()).collect()
    }
}

/// The value of control `name` on an event.
pub(super) fn ctl(e: &AudioEvent, name: &str) -> Option<Ctl> {
    let id = crate::dsp::controls::row(name).expect("row").ctl;
    e.controls().iter().find(|(c, _)| *c == id).map(|(_, v)| *v)
}

/// The constant value of control `name`.
pub(super) fn cval(e: &AudioEvent, name: &str) -> Option<f32> {
    match ctl(e, name)? {
        Ctl::Const(v) => Some(v),
        Ctl::Cell(_) => None,
    }
}

/// `subject > name value` built in Rust (controls without a native),
/// spanned like its subject.
pub(super) fn with_ctl(subject: Pat, name: &str, v: Value) -> Pat {
    let value = crate::pattern::build::pure(v, None);
    let span = subject.span;
    control(intern_kw(name), Rc::new(value), Rc::new(subject), span)
}

/// The pattern bound to a slot by source text, e.g. `s :bd`.
pub(super) fn pat_of(rig: &mut Rig, src: &str) -> Pat {
    let out = rig.eval(src);
    match &out.last().expect("a form").value {
        Ok(Value::Pattern(p)) => (**p).clone(),
        other => panic!("{src:?} is not a pattern: {other:?}"),
    }
}

/// One started (or dropped) voice in the sink model.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Voice {
    pub ev: AudioEvent,
    /// `None`: dropped before it started.
    pub start: Option<f64>,
    /// When the voice was gated (a stale start cut, a hush), if ever.
    pub cut: Option<f64>,
}

/// The sink side of 11.3 over the recorded audio calls: each control lands
/// `delay` after it was sent (a `lost` index never lands); an event batch
/// carrying a newer generation lands at its arrival (the piggyback).
pub(super) fn played(rig: &Rig, delay: f64, lost: &[usize]) -> Vec<Voice> {
    let calls = rig.audio.calls();
    let mut landed: Vec<(f64, SlotControl)> = Vec::new();
    let mut piggy: Vec<(f64, AudioEvent)> = Vec::new();
    let mut ci = 0;
    for (t, c) in &calls {
        match c {
            AudioCall::Control(c) => {
                if !lost.contains(&ci) {
                    landed.push((t + delay, *c));
                }
                ci += 1;
            }
            AudioCall::Send(e) => piggy.push((*t, *e)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (_, e) in &piggy {
        let mut v = Voice {
            ev: *e,
            start: Some(e.time),
            cut: None,
        };
        for (land, c) in &landed {
            if c.slot != e.slot || c.new_gen <= e.gen {
                continue;
            }
            if e.time >= c.effective_time {
                if *land <= e.time {
                    v.start = None; // dropped from the queue
                } else {
                    v.cut = Some(v.cut.map_or(*land, |x: f64| x.min(*land))); // late start
                }
            } else if c.release == Release::Panic {
                let at = land.max(c.effective_time);
                v.cut = Some(v.cut.map_or(at, |x: f64| x.min(at)));
            }
        }
        // Piggyback default policy: a newer-gen batch whose control has not
        // landed yet drops queued older events and short-gates stale-started
        // ones from its receipt on (it cannot know the effective time).
        for (arr, n) in &piggy {
            let known = landed
                .iter()
                .any(|(land, c)| c.slot == n.slot && c.new_gen >= n.gen && land <= arr);
            if n.slot == e.slot && n.gen > e.gen && !known {
                if v.start.is_some_and(|s| s >= *arr) {
                    v.start = None;
                } else if v.start.is_some() && v.cut.is_none_or(|c| c > *arr) {
                    v.cut = Some(*arr);
                }
            }
        }
        out.push(v);
    }
    out
}

/// The voices that actually started.
pub(super) fn started(v: &[Voice]) -> Vec<Voice> {
    v.iter().copied().filter(|x| x.start.is_some()).collect()
}
