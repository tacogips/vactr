//! Commit: one staged occurrence becomes POD for its sink (design 11.4,
//! 12.8.7, 16.1).
//!
//! Seconds are computed here, once, through the `Clock`. The event's sound
//! picks its `Route`; controls map through the control table (`note`/`n`
//! through the scale to `freq`, `gain` to `amp`, keywords and bools encoded,
//! `orbit`/`cut` into the voice hint). A control whose value came from a var
//! or tweak becomes `Ctl::Cell` (or the pre-ack `Ctl::Const` downgrade).
//! MIDI and OSC bake values at transmission (the documented sink deviation).
//! Any failure is event-local: that event (and its held output) is dropped.

use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

use crate::clock::clock::Clock;
use crate::clock::tempo::Tempo;
use crate::dsp::caps::{Cap, CapabilitySet};
use crate::dsp::controls::{self, ControlRow, CtlRoute};
use crate::host::caps::{Hosts, InstResolver, MidiEvent, OscArg, OscEvent, Route, SampleSrc};
use crate::host::wire::{AudioEvent, Ctl, HostMsg};
use crate::ns::namespace::VarSlotRef;
use crate::pattern::combinators::music::note_number;
use crate::pattern::combinators::region::SpeedFit;
use crate::pattern::eval::num_f64;
use crate::pattern::query::Event;
use crate::reader::span::Span;
use crate::sched::cells::{CellKey, CellMap, ControlCells};
use crate::sched::runtime::{grid, Runtime, TickReport};
use crate::sched::slots::SlotId;
use crate::sched::slots::{Slot, SlotKind};
use crate::sched::telemetry::PlayingEvent;
use crate::types::diag::{Diagnostic, RunOrigin};
use crate::value::intern::{name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::value::value::{Sound, Value};
use crate::vm::fail::{FailCode, Failure, Origin};

/// A note number as a frequency (MIDI numbering, A4 = 69 = 440 Hz).
#[must_use]
pub fn note_to_freq(note: f64) -> f64 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

/// A note value: a number, or a note-name keyword (`:a4`, `:c`).
#[must_use]
pub fn note_of(v: &Value) -> Option<f64> {
    match v {
        Value::Keyword(k) => note_number(&name_of_kw(*k)).map(|n| n as f64),
        other => num_f64(other),
    }
}

/// A source value as a cell's `f32`.
#[must_use]
pub fn encode_value(row: &ControlRow, map: CellMap, v: &Value) -> Option<f32> {
    match map {
        CellMap::Direct => controls::encode(row, v).ok(),
        #[allow(clippy::cast_possible_truncation)]
        CellMap::NoteToFreq => note_of(v).map(|n| note_to_freq(n) as f32),
    }
}

/// The lifecycle of an installed sample (16.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SampleState {
    Loading,
    Installed,
    Retiring,
    /// The loader failed; reported once.
    Failed,
}

#[derive(Clone, Debug)]
struct SampleEntry {
    src: SampleSrc,
    id: u32,
    state: SampleState,
    reported: bool,
    /// The loaded sample's duration, seconds (0.0 when it failed to load).
    seconds: f64,
}

/// Sample resources: `SampleSrc -> resource id` (16.1 "the scheduler
/// references a resource only after that ack"). A captured or rendered
/// buffer (`SampleSrc::Buffer`) is installed from its own frames, not
/// through the loader, once it is `Ready` (14.5.9).
#[derive(Clone, Debug, Default)]
pub struct SampleTable {
    entries: Vec<SampleEntry>,
    next: u32,
    buffers: BTreeMap<u64, Weak<SampleBuf>>,
}

fn describe(src: &SampleSrc) -> String {
    match src {
        SampleSrc::Bank { kw, index } => format!(":{} {index}", name_of_kw(*kw)),
        SampleSrc::Path(p) => p.text.to_string(),
        SampleSrc::Buffer { id } => format!("buffer #{id}"),
    }
}

