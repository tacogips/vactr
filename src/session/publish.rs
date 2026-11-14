//! Reactive publication (design 14.5.5) and the wire forms of diagnostics,
//! sites and telemetry.
//!
//! `bindings_from` builds exactly ONE `bindings` message from a COMPLETED
//! pass's `PassReport`: by the time the report exists, validation and
//! rollback have run, so a provisional value, a staged bind, a revocation
//! or a cell update of a rolled-back form cannot reach a subscriber.
//! `PassEvent`s are never forwarded.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::clock::clock::ClockSource;
use crate::directives::attach::{CallSite, Doc};
use crate::dsp::effects::{analyzer::cells as analyzer_cell_count, param_ctl};
use crate::dsp::graph::{EffectKind, EffectSpec};
use crate::host::wire::Ctl;
use crate::ns::depgraph::{FormId, FormRec, FormState, WriteItem};
use crate::ns::evaluator::{Evaluator, PassReport};
use crate::ns::tweak::{SiteOrigin, SiteTier, TweakSite};
use crate::pattern::eval::AnalyzerId;
use crate::reader::span::{FileId, Span, SrcRef};
use crate::sched::announce::{AnnounceDrain, ANNOUNCE_CAP};
use crate::sched::runtime::Runtime;
use crate::sched::telemetry::PlayingEvent;
use crate::session::protocol::{
    BindingsBody, DiagBody, LevelsBody, PlayingBody, ServerMsg, TempoBody, TransportSample,
    WireAnalyzer, WireCall, WireChanged, WireClear, WireClock, WireDiag, WireFormState, WireLevel,
    WireOrigin, WirePlaying, WireSite, WireSpan, WireSrcRef, WireState, WireTier,
};
use crate::session::session::{DocState, Outgoing, Session};
use crate::types::diag::Diagnostic;
use crate::value::intern::{name_of_kw, name_of_sym};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// The shortest interval between two `levels` messages, seconds.
const LEVELS_PERIOD: f64 = 0.1;
/// Maximum transport snapshot cadence: twenty per second.
const TRANSPORT_PERIOD: f64 = 0.05;

/// Provenance of native output-latency telemetry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LatencyKind {
    Measured,
    Estimate,
    Unavailable,
}

impl LatencyKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Estimate => "estimate",
            Self::Unavailable => "unavailable",
        }
    }
}

/// A host's current processing-time and output-latency observation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockReading {
    pub processing_time: f64,
    pub latency_seconds: Option<f64>,
    pub latency_kind: LatencyKind,
    pub uncertainty_seconds: Option<f64>,
}

/// The display name of `id` in `files`.
#[must_use]
pub fn file_name(files: &[Rc<str>], id: FileId) -> String {
    files
        .get(id.get() as usize)
        .map_or_else(|| format!("<file {}>", id.get()), ToString::to_string)
}

#[must_use]
pub fn wire_span(s: Span) -> WireSpan {
    WireSpan::new(s.start, s.end)
}

/// `[num, den]`.
#[must_use]
pub fn ratio_pair(r: Ratio64) -> [i64; 2] {
    [r.num(), r.den()]
}

/// A static or runtime diagnostic on the wire.
#[must_use]
pub fn diag_wire(files: &[Rc<str>], d: &Diagnostic) -> WireDiag {
    WireDiag {
        code: d.code.as_str().to_string(),
        severity: d.severity.as_str().to_string(),
        message: d.message.clone(),
        span: wire_span(d.span),
        file: file_name(files, d.span.file),
        slot: d
            .origin
            .as_ref()
            .and_then(|o| o.slot)
            .map(|k| name_of_kw(k).to_string()),
        beat: d.origin.as_ref().and_then(|o| o.beat).map(ratio_pair),
    }
}

/// A failure on the wire, at its origin (else `at`).
#[must_use]
pub fn failure_wire(files: &[Rc<str>], f: &Failure, at: Span) -> WireDiag {
    let span = f.origin.span.unwrap_or(at);
    WireDiag {
        code: f.code.as_str().to_string(),
        severity: "error".to_string(),
        message: f.message.clone(),
        span: wire_span(span),
        file: file_name(files, span.file),
        slot: f.origin.slot.map(|k| name_of_kw(k).to_string()),
        beat: f.origin.beat.map(ratio_pair),
    }
}

/// A number as `f64` (`0.0` for a non-number).
#[must_use]
pub fn num_f64(v: &Value) -> f64 {
    match v {
        Value::Int(n) => f64::from(*n),
        #[allow(clippy::cast_precision_loss)]
        Value::Int64(n) => *n as f64,
        Value::Float(x) => f64::from(*x),
        Value::Float64(x) => *x,
        Value::Ratio(r) => r.to_f64(),
        Value::VarRef(s) => num_f64(&s.get()),
        _ => 0.0,
    }
}

