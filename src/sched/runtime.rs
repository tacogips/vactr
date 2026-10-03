//! The runtime: slot table, two-horizon scheduler, control channel, cells,
//! telemetry and the host bundle (design 11.3, 12.8.1, 12.8.3).
//!
//! `Runtime::new` returns the runtime and a `RuntimeSink`; the caller builds
//! the `Evaluator` with the sink and calls `drain` after every evaluation
//! (binds are dry-run there) and `tick` on every host tick. A tick runs the
//! six steps of 11.3: advance the position, activate pending bindings,
//! query newly uncovered (and invalidated) spans split at cycle boundaries,
//! commit what enters the commit horizon, forward faults and telemetry,
//! then run due `at` thunks. Logical time is `Ratio64`; host seconds appear
//! only at the host boundary and in commit.

mod song;
pub use song::SongNotice;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::clock::clock::{Clock, ClockSource};
use crate::clock::tempo::Tempo;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::AtomicCells;
use crate::host::caps::{GraphHandle, Hosts, InstResolver, Route, SampleSrc};
use crate::host::wire::{Ctl, HostMsg};
use crate::ns::evaluator::Evaluator;
use crate::ns::stage::{EffectSink, SlotKey, StagedEffect};
use crate::pattern::eval::{HostSig, InputCells, QueryCtx};
use crate::pattern::query::{Event, TimeSpan};
use crate::reader::span::{FileId, Span};
use crate::sched::cells::{ControlCells, Tier};
use crate::sched::commit::{note_fault, SampleTable};
use crate::sched::control::ControlChannel;
use crate::sched::dryrun::dry_run;
use crate::sched::oneshot::{run_thunk, AtQueue};
use crate::sched::slots::{Binding, SlotTable};
use crate::sched::staging::query_lane;
use crate::sched::telemetry::{PlayingEvent, Telemetry};
use crate::tex::shader::{compile_tex, ShaderDesc};
use crate::tex::uniforms::UniformPlan;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::{intern_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure, Origin};
use crate::vm::query_vm::VmQuery;

/// Logical positions derived from host time are quantized to this grid.
const GRID: i64 = 960;

/// Scheduler settings (design 12.8.4).
#[derive(Debug)]
pub struct RuntimeConfig {
    /// Query horizon, seconds.
    pub lookahead: f64,
    /// Commit horizon, seconds.
    pub commit_lead: f64,
    pub resend_ticks: u32,
    pub transport_diag_ticks: u32,
    /// Late events within one second that widen the commit lead.
    pub late_threshold: u32,
    pub widen_step: f64,
    pub widen_cap: f64,
    pub cell_pool: usize,
    pub tier: Tier,
    /// The session seed of the pure hash RNG.
    pub seed: u64,
    /// MIDI clock loss timeout, seconds (11.7).
    pub midi_clock_timeout: f64,
    /// Smoothing factor of the MIDI clock pulse period (11.7).
    pub clock_smoothing: f64,
    /// The audio host's sample rate, Hz: the length of a capture (14.5.9).
    pub sample_rate: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            lookahead: 0.120,
            commit_lead: 0.030,
            resend_ticks: 3,
            transport_diag_ticks: 20,
            late_threshold: 4,
            widen_step: 0.010,
            widen_cap: 0.200,
            cell_pool: 1024,
            tier: Tier::Native(AtomicCells::new(1024)),
            seed: 0,
            midi_clock_timeout: 0.5,
            clock_smoothing: 0.1,
            sample_rate: 48_000,
        }
    }
}

/// Released effects waiting for `drain`.
#[derive(Debug, Default)]
pub struct CommandQueue {
    items: VecDeque<StagedEffect>,
}

/// The `EffectSink` the evaluator releases into.
#[derive(Debug, Clone)]
pub struct RuntimeSink(Rc<RefCell<CommandQueue>>);

impl EffectSink for RuntimeSink {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().items.push_back(effect);
    }
}

/// What `drain` did.
#[derive(Debug, Default)]
pub struct DrainReport {
    pub diags: Vec<Diagnostic>,
    /// Dry-run and bind failures (the old binding keeps playing).
    pub faults: Vec<Failure>,
    /// Captured `print` output of dry runs: the report only, never the
    /// console (10.4).
    pub dry_output: Vec<(Origin, Rc<str>)>,
    /// `print` lines of evaluated forms, forwarded as they are.
    pub console: Vec<Rc<str>>,
}