impl SampleTable {
    /// Remembers the buffer a `sound` value plays, so `request` can install
    /// its frames.
    pub fn note_value(&mut self, v: &Value) {
        if let Value::Sound(s) = v {
            if let Sound::Buffer(b) = &**s {
                self.buffers.insert(b.id, Rc::downgrade(b));
            }
        }
    }

    /// The state of a buffer source: `Ok` when it can be installed (or the
    /// source is not a buffer); a pending or failed buffer's failure.
    fn buffer_ready(&self, src: &SampleSrc) -> Result<(), Failure> {
        let SampleSrc::Buffer { id } = src else {
            return Ok(());
        };
        match self.buffers.get(id).and_then(Weak::upgrade) {
            Some(b) => b.ready_frames().map(|_| ()),
            None => Err(Failure::new(
                FailCode::HostUnavailable,
                format!("the sound buffer #{id} is gone"),
            )),
        }
    }

    /// Requests every source this table holds on `other` (an offline
    /// render's table, 14.5.9).
    pub fn preload(&self, other: &mut SampleTable, hosts: &mut Hosts) {
        other.buffers.clone_from(&self.buffers);
        for e in &self.entries {
            other.request(&e.src, hosts);
        }
    }

    /// Requests a sample once: loads it and installs it on the audio side.
    /// The entry is `Loading` until `HostMsg::Installed`. A buffer that is
    /// not `Ready` yet is not requested.
    pub fn request(&mut self, src: &SampleSrc, hosts: &mut Hosts) {
        if self.entries.iter().any(|e| &e.src == src) || self.buffer_ready(src).is_err() {
            return;
        }
        self.next += 1;
        let id = self.next;
        let loaded = match src {
            SampleSrc::Buffer { id } => self.buffers.get(id).and_then(Weak::upgrade).map_or_else(
                || Err(Failure::new(FailCode::HostUnavailable, "")),
                |b| b.to_sample_data(),
            ),
            _ => hosts.samples.load(src),
        };
        let (state, seconds) = match loaded {
            Ok(data) => {
                let channels = f64::from(data.channels.max(1));
                let rate = f64::from(data.rate.max(1));
                #[allow(clippy::cast_precision_loss)]
                let seconds = data.frames.len() as f64 / channels / rate;
                hosts.audio.install_sample(id, data);
                (SampleState::Loading, seconds)
            }
            Err(_) => (SampleState::Failed, 0.0),
        };
        self.entries.push(SampleEntry {
            src: src.clone(),
            id,
            state,
            reported: false,
            seconds,
        });
    }

    /// The id and state of a source.
    #[must_use]
    pub fn state(&self, src: &SampleSrc) -> Option<(u32, SampleState)> {
        self.entries
            .iter()
            .find(|e| &e.src == src)
            .map(|e| (e.id, e.state))
    }

    /// The duration of the sample installed under `id`, seconds (design
    /// 10.1's `speed-fit` resolution); `None` for an id this table never
    /// requested.
    #[must_use]
    pub fn seconds(&self, id: u32) -> Option<f64> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.seconds)
    }

    /// Applies `Installed` and `Retired`.
    pub fn on_msg(&mut self, msg: &HostMsg) {
        match *msg {
            HostMsg::Installed { resource, .. } => {
                for e in &mut self.entries {
                    if e.id == resource && e.state == SampleState::Loading {
                        e.state = SampleState::Installed;
                    }
                }
            }
            HostMsg::Retired { resource } => self.entries.retain(|e| e.id != resource),
            _ => {}
        }
    }

    /// The gate at commit: `Ok(Some(id))` when installed; otherwise the
    /// load is requested and the event dropped, reported ONCE per resource
    /// (`Err`) and silently after that (`Ok(None)`).
    fn gate(&mut self, src: &SampleSrc, hosts: &mut Hosts) -> Result<Option<u32>, Failure> {
        // A pending or failed buffer fails each of its events (14.5.9).
        self.buffer_ready(src)?;
        self.request(src, hosts);
        let Some(e) = self.entries.iter_mut().find(|e| &e.src == src) else {
            return Ok(None);
        };
        if e.state == SampleState::Installed {
            return Ok(Some(e.id));
        }
        if e.reported {
            return Ok(None);
        }
        e.reported = true;
        let what = describe(src);
        let message = match e.state {
            SampleState::Failed => format!("the sample `{what}` could not be loaded"),
            _ => format!("the sample `{what}` is still loading"),
        };
        Err(Failure::new(FailCode::HostUnavailable, message))
    }
}