/// One site on the wire, with its EFFECTIVE tier. `doc` is the site's file's
/// document model (`DirectiveTable::doc`), used to find its enclosing call
/// (TASK-010 G3); an empty `Doc` (no document available) simply gives
/// `call: None`.
#[must_use]
pub fn site_wire(ev: &Evaluator, site: &TweakSite, key: Option<String>, doc: &Doc) -> WireSite {
    let tier = match ev.site_tier(site.id).unwrap_or(site.tier) {
        SiteTier::Direct => WireTier::Direct,
        SiteTier::Reeval => WireTier::Reeval,
        SiteTier::Manual => WireTier::Manual,
    };
    let origin = match site.origin {
        SiteOrigin::PatternLiteral => WireOrigin::PatternLiteral,
        SiteOrigin::Binding => WireOrigin::Binding,
        SiteOrigin::InstDefault => WireOrigin::InstDefault,
    };
    WireSite {
        id: site.id.get(),
        span: wire_span(site.span),
        tier,
        origin,
        value: num_f64(&site.slot.get()),
        form_gen: site.form_gen.get(),
        key,
        call: call_of(doc, site.span),
    }
}

/// The nearest enclosing symbol-headed call of `site_span` (design 15.1.2
/// G3): the `CallSite` of `doc` whose argument extent containing
/// `site_span` is smallest, directly or inside a list or pattern argument.
/// `None` with no enclosing call (e.g. a `let`/`inst`/`fn` header literal).
fn call_of(doc: &Doc, site_span: Span) -> Option<WireCall> {
    let mut best: Option<(&CallSite, u16, Span)> = None;
    for call in &doc.sites {
        for (idx, (_, extent)) in call.args.iter().enumerate() {
            if extent.start > site_span.start || extent.end < site_span.end {
                continue;
            }
            let smaller = best
                .as_ref()
                .is_none_or(|(_, _, b)| extent.end - extent.start < b.end - b.start);
            if smaller {
                let idx = u16::try_from(idx).unwrap_or(u16::MAX);
                best = Some((call, idx, *extent));
            }
        }
    }
    let (call, arg, _) = best?;
    let param = call
        .args
        .get(usize::from(arg))
        .and_then(|(kw, _)| kw.clone())
        .map(|k| k.to_string())
        .or_else(|| call.params.get(usize::from(arg)).map(ToString::to_string));
    Some(WireCall {
        name: call.name.to_string(),
        head: wire_span(call.head),
        ordinal: ordinal_of(doc, call),
        arg,
        param,
    })
}

/// The 1-based count of same-named `CallSite`s within `call`'s top-level
/// form, in source order (design 15.1.2 G3).
fn ordinal_of(doc: &Doc, call: &CallSite) -> u16 {
    let scope = doc
        .targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.extent.start <= call.head.start && call.head.end <= t.extent.end)
        .min_by_key(|(_, t)| t.extent.end - t.extent.start)
        .map(|(k, _)| doc.targets[doc.top_of(k)].extent);
    let Some(scope) = scope else { return 1 };
    let pos = doc
        .sites
        .iter()
        .filter(|s| s.name == call.name && s.head.start >= scope.start && s.head.end <= scope.end)
        .position(|s| s.head == call.head)
        .unwrap_or(0);
    u16::try_from(pos + 1).unwrap_or(u16::MAX)
}

/// The name a form is published under: its first defined name, else its
/// first bound slot.
#[must_use]
pub fn form_name(rec: &FormRec) -> String {
    rec.writes
        .iter()
        .find_map(|w| match w {
            WriteItem::Name(s) => Some(name_of_sym(s.name()).to_string()),
            WriteItem::Bind(_) => None,
        })
        .or_else(|| {
            rec.writes.iter().find_map(|w| match w {
                WriteItem::Bind(k) => Some(k.name()),
                WriteItem::Name(_) => None,
            })
        })
        .unwrap_or_default()
}

/// The committed display value of a form: its first defined name's value.
fn form_value(rec: &FormRec) -> String {
    rec.names()
        .next()
        .map_or_else(|| "nil".to_string(), |s| s.get().to_string())
}

