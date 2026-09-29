//! BE-MIDI tests (design 11.7; vactr-core.md TASK-007 criteria 6, 7 and
//! the open-input half of 10).
//!
//! Every test drives a real `Evaluator` + `Runtime` on a mock clock with
//! recording hosts and a scripted `MidiInHost`. The note-lifetime tests feed
//! the recorded priority-channel records into a real `dsp::Engine` at their
//! arrival times and assert the engine's voice state; every engine block
//! runs with the allocation probe armed.

mod clock;
mod input;
mod lifetime;
mod transport;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use super::sched::{Stub, DT, SYNTH};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::StoreKind;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::AtomicCells;
use crate::dsp::engine::{Engine, EngineConfig, EngineIo};
use crate::dsp::graph::{Edge, InstDef, UGenSpec};
use crate::dsp::ring::{
    AckConsumer, AckProducer, Consumer, EventConsumer, EventProducer, EventRing, Garbage,
    NativeInstall, NativeRecord, Producer, SpscRing,
};
use crate::dsp::ugen::Template;
use crate::dsp::voice::Voice;
use crate::host::caps::{Hosts, MidiEvent, MidiInEvent, MidiInHost};
use crate::host::noop::NoopHost;
use crate::host::testing::{
    AudioCall, MockClock, RecordingAudioHost, RecordingMidiHost, RecordingOscHost,
    RecordingRenderHost, SinkCall,
};
use crate::host::wire::{CtlMsg, HostMsg, SlotControl, SlotControlAck, VoiceTag};
use crate::ns::evaluator::{Evaluator, FormOutcome};
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, StagedEffect};
use crate::reader::span::FileId;
use crate::sched::runtime::{DrainReport, Runtime, RuntimeConfig, RuntimeSink, TickReport};
use crate::types::diag::{DiagCode, Diagnostic};

const FILE: FileId = FileId::new(1);

/// A `MidiInHost` whose events the test scripts; `poll` hands over
/// everything queued since the last poll.
#[derive(Clone, Debug, Default)]
pub(super) struct ScriptedMidiIn {
    queue: Rc<RefCell<VecDeque<MidiInEvent>>>,
    polled: Vec<MidiInEvent>,
}

impl ScriptedMidiIn {
    pub(super) fn push(&self, e: MidiInEvent) {
        self.queue.borrow_mut().push_back(e);
    }
}

impl MidiInHost for ScriptedMidiIn {
    fn poll(&mut self) -> &[MidiInEvent] {
        self.polled = self.queue.borrow_mut().drain(..).collect();
        &self.polled
    }
}

struct Shared(RuntimeSink);

impl EffectSink for Shared {
    fn apply(&mut self, e: StagedEffect) {
        self.0.apply(e);
    }
}

/// An evaluator and a runtime over recording hosts and scripted MIDI input.
pub(super) struct MidiRig {
    pub ev: Evaluator,
    pub rt: Runtime,
    pub clock: MockClock,
    pub audio: RecordingAudioHost,
    pub midi: RecordingMidiHost,
    pub osc: RecordingOscHost,
    pub input: ScriptedMidiIn,
    pub ticks: Vec<TickReport>,
    acked: usize,
}

