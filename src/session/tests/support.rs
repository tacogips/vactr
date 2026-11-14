//! The session test rig: a `Session` over recording hosts on one mock
//! clock, a subscriber log of every server message, and helpers.

use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellRead;
use crate::host::caps::{Hosts, SampleLoader};
use crate::host::noop::NoopHost;
use crate::host::testing::{AudioCall, MockClock, RecordingAudioHost};
use crate::host::wire::{AudioEvent, Ctl};
use crate::session::changes::Change;
use crate::session::protocol::{
    BindingsBody, ClientMsg, DocChangedBody, Envelope, EvalBody, EvalResultBody, ServerMsg,
    SetTweakBody, SetVarBody, StaleBindingBody, SubscribeBody, WireNum, WireSite, WireSpan,
    WireValue,
};
use crate::session::session::{Session, SessionConfig};

/// The tick period of every test, seconds.
pub(super) const DT: f64 = 0.01;

/// The document every test edits.
pub(super) const DOC: &str = "main.vact";

/// A session over a recording audio host; every server message it
/// produced is in `log`, in order.
pub(super) struct Rig {
    pub s: Session,
    pub clock: MockClock,
    pub audio: RecordingAudioHost,
    pub log: Vec<ServerMsg>,
    seq: u64,
}

impl Rig {
    /// Native caps, default runtime, no packages.
    pub(super) fn new() -> Rig {
        Rig::with(SessionConfig::new(CapabilitySet::native()))
    }

    pub(super) fn with(cfg: SessionConfig) -> Rig {
        Rig::with_samples(cfg, Box::new(NoopHost))
    }

    pub(super) fn with_samples(cfg: SessionConfig, samples: Box<dyn SampleLoader>) -> Rig {
        let clock = MockClock::new(0.0);
        let audio = RecordingAudioHost::new(clock.clone());
        let hosts = Hosts {
            audio: Box::new(audio.clone()),
            samples,
            ..Hosts::noop()
        };
        let mut rig = Rig {
            s: Session::new(cfg, hosts),
            clock,
            audio,
            log: Vec::new(),
            seq: 0,
        };
        rig.send(ClientMsg::Subscribe(SubscribeBody {
            telemetry: true,
            levels: false,
            diagnostics: true,
        }));
        rig
    }

    /// Applies one message as connection 0; returns (and logs) what it
    /// produced.
    pub(super) fn send(&mut self, msg: ClientMsg) -> Vec<ServerMsg> {
        self.seq += 1;
        let out: Vec<ServerMsg> = self
            .s
            .apply(Envelope::new(self.seq, None, msg))
            .into_iter()
            .map(|e| e.body)
            .collect();
        self.log.extend(out.iter().cloned());
        out
    }

    /// `eval` of the whole document `DOC`.
    pub(super) fn eval(
        &mut self,
        code: &str,
        rev: u64,
        epoch: u64,
    ) -> (EvalResultBody, Vec<ServerMsg>) {
        let out = self.send(ClientMsg::Eval(EvalBody {
            file: DOC.to_string(),
            code: code.to_string(),
            span: None,
            doc_revision: rev,
            edit_epoch: epoch,
        }));
        let mut it = out.into_iter();
        let Some(ServerMsg::EvalResult(r)) = it.next() else {
            panic!("eval-result first");
        };
        (r, it.collect())
    }

    /// `eval` that must report no failed form.
    pub(super) fn ok(&mut self, code: &str, rev: u64) -> EvalResultBody {
        let (r, _) = self.eval(code, rev, 0);
        for f in &r.forms {
            assert!(f.failure.is_none(), "{code:?}: {:?}", f.failure);
        }
        r
    }

    pub(super) fn set_var(
        &mut self,
        name: &str,
        value: WireValue,
        gen: u64,
        epoch: u64,
    ) -> Vec<ServerMsg> {
        self.send(ClientMsg::SetVar(SetVarBody {
            file: DOC.to_string(),
            name: name.to_string(),
            value,
            defining_form_gen: gen,
            edit_epoch: epoch,
        }))
    }