/// What one tick did.
#[derive(Debug, Default)]
pub struct TickReport {
    pub diags: Vec<Diagnostic>,
    /// Captured query output forwarded at its commit point, and console
    /// lines of `at` thunks.
    pub console: Vec<Rc<str>>,
    /// Query and commit faults, each with slot and beat.
    pub faults: Vec<Failure>,
    /// Slots whose runtime diagnostics cleared after a clean cycle.
    pub cleared: Vec<SlotKey>,
    /// Events handed to sinks.
    pub committed: usize,
}

/// The scheduler and its hosts.
pub struct Runtime {
    pub(crate) cfg: RuntimeConfig,
    pub(crate) hosts: Hosts,
    pub(crate) resolver: Rc<dyn InstResolver>,
    pub(crate) caps: CapabilitySet,
    queue: Rc<RefCell<CommandQueue>>,
    /// Retained immutable request; playback admission is added by SONG-11.
    pub(crate) pending_song: Option<Rc<crate::song::Song>>,
    song_receipts: crate::song::routing::SongReceipts,
    pending_song_ack: Option<crate::song::routing::SongHostAck>,
    song: song::SongRuntime,
    pub(crate) slots: SlotTable,
    pub(crate) clock: Clock,
    pub(crate) cells: ControlCells,
    pub(crate) input: InputCells,
    pub(crate) control: ControlChannel,
    pub(crate) samples: SampleTable,
    pub(crate) telemetry: Telemetry,
    pub(crate) at: AtQueue,
    pub(crate) pos: Ratio64,
    pub(crate) last_now: Option<f64>,
    pub(crate) commit_lead: f64,
    late: VecDeque<f64>,
    /// `use-clock` and `midi-clock-out`, stored for BE-MIDI (inert here).
    pub(crate) clock_request: Option<KwId>,
    pub(crate) midi_clock_out: bool,
    pub(crate) once_seq: u32,
    /// Committed MIDI notes still possibly sounding, for the stale-start
    /// note-off (11.3 "MidiHost sends an immediate note-off").
    pub(crate) recent_midi: Vec<crate::host::caps::MidiEvent>,
    /// MIDI input and note lifetime (`sched/midi_in.rs`, BE-MIDI).
    pub(crate) midi_in: crate::sched::midi_in::MidiIn,
    /// Clock slave, transport and clock master (`sched/midi_clock.rs`).
    pub(crate) midi_clock: crate::sched::midi_clock::MidiClockState,
    /// Armed captures (`sched/tap.rs`, 14.5.9).
    pub(crate) captures: Vec<crate::sched::tap::PendingCapture>,
    /// Step (6) runs `at` thunks; an offline render turns it off.
    pub(crate) run_thunks: bool,
    /// The evaluator-owned uniform plan `compile_tex` returned for each
    /// output `o0`..`o3` (design 9.2-9.3, G5), kept here since `activate`
    /// discarded it before. `render.rs`'s `render_frame` resolves it every
    /// frame and clears the entry once the output's slot is no longer
    /// bound.
    pub(crate) uniform_plans: [Option<UniformPlan>; 4],
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

/// A host-derived position on the grid (`up` rounds up).
pub(crate) fn grid(cycles: f64, up: bool) -> Ratio64 {
    if !cycles.is_finite() {
        return Ratio64::ZERO;
    }
    let x = cycles * GRID as f64;
    let n = if up { x.ceil() } else { x.floor() };
    let n = n.clamp(-9.0e15, 9.0e15);
    #[allow(clippy::cast_possible_truncation)]
    Ratio64::new(n as i64, GRID).unwrap_or(Ratio64::ZERO)
}

pub(crate) fn ceil(r: Ratio64) -> Ratio64 {
    if r.is_integral() {
        r
    } else {
        Ratio64::from_int(r.floor().saturating_add(1))
    }
}

pub(crate) fn nowhere() -> Span {
    Span::new(FileId::new(0), 0, 0)
}

impl Runtime {
    /// A runtime over `hosts` at 120 bpm, cycle 0 at host time 0.
    #[must_use]
    pub fn new(
        hosts: Hosts,
        resolver: Rc<dyn InstResolver>,
        caps: CapabilitySet,
        mut cfg: RuntimeConfig,
    ) -> (Runtime, RuntimeSink) {
        let queue = Rc::new(RefCell::new(CommandQueue::default()));
        let tier = std::mem::replace(&mut cfg.tier, Tier::Native(AtomicCells::new(0)));
        let cells = ControlCells::new(
            tier,
            cfg.cell_pool,
            cfg.resend_ticks,
            cfg.transport_diag_ticks,
        );
        // The default tempo's cps is exactly 1/2, so this cannot fail on any
        // input.
        let clock = Clock::new(ClockSource::Internal, Tempo::default(), 0.0)
            .expect("the default tempo has a finite cps");
        let rt = Runtime {
            control: ControlChannel::new(cfg.resend_ticks, cfg.transport_diag_ticks),
            commit_lead: cfg.commit_lead,
            cfg,
            hosts,
            resolver,
            caps,
            queue: Rc::clone(&queue),
            pending_song: None,
            song_receipts: crate::song::routing::SongReceipts::default(),
            pending_song_ack: None,
            song: song::SongRuntime::default(),
            slots: SlotTable::new(),
            clock,
            cells,
            input: InputCells::new(),
            samples: SampleTable::default(),
            telemetry: Telemetry::default(),
            at: AtQueue::default(),
            pos: Ratio64::ZERO,
            last_now: None,
            late: VecDeque::new(),
            clock_request: None,
            midi_clock_out: false,
            once_seq: 0,
            recent_midi: Vec::new(),
            midi_in: crate::sched::midi_in::MidiIn::default(),
            midi_clock: crate::sched::midi_clock::MidiClockState::default(),
            captures: Vec::new(),
            run_thunks: true,
            uniform_plans: [None, None, None, None],
        };
        (rt, RuntimeSink(queue))
    }

