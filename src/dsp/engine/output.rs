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
        if wire != self.last_reported {
            self.pending_report = Some((wire, frame));
        }
    }

    pub fn stop(&mut self, mode: OutputMode, frame: u64, sr: f32, fade_seconds: f32) {
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
        self.pending_report = Some((self.wire_phase(), frame));
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
