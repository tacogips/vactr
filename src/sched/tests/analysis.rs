//! SS-ANALYSIS: captures, offline render and buffer playback through a real
//! `Evaluator` + `Runtime` with recording hosts, the prelude instruments
//! and a mock clock (design 14.5.9). The default tempo makes a cycle 2 s.

use std::cell::RefCell;
use std::rc::Rc;

use crate::dsp::caps::CapabilitySet;
use crate::dsp::offline::OFFLINE_RATE;
use crate::host::caps::{AnalysisCx, Hosts, InstResolver, SampleSrc};
use crate::host::noop::NoopHost;
use crate::host::testing::{AudioCall, MockClock, RecordingAudioHost, SYNTH_RATE};
use crate::host::wire::HostMsg;
use crate::ns::evaluator::Evaluator;
use crate::ns::insts::InstRegistry;
use crate::ns::load::SourceLoader;
use crate::ns::namespace::Prelude;
use crate::reader::span::FileId;
use crate::sched::runtime::{DrainReport, Runtime, RuntimeConfig, TickReport};
use crate::value::sample::SampleBuf;
use crate::value::value::{PathVal, Sound, Value};
use crate::vm::fail::{FailCode, Failure};

const FILE: FileId = FileId::new(1);
const DT: f64 = 0.01;

/// A loader that reads nothing and carries the analysis context.
struct AnalysisLoader(AnalysisCx);

impl SourceLoader for AnalysisLoader {
    fn read(&mut self, _: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        Err(Failure::new(FailCode::HostUnavailable, "no files"))
    }

    fn analysis(&mut self) -> Option<&mut AnalysisCx> {
        Some(&mut self.0)
    }
}

struct Rig {
    ev: Evaluator,
    rt: Runtime,
    clock: MockClock,
    audio: RecordingAudioHost,
    ticks: Vec<TickReport>,
    reg: Rc<RefCell<InstRegistry>>,
}

impl Rig {
    fn new() -> Rig {
        let clock = MockClock::new(0.0);
        let audio = RecordingAudioHost::new(clock.clone());
        let hosts = Hosts {
            audio: Box::new(audio.clone()),
            midi: Box::new(NoopHost),
            osc: Box::new(NoopHost),
            render: Box::new(NoopHost),
            midi_in: Box::new(NoopHost),
            samples: Box::new(NoopHost),
        };
        let reg = InstRegistry::shared();
        let resolver: Rc<dyn InstResolver> = Rc::new(Rc::clone(&reg));
        let caps = CapabilitySet::native();
        let (mut rt, sink) = Runtime::new(hosts, resolver, caps, RuntimeConfig::default());
        let taps = rt.tap_reader();
        let loader = AnalysisLoader(AnalysisCx { caps, taps });
        let mut ev = Evaluator::with_insts(
            Prelude::core(),
            Box::new(loader),
            Box::new(sink),
            Rc::clone(&reg),
        );
        let first = rt.drain(&mut ev);
        assert!(first.faults.is_empty(), "{:?}", first.faults);
        Rig {
            ev,
            rt,
            clock,
            audio,
            ticks: Vec::new(),
            reg,
        }
    }

    /// Evaluates `src` (every form must succeed) and drains.
    fn run(&mut self, src: &str) -> DrainReport {
        let out = self.ev.eval_str(src, FILE).expect("reads");
        for o in &out {
            assert!(o.value.is_ok(), "{src:?}: {:?}", o.value);
        }
        self.rt.drain(&mut self.ev)
    }

    /// The value of a session name.
    fn value(&mut self, name: &str) -> Value {
        let out = self.ev.eval_str(name, FILE).expect("reads");
        out[0].value.clone().expect("a value")
    }