    /// Latest explicit immutable request; it has not activated playback.
    #[must_use]
    pub fn pending_song(&self) -> Option<&Rc<crate::song::Song>> {
        self.pending_song.as_ref()
    }

    /// The input cells signals read (cc, analysis, telemetry).
    pub fn input_cells(&mut self) -> &mut InputCells {
        &mut self.input
    }

    /// The generation of a slot.
    #[must_use]
    pub fn slot_gen(&self, key: SlotKey) -> Option<u32> {
        self.slots.get(key).map(|s| s.gen)
    }

    /// The slot table (read-only).
    #[must_use]
    pub fn slots(&self) -> &SlotTable {
        &self.slots
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// The authoritative cells.
    #[must_use]
    pub fn cells(&self) -> &ControlCells {
        &self.cells
    }

    /// The sample table.
    #[must_use]
    pub fn samples(&self) -> &SampleTable {
        &self.samples
    }

    /// The current commit lead (widened after late events).
    #[must_use]
    pub fn commit_lead(&self) -> f64 {
        self.commit_lead
    }

    /// Drains the telemetry queue.
    pub fn telemetry(&mut self) -> Vec<PlayingEvent> {
        self.telemetry.drain()
    }

    /// Re-synchronizes the browser cell mirror after a reconnect.
    pub fn resync_cells(&mut self) {
        self.cells.resync();
    }

    /// Scoped semantic invalidation of `span` in every lane of `slot`
    /// (re-queried at the next tick).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn invalidate(&mut self, slot: SlotKey, span: TimeSpan) {
        if let Some(s) = self.slots.get_mut(slot) {
            for lane in &mut s.lanes {
                lane.invalidate(span);
            }
        }
    }

    /// The host time and grid position now (drain side).
    pub(crate) fn now_pos(&self) -> (f64, Ratio64) {
        let now = self.hosts.audio.now();
        (now, grid(self.clock.to_cycles(now), false).max(self.pos))
    }

    /// Applies every released effect (design 12.8.3 "Effect intake").
    pub fn drain(&mut self, ev: &mut Evaluator) -> DrainReport {
        let mut rep = DrainReport::default();
        loop {
            let next = self.queue.borrow_mut().items.pop_front();
            let Some(cmd) = next else { break };
            self.apply(ev, cmd, &mut rep);
        }
        rep
    }

