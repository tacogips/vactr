//! The session half's render and MIDI-input hosts (design 15.1.2 G1,
//! `command.md` "Browser transport (raw wasm ABI, TASK-010)").
//!
//! [`WasmRenderHost`] turns every `RenderHost` call into a `TAG_RENDER` JSON
//! record. [`WasmMidiIn`] is the `MidiInHost` twin of the native
//! `NativeMidiIn` (`src/host/native/midi.rs`): [`parse_midi`] decodes raw
//! MIDI bytes with the identical semantics, [`session_half::session_midi_in`]
//! pushes them onto a shared [`MidiQueue`], and `poll` drains that queue
//! once per tick (11.7). This module performs no I/O of its own; every
//! record crosses through [`push_json`] into the outbox (`abi::push_record`).
//!
//! [`session_half::session_midi_in`]: crate::host::wasm::session_half::session_midi_in

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;

use crate::host::caps::{MidiInEvent, MidiInHost, RenderHost};
use crate::host::wasm::abi::{push_record, TAG_RENDER, TAG_SESSION};
use crate::session::codec;
use crate::session::protocol::{Envelope, ServerMsg};
use crate::tex::shader::ShaderDesc;
use crate::tex::texnode::OutId;
use crate::tex::uniforms::Uniforms;

/// Pushes one framed record `[tag][UTF-8 bytes of `text`]`.
pub fn push_json(tag: u8, text: &str) {
    let mut rec = Vec::with_capacity(1 + text.len());
    rec.push(tag);
    rec.extend_from_slice(text.as_bytes());
    push_record(&rec);
}

/// Pushes one Session Protocol v1 server envelope as a `TAG_SESSION` record.
pub fn push_envelope(env: &Envelope<ServerMsg>) {
    push_json(TAG_SESSION, &codec::encode(env));
}

/// The session half's `RenderHost`: every call becomes a `TAG_RENDER` JSON
/// record (design 9.2, 9.3). An `OutId` above 3 (o0..o3 only) pushes
/// nothing.
pub struct WasmRenderHost;

impl RenderHost for WasmRenderHost {
    fn set_program(&mut self, o: OutId, sh: ShaderDesc) {
        let n = o.get();
        if n > 3 {
            return;
        }
        let assets: Vec<serde_json::Value> = sh
            .assets
            .iter()
            .map(|a| json!({"id": a.id, "text": a.text}))
            .collect();
        let uniform_names: Vec<&str> = sh.uniform_names.iter().map(String::as_str).collect();
        let rec = json!({
            "op": "program",
            "out": n,
            "source": sh.source,
            "uniform_names": uniform_names,
            "assets": assets,
        });
        push_json(TAG_RENDER, &rec.to_string());
    }

    fn set_uniforms(&mut self, o: OutId, u: &Uniforms) {
        let n = o.get();
        if n > 3 {
            return;
        }
        let values: &[f32] = &u.values;
        let rec = json!({"op": "uniforms", "out": n, "values": values});
        push_json(TAG_RENDER, &rec.to_string());
    }
}

/// Parses one complete MIDI message received at `time`. An identical copy
/// of `host::native::midi::parse_midi` (lines 36-77): note on, note off (a
/// note on with velocity 0 is an off), control change, clock, start, stop
/// and continue are events; everything else (running-status data, short
/// messages, other status bytes, sysex) is `None`. The native module is not
/// compiled on wasm32, so this half carries its own copy.
#[must_use]
pub fn parse_midi(bytes: &[u8], time: f64) -> Option<MidiInEvent> {
    let (&status, data) = bytes.split_first()?;
    if status < 0x80 {
        return None;
    }
    let ch = (status & 0x0F) + 1;
    let two = || match data {
        [a, b, ..] if a & 0x80 == 0 && b & 0x80 == 0 => Some((*a, *b)),
        _ => None,
    };
    match status {
        0x80..=0x8F => two().map(|(note, _)| MidiInEvent::NoteOff { ch, note, time }),
        0x90..=0x9F => two().map(|(note, vel)| {
            if vel == 0 {
                MidiInEvent::NoteOff { ch, note, time }
            } else {
                MidiInEvent::NoteOn {
                    ch,
                    note,
                    vel,
                    time,
                }
            }
        }),
        0xB0..=0xBF => two().map(|(controller, value)| MidiInEvent::Cc {
            ch,
            controller,
            value,
            time,
        }),
        0xF8 => Some(MidiInEvent::Clock { time }),
        0xFA => Some(MidiInEvent::Start),
        0xFB => Some(MidiInEvent::Continue),
        0xFC => Some(MidiInEvent::Stop),
        _ => None,
    }
}

/// The queue `session_midi_in` fills and [`WasmMidiIn::poll`] drains, shared
/// with the `Hosts` bundle at `session_init`.
#[derive(Clone, Default)]
pub struct MidiQueue(pub Rc<RefCell<Vec<MidiInEvent>>>);

impl MidiQueue {
    /// Queues one decoded event.
    pub fn push(&self, e: MidiInEvent) {
        self.0.borrow_mut().push(e);
    }
}

/// The session half's `MidiInHost` (design 11.7): `poll` moves every event
/// queued since the previous poll into the buffer it returns, the same
/// shape as `ScriptedMidiIn` (`src/sched/tests/midi.rs`).
pub struct WasmMidiIn {
    queue: MidiQueue,
    polled: Vec<MidiInEvent>,
}

impl WasmMidiIn {
    /// A `MidiInHost` over the shared queue `session_midi_in` fills.
    #[must_use]
    pub fn new(queue: MidiQueue) -> Self {
        Self {
            queue,
            polled: Vec::new(),
        }
    }
}

impl MidiInHost for WasmMidiIn {
    fn poll(&mut self) -> &[MidiInEvent] {
        self.polled = self.queue.0.borrow_mut().drain(..).collect();
        &self.polled
    }
}
