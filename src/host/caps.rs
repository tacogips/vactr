//! Capability traits (design 11.5, 11.7, 12.8.3).
//!
//! The core reaches every platform service through these traits, so it
//! builds for wasm32 and runs headless in tests. Every event sink shares the
//! `(slot, gen)` identity and the `SlotControl` revocation contract of 11.3.

use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::cells::CellId;
use crate::dsp::graph::{BusDef, BusId, InstDef, InstId};
use crate::host::noop::NoopHost;
use crate::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use crate::pattern::signal::Sig;
use crate::sched::slots::SlotId;
use crate::tex::shader::ShaderDesc;
use crate::tex::texnode::OutId;
use crate::tex::uniforms::Uniforms;
use crate::value::intern::KwId;
use crate::value::value::{PathVal, Sound};
use crate::vm::fail::Failure;

/// The audio sink (design 11.5, 12.8.5).
pub trait AudioHost {
    /// Queues an event on the time-ordered ring.
    fn send(&mut self, ev: AudioEvent);
    /// Sends a slot control on the priority channel (11.3).
    fn control(&mut self, c: SlotControl);
    /// Sends any other priority-channel record.
    fn post(&mut self, msg: CtlMsg);
    /// Moves every record the audio side sent back into `out`.
    fn drain(&mut self, out: &mut Vec<HostMsg>);
    /// Host seconds on the audio timebase (12.8.4).
    fn now(&self) -> f64;
    /// Installs or replaces an instrument, bus or master graph.
    fn swap_graph(&mut self, g: GraphHandle);
    /// Installs a sample. Native hands the `Arc` over; the browser runs the
    /// 16.1 sender-paced slice window. Completion arrives as
    /// `HostMsg::Installed`.
    fn install_sample(&mut self, id: u32, data: Arc<SampleData>);
    /// Retires a sample; `HostMsg::Retired` reports when no user is left.
    fn retire_sample(&mut self, id: u32);
    /// The host signals (`amp`, `fft`).
    fn analysis(&self) -> HostSigs;
}

/// The MIDI-out sink.
pub trait MidiHost {
    fn send(&mut self, ev: MidiEvent);
    fn control(&mut self, c: SlotControl);
}

/// The OSC sink.
pub trait OscHost {
    fn send(&mut self, ev: OscEvent);
    fn control(&mut self, c: SlotControl);
}

/// The visual output sink (design 9).
pub trait RenderHost {
    fn set_program(&mut self, o: OutId, sh: ShaderDesc);
    fn set_uniforms(&mut self, o: OutId, u: &Uniforms);
}

/// MIDI input, drained once per tick on the evaluator thread (11.7).
pub trait MidiInHost {
    fn poll(&mut self) -> &[MidiInEvent];
}

/// Loads sample data for installation.
pub trait SampleLoader {
    /// # Errors
    /// Any failure to read or decode; `host-unavailable` with no host.
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure>;
}

/// Resolves a sound to where its events go (design 12.8.3). BE-INST
/// implements it over the instrument registry; scheduler tests use stubs.
pub trait InstResolver {
    /// The route of one committed event's sound.
    ///
    /// # Errors
    /// `unknown-sound` and other event-local failures.
    fn route(&self, sound: &Sound) -> Result<Route, Failure>;
    /// An installed instrument template.
    fn inst(&self, id: InstId) -> Option<Arc<InstDef>>;
    /// The signals installed instruments read as control cells (12.8.6);
    /// the scheduler samples each once per tick.
    fn signal_inputs(&self) -> Vec<SignalInput>;
    /// The id of the bus declared as `name` (R3, design 12.5): `None` for
    /// an undeclared name (the `bus` control then reaches nothing and the
    /// event stays on `master`). Stub resolvers (scheduler tests) keep the
    /// default: no bus ever resolves.
    fn bus(&self, _name: KwId) -> Option<BusId> {
        None
    }
}

/// Where one event goes: MIDI and OSC are instruments too (12.8.3).
#[derive(Clone, PartialEq, Debug)]
pub enum Route {
    /// `sample` is set when the instrument plays a host bank or a sample
    /// file, so commit can gate on the resource.
    Audio {
        inst: InstId,
        sample: Option<SampleSrc>,
    },
    Midi {
        ch: u8,
    },
    Osc {
        addr: Rc<str>,
    },
}

/// A signal sampled into a control cell each tick.
#[derive(Clone, Debug)]
pub struct SignalInput {
    pub cell: CellId,
    pub sig: Rc<Sig>,
}

/// Where sample data comes from.
#[derive(Clone, PartialEq, Debug)]
pub enum SampleSrc {
    /// A host sample set; `index` is the event's `n` (7.1.4).
    Bank {
        kw: KwId,
        index: u32,
    },
    Path(PathVal),
}

/// Decoded, interleaved sample frames.
#[derive(Clone, PartialEq, Debug)]
pub struct SampleData {
    pub rate: u32,
    pub channels: u8,
    pub frames: Box<[f32]>,
}

/// A graph to install on the audio side.
#[derive(Clone, PartialEq, Debug)]
pub enum GraphHandle {
    Inst { id: InstId, def: Arc<InstDef> },
    Bus { id: BusId, def: Arc<BusDef> },
    Master(Arc<BusDef>),
}

/// A MIDI-out event; every variant but the clock carries the slot identity.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MidiEvent {
    Note {
        time: f64,
        slot: SlotId,
        gen: u32,
        ch: u8,
        note: u8,
        vel: u8,
        dur: f64,
    },
    /// The immediate note-off of a stale-started note (11.3).
    NoteOff {
        time: f64,
        slot: SlotId,
        gen: u32,
        ch: u8,
        note: u8,
    },
    /// Clock-master output (11.7).
    Clock {
        time: f64,
    },
    Start {
        time: f64,
    },
    Stop {
        time: f64,
    },
    Continue {
        time: f64,
    },
}

/// An OSC-out event.
#[derive(Clone, PartialEq, Debug)]
pub struct OscEvent {
    pub time: f64,
    pub slot: SlotId,
    pub gen: u32,
    pub addr: Rc<str>,
    pub args: Vec<OscArg>,
}

/// An OSC argument.
#[derive(Clone, PartialEq, Debug)]
pub enum OscArg {
    F(f32),
    I(i32),
    S(Rc<str>),
}

/// One MIDI input event (design 11.7).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MidiInEvent {
    Cc {
        ch: u8,
        controller: u8,
        value: u8,
        time: f64,
    },
    NoteOn {
        ch: u8,
        note: u8,
        vel: u8,
        time: f64,
    },
    NoteOff {
        ch: u8,
        note: u8,
        time: f64,
    },
    Clock {
        time: f64,
    },
    Start,
    Stop,
    Continue,
}

/// The host signals published by the audio side.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct HostSigs {
    pub amp: f32,
    pub fft: [f32; 8],
}

/// The host bundle the runtime owns.
pub struct Hosts {
    pub audio: Box<dyn AudioHost>,
    pub midi: Box<dyn MidiHost>,
    pub osc: Box<dyn OscHost>,
    pub render: Box<dyn RenderHost>,
    pub midi_in: Box<dyn MidiInHost>,
    pub samples: Box<dyn SampleLoader>,
}

impl Hosts {
    /// Every capability a `NoopHost`: the dry-run and headless bundle.
    #[must_use]
    pub fn noop() -> Self {
        Self {
            audio: Box::new(NoopHost),
            midi: Box::new(NoopHost),
            osc: Box::new(NoopHost),
            render: Box::new(NoopHost),
            midi_in: Box::new(NoopHost),
            samples: Box::new(NoopHost),
        }
    }
}