/// What one commit produced.
#[derive(Clone, Debug)]
pub enum Committed {
    /// One audio event per note (a chord value gives several).
    Audio(Vec<AudioEvent>),
    Midi(Vec<MidiEvent>),
    Osc(OscEvent),
    /// Dropped without a new report (a sample still loading, already
    /// reported).
    Dropped,
}

/// The runtime state a commit reads and writes.
pub struct CommitCx<'a> {
    pub clock: &'a Clock,
    pub cells: &'a mut ControlCells,
    pub resolver: &'a dyn InstResolver,
    pub samples: &'a mut SampleTable,
    pub hosts: &'a mut Hosts,
    pub caps: &'a CapabilitySet,
    pub diags: &'a mut Vec<Diagnostic>,
}

/// The identity of the event being committed.
pub struct EventCx<'a> {
    pub slot: SlotId,
    pub slot_name: KwId,
    pub gen: u32,
    /// The binding form's span, for events without provenance (11.6).
    pub span: Option<Span>,
    /// `once` overrides (they replace the event's own control).
    pub overrides: &'a [(KwId, Value)],
}

/// One control value and the slot it was read from, if late.
type Controls = BTreeMap<KwId, (Value, Option<VarSlotRef>)>;

fn controls_of(ev: &Event, overrides: &[(KwId, Value)]) -> Controls {
    let mut out: Controls = ev
        .controls
        .iter()
        .map(|(k, v)| (*k, (v.clone(), ev.cells.get(k).cloned())))
        .collect();
    for (k, v) in overrides {
        out.insert(*k, (v.clone(), None));
    }
    out
}

/// The current value of a control (a late one is read NOW, 11.3).
fn current(entry: &(Value, Option<VarSlotRef>)) -> Value {
    match &entry.1 {
        Some(slot) => slot.get(),
        None => entry.0.clone(),
    }
}

fn number(controls: &Controls, name: &str) -> Option<f64> {
    controls
        .iter()
        .find(|(k, _)| &*name_of_kw(**k) == name)
        .and_then(|(_, e)| num_f64(&current(e)))
}

fn entry<'c>(controls: &'c Controls, name: &str) -> Option<&'c (Value, Option<VarSlotRef>)> {
    controls
        .iter()
        .find(|(k, _)| &*name_of_kw(**k) == name)
        .map(|(_, e)| e)
}

fn sound_of(v: &Value) -> Result<Sound, Failure> {
    match v {
        Value::Sound(s) => Ok((**s).clone()),
        Value::Inst(id) => Ok(Sound::Inst(*id)),
        _ => Err(Failure::new(
            FailCode::UnknownSound,
            "an event plays no sound: a played pattern starts with `s`",
        )),
    }
}

/// Converts one event (design 11.4). `Err` is an event-local failure with
/// the event's origin; the caller drops the event and its held output.
///
/// # Errors
/// `unknown-sound`, `type` (a control outside its domain),
/// `too-many-controls`, `host-unavailable` (a sample still loading).
pub fn commit(ev: &Event, cx: &mut CommitCx<'_>, ecx: &EventCx<'_>) -> Result<Committed, Failure> {
    commit_inner(ev, cx, ecx).map_err(|mut f| {
        let o = origin(ev, cx.clock, ecx);
        f.origin.span = f.origin.span.or(o.span);
        f.origin.slot = f.origin.slot.or(o.slot);
        f.origin.beat = f.origin.beat.or(o.beat);
        f
    })
}