impl MidiRig {
    pub(super) fn new() -> MidiRig {
        let clock = MockClock::new(0.0);
        let audio = RecordingAudioHost::new(clock.clone());
        let midi = RecordingMidiHost::new(clock.clone());
        let osc = RecordingOscHost::new(clock.clone());
        let input = ScriptedMidiIn::default();
        let hosts = Hosts {
            audio: Box::new(audio.clone()),
            midi: Box::new(midi.clone()),
            osc: Box::new(osc.clone()),
            render: Box::new(RecordingRenderHost::new(clock.clone())),
            midi_in: Box::new(input.clone()),
            samples: Box::new(NoopHost),
        };
        let (rt, sink) = Runtime::new(
            hosts,
            Rc::new(Stub),
            CapabilitySet::native(),
            RuntimeConfig::default(),
        );
        let ev = Evaluator::new(Prelude::core(), Box::new(NoopHost), Box::new(Shared(sink)));
        MidiRig {
            ev,
            rt,
            clock,
            audio,
            midi,
            osc,
            input,
            ticks: Vec::new(),
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

    pub(super) fn eval(&mut self, src: &str) -> Vec<FormOutcome> {
        self.ev
            .eval_str(src, FILE)
            .unwrap_or_else(|d| panic!("{src:?}: {d}"))
    }

    /// Queues MIDI input for the next tick.
    pub(super) fn send(&self, e: MidiInEvent) {
        self.input.push(e);
    }

    /// One tick at `t`; recorded slot controls are acknowledged at once.
    pub(super) fn tick_at(&mut self, t: f64) -> &TickReport {
        self.clock.set(t);
        let controls = self.slot_controls();
        for (_, c) in &controls[self.acked..] {
            self.audio.reply(HostMsg::SlotControlAck(SlotControlAck {
                slot: c.slot,
                gen: c.new_gen,
            }));
        }
        self.acked = controls.len();
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

    /// One more tick, `DT` after the last.
    pub(super) fn step(&mut self) -> &TickReport {
        let t = self.clock.now() + DT;
        self.tick_at(t)
    }

    /// Every priority-channel record other than slot controls.
    pub(super) fn posts(&self) -> Vec<(f64, CtlMsg)> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                AudioCall::Post(m) => Some((t, m)),
                _ => None,
            })
            .collect()
    }

    /// Every live start, in order.
    pub(super) fn live_starts(&self) -> Vec<VoiceTag> {
        self.posts()
            .into_iter()
            .filter_map(|(_, m)| match m {
                CtlMsg::LiveNoteOn { tag, .. } => Some(tag),
                _ => None,
            })
            .collect()
    }

    /// Every voice release, in order.
    pub(super) fn releases(&self) -> Vec<VoiceTag> {
        self.posts()
            .into_iter()
            .filter_map(|(_, m)| match m {
                CtlMsg::VoiceRelease { tag } => Some(tag),
                _ => None,
            })
            .collect()
    }

    /// Every slot control the audio host received.
    pub(super) fn slot_controls(&self) -> Vec<(f64, SlotControl)> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                AudioCall::Control(c) => Some((t, c)),
                _ => None,
            })
            .collect()
    }

    /// Every event on the time-ordered ring.
    pub(super) fn ring_events(&self) -> usize {
        self.audio
            .calls()
            .iter()
            .filter(|(_, c)| matches!(c, AudioCall::Send(_)))
            .count()
    }

    /// Every MIDI event sent (controls excluded).
    pub(super) fn midi_sent(&self) -> Vec<(f64, MidiEvent)> {
        self.midi
            .calls()
            .into_iter()
            .filter_map(|(t, c)| match c {
                SinkCall::Send(e) => Some((t, e)),
                SinkCall::Control(_) => None,
            })
            .collect()
    }

    /// Every diagnostic any tick reported.
    pub(super) fn diags(&self) -> Vec<Diagnostic> {
        self.ticks.iter().flat_map(|t| t.diags.clone()).collect()
    }

    /// The diagnostics with `code`.
    pub(super) fn diags_with(&self, code: DiagCode) -> Vec<Diagnostic> {
        self.diags()
            .into_iter()
            .filter(|d| d.code == code)
            .collect()
    }
}

/// `NoteOn` on `ch` at velocity 100.
pub(super) fn on(ch: u8, note: u8) -> MidiInEvent {
    MidiInEvent::NoteOn {
        ch,
        note,
        vel: 100,
        time: 0.0,
    }
}

/// `NoteOff` on `ch`.
pub(super) fn off(ch: u8, note: u8) -> MidiInEvent {
    MidiInEvent::NoteOff {
        ch,
        note,
        time: 0.0,
    }
}

/// The engine sample rate and block of the lifetime tests.
const SR: f32 = 48_000.0;
const BLOCK: usize = 128;

/// A real audio engine fed with a runtime's recorded audio calls.
pub(super) struct EngineRig {
    pub engine: Engine,
    events: EventProducer,
    events_rx: EventConsumer,
    controls: Producer<NativeRecord>,
    controls_rx: Consumer<NativeRecord>,
    acks_tx: AckProducer,
    pub acks_rx: AckConsumer,
    garbage_tx: Producer<Garbage>,
    _garbage_rx: Consumer<Garbage>,
    cells: AtomicCells,
    buf: Vec<f32>,
    fed: usize,
}