/// The ONE `bindings` batch of a completed pass, or `None` when the pass
/// scheduled no form.
#[must_use]
pub fn bindings_from(
    report: &PassReport,
    ev: &Evaluator,
    pass: u64,
    files: &[Rc<str>],
    docs: &BTreeMap<FileId, DocState>,
) -> Option<ServerMsg> {
    let empty_doc = Doc::default();
    let mut finals: BTreeMap<FormId, FormState> = BTreeMap::new();
    let mut order: Vec<FormId> = Vec::new();
    for (f, st) in &report.states {
        if finals.insert(*f, st.clone()).is_none() {
            order.push(*f);
        }
    }
    if order.is_empty() {
        return None;
    }
    let changed = report
        .bindings
        .iter()
        .map(|(sym, v)| {
            let name = name_of_sym(*sym).to_string();
            let form_gen = ev
                .form_of(&name)
                .and_then(|f| ev.graph().get(f))
                .map_or(0, |r| r.gen.get());
            WireChanged {
                name,
                value: v.to_string(),
                form_gen,
            }
        })
        .collect();
    let mut states = Vec::new();
    let mut sites = Vec::new();
    for f in order {
        let Some(rec) = ev.graph().get(f) else {
            continue;
        };
        let st = &finals[&f];
        let (state, blocked_on, diagnostic) = match st {
            FormState::Clean | FormState::Recomputed => (WireState::Ok, None, None),
            FormState::Failed(e) => (
                WireState::Failed,
                None,
                Some(failure_wire(files, e, rec.node.span)),
            ),
            FormState::Blocked { on } => {
                (WireState::Blocked, Some(name_of_sym(*on).to_string()), None)
            }
        };
        if *st == FormState::Recomputed {
            let table = ev.ns().tweaks().borrow();
            sites.extend(table.sites_of(rec.gen).iter().map(|s| {
                let doc = docs
                    .get(&s.span.file)
                    .map_or(&empty_doc, |d| &d.directives.doc);
                site_wire(ev, s, None, doc)
            }));
        }
        states.push(WireFormState {
            name: form_name(rec),
            state,
            value: form_value(rec),
            blocked_on,
            diagnostic,
        });
    }
    Some(ServerMsg::Bindings(BindingsBody {
        pass,
        changed,
        sites,
        states,
    }))
}

/// A duration in host seconds as beats on a 1/960 grid.
fn dur_beats(secs: f64, bpm: f64) -> [i64; 2] {
    let beats = (secs * bpm / 60.0).max(0.0);
    #[allow(clippy::cast_possible_truncation)]
    let n = (beats * 960.0).round().clamp(0.0, 9.0e15) as i64;
    ratio_pair(Ratio64::new(n, 960).unwrap_or(Ratio64::ZERO))
}

/// One `playing` event, its `SrcRef` stamped with the revision `rev_of`
/// reports for its form generation.
#[must_use]
pub fn playing_wire(
    e: &PlayingEvent,
    files: &[Rc<str>],
    bpm: f64,
    rev_of: &dyn Fn(&SrcRef) -> u64,
) -> WirePlaying {
    WirePlaying {
        epoch: None,
        end_time: Some(e.time + e.dur),
        id: e.id,
        slot: name_of_kw(e.slot).to_string(),
        beat: ratio_pair(e.beat),
        time: e.time,
        dur: dur_beats(e.dur, bpm),
        src: e.src.map(|s| WireSrcRef {
            file: file_name(files, s.span.file),
            span: wire_span(s.span),
            doc_revision: rev_of(&s),
            form_gen: s.form_gen.get(),
        }),
    }
}

/// Builds the bounded playing envelope, keeping committed events ahead of previews.
pub(crate) fn playing_body(
    events: &[PlayingEvent],
    announced: AnnounceDrain,
    files: &[Rc<str>],
    bpm: f64,
    epoch: &str,
    rev_of: &dyn Fn(&SrcRef) -> u64,
) -> Option<PlayingBody> {
    if events.is_empty() && announced.ahead.is_empty() && announced.retract.is_empty() {
        return None;
    }
    let events: Vec<_> = events
        .iter()
        .take(ANNOUNCE_CAP)
        .map(|event| {
            let mut wire = playing_wire(event, files, bpm, rev_of);
            wire.epoch = Some(epoch.to_owned());
            wire
        })
        .collect();
    let ahead_limit = ANNOUNCE_CAP.saturating_sub(events.len());
    let mut ahead: Vec<_> = announced
        .ahead
        .iter()
        .take(ahead_limit)
        .map(|event| {
            let mut wire = playing_wire(event, files, bpm, rev_of);
            wire.epoch = Some(epoch.to_owned());
            wire
        })
        .collect();
    ahead.sort_by(|a, b| {
        a.time
            .total_cmp(&b.time)
            .then_with(|| a.slot.cmp(&b.slot))
            .then_with(|| a.id.cmp(&b.id))
    });
    Some(PlayingBody {
        events,
        ahead,
        retract: announced.retract.into_iter().take(ANNOUNCE_CAP).collect(),
    })
}