    fn buffer(&mut self, name: &str) -> Rc<SampleBuf> {
        match self.value(name) {
            Value::Sound(s) => match &*s {
                Sound::Buffer(b) => Rc::clone(b),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    fn tick_at(&mut self, t: f64) {
        self.clock.set(t);
        let rep = self.rt.tick(&mut self.ev, t);
        self.ticks.push(rep);
    }

    /// Ticks every `DT` after the current time up to `end`.
    fn run_to(&mut self, end: f64) {
        let mut t = self.clock.now();
        if self.ticks.is_empty() {
            self.tick_at(t);
        }
        while t + DT <= end + 1e-9 {
            t += DT;
            self.tick_at(t);
        }
    }

    fn faults(&self) -> Vec<Failure> {
        self.ticks.iter().flat_map(|t| t.faults.clone()).collect()
    }

    fn sends(&self) -> usize {
        self.audio
            .calls()
            .iter()
            .filter(|(_, c)| matches!(c, AudioCall::Send(_)))
            .count()
    }
}

#[test]
fn a_capture_is_pending_until_one_cycle_after_the_next_boundary() {
    let mut rig = Rig::new();
    rig.run_to(0.5);
    let rep = rig.run("let hit capture :master 1");
    assert!(rep.faults.is_empty(), "{:?}", rep.faults);
    assert_eq!(rig.rt.pending_captures(), 1);
    let hit = rig.buffer("hit");
    // The next boundary is cycle 1 (2 s); one cycle later is 4 s.
    rig.run_to(3.98);
    assert_eq!(hit.frames(), None, "still pending");
    rig.run_to(4.01);
    assert_eq!(rig.rt.pending_captures(), 0);
    let frames = 2 * SYNTH_RATE as usize;
    assert_eq!(hit.frames(), Some(frames));
    assert_eq!(hit.rate(), SYNTH_RATE);
    let data = hit.ready_frames().expect("ready");
    assert!(data.iter().any(|x| x.abs() > 0.1), "the synthetic signal");
}

#[test]
fn a_capture_longer_than_the_tier_allows_fails_with_its_origin() {
    let mut rig = Rig::new();
    let rep = rig.run("let long capture :master 16");
    assert_eq!(rep.faults.len(), 1, "{:?}", rep.faults);
    assert_eq!(rep.faults[0].code, FailCode::BeyondCapability);
    assert!(rep.faults[0].origin.span.is_some());
    let long = rig.buffer("long");
    assert_eq!(
        long.ready_frames().expect_err("failed").code,
        FailCode::BeyondCapability
    );
}

#[test]
fn render_gives_a_non_silent_finite_buffer_and_leaves_the_live_runtime_alone() {
    let mut rig = Rig::new();
    rig.run("s :analog > note [:a4] > d1");
    rig.run_to(0.3);
    // A one-shot thunk and a `once` pattern the render must not run.
    rig.run("at 2:\n\tprint \"boom\"\ns :analog > note [:c4] > once");
    let slots: Vec<(String, u32)> = rig
        .rt
        .slots()
        .iter()
        .map(|s| (format!("{:?}", s.key), s.gen))
        .collect();
    let pos = rig.rt.clock().pos();
    let tempo = rig.rt.clock().tempo();
    let calls = rig.audio.calls().len();
    let rep = rig.run("let out render 2");
    assert!(rep.faults.is_empty(), "{:?}", rep.faults);
    assert!(rep.console.is_empty(), "no thunk ran: {:?}", rep.console);
    let out = rig.buffer("out");
    // Two cycles of 2 s at the offline rate.
    assert_eq!(out.frames(), Some(4 * OFFLINE_RATE as usize));
    assert_eq!(out.rate(), OFFLINE_RATE);
    let data = out.ready_frames().expect("ready");
    assert!(data.iter().all(|x| x.is_finite()));
    let peak = data.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01, "silent render: peak {peak}");
    // The live runtime is untouched: slots, generations, clock, host calls.
    let after: Vec<(String, u32)> = rig
        .rt
        .slots()
        .iter()
        .map(|s| (format!("{:?}", s.key), s.gen))
        .collect();
    assert_eq!(after, slots);
    assert_eq!(rig.rt.clock().pos(), pos);
    assert_eq!(rig.rt.clock().tempo(), tempo);
    assert_eq!(rig.audio.calls().len(), calls, "no live host call");
    // The thunk still runs, once, in the live session.
    rig.run_to(3.0);
    let booms = rig
        .ticks
        .iter()
        .flat_map(|t| t.console.iter())
        .filter(|l| l.contains("boom"))
        .count();
    assert_eq!(booms, 1);
}

#[test]
fn a_ready_buffer_plays_through_the_sampler_from_its_own_frames() {
    let mut rig = Rig::new();
    rig.run("s :analog > note [:a4] > d1");
    rig.run_to(0.1);
    rig.run("let out render 1\nhush");
    rig.run_to(0.5);
    let out = rig.buffer("out");
    assert!(out.frames().is_some());
    let before = rig.audio.calls().len();
    rig.run("s out > d2");
    let installs: Vec<(u32, u32, u8, usize)> = rig.audio.calls()[before..]
        .iter()
        .filter_map(|(_, c)| match c {
            AudioCall::InstallSample(id, d) => Some((*id, d.rate, d.channels, d.frames.len())),
            _ => None,
        })
        .collect();
    assert_eq!(installs.len(), 1, "{installs:?}");
    let (resource, rate, channels, len) = installs[0];
    assert_eq!((rate, channels), (OFFLINE_RATE, 2));
    assert_eq!(len, 2 * out.frames().expect("ready"));
    let src = SampleSrc::Buffer { id: out.id };
    assert!(rig.rt.samples().state(&src).is_some());
    rig.audio.reply(HostMsg::Installed { resource, gen: 1 });
    let sent = rig.sends();
    rig.run_to(2.5);
    assert!(rig.sends() > sent, "the buffer plays on d2");
    // Once installed, a `once` of the buffer commits one event.
    rig.run("hush");
    rig.run_to(3.0);
    let sent = rig.sends();
    rig.run("s out > once");
    rig.run_to(3.5);
    assert_eq!(rig.sends() - sent, 1);
    let router = rig.reg.borrow();
    assert!(matches!(
        InstResolver::route(&*router, &Sound::Buffer(Rc::clone(&out))),
        Ok(crate::host::caps::Route::Audio {
            sample: Some(SampleSrc::Buffer { .. }),
            ..
        })
    ));
}

#[test]
fn a_pending_buffer_fails_its_events_capture_pending() {
    let mut rig = Rig::new();
    rig.run("let hit capture :master 1\ns hit > d1");
    rig.run_to(1.5);
    let pending: Vec<Failure> = rig
        .faults()
        .into_iter()
        .filter(|f| f.code == FailCode::CapturePending)
        .collect();
    assert!(!pending.is_empty(), "{:?}", rig.faults());
    assert!(pending.iter().all(|f| f.origin.slot.is_some()));
    assert_eq!(rig.sends(), 0, "no event of a pending buffer is sent");
}