impl EngineRig {
    /// An engine with `voices` voices and the sustaining `SYNTH`
    /// instrument (`sin-osc * env-adsr`) installed.
    pub(super) fn new(voices: u8) -> EngineRig {
        let caps = CapabilitySet {
            max_voices: u16::from(voices),
            ..CapabilitySet::browser()
        };
        let cfg = EngineConfig {
            template_slots: 4,
            bus_slots: 2,
            bus_seconds: 0.1,
            voice_seconds: 0.05,
            orbits: 1,
            orbit_delay_seconds: 0.05,
            analysis_cells: 16,
            event_capacity: 256,
            ..EngineConfig::new(&caps, SR, BLOCK, StoreKind::NativeArc)
        };
        let engine = Engine::with_config(cfg);
        let (events, events_rx) = EventRing::split(1024);
        let (controls, controls_rx) = SpscRing::split(1024);
        let (acks_tx, acks_rx) = SpscRing::split(4096);
        let (garbage_tx, garbage_rx) = SpscRing::split(64);
        let mut rig = EngineRig {
            engine,
            events,
            events_rx,
            controls,
            controls_rx,
            acks_tx,
            acks_rx,
            garbage_tx,
            _garbage_rx: garbage_rx,
            cells: AtomicCells::new(1024),
            buf: vec![0.0; 2 * BLOCK],
            fed: 0,
        };
        let def = InstDef {
            id: SYNTH,
            params: Box::new([]),
            nodes: Box::new([UGenSpec::SinOsc, UGenSpec::EnvAdsr, UGenSpec::Mul]),
            edges: Box::new([
                Edge {
                    from: 0,
                    to: 2,
                    port: 0,

                    output: 0,
                },
                Edge {
                    from: 1,
                    to: 2,
                    port: 1,

                    output: 0,
                },
            ]),
            node_params: Box::new([]),
        };
        let template = Template::from_inst(&def, &rig.engine.build_env()).expect("template");
        let install = NativeInstall::Inst {
            resource: SYNTH.get(),
            gen: 1,
            template,
        };
        assert!(rig.controls.push(NativeRecord::Install(install)).is_ok());
        rig.block();
        rig
    }

    /// Renders one block with the allocation probe armed.
    pub(super) fn block(&mut self) {
        let mut io = EngineIo {
            events: &mut self.events_rx,
            controls: &mut self.controls_rx,
            acks: &mut self.acks_tx,
            cells: &mut self.cells,
            garbage: Some(&mut self.garbage_tx),
        };
        let engine = &mut self.engine;
        let buf = &mut self.buf;
        let ((), allocs) = armed(|| engine.process(&mut io, buf, BLOCK));
        assert_eq!(allocs, 0, "Engine::process allocated");
    }

    /// Renders until the engine clock reaches `t`.
    pub(super) fn run_until(&mut self, t: f64) {
        while self.engine.now() < t {
            self.block();
        }
    }

    /// Posts one priority record.
    pub(super) fn post(&mut self, m: CtlMsg) {
        assert!(self.controls.push(NativeRecord::Msg(m)).is_ok());
    }

    /// Feeds the audio calls recorded since the last feed, each once the
    /// engine clock reached its arrival time, and renders one more block.
    pub(super) fn feed(&mut self, rig: &MidiRig) {
        let calls = rig.audio.calls();
        for (t, c) in &calls[self.fed..] {
            self.run_until(*t);
            match c {
                AudioCall::Control(c) => self.post(CtlMsg::SlotControl(*c)),
                AudioCall::Post(m) => self.post(*m),
                AudioCall::Send(e) => assert!(self.events.push(*e).is_ok()),
                _ => {}
            }
        }
        self.fed = calls.len();
        self.block();
    }

    /// The active voice playing `tag`.
    pub(super) fn voice(&self, tag: VoiceTag) -> Option<&Voice> {
        self.engine
            .voices()
            .voices
            .iter()
            .find(|v| v.active && v.tag == Some(tag))
    }

    /// Everything the engine sent back.
    pub(super) fn acks(&mut self) -> Vec<HostMsg> {
        std::iter::from_fn(|| self.acks_rx.pop()).collect()
    }
}