    pub(crate) fn apply(&mut self, ev: &mut Evaluator, cmd: StagedEffect, rep: &mut DrainReport) {
        match cmd {
            StagedEffect::PlaySong(song) => {
                self.pending_song = Some(song);
                rep.faults.push(Failure::new(
                    crate::vm::fail::FailCode::BeyondCapability,
                    "finite song playback is unavailable until song transport is installed",
                ));
            }
            StagedEffect::SlotBind { slot, value } => self.bind(ev, slot, &value, rep),
            StagedEffect::Revoke(key) => {
                self.revoke(key);
                let now = self.hosts.audio.now();
                self.close_live_notes(key, now);
            }
            StagedEffect::CellUpdate { slot, .. } => {
                self.cells.write_slot(&slot);
                self.invalidate_uncommitted();
            }
            StagedEffect::TweakRefresh(id) => {
                let site = ev.ns().tweaks().borrow().get(id).cloned();
                if let Some(site) = site {
                    self.cells.write_slot(&site.slot);
                    if let Some(registry) = ev.insts() {
                        for entry in registry.borrow().entries() {
                            for (cell, slot) in &entry.cells {
                                if slot.id() == site.slot.id() {
                                    if let Some(value) = entry.default_cell_value(*cell) {
                                        self.cells.write_external(*cell, value);
                                    }
                                }
                            }
                        }
                    }
                }
                self.invalidate_uncommitted();
            }
            StagedEffect::Bindings(items) => {
                for (name, _) in items {
                    if let Some(s) = ev.ns().session_slot(name) {
                        self.cells.write_slot(&s);
                    }
                }
                self.invalidate_uncommitted();
            }
            StagedEffect::Tempo(t) => {
                self.tempo(t, &mut rep.diags, &mut rep.faults);
                self.apply_clock_settings();
            }
            StagedEffect::OneShot {
                at,
                value,
                overrides,
            } => self.one_shot(at, value, overrides, &mut rep.faults),
            StagedEffect::Console(s) => rep.console.push(s),
            StagedEffect::Install(g) => {
                // Cell-backed defaults are initialized with the install.
                if let GraphHandle::Inst { def, .. } = &g {
                    let registry = ev.insts();
                    let registry = registry.as_ref().map(|r| r.borrow());
                    let entry = registry.as_ref().and_then(|r| r.entry(def.id));
                    for (ctl, c) in def.params.iter() {
                        if let Ctl::Cell(cell) = c {
                            let v = entry
                                .filter(|entry| entry.def.as_ref() == def.as_ref())
                                .and_then(|entry| entry.default_cell_value(*cell))
                                .unwrap_or_else(|| {
                                    crate::dsp::controls::row_by_id(*ctl).map_or(0.0, |r| r.default)
                                });
                            self.cells.ensure_external(*cell, v);
                        }
                    }
                }
                self.hosts.audio.swap_graph(g);
            }
            StagedEffect::Capture {
                buf,
                src,
                cycles,
                origin,
            } => self.capture(buf, src, cycles, origin, &mut rep.faults),
            StagedEffect::Render {
                buf,
                cycles,
                origin,
            } => self.render_effect(ev, &buf, cycles, origin, &mut rep.faults),
        }
    }

    /// A control write: every uncommitted span is re-queried (structure
    /// and values of staged events follow the new read snapshot, 11.3).
    fn invalidate_uncommitted(&mut self) {
        let (_, pos) = self.now_pos();
        for s in self.slots.iter_mut() {
            for lane in &mut s.lanes {
                let span = TimeSpan {
                    begin: pos,
                    end: lane.queried_to.max(pos),
                };
                lane.invalidate(span);
            }
        }
    }