/// The event's origin: its span (or the binding's), slot and beat.
#[must_use]
pub fn origin(ev: &Event, clock: &Clock, ecx: &EventCx<'_>) -> Origin {
    Origin {
        span: ev.src.map(|s| s.span).or(ecx.span),
        slot: Some(ecx.slot_name),
        beat: clock.tempo().beats_at(ev.anchor()).ok(),
    }
}

fn commit_inner(
    ev: &Event,
    cx: &mut CommitCx<'_>,
    ecx: &EventCx<'_>,
) -> Result<Committed, Failure> {
    let whole = ev.whole.unwrap_or(ev.part);
    let time = cx.clock.to_host(whole.begin);
    let dur = (cx.clock.to_host(whole.end) - time).max(0.0);
    let sound = sound_of(&ev.value)?;
    cx.samples.note_value(&ev.value);
    let route = cx.resolver.route(&sound)?;
    let controls = controls_of(ev, ecx.overrides);
    match route {
        Route::Audio { inst, sample } => {
            let bank = match sample {
                None => None,
                Some(src) => {
                    let src = match src {
                        SampleSrc::Bank { kw, .. } => {
                            // R2c: `bank`/`table`/`source` is an ordinary
                            // pattern control (`s :sampler > bank :sn-dub`);
                            // a keyword-valued one overrides the route's
                            // default keyword.
                            let kw = ["bank", "table", "source"]
                                .into_iter()
                                .find_map(|name| entry(&controls, name).map(current))
                                .and_then(|v| match v {
                                    Value::Keyword(k) => Some(k),
                                    _ => None,
                                })
                                .unwrap_or(kw);
                            let n = number(&controls, "n").unwrap_or(0.0);
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            let index = if n > 0.0 { n as u32 } else { 0 };
                            SampleSrc::Bank { kw, index }
                        }
                        other => other,
                    };
                    match cx.samples.gate(&src, cx.hosts)? {
                        Some(id) => Some(id),
                        None => return Ok(Committed::Dropped),
                    }
                }
            };
            // The installed sample's duration, for a `speed-fit` marker
            // (`splice`/`loop-at`/`fit`, design 10.1) to resolve against.
            let secs = bank.and_then(|id| cx.samples.seconds(id));
            admit_grains(ev, &controls, inst, cx, ecx);
            audio_events(time, inst, bank, secs, &controls, cx, ecx).map(Committed::Audio)
        }
        Route::Midi { ch } => Ok(Committed::Midi(midi_events(time, dur, ch, &controls, ecx))),
        Route::Osc { addr } => Ok(Committed::Osc(OscEvent {
            time,
            slot: ecx.slot,
            gen: ecx.gen,
            addr,
            args: osc_args(&controls),
        })),
    }
}

/// The notes of an event: its `note` (or, off a bank, `n`) control; a
/// chord gives one entry per tone. `None` when the event names no note.
fn notes(controls: &Controls, bank: bool) -> Option<(Vec<f64>, Option<VarSlotRef>)> {
    let e = entry(controls, "note").or_else(|| (!bank).then(|| entry(controls, "n")).flatten())?;
    match current(e) {
        Value::List(l) => Some((l.items.iter().filter_map(note_of).collect(), None)),
        v => note_of(&v).map(|n| (vec![n], e.1.clone())),
    }
}

