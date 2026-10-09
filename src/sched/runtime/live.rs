//! Live output tracking and stop commands, implemented in LP-SESSION-STOP.

use crate::host::wire::{CtlMsg, OutputMode, OutputPhase, Release};
use crate::ns::evaluator::Evaluator;
use crate::ns::stage::SlotKey;
use crate::sched::runtime::{DrainReport, Runtime};
use crate::sched::song::SongTransportState;
use crate::song::routing::SongEndpoints;

#[derive(Debug, Default)]
pub(crate) struct OutputTracker {
    phase: Option<OutputPhase>,
    frame: u64,
    request_frame: u64,
    requested: Option<(OutputMode, u32, u8)>,
    pending_diagnostic: Option<String>,
    // Read by the session momentary layer after LP-SESSION-STOP lands.
    #[allow(dead_code)]
    cuts: u64,
}

impl OutputTracker {
    pub(crate) fn observe(&mut self, phase: OutputPhase, frame: u64) {
        self.phase = Some(phase);
        self.frame = frame;
        if self.frame >= self.request_frame {
            if self.requested.is_some() {
                self.requested = None;
            }
            self.pending_diagnostic = None;
        }
    }

    fn effective_request(&self, mode: OutputMode) -> OutputMode {
        match (self.requested, mode) {
            (Some((OutputMode::Cut, _, _)), OutputMode::Gentle) => OutputMode::Cut,
            _ => mode,
        }
    }

    fn request(&mut self, mode: OutputMode, frame: u64) {
        self.request_frame = frame;
        self.requested = Some((mode, 0, 0));
    }

    fn tick(&mut self, resend_ticks: u32) -> Option<Result<OutputMode, String>> {
        let (mode, waiting, resends) = self.requested?;
        let waiting = waiting.saturating_add(1);
        if waiting < resend_ticks.max(1) {
            self.requested = Some((mode, waiting, resends));
            return None;
        }
        if resends < 3 {
            self.requested = Some((mode, 0, resends + 1));
            Some(Ok(mode))
        } else {
            self.requested = None;
            Some(Err(format!(
                "whole-output stop {mode:?} is not acknowledged after {} resends",
                resends
            )))
        }
    }

    pub(crate) fn wire(&self) -> Option<String> {
        self.phase.map(|phase| {
            match phase {
                OutputPhase::Running => "running",
                OutputPhase::Draining => "draining",
                OutputPhase::Cutting => "cutting",
                OutputPhase::Idle => "idle",
            }
            .to_string()
        })
    }

    // Reserved for LP-SESSION-MOMENTARY, which clears overrides after cuts.
    #[allow(dead_code)]
    pub(crate) fn cuts(&self) -> u64 {
        self.cuts
    }
}

impl Runtime {
    pub(crate) fn output_tick(&mut self) {
        if let Some(message) = self.output.pending_diagnostic.take() {
            self.report_output_diagnostic(message);
        }
        match self.output.tick(self.cfg.resend_ticks) {
            Some(Ok(mode)) => self.hosts.audio.post(CtlMsg::OutputStop { mode }),
            Some(Err(message)) => self.report_output_diagnostic(message),
            None => {}
        }
    }

    fn report_output_diagnostic(&mut self, message: String) {
        if self.control.transport_reported() {
            return;
        }
        if self.control.has_outstanding() {
            self.output.pending_diagnostic = Some(message);
        } else {
            self.control.report_transport(message);
        }
    }

    pub(crate) fn stop_all(&mut self, _ev: &mut Evaluator, rep: &mut DrainReport) {
        let now = self.revoke_with(SlotKey::All, Release::Natural);
        self.close_live_notes(SlotKey::All, now);
        let frame = host_frame(now, self.cfg.sample_rate);
        let mode = self.output.effective_request(OutputMode::Gentle);
        self.hosts.audio.post(CtlMsg::OutputStop { mode });
        self.output.request(mode, frame);
        self.stop_song(mode, rep);
    }

    pub(crate) fn cut(&mut self, _ev: &mut Evaluator, rep: &mut DrainReport) {
        self.output.cuts = self.output.cuts.saturating_add(1);
        let now = self.revoke_with(SlotKey::All, Release::Panic);
        self.close_live_notes(SlotKey::All, now);
        let frame = host_frame(now, self.cfg.sample_rate);
        let mode = self.output.effective_request(OutputMode::Cut);
        self.hosts.audio.post(CtlMsg::OutputStop { mode });
        self.output.request(mode, frame);
        self.stop_song(mode, rep);
    }
}

fn host_frame(now: f64, sample_rate: u32) -> u64 {
    let frame = now * f64::from(sample_rate);
    if frame.is_finite() && frame > 0.0 {
        frame.round() as u64
    } else {
        0
    }
}

pub(crate) fn stop_endpoints(
    mode: OutputMode,
    state: SongTransportState,
    now_frame: u64,
    lead_frames: u64,
    activation_frame: u64,
    old: SongEndpoints,
) -> Option<SongEndpoints> {
    match mode {
        OutputMode::Gentle
            if matches!(
                state,
                SongTransportState::Playing | SongTransportState::Prepared
            ) =>
        {
            let arrangement = now_frame
                .saturating_add(lead_frames)
                .max(activation_frame)
                .min(old.arrangement);
            let old_tail = old.tail_deadline.saturating_sub(old.arrangement);
            let tail_deadline = arrangement.saturating_add(old_tail).min(old.tail_deadline);
            Some(SongEndpoints {
                epoch: old.epoch,
                arrangement,
                tail_deadline,
            })
        }
        OutputMode::Cut
            if matches!(
                state,
                SongTransportState::Prepared
                    | SongTransportState::Playing
                    | SongTransportState::Draining
            ) =>
        {
            let arrangement = now_frame.max(activation_frame).min(old.arrangement);
            let tail_deadline = now_frame.max(arrangement).min(old.tail_deadline);
            Some(SongEndpoints {
                epoch: old.epoch,
                arrangement,
                tail_deadline,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
impl Runtime {
    pub(crate) fn stop_endpoints_for_test(
        mode: OutputMode,
        state: SongTransportState,
        now_frame: u64,
        lead_frames: u64,
        activation_frame: u64,
        old: SongEndpoints,
    ) -> Option<SongEndpoints> {
        stop_endpoints(mode, state, now_frame, lead_frames, activation_frame, old)
    }
}