    /// A slot bind: dry run, then pending at the next boundary (11.2, 11.5).
    fn bind(&mut self, ev: &mut Evaluator, key: SlotKey, value: &Value, rep: &mut DrainReport) {
        if self.owns_song_resources() {
            rep.faults.push(Failure::new(
                crate::vm::fail::FailCode::BeyondCapability,
                "legacy slot binding is unavailable while a finite song owns resources",
            ));
            return;
        }
        let name = intern_kw(&key.name());
        let binding = match Binding::from_value(value) {
            Ok(b) => b,
            Err(mut f) => {
                f.origin.slot = Some(name);
                rep.faults.push(f);
                return;
            }
        };
        let r = {
            let (vm, ns) = ev.vm_and_ns();
            let mut h = VmQuery::new(vm, ns);
            dry_run(&binding, &mut h, &self.input, self.cfg.seed)
        };
        rep.dry_output.extend(r.output);
        if !r.faults.is_empty() {
            rep.faults.extend(r.faults.into_iter().map(|mut f| {
                f.origin.slot = f.origin.slot.or(Some(name));
                f
            }));
            return;
        }
        for e in &r.events {
            self.samples.note_value(&e.value);
            if let Some(src) = self.sample_of(e) {
                self.samples.request(&src, &mut self.hosts);
            }
        }
        if let Err(mut f) = self.rebind(key, binding) {
            f.origin.slot = Some(name);
            rep.faults.push(f);
        }
    }

    /// The sample a dry-run event will need, if any.
    fn sample_of(&self, e: &Event) -> Option<SampleSrc> {
        let sound = match &e.value {
            Value::Sound(s) => (**s).clone(),
            _ => return None,
        };
        match self.resolver.route(&sound).ok()? {
            Route::Audio {
                sample: Some(SampleSrc::Bank { kw, .. }),
                ..
            } => {
                let n = e
                    .controls
                    .get(&intern_kw("n"))
                    .and_then(crate::pattern::eval::int_of)
                    .unwrap_or(0);
                Some(SampleSrc::Bank {
                    kw,
                    index: u32::try_from(n).unwrap_or(0),
                })
            }
            Route::Audio { sample, .. } => sample,
            _ => None,
        }
    }

    /// One host tick (design 11.3 steps (1)-(6)).
    pub fn tick(&mut self, ev: &mut Evaluator, host_now: f64) -> TickReport {
        let mut rep = TickReport::default();
        self.take_host_msgs(&mut rep);
        self.tick_song(&mut rep);
        self.poll_captures(&mut rep.faults);
        // MIDI input: cc cells, clock slave pulses and transport (11.7).
        self.take_midi_in(host_now, &mut rep);
        if self.transport_frozen() {
            return self.frozen_tick(ev, host_now, rep);
        }
        // (1) advance.
        self.pos = grid(self.clock.to_cycles(host_now), false).max(self.pos);
        self.clock.set_pos(self.pos);
        // (2) activate pending bindings whose boundary is crossed.
        self.activate();
        self.play_live_notes(ev, host_now, &mut rep);
        // (3) query newly uncovered and invalidated spans.
        let target = grid(self.clock.to_cycles(host_now + self.cfg.lookahead), true);
        self.query_all(ev, target, &mut rep);
        // (4) commit what enters the commit horizon.
        self.commit_all(host_now, &mut rep);
        self.emit_midi_clock(host_now);
        // (5) telemetry, signal inputs, control and cell transport.
        self.telemetry.refresh(host_now, &mut self.input);
        self.sample_signals(ev);
        rep.diags
            .extend(self.control.tick(host_now, &mut self.hosts));
        rep.diags.extend(self.cells.tick());
        self.retire(&mut rep);
        // (6) due `at` thunks, in Normal mode (never in an offline render).
        let due = if self.run_thunks {
            self.at.take_due(self.pos)
        } else {
            Vec::new()
        };
        for body in due {
            match run_thunk(ev, &body) {
                Ok(effects) => {
                    let mut dr = DrainReport::default();
                    for e in effects {
                        self.apply(ev, e, &mut dr);
                    }
                    rep.diags.extend(dr.diags);
                    rep.faults.extend(dr.faults);
                    rep.console.extend(dr.console);
                }
                Err(f) => rep.faults.push(f),
            }
        }
        self.widen(host_now, 0, &mut rep.diags);
        self.last_now = Some(host_now);
        rep
    }

