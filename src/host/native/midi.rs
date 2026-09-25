//! Native MIDI through midir (design 11.3, 11.7, 12.8.10).
//!
//! - Input: midir calls back on its own thread; `parse_midi` turns the raw
//!   bytes into a `MidiInEvent` stamped with the audio timebase, and a
//!   bounded channel carries it to `NativeMidiIn::poll` on the evaluator
//!   thread (a full channel drops and counts).
//! - Output: `MidiEvent`s are scheduled ahead of time by the scheduler, so
//!   a sender thread owns the midir connection and a `MidiOutQueue` and
//!   sends each message when the frame clock reaches its time. A
//!   `SlotControl` revokes the not yet started notes of older generations
//!   at or after its effective time; a panic release also gates their
//!   sounding notes there (11.3).
//!
//! Channels are the user's 1..=16 on both sides (`{midi 1}`,
//! `cc n channel: 1`); the wire nibble is `ch - 1`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::host::caps::{MidiEvent, MidiHost, MidiInEvent, MidiInHost};
use crate::host::native::audio::FrameClock;
use crate::host::native::unavailable;
use crate::host::wire::{Release, SlotControl};
use crate::sched::slots::SlotId;
use crate::types::diag::Diagnostic;

/// Input events buffered between two polls.
pub const MIDI_IN_QUEUE: usize = 1024;
/// The longest the sender thread sleeps between clock reads.
const OUT_POLL: Duration = Duration::from_millis(1);

/// Parses one complete MIDI message received at `time`. Note on, note off
/// (a note on with velocity 0 is an off), control change, clock, start,
/// stop and continue are events; everything else (running-status data,
/// short messages, other status bytes, sysex) is `None`.
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

/// One outgoing MIDI message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MidiMsg {
    bytes: [u8; 3],
    len: u8,
}

impl MidiMsg {
    fn one(b: u8) -> Self {
        Self {
            bytes: [b, 0, 0],
            len: 1,
        }
    }

