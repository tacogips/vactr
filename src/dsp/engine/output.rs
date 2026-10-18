use crate::dsp::ring::CHANNEL_CAPACITY;
use crate::host::wire::{OutputMode, OutputPhase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Running,
    Draining,
    Cutting,
    ClearingCut,
    ClearingIdle,
    Idle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReportSource {
    PhaseChange,
    StopAck,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ClearCursor {
    pub orbit: usize,
    pub bus: usize,
    pub offset: usize,
    pub region: usize,
    pub reinitializing: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct OutputStage {
    pub phase: Phase,
    pub gate: f32,
    gate_step: f32,
    pub since_frame: u64,
    pub below_frames: u64,
    pub clear: ClearCursor,
    pub last_reported: OutputPhase,
    pub pending_report: Option<(OutputPhase, u64)>,
    stop_acks: [Option<(OutputPhase, u64)>; CHANNEL_CAPACITY],
    stop_ack_head: usize,
    stop_ack_len: usize,
    admitted_during_cut_clear: bool,
    fade_in: bool,
}

impl OutputStage {
    pub fn new() -> Self {
        Self {
            phase: Phase::Running,
            gate: 1.0,
            gate_step: 0.0,
            since_frame: 0,
            below_frames: 0,
            clear: ClearCursor::default(),
            last_reported: OutputPhase::Running,
            pending_report: None,
            stop_acks: [None; CHANNEL_CAPACITY],
            stop_ack_head: 0,
            stop_ack_len: 0,
            admitted_during_cut_clear: false,
            fade_in: false,
        }
    }

    pub fn wire_phase(&self) -> OutputPhase {
        match self.phase {
            Phase::Running => OutputPhase::Running,
            Phase::Draining | Phase::ClearingIdle => OutputPhase::Draining,
            Phase::Cutting | Phase::ClearingCut => OutputPhase::Cutting,
            Phase::Idle => OutputPhase::Idle,
        }
    }

    fn transition(&mut self, phase: Phase, frame: u64) {
        self.phase = phase;
        let wire = self.wire_phase();
        if wire != self.latest_report_phase() {
            self.pending_report = Some((wire, frame));
        }
    }

    fn latest_report_phase(&self) -> OutputPhase {
        let queued = (self.stop_ack_len > 0)
            .then(|| {
                let tail = (self.stop_ack_head + self.stop_ack_len - 1) % CHANNEL_CAPACITY;
                self.stop_acks[tail]
            })
            .flatten();
        match (self.pending_report, queued) {
            (Some(pending), Some(queued)) if pending.1 > queued.1 => pending.0,
            (Some(_), Some(queued)) => queued.0,
            (Some(pending), None) => pending.0,
            (None, Some(queued)) => queued.0,
            (None, None) => self.last_reported,
        }
    }

    pub fn stop_acks_full(&self) -> bool {
        self.stop_ack_len == CHANNEL_CAPACITY
    }

    pub fn next_report(&self) -> Option<(ReportSource, OutputPhase, u64)> {
        let queued = if self.stop_ack_len > 0 {
            self.stop_acks[self.stop_ack_head]
        } else {
            None
        };
        match (self.pending_report, queued) {
            (Some(pending), Some(queued)) if pending.1 < queued.1 => {
                Some((ReportSource::PhaseChange, pending.0, pending.1))
            }
            (Some(_), Some(queued)) => Some((ReportSource::StopAck, queued.0, queued.1)),
            (Some(pending), None) => Some((ReportSource::PhaseChange, pending.0, pending.1)),
            (None, Some(queued)) => Some((ReportSource::StopAck, queued.0, queued.1)),
            (None, None) => None,
        }
    }

    pub fn consume_report(&mut self, source: ReportSource) {
        match source {
            ReportSource::PhaseChange => self.pending_report = None,
            ReportSource::StopAck => {
                if self.stop_ack_len == 0 {
                    return;
                }
                self.stop_acks[self.stop_ack_head] = None;
                self.stop_ack_head = (self.stop_ack_head + 1) % CHANNEL_CAPACITY;
                self.stop_ack_len -= 1;
            }
        }
    }

    fn push_stop_ack(&mut self, ack: (OutputPhase, u64)) {
        debug_assert!(
            !self.stop_acks_full(),
            "stop ack FIFO must be backpressured"
        );
        let tail = (self.stop_ack_head + self.stop_ack_len) % CHANNEL_CAPACITY;
        self.stop_acks[tail] = Some(ack);
        self.stop_ack_len += 1;
    }

    pub fn stop(&mut self, mode: OutputMode, frame: u64, sr: f32, fade_seconds: f32) {
        let pending_before_stop = self.pending_report;
        match mode {
            OutputMode::Gentle => match self.phase {
                Phase::Running | Phase::Draining => {
                    self.since_frame = frame;
                    self.below_frames = 0;
                    self.transition(Phase::Draining, frame);
                }
                Phase::Cutting | Phase::ClearingCut | Phase::ClearingIdle | Phase::Idle => {}
            },
            OutputMode::Cut if self.phase != Phase::ClearingCut => {
                let fade = (fade_seconds * sr).round().max(1.0);
                self.gate_step = self.gate / fade;
                self.transition(Phase::Cutting, frame);
            }
            OutputMode::Cut => {}
        }
        let phase = self.wire_phase();
        self.pending_report = pending_before_stop;
        self.push_stop_ack((phase, frame));
    }

    pub fn admit(&mut self, frame: u64) {
        match self.phase {
            Phase::Draining | Phase::ClearingIdle | Phase::Idle => {
                self.gate = 1.0;
                self.gate_step = 0.0;
                self.below_frames = 0;
                self.transition(Phase::Running, frame);
            }
            Phase::ClearingCut => self.admitted_during_cut_clear = true,
            Phase::Running | Phase::Cutting => {}
        }
    }

    pub fn skips_effects(&self) -> bool {
        matches!(
            self.phase,
            Phase::Idle | Phase::ClearingIdle | Phase::ClearingCut
        )
    }

    pub fn gate_sample(&mut self) -> f32 {
        let gain = self.gate;
        match self.phase {
            Phase::Cutting => {
                let next_gate = self.gate - self.gate_step;
                self.gate = if next_gate <= self.gate_step * 0.001 {
                    0.0
                } else {
                    next_gate
                };
            }
            Phase::Running if self.fade_in => {
                self.gate = (self.gate + self.gate_step).min(1.0);
                if self.gate >= 1.0 {
                    self.gate_step = 0.0;
                    self.fade_in = false;
                }
            }
            Phase::ClearingCut | Phase::ClearingIdle | Phase::Idle => self.gate = 0.0,
            _ => {}
        }
        gain
    }

    // Mirrors the engine-plan state inputs so render can compute them once.
    #[allow(clippy::too_many_arguments)]
    pub fn after_block(
        &mut self,
        peak: f32,
        voices_active: bool,
        song_active: bool,
        frame: u64,
        frames: usize,
        sample_rate: f32,
        config: &crate::dsp::ring::EngineConfig,
        clear_done: bool,
    ) {
        match self.phase {
            Phase::Draining => {
                if peak < config.silence_peak && !voices_active && !song_active {
                    self.below_frames = self.below_frames.saturating_add(frames as u64);
                } else {
                    self.below_frames = 0;
                }
                if self.below_frames >= (config.tail_hold_seconds * sample_rate).round() as u64 {
                    self.clear = ClearCursor::default();
                    self.transition(Phase::ClearingIdle, frame);
                } else if frame.saturating_sub(self.since_frame)
                    >= (config.drain_cap_seconds * sample_rate).round() as u64
                {
                    let fade = (config.cut_fade_seconds * sample_rate).round().max(1.0);
                    self.gate_step = self.gate / fade;
                    self.transition(Phase::Cutting, frame);
                }
            }
            Phase::Cutting if self.gate == 0.0 => {
                self.clear = ClearCursor::default();
                self.transition(Phase::ClearingCut, frame);
            }
            Phase::ClearingCut | Phase::ClearingIdle if clear_done => {
                if self.phase == Phase::ClearingCut && self.admitted_during_cut_clear {
                    self.admitted_during_cut_clear = false;
                    self.gate = 0.0;
                    self.gate_step = 1.0 / (config.cut_fade_seconds * sample_rate).round().max(1.0);
                    self.fade_in = true;
                    self.transition(Phase::Running, frame);
                } else {
                    self.gate = 0.0;
                    self.transition(Phase::Idle, frame);
                }
            }
            _ => {}
        }
    }
}