    pub(crate) fn take_host_msgs(&mut self, rep: &mut TickReport) {
        let mut msgs = Vec::new();
        if self.song_enabled() {
            self.drain_owned_song_messages(rep, &mut msgs);
        } else if self.song_receipts.expected_epoch().is_some() {
            self.poll_song_messages(rep, &mut msgs);
        } else {
            self.hosts.audio.drain(&mut msgs);
        }
        self.cells.drain_port(&mut msgs);
        let mut late = 0;
        for m in &msgs {
            match *m {
                HostMsg::Song(_) => {
                    rep.faults.push(Failure::new(
                        FailCode::Type,
                        "unsolicited song acknowledgment in legacy-only runtime",
                    ));
                }
                HostMsg::SlotControlAck(a) => self.control.ack(a),
                HostMsg::CellInitAck { .. }
                | HostMsg::CellBatchAck { .. }
                | HostMsg::CellRetired { .. } => self.cells.on_msg(m),
                HostMsg::Installed { .. } | HostMsg::Retired { .. } => self.samples.on_msg(m),
                HostMsg::Counters {
                    late: l, stolen, ..
                } => {
                    late += l;
                    self.note_stolen(stolen, &mut rep.diags);
                }
                HostMsg::AnalysisCell { id, value } => self
                    .input
                    .set_analyzer(crate::pattern::eval::AnalyzerId::new(id), value),
                HostMsg::SliceOk { .. } => {}
            }
        }
        let sigs = self.hosts.audio.analysis();
        self.input.set_host(HostSig::Amp, sigs.amp);
        for (i, v) in sigs.fft.iter().enumerate() {
            self.input
                .set_host(HostSig::Fft(u16::try_from(i).unwrap_or(0)), *v);
        }
        if late > 0 {
            let now = self.hosts.audio.now();
            self.widen(now, late, &mut rep.diags);
        }
    }

    pub(crate) fn activate(&mut self) {
        let pos = self.pos;
        for slot in self.slots.iter_mut() {
            let due = slot.pending.as_ref().is_some_and(|(_, b)| *b <= pos);
            if !due {
                continue;
            }
            let Some((b, _)) = slot.pending.take() else {
                continue;
            };
            if let (Binding::Texture(t), Some(out)) = (&b, slot.out_id()) {
                if let Ok((desc, plan)) = compile_tex(t) {
                    self.hosts.render.set_program(out, desc);
                    let idx = usize::try_from(out.get()).unwrap_or(0);
                    if let Some(entry) = self.uniform_plans.get_mut(idx) {
                        *entry = Some(plan);
                    }
                }
            }
            slot.bound = Some(b);
        }
    }

    fn query_all(&mut self, ev: &mut Evaluator, target: Ratio64, rep: &mut TickReport) {
        let (vm, ns) = ev.vm_and_ns();
        let mut h = VmQuery::new(vm, ns);
        let tempo = self.clock.tempo();
        for slot in self.slots.iter_mut() {
            let name = slot.name();
            let mut faults = Vec::new();
            for lane in &mut slot.lanes {
                query_lane(
                    lane,
                    target,
                    &mut h,
                    &self.input,
                    self.cfg.seed,
                    tempo,
                    &mut faults,
                );
            }
            for mut f in faults {
                f.origin.slot = f.origin.slot.or(Some(name));
                note_fault(slot, &f, tempo);
                rep.faults.push(f);
            }
        }
    }

    fn sample_signals(&mut self, ev: &mut Evaluator) {
        let inputs = self.resolver.signal_inputs();
        if inputs.is_empty() {
            return;
        }
        let (vm, ns) = ev.vm_and_ns();
        let mut h = VmQuery::new(vm, ns);
        let mut cx = QueryCtx::new(&mut h, &self.input, self.cfg.seed);
        cx.tempo = self.clock.tempo();
        let cx = cx;
        let mut writes = Vec::new();
        for i in inputs {
            if let Ok(v) = i.sig.value_at(self.pos, &cx) {
                if let Some(x) = crate::pattern::eval::num_f64(&v) {
                    #[allow(clippy::cast_possible_truncation)]
                    writes.push((i.cell, x as f32));
                }
            }
        }
        for (cell, v) in writes {
            self.cells.write_external(cell, v);
        }
    }

