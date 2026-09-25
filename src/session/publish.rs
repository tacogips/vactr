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

use crate::ns::depgraph::{FormId, FormRec, FormState, WriteItem};
use crate::ns::evaluator::{Evaluator, PassReport};
use crate::ns::tweak::{SiteOrigin, SiteTier, TweakSite};
use crate::reader::span::{FileId, Span, SrcRef};
use crate::sched::telemetry::PlayingEvent;
use crate::session::protocol::{
    BindingsBody, DiagBody, LevelsBody, PlayingBody, ServerMsg, TempoBody, WireChanged, WireClear,
    WireDiag, WireFormState, WireLevel, WireOrigin, WirePlaying, WireSite, WireSpan, WireSrcRef,
    WireState, WireTier,
};
use crate::session::session::{Outgoing, Session};
use crate::types::diag::Diagnostic;
use crate::value::intern::{name_of_kw, name_of_sym};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// The shortest interval between two `levels` messages, seconds.
const LEVELS_PERIOD: f64 = 0.1;

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

/// One site on the wire, with its EFFECTIVE tier.
#[must_use]
pub fn site_wire(ev: &Evaluator, site: &TweakSite, key: Option<String>) -> WireSite {
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
    }
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
) -> Option<ServerMsg> {
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
            sites.extend(
                table
                    .sites_of(rec.gen)
                    .iter()
                    .map(|s| site_wire(ev, s, None)),
            );
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
        let mut out = std::mem::take(&mut self.outbox);
        for (conn, msg) in self.apply_pending() {
            let routed = self.route(conn.unwrap_or(0), None, vec![msg]);
            out.extend(routed);
        }
        let drained = self.rt.drain(&mut self.ev);
        let rep = self.rt.tick(&mut self.ev, host_now);
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
        let events = self.rt.telemetry();
        if !events.is_empty() && self.subs.values().any(|s| s.telemetry) {
            let bpm = tempo.bpm.to_f64();
            let rev_of = |s: &SrcRef| self.revision_of(s);
            let events = events
                .iter()
                .map(|e| playing_wire(e, &self.files, bpm, &rev_of))
                .collect();
            msgs.push(ServerMsg::Playing(PlayingBody { events }));
        }
        let due = self
            .last_levels
            .map_or(true, |t| host_now - t >= LEVELS_PERIOD || host_now < t);
        if due && self.subs.values().any(|s| s.levels) {
            self.last_levels = Some(host_now);
            let rms = f64::from(self.rt.hosts.audio.analysis().amp);
            msgs.push(ServerMsg::Levels(LevelsBody {
                levels: vec![WireLevel {
                    source: ":master".to_string(),
                    rms,
                }],
            }));
        }
        let key = (tempo.bpm, tempo.beats_per_cycle);
        if self.last_tempo != Some(key) {
            self.last_tempo = Some(key);
            msgs.push(ServerMsg::Tempo(TempoBody {
                bpm: tempo.bpm.to_f64(),
                beats_per_cycle: tempo.beats_per_cycle.floor(),
                cycle: ratio_pair(self.rt.pos),
            }));
        }
        let routed = self.route(0, None, msgs);
        out.extend(routed);
        out
    }
}