fn audio_events(
    time: f64,
    inst: crate::dsp::graph::InstId,
    bank: Option<u32>,
    secs: Option<f64>,
    controls: &Controls,
    cx: &mut CommitCx<'_>,
    ecx: &EventCx<'_>,
) -> Result<Vec<AudioEvent>, Failure> {
    let mut base = AudioEvent::new(time, ecx.slot, ecx.gen, inst);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        let orbit = number(controls, "orbit").unwrap_or(0.0).clamp(0.0, 255.0) as u32;
        let cut = number(controls, "cut").unwrap_or(0.0).clamp(0.0, 255.0) as u32;
        base.voice_hint = orbit | (cut << 8);
    }
    if let (Some(id), Some(row)) = (bank, controls::row("bank")) {
        #[allow(clippy::cast_precision_loss)]
        base.push_ctl(row.ctl, Ctl::Const(id as f32))?;
    }
    // R3: `bus` is a `CtlRoute::Scheduler` row (skipped by the generic loop
    // below), resolved here instead, the same way `bank` is. An unknown
    // bus keyword pushes nothing, so the engine routes the event to
    // `master` (`dsp::bus::BusGraph::route`'s fallback).
    if let (Some(Value::Keyword(kw)), Some(row)) =
        (entry(controls, "bus").map(current), controls::row("bus"))
    {
        if let Some(id) = cx.resolver.bus(kw) {
            #[allow(clippy::cast_precision_loss)]
            base.push_ctl(row.ctl, Ctl::Const(id.get() as f32))?;
        }
    }
    // `speed-fit` (design 10.1: `splice`/`loop-at`/`fit`) is a marker
    // control with no control-table row of its own (silently ignored by
    // the generic loop below); resolved here into the `speed` control,
    // against the installed sample's own duration and the committed cycle
    // length. No resolution (the sample's duration is not yet known, or
    // there is no marker) leaves `speed` at its ordinary default/control.
    if let (Some(secs), Some(row)) = (secs, controls::row("speed")) {
        if let Some(fit) = entry(controls, "speed-fit")
            .map(current)
            .and_then(|v| SpeedFit::from_value(&v))
        {
            let cycle_secs = cx.clock.tempo().cycle_seconds().unwrap_or(1.0);
            #[allow(clippy::cast_possible_truncation)]
            let speed = fit.resolve(secs, cycle_secs) as f32;
            base.push_ctl(row.ctl, Ctl::Const(speed))?;
        }
    }
    for (k, e) in controls {
        let name = name_of_kw(*k);
        let row = controls::row(&name);
        if row.is_none() {
            if &*name == "speed-fit" {
                continue; // consumed above by sample speed resolution
            }
            let param = cx.resolver.declared_param(inst, *k).ok_or_else(|| {
                Failure::new(
                    FailCode::Type,
                    format!("unknown instrument control `{name}`"),
                )
            })?;
            let value = match &e.1 {
                Some(slot) => {
                    let v = param.encode(&slot.get())?;
                    cx.cells.ctl_for(
                        CellKey::Site {
                            slot: slot.id(),
                            ctl: param.ctl,
                        },
                        None,
                        CellMap::Direct,
                        v,
                        cx.diags,
                    )
                }
                None => Ctl::Const(param.encode(&e.0)?),
            };
            base.push_ctl(param.ctl, value)?;
            continue;
        }
        let Some(row) = row else {
            unreachable!("custom controls were handled above")
        };
        if row.route == CtlRoute::Scheduler
            || matches!(row.name, "note" | "n" | "bank" | "table" | "source")
        {
            continue;
        }
        if let Some(param) = cx.resolver.declared_param(inst, *k) {
            if matches!(
                row.domain,
                crate::dsp::controls::CtlDomain::Float | crate::dsp::controls::CtlDomain::Bool
            ) && param.ty != crate::dsp::controls::ScalarType::Unsupported
            {
                param.encode(&current(e))?;
            }
        }
        let ctl = match &e.1 {
            Some(slot) => {
                let v = controls::encode(row, &slot.get())?;
                let key = CellKey::Site {
                    slot: slot.id(),
                    ctl: row.ctl,
                };
                cx.cells
                    .ctl_for(key, Some(row), CellMap::Direct, v, cx.diags)
            }
            None => Ctl::Const(controls::encode(row, &e.0)?),
        };
        base.push_ctl(row.ctl, ctl)?;
    }
    let has_freq = entry(controls, "freq").is_some();
    let Some((notes, late)) = notes(controls, bank.is_some()).filter(|_| !has_freq) else {
        return Ok(vec![base]);
    };
    let freq = controls::row("freq").map_or(crate::sched::slots::CtlId::new(0), |r| r.ctl);
    let mut out = Vec::with_capacity(notes.len());
    for n in notes {
        let mut e = base;
        #[allow(clippy::cast_possible_truncation)]
        let hz = note_to_freq(n) as f32;
        let ctl = match (&late, controls::row("freq")) {
            (Some(slot), Some(row)) => cx.cells.ctl_for(
                CellKey::Site {
                    slot: slot.id(),
                    ctl: row.ctl,
                },
                Some(row),
                CellMap::NoteToFreq,
                hz,
                cx.diags,
            ),
            _ => Ctl::Const(hz),
        };
        e.push_ctl(freq, ctl)?;
        out.push(e);
    }
    Ok(out)
}