    /// Expires staging and ledgers behind the position, retires ended
    /// lanes and `once` slots, and clears slot diagnostics after a clean
    /// cycle (10.3).
    fn retire(&mut self, rep: &mut TickReport) {
        let pos = self.pos;
        let mut gone = Vec::new();
        for slot in self.slots.iter_mut() {
            for lane in &mut slot.lanes {
                lane.staging.expire(pos);
                lane.ledger.expire(pos);
            }
            slot.lanes.retain(|l| {
                l.until.is_none_or(|u| u > pos) || !l.staging.emittable(l.from, l.until).is_empty()
            });
            if slot.ephemeral && slot.lanes.is_empty() {
                gone.push(slot.key);
            }
            if slot.has_faults {
                let done = pos.floor().saturating_sub(1);
                let queried = slot
                    .lanes
                    .iter()
                    .all(|l| l.queried_to >= Ratio64::from_int(done.saturating_add(1)));
                let last_fault = slot.fault_cycles.iter().copied().max().unwrap_or(i64::MIN);
                if queried && done > last_fault {
                    slot.has_faults = false;
                    slot.fault_cycles.clear();
                    rep.cleared.push(slot.key);
                }
            }
        }
        for k in gone {
            self.slots.remove(k);
        }
    }

    /// Late events: more than `late_threshold` within one second widen the
    /// commit lead by `widen_step` up to `widen_cap` (`latency-widened`).
    pub(crate) fn widen(&mut self, now: f64, late: u32, diags: &mut Vec<Diagnostic>) {
        for _ in 0..late {
            self.late.push_back(now);
        }
        while self.late.front().is_some_and(|t| *t < now - 1.0) {
            self.late.pop_front();
        }
        if self.late.len() > self.cfg.late_threshold as usize
            && self.commit_lead < self.cfg.widen_cap
        {
            self.commit_lead = (self.commit_lead + self.cfg.widen_step).min(self.cfg.widen_cap);
            self.late.clear();
            diags.push(Diagnostic {
                severity: DiagCode::LatencyWidened.default_severity(),
                ..Diagnostic::error(
                    DiagCode::LatencyWidened,
                    nowhere(),
                    format!(
                        "late events: the commit lead widened to {:.0} ms",
                        self.commit_lead * 1000.0
                    ),
                )
            });
        }
    }
}

/// The black program of a stopped visual output.
pub(crate) fn empty_program() -> ShaderDesc {
    ShaderDesc {
        source: String::new(),
        uniform_names: Box::new([]),
        assets: Box::new([]),
    }
}

#[cfg(test)]
impl Runtime {
    /// Queries `span` of every lane of `slot` in coverage-extension mode,
    /// as one fragment per cycle piece (tests drive overlapping, repeated
    /// and out-of-order windows through it).
    pub(crate) fn stage_span(
        &mut self,
        ev: &mut Evaluator,
        slot: SlotKey,
        span: TimeSpan,
    ) -> Vec<Failure> {
        let mut faults = Vec::new();
        let cx = crate::sched::staging::Probe {
            input: &self.input,
            seed: self.cfg.seed,
            tempo: self.clock.tempo(),
        };
        let (vm, ns) = ev.vm_and_ns();
        let mut h = VmQuery::new(vm, ns);
        if let Some(s) = self.slots.get_mut(slot) {
            for lane in &mut s.lanes {
                crate::sched::staging::extend(lane, span, &mut h, &cx, &mut faults);
            }
        }
        faults
    }

    /// Re-queries the invalidated spans of `slot` (step (3) for dirty spans
    /// only).
    pub(crate) fn requery_dirty(&mut self, ev: &mut Evaluator, slot: SlotKey) -> Vec<Failure> {
        let mut faults = Vec::new();
        let tempo = self.clock.tempo();
        let (vm, ns) = ev.vm_and_ns();
        let mut h = VmQuery::new(vm, ns);
        if let Some(s) = self.slots.get_mut(slot) {
            for lane in &mut s.lanes {
                let target = lane.queried_to;
                query_lane(
                    lane,
                    target,
                    &mut h,
                    &self.input,
                    self.cfg.seed,
                    tempo,
                    &mut faults,
                );
            }
        }
        faults
    }

    /// Released effects not yet drained.
    pub(crate) fn queued(&self) -> usize {
        self.queue.borrow().items.len()
    }

    /// Steps (1), (2) and (4) only: advance, activate and commit without
    /// querying anything new.
    pub(crate) fn commit_only(&mut self, host_now: f64) -> TickReport {
        let mut rep = TickReport::default();
        self.take_host_msgs(&mut rep);
        self.pos = grid(self.clock.to_cycles(host_now), false).max(self.pos);
        self.clock.set_pos(self.pos);
        self.activate();
        self.commit_all(host_now, &mut rep);
        self.last_now = Some(host_now);
        rep
    }
}