/// The constant `id` parameter of an analyzer's `EffectSpec`, when it has
/// one (a `Ctl::Cell` default is skipped, design 15.1.2 G4).
fn analyzer_id(spec: &EffectSpec) -> Option<u32> {
    let ctl = param_ctl(spec.kind, "id")?;
    spec.params
        .iter()
        .find(|(id, _)| *id == ctl)
        .and_then(|(_, c)| match c {
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            Ctl::Const(v) => Some(*v as u32),
            Ctl::Cell(_) => None,
        })
}

/// Every `EffectKind::Analyzer` unit of `chain` with a constant `id`, with
/// its current cells read from the runtime's input cells (design 12.5,
/// 15.1.2 G4).
fn analyzers_of(rt: &Runtime, bus: &str, chain: &[EffectSpec]) -> Vec<WireAnalyzer> {
    chain
        .iter()
        .filter_map(|spec| {
            let EffectKind::Analyzer(kind) = spec.kind else {
                return None;
            };
            let id = analyzer_id(spec)?;
            let count = analyzer_cell_count(kind);
            let cells = (0..count)
                .map(|k| {
                    let off = u32::try_from(k).unwrap_or(0);
                    rt.input.analyzer(AnalyzerId::new(id.saturating_add(off)))
                })
                .collect();
            Some(WireAnalyzer {
                bus: bus.to_string(),
                kind: spec.kind.name().to_string(),
                id,
                cells,
            })
        })
        .collect()
}

impl Session {
    /// The revision a telemetry `SrcRef` belongs to: the revision its
    /// form (by generation) was evaluated at.
    fn revision_of(&self, s: &SrcRef) -> u64 {
        let gen = s.form_gen;
        self.ev
            .graph()
            .form_of_gen(gen)
            .and_then(|f| self.form_revs.get(&f))
            .or_else(|| self.gen_revs.get(&gen.get()))
            .map(|(_, rev)| *rev)
            .or_else(|| self.docs.get(&s.span.file).map(|d| d.text_rev))
            .unwrap_or(0)
    }