/// Granular admission (12.6): a density, grain size or capture depth
/// beyond the tier cap is a `beyond-capability` diagnostic with the event's
/// origin; the event still commits and the audio side clamps spawning.
/// Capture depth applies to live sources (a `source` control): the larger
/// of `size` and `position`, in seconds back from the write head.
fn admit_grains(
    ev: &Event,
    controls: &Controls,
    inst: crate::dsp::graph::InstId,
    cx: &mut CommitCx<'_>,
    ecx: &EventCx<'_>,
) {
    // A control the instrument declares goes to it first (12.8.7): only a
    // granular instrument (one that declares `density`) takes grain
    // controls; with no known definition, a `density` control marks it.
    let granular = match (cx.resolver.inst(inst), controls::row("density")) {
        (Some(d), Some(row)) => d.params.iter().any(|(c, _)| *c == row.ctl),
        (None, _) => entry(controls, "density").is_some(),
        (Some(_), None) => false,
    };
    if !granular {
        return;
    }
    let o = origin(ev, cx.clock, ecx);
    let mut checks = Vec::new();
    #[allow(clippy::cast_possible_truncation)]
    {
        if let Some(d) = number(controls, "density") {
            checks.push(Cap::GrainDensity(d as f32));
        }
        let size = number(controls, "size");
        if let Some(s) = size {
            checks.push(Cap::GrainSize(s as f32));
        }
        if entry(controls, "source").is_some() {
            let depth = size
                .unwrap_or(0.0)
                .max(number(controls, "position").unwrap_or(0.0));
            checks.push(Cap::CaptureSeconds(depth as f32));
        }
    }
    for c in checks {
        if let Err(mut d) = cx.caps.require(c, o.span) {
            d.origin = Some(RunOrigin {
                slot: o.slot,
                beat: o.beat,
            });
            cx.diags.push(d);
        }
    }
}