    fn three(status: u8, ch: u8, a: u8, b: u8) -> Self {
        let nibble = ch.clamp(1, 16) - 1;
        Self {
            bytes: [status | nibble, a & 0x7F, b & 0x7F],
            len: 3,
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    due: f64,
    seq: u64,
    msg: MidiMsg,
    /// The `(slot, gen)` of a note message; `None` for system messages.
    owner: Option<(SlotId, u32)>,
    /// `Some(pair)` for a note on (its own seq) and its note off.
    pair: Option<u64>,
    on: bool,
}

/// The time-ordered outgoing queue with 11.3 revocation. Pure logic: the
/// sender thread drives it with the frame clock.
#[derive(Debug, Default)]
pub struct MidiOutQueue {
    entries: Vec<Entry>,
    seq: u64,
}

impl MidiOutQueue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queued messages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn push(&mut self, due: f64, msg: MidiMsg, owner: Option<(SlotId, u32)>, pair: Option<u64>) {
        let seq = self.seq;
        self.seq += 1;
        let on = pair == Some(seq);
        self.entries.push(Entry {
            // `+inf` is the off of an open note (never due by time).
            due: if due.is_nan() || due == f64::NEG_INFINITY {
                0.0
            } else {
                due
            },
            seq,
            msg,
            owner,
            pair,
            on,
        });
        self.sort();
    }

    fn sort(&mut self) {
        self.entries
            .sort_by(|a, b| a.due.total_cmp(&b.due).then(a.seq.cmp(&b.seq)));
    }

    /// Queues an event: a note becomes a note on at `time` and a note off
    /// at `time + dur`. With an infinite `dur` the note is open: its off
    /// waits for an explicit `NoteOff`, a panic, or shutdown.
    pub fn schedule(&mut self, ev: MidiEvent) {
        match ev {
            MidiEvent::Note {
                time,
                slot,
                gen,
                ch,
                note,
                vel,
                dur,
            } => {
                let pair = self.seq;
                let owner = Some((slot, gen));
                self.push(time, MidiMsg::three(0x90, ch, note, vel), owner, Some(pair));
                let off = time + dur.max(0.0);
                self.push(off, MidiMsg::three(0x80, ch, note, 0), owner, Some(pair));
            }
            MidiEvent::NoteOff {
                time,
                slot,
                gen,
                ch,
                note,
            } => {
                let msg = MidiMsg::three(0x80, ch, note, 0);
                // Closes the earliest open note of this slot and pitch.
                let open = self.entries.iter_mut().find(|e| {
                    !e.on
                        && e.due == f64::INFINITY
                        && e.msg == msg
                        && matches!(e.owner, Some((s, _)) if s == slot)
                });
                if let Some(e) = open {
                    e.due = if time.is_finite() { time } else { 0.0 };
                    self.sort();
                } else {
                    self.push(time, msg, Some((slot, gen)), None);
                }
            }
            MidiEvent::Clock { time } => self.push(time, MidiMsg::one(0xF8), None, None),
            MidiEvent::Start { time } => self.push(time, MidiMsg::one(0xFA), None, None),
            MidiEvent::Continue { time } => self.push(time, MidiMsg::one(0xFB), None, None),
            MidiEvent::Stop { time } => self.push(time, MidiMsg::one(0xFC), None, None),
        }
    }

    /// Applies a slot control (11.3): note ons of `slot` older than
    /// `new_gen` due at or after the effective time are dropped with their
    /// note offs; with a panic release the older notes still sounding at
    /// the effective time are cut there.
    pub fn control(&mut self, c: SlotControl) {
        let old = |e: &Entry| matches!(e.owner, Some((s, g)) if s == c.slot && g < c.new_gen);
        let mut revoked = Vec::new();
        self.entries.retain(|e| {
            let drop = e.on && old(e) && e.due >= c.effective_time;
            if drop {
                revoked.extend(e.pair);
            }
            !drop
        });
        self.entries
            .retain(|e| e.on || !e.pair.is_some_and(|p| revoked.contains(&p)));
        if c.release == Release::Panic {
            for e in &mut self.entries {
                if !e.on && e.owner.is_some() && old(e) && e.due > c.effective_time {
                    e.due = c.effective_time;
                }
            }
            self.sort();
        }
    }

    /// Moves every message due at `now` into `out`, in time order.
    pub fn take_due(&mut self, now: f64, out: &mut Vec<MidiMsg>) {
        let n = self.entries.iter().take_while(|e| e.due <= now).count();
        out.extend(self.entries.drain(..n).map(|e| e.msg));
    }

    /// The time of the next message.
    #[must_use]
    pub fn next_due(&self) -> Option<f64> {
        self.entries.first().map(|e| e.due)
    }

    /// Shutdown: the note offs of every sounding note (their note on was
    /// taken); everything else is discarded.
    pub fn release_all(&mut self, out: &mut Vec<MidiMsg>) {
        let pending_on: Vec<u64> = self
            .entries
            .iter()
            .filter(|e| e.on)
            .filter_map(|e| e.pair)
            .collect();
        for e in self.entries.drain(..) {
            let sounding = !e.on && e.pair.is_some_and(|p| !pending_on.contains(&p));
            if sounding {
                out.push(e.msg);
            }
        }
    }
}

/// Picks the configured port (exact name, then substring) or the first.
fn pick<P: Clone>(ports: &[P], name_of: impl Fn(&P) -> String, want: Option<&str>) -> Option<P> {
    let named: Vec<(P, String)> = ports.iter().map(|p| (p.clone(), name_of(p))).collect();
    match want {
        None => named.into_iter().next().map(|(p, _)| p),
        Some(w) => named
            .iter()
            .find(|(_, n)| n == w)
            .or_else(|| named.iter().find(|(_, n)| n.contains(w)))
            .map(|(p, _)| p.clone()),
    }
}

/// MIDI input from one midir port.
pub struct NativeMidiIn {
    rx: Receiver<MidiInEvent>,
    buf: Vec<MidiInEvent>,
    dropped: Arc<AtomicU32>,
    port: String,
    _conn: MidiInputConnection<()>,
}

impl NativeMidiIn {
    /// Connects to `port` (or the first input port); events are stamped
    /// with `clock`.
    ///
    /// # Errors
    /// `beyond-capability` when MIDI input is unavailable or no port
    /// matches.
    pub fn open(port: Option<&str>, clock: FrameClock) -> Result<Self, Diagnostic> {
        let input = MidiInput::new("vactrol")
            .map_err(|e| unavailable(format!("MIDI input is not available on this host ({e})")))?;
        let ports = input.ports();
        let chosen =
            pick(&ports, |p| input.port_name(p).unwrap_or_default(), port).ok_or_else(|| {
                unavailable(match port {
                    Some(p) => format!("MIDI input port `{p}` is not available on this host"),
                    None => "MIDI input is not available on this host (no input port)".into(),
                })
            })?;
        let name = input.port_name(&chosen).unwrap_or_default();
        let (tx, rx) = sync_channel(MIDI_IN_QUEUE);
        let dropped = Arc::new(AtomicU32::new(0));
        let lost = Arc::clone(&dropped);
        let conn = input
            .connect(
                &chosen,
                "vactrol-in",
                move |_, bytes, ()| {
                    if let Some(ev) = parse_midi(bytes, clock.now()) {
                        if tx.try_send(ev).is_err() {
                            lost.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                },
                (),
            )
            .map_err(|e| {
                unavailable(format!(
                    "MIDI input port `{name}` is not available on this host ({e})"
                ))
            })?;
        Ok(Self {
            rx,
            buf: Vec::with_capacity(MIDI_IN_QUEUE),
            dropped,
            port: name,
            _conn: conn,
        })
    }

    #[must_use]
    pub fn port_name(&self) -> &str {
        &self.port
    }

    /// Events lost to a full queue.
    #[must_use]
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl MidiInHost for NativeMidiIn {
    fn poll(&mut self) -> &[MidiInEvent] {
        self.buf.clear();
        self.buf.extend(self.rx.try_iter());
        &self.buf
    }
}

enum Cmd {
    Event(MidiEvent),
    Control(SlotControl),
}

/// MIDI output to one midir port through a sender thread.
pub struct NativeMidiOut {
    tx: Option<Sender<Cmd>>,
    thread: Option<JoinHandle<()>>,
    port: String,
}

impl NativeMidiOut {
    /// Connects to `port` (or the first output port) and starts the sender
    /// thread, which times messages by `clock`.
    ///
    /// # Errors
    /// `beyond-capability` when MIDI output is unavailable or no port
    /// matches.
    pub fn open(port: Option<&str>, clock: FrameClock) -> Result<Self, Diagnostic> {
        let output = MidiOutput::new("vactrol")
            .map_err(|e| unavailable(format!("MIDI output is not available on this host ({e})")))?;
        let ports = output.ports();
        let chosen =
            pick(&ports, |p| output.port_name(p).unwrap_or_default(), port).ok_or_else(|| {
                unavailable(match port {
                    Some(p) => format!("MIDI output port `{p}` is not available on this host"),
                    None => "MIDI output is not available on this host (no output port)".into(),
                })
            })?;
        let name = output.port_name(&chosen).unwrap_or_default();
        let conn = output.connect(&chosen, "vactrol-out").map_err(|e| {
            unavailable(format!(
                "MIDI output port `{name}` is not available on this host ({e})"
            ))
        })?;
        let (tx, rx) = channel();
        let thread = thread::Builder::new()
            .name("vactrol-midi-out".into())
            .spawn(move || sender(conn, &rx, &clock))
            .map_err(|e| unavailable(format!("MIDI output is not available on this host ({e})")))?;
        Ok(Self {
            tx: Some(tx),
            thread: Some(thread),
            port: name,
        })
    }

    #[must_use]
    pub fn port_name(&self) -> &str {
        &self.port
    }

    fn cmd(&mut self, c: Cmd) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }
}

impl MidiHost for NativeMidiOut {
    fn send(&mut self, ev: MidiEvent) {
        self.cmd(Cmd::Event(ev));
    }

    fn control(&mut self, c: SlotControl) {
        self.cmd(Cmd::Control(c));
    }
}

impl Drop for NativeMidiOut {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The sender thread: applies commands, sends due messages, and on
/// shutdown sends the note offs of sounding notes.
fn sender(mut conn: MidiOutputConnection, rx: &Receiver<Cmd>, clock: &FrameClock) {
    let mut queue = MidiOutQueue::new();
    let mut due = Vec::new();
    loop {
        let now = clock.now();
        queue.take_due(now, &mut due);
        for m in due.drain(..) {
            let _ = conn.send(m.as_bytes());
        }
        let wait = queue.next_due().map_or(OUT_POLL, |t| {
            Duration::from_secs_f64((t - now).clamp(0.0, OUT_POLL.as_secs_f64()))
        });
        match rx.recv_timeout(wait) {
            Ok(Cmd::Event(ev)) => queue.schedule(ev),
            Ok(Cmd::Control(c)) => queue.control(c),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    queue.release_all(&mut due);
    for m in &due {
        let _ = conn.send(m.as_bytes());
    }
    conn.close();
}