    pub(super) fn set_tweak(&mut self, site: &WireSite, value: f64, epoch: u64) -> Vec<ServerMsg> {
        self.send(ClientMsg::SetTweak(SetTweakBody {
            file: DOC.to_string(),
            id: site.id,
            form_gen: site.form_gen,
            value: WireNum::Float(value),
            edit_epoch: epoch,
        }))
    }

    pub(super) fn doc_changed(
        &mut self,
        rev: u64,
        base: u64,
        changes: &[(u32, u32, u32)],
        dirty: &[(u32, u32)],
        epoch: u64,
    ) -> Vec<ServerMsg> {
        self.send(ClientMsg::DocChanged(DocChangedBody {
            file: DOC.to_string(),
            doc_revision: rev,
            base_revision: base,
            changes: changes
                .iter()
                .map(|(from, to, insert_len)| Change {
                    from: *from,
                    to: *to,
                    insert_len: *insert_len,
                })
                .collect(),
            dirty: dirty.iter().map(|(s, e)| WireSpan::new(*s, *e)).collect(),
            edit_epoch: epoch,
        }))
    }

    /// One tick at the current mock time (logged).
    pub(super) fn tick(&mut self) -> Vec<ServerMsg> {
        let out = self.s.tick(self.clock.now());
        self.log.extend(out.iter().cloned());
        out
    }

    /// Ticks every `DT` up to and including `end` (logged).
    pub(super) fn run_to(&mut self, end: f64) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        loop {
            out.extend(self.tick());
            let next = self.clock.now() + DT;
            if next > end + 1e-9 {
                break;
            }
            self.clock.set(next);
        }
        out
    }

    /// Every event the audio host received, with its time.
    pub(super) fn sent(&self) -> Vec<AudioEvent> {
        self.audio
            .calls()
            .into_iter()
            .filter_map(|(_, c)| match c {
                AudioCall::Send(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    /// The value of control `name` of `e` (a cell reads the native cells).
    pub(super) fn ctl(&self, e: &AudioEvent, name: &str) -> Option<f32> {
        let id = crate::dsp::controls::row(name).expect("row").ctl;
        let (_, c) = e.controls().iter().find(|(c, _)| *c == id).copied()?;
        match c {
            Ctl::Const(v) => Some(v),
            Ctl::Cell(cell) => self.s.runtime().cells().native().map(|n| n.get(cell)),
        }
    }

    /// Clears the log.
    pub(super) fn clear(&mut self) {
        self.log.clear();
    }
}

/// The `bindings` batches among `msgs`.
pub(super) fn batches(msgs: &[ServerMsg]) -> Vec<BindingsBody> {
    msgs.iter()
        .filter_map(|m| match m {
            ServerMsg::Bindings(b) => Some(b.clone()),
            _ => None,
        })
        .collect()
}

/// The `stale-binding` replies among `msgs`.
pub(super) fn stales(msgs: &[ServerMsg]) -> Vec<StaleBindingBody> {
    msgs.iter()
        .filter_map(|m| match m {
            ServerMsg::StaleBinding(b) => Some(b.clone()),
            _ => None,
        })
        .collect()
}

/// The site whose span covers `needle` (first occurrence) in `src`.
pub(super) fn site_at<'a>(r: &'a EvalResultBody, src: &str, needle: &str) -> &'a WireSite {
    let at =
        u32::try_from(src.find(needle).unwrap_or_else(|| panic!("`{needle}`"))).expect("small");
    r.sites
        .iter()
        .find(|s| s.span.start == at)
        .unwrap_or_else(|| panic!("no site at `{needle}` ({at}): {:?}", r.sites))
}

/// The byte offset of the `nth` occurrence of `needle`.
pub(super) fn offset(src: &str, needle: &str, nth: usize) -> u32 {
    let at = src
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("`{needle}` #{nth}"))
        .0;
    u32::try_from(at).expect("small")
}