fn midi_events(
    time: f64,
    dur: f64,
    ch: u8,
    controls: &Controls,
    ecx: &EventCx<'_>,
) -> Vec<MidiEvent> {
    let notes = notes(controls, false).map_or(vec![60.0], |(n, _)| n);
    let vel = number(controls, "velocity")
        .or_else(|| number(controls, "gain").map(|g| g.min(1.0)))
        .unwrap_or(100.0 / 127.0);
    let legato = number(controls, "legato").unwrap_or(1.0).max(0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    notes
        .into_iter()
        .map(|n| MidiEvent::Note {
            time,
            slot: ecx.slot,
            gen: ecx.gen,
            ch,
            note: n.round().clamp(0.0, 127.0) as u8,
            vel: (vel * 127.0).round().clamp(0.0, 127.0) as u8,
            dur: dur * legato,
        })
        .collect()
}

fn osc_args(controls: &Controls) -> Vec<OscArg> {
    let mut named: Vec<(String, Value)> = controls
        .iter()
        .map(|(k, e)| (name_of_kw(*k).to_string(), current(e)))
        .collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = Vec::with_capacity(named.len() * 2);
    for (name, v) in named {
        let arg = match &v {
            Value::Int(i) => OscArg::I(*i),
            Value::Str(s) => OscArg::S(s.clone()),
            Value::Keyword(k) => OscArg::S(name_of_kw(*k)),
            other => match num_f64(other) {
                #[allow(clippy::cast_possible_truncation)]
                Some(x) => OscArg::F(x as f32),
                None => continue,
            },
        };
        out.push(OscArg::S(name.into()));
        out.push(arg);
    }
    out
}

impl Runtime {
    pub(crate) fn commit_all(&mut self, now: f64, rep: &mut TickReport) {
        let horizon = now + self.commit_lead;
        let horizon_pos = grid(self.clock.to_cycles(horizon), false);
        let mut out: Vec<(f64, Committed)> = Vec::new();
        let mut diags = Vec::new();
        let tempo = self.clock.tempo();
        let prev = self.last_now;
        let lead = self.commit_lead;
        let mut late = 0;
        for slot in self.slots.iter_mut() {
            let (id, name, muted) = (slot.id, slot.name(), slot.muted);
            let mut faults = Vec::new();
            for lane in &mut slot.lanes {
                for key in lane.staging.emittable(lane.from, lane.until) {
                    let Some(rec) = lane.staging.get(&key) else {
                        continue;
                    };
                    let onset = self.clock.to_host(rec.whole.begin);
                    if onset > horizon {
                        break;
                    }
                    let ev = rec.payload.clone();
                    let whole = rec.whole;
                    lane.ledger.insert(key.clone(), whole);
                    if muted {
                        lane.staging.mark_committed(&key);
                        continue;
                    }
                    let ecx = EventCx {
                        slot: id,
                        slot_name: name,
                        gen: lane.gen,
                        span: lane.pat.span,
                        overrides: &lane.overrides,
                    };
                    let mut cx = CommitCx {
                        clock: &self.clock,
                        cells: &mut self.cells,
                        resolver: self.resolver.as_ref(),
                        samples: &mut self.samples,
                        hosts: &mut self.hosts,
                        caps: &self.caps,
                        diags: &mut diags,
                    };
                    match commit(&ev, &mut cx, &ecx) {
                        Ok(c) => {
                            rep.console.extend(lane.staging.mark_committed(&key));
                            if matches!(c, Committed::Dropped) {
                                continue;
                            }
                            if onset < now {
                                late += 1;
                            }
                            let pe = PlayingEvent {
                                slot: name,
                                beat: tempo.beats_at(whole.begin).unwrap_or(Ratio64::ZERO),
                                time: onset,
                                src: ev.src,
                                dur: (self.clock.to_host(whole.end) - onset).max(0.0),
                                kind: SlotKind::Pattern,
                                reduced_lead: prev.is_some_and(|p| onset <= p + lead),
                            };
                            let ctls: Vec<(KwId, Value)> =
                                ev.controls.iter().map(|(k, v)| (*k, v.clone())).collect();
                            self.telemetry.publish(pe, &ctls);
                            out.push((onset, c));
                        }
                        Err(f) => {
                            lane.staging.drop_failed(&key);
                            faults.push(f);
                        }
                    }
                }
                rep.console.extend(lane.staging.take_keyless(horizon_pos));
            }
            for f in faults {
                note_fault(slot, &f, tempo);
                rep.faults.push(f);
            }
        }
        rep.diags.extend(diags);
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, c) in out {
            rep.committed += self.send(c);
        }
        if late > 0 {
            self.widen(now, late, &mut rep.diags);
        }
    }

    /// Hands one commit to its sink; returns the events sent.
    pub(crate) fn send(&mut self, c: Committed) -> usize {
        match c {
            Committed::Audio(evs) => {
                let n = evs.len();
                for e in evs {
                    self.hosts.audio.send(e);
                }
                n
            }
            Committed::Midi(evs) => {
                let n = evs.len();
                for e in evs {
                    self.hosts.midi.send(e);
                    self.recent_midi.push(e);
                }
                n
            }
            Committed::Osc(e) => {
                self.hosts.osc.send(e);
                1
            }
            Committed::Dropped => 0,
        }
    }
}

/// Records a fault's cycle for the clean-cycle rule.
pub(crate) fn note_fault(slot: &mut Slot, f: &Failure, tempo: Tempo) {
    slot.has_faults = true;
    let cycle = f
        .origin
        .beat
        .and_then(|b| b.checked_div(tempo.beats_per_cycle).ok())
        .map_or(0, Ratio64::floor);
    slot.fault_cycles.push(cycle);
}