    /// One scheduler step (14.5.4 "tick"), routed: the coalesced writes
    /// (their passes publish `bindings`), `Runtime::tick` (which polls
    /// captures), then `diag`, `playing` (when a connection subscribed to
    /// telemetry), `levels` (at most 10 per second) and `tempo` (on
    /// change).
    pub fn tick_routed(&mut self, host_now: f64) -> Vec<Outgoing> {
        if !host_now.is_finite() || host_now < 0.0 {
            return Vec::new();
        }
        self.last_host_now = host_now;
        let mut out = std::mem::take(&mut self.outbox);
        for (conn, msg) in self.apply_pending() {
            let routed = self.route(conn.unwrap_or(0), None, vec![msg]);
            out.extend(routed);
        }
        let drained = self.rt.drain(&mut self.ev);
        let rep = self.rt.tick(&mut self.ev, host_now);
        out.extend(self.publish_song_notices());
        self.console.extend(
            drained
                .console
                .iter()
                .chain(rep.console.iter())
                .map(ToString::to_string),
        );
        let mut msgs = Vec::new();
        let files = &self.files;
        let nowhere = Span::new(FileId::CONSOLE, 0, 0);
        let mut add: Vec<_> = drained
            .diags
            .iter()
            .chain(rep.diags.iter())
            .map(|d| diag_wire(files, d))
            .collect();
        add.extend(
            drained
                .faults
                .iter()
                .chain(rep.faults.iter())
                .map(|f| failure_wire(files, f, nowhere)),
        );
        let clear: Vec<WireClear> = rep
            .cleared
            .iter()
            .map(|k| WireClear { slot: k.name() })
            .collect();
        if !add.is_empty() || !clear.is_empty() {
            msgs.push(ServerMsg::Diag(DiagBody { add, clear }));
        }
        let tempo = self.rt.clock().tempo();
        let source = self.rt.clock().source();
        let frozen = self.rt.midi_clock.is_frozen();
        let lost = self.rt.midi_clock.is_lost();
        let state = (host_now, self.rt.pos, source, frozen, lost);
        let generation = self.rt.midi_clock().restart_generation();
        let restarted = generation != self.transport_restart_generation;
        self.transport_restart_generation = generation;
        if restarted
            || self
                .transport_state
                .is_some_and(|(time, pos, old_source, paused, old_lost)| {
                    host_now < time
                        || self.rt.pos < pos
                        || source != old_source
                        || frozen != paused
                        || lost != old_lost
                })
        {
            self.transport_epoch = self.transport_epoch.saturating_add(1);
            // A changed epoch invalidates any previously projected event times.
        }
        if !lost {
            self.transport_cycle = self.rt.pos;
        } else if self.transport_state.is_some_and(|s| !s.4) {
            self.transport_cycle = self.transport_state.map_or(self.rt.pos, |s| s.1);
        }
        self.transport_state = Some(state);
        let epoch = format!("session-{}-{}", self.transport_id, self.transport_epoch);
        let events = self.rt.telemetry();
        let announced = self.rt.announced();
        let has_telemetry_subscriber = self.subs.values().any(|s| s.telemetry);
        if has_telemetry_subscriber {
            let bpm = tempo.bpm.to_f64();
            let rev_of = |s: &SrcRef| self.revision_of(s);
            if let Some(body) = playing_body(&events, announced, &self.files, bpm, &epoch, &rev_of)
            {
                msgs.push(ServerMsg::Playing(body));
            }
        }
        let due = self
            .last_levels
            .is_none_or(|t| host_now - t >= LEVELS_PERIOD || host_now < t);
        if due && self.subs.values().any(|s| s.levels) {
            self.last_levels = Some(host_now);
            let sigs = self.rt.hosts.audio.analysis();
            let rms = f64::from(sigs.amp);
            let analyzers = self.ev.insts().map(|reg| {
                let reg = reg.borrow();
                reg.buses()
                    .flat_map(|(name, bus)| {
                        let bus_name = name
                            .map_or_else(|| ":master".to_string(), |k| name_of_kw(k).to_string());
                        analyzers_of(&self.rt, &bus_name, &bus.def.chain)
                    })
                    .collect::<Vec<_>>()
            });
            msgs.push(ServerMsg::Levels(LevelsBody {
                levels: vec![WireLevel {
                    source: ":master".to_string(),
                    rms,
                    bands: Some(sigs.fft),
                }],
                analyzers: analyzers.filter(|v| !v.is_empty()),
                time: Some(host_now),
                epoch: Some(epoch.clone()),
            }));
        }
        let clock = if source == ClockSource::MidiClock {
            WireClock {
                source: "midi".to_string(),
                locked: Some(!self.rt.midi_clock.is_lost()),
            }
        } else {
            WireClock {
                source: "internal".to_string(),
                locked: None,
            }
        };
        let key = (tempo.bpm, tempo.beats_per_cycle, source, clock.locked);
        let due = self
            .last_transport
            .is_none_or(|t| host_now - t >= TRANSPORT_PERIOD || host_now < t);
        let transport = if due && self.subs.values().any(|s| s.telemetry) {
            self.last_transport = Some(host_now);
            // `sched::runtime::GRID` is private; keep its current value here for the guard.
            let cps = tempo.bpm.to_f64() / 60.0 / tempo.beats_per_cycle.to_f64();
            let grid_period = 1.0 / (960.0 * cps);
            let candidate = self.rt.clock().to_host(self.transport_cycle);
            let sample_time = if !frozen
                && !lost
                && candidate.is_finite()
                && candidate >= 0.0
                && (candidate - host_now).abs() <= grid_period + 1e-9
            {
                candidate
            } else {
                host_now
            };
            Some(TransportSample {
                epoch,
                sample_time,
                cycle: ratio_pair(self.transport_cycle),
                bpm: tempo.bpm.to_f64(),
                beats_per_cycle: tempo.beats_per_cycle.to_f64(),
                running: !frozen && !lost,
                latency_seconds: self
                    .observed_clock
                    .and_then(|reading| reading.latency_seconds),
                latency_kind: self
                    .observed_clock
                    .map_or("unavailable", |reading| reading.latency_kind.as_str())
                    .to_string(),
                uncertainty_seconds: self
                    .observed_clock
                    .and_then(|reading| reading.uncertainty_seconds),
            })
        } else {
            None
        };
        if self.last_tempo != Some(key) || transport.is_some() {
            self.last_tempo = Some(key);
            msgs.push(ServerMsg::Tempo(TempoBody {
                transport,
                bpm: tempo.bpm.to_f64(),
                beats_per_cycle: tempo.beats_per_cycle.floor(),
                cycle: ratio_pair(self.rt.pos),
                clock: Some(clock),
            }));
        }
        let routed = self.route(0, None, msgs);
        out.extend(routed);
        out
    }
}
