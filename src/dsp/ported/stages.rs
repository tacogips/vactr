//! Stages ProcessFn inventory at the pinned Eurorack revision.
//! Runnable roles are analytic Vactrol adaptations, not source ports.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageCoverage {
    Adaptation,
    Pending,
    Excluded,
}

pub const STAGES_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageFunctionSpec {
    pub cell: u8,
    pub source_function: &'static str,
    pub role: &'static str,
    pub coverage: StageCoverage,
    pub vactrol_template: Option<&'static str>,
}

const fn cell(
    cell: u8,
    source_function: &'static str,
    role: &'static str,
    runnable: bool,
) -> StageFunctionSpec {
    StageFunctionSpec {
        cell,
        source_function,
        role,
        coverage: if runnable {
            StageCoverage::Adaptation
        } else if cell == 0 || cell == 12 || cell == 19 {
            StageCoverage::Excluded
        } else {
            StageCoverage::Pending
        },
        vactrol_template: if runnable { Some("stage-voice") } else { None },
    }
}

/// Source order is type (ramp, step, hold, alternate) by trigger/loop flags.
/// Zero entries are intentionally non-runnable source cells.
pub const STAGES_CELLS: [StageFunctionSpec; 16] = [
    cell(0, "Zero", "silent ramp cell", false),
    cell(1, "FreeRunningLFO", "free LFO", true),
    cell(2, "DecayEnvelope", "decay envelope", true),
    cell(3, "TapLFO", "clocked LFO", true),
    cell(4, "Portamento", "slewed step", true),
    cell(5, "Portamento", "slewed step", true),
    cell(6, "SampleAndHold", "sample/hold", true),
    cell(7, "SampleAndHold", "sample/hold", true),
    cell(8, "Delay", "CV delay and phase", true),
    cell(9, "Delay", "CV delay and phase", true),
    cell(10, "TimedPulseGenerator", "timed pulse", true),
    cell(11, "GateGenerator", "gate", true),
    cell(12, "Zero", "silent alternate cell", false),
    cell(13, "FreeRunningOscillator", "audio oscillator", true),
    cell(14, "DecayEnvelope", "decay envelope", true),
    cell(15, "PLLOscillator", "clocked audio oscillator", true),
];

pub const STAGES_OTHER: [StageFunctionSpec; 5] = [
    StageFunctionSpec {
        cell: 16,
        source_function: "MultiSegment",
        role: "one six-segment module with value/phase outputs",
        coverage: StageCoverage::Adaptation,
        vactrol_template: Some("stage-chain-voice"),
    },
    StageFunctionSpec {
        cell: 17,
        source_function: "Sequencer",
        role: "immutable director-plus-step list; seven event-local traversal modes, value/position outputs",
        coverage: StageCoverage::Adaptation,
        vactrol_template: Some("stage-sequencer-voice"),
    },
    cell(18, "Slave", "linked slave segment", false),
    cell(
        19,
        "ClockedSampleAndHold",
        "commented-out registration",
        false,
    ),
    StageFunctionSpec {
        cell: 20,
        source_function: "MultiSegment",
        role: "immutable 1–36 segment event-local chain; source serial grouping and slave remain pending",
        coverage: StageCoverage::Adaptation,
        vactrol_template: Some("stage-linked-voice"),
    },
];

pub const fn stages_cells() -> &'static [StageFunctionSpec; 16] {
    &STAGES_CELLS
}

pub fn stages_coverage_summary() -> String {
    let runnable = STAGES_CELLS
        .iter()
        .filter(|row| row.coverage == StageCoverage::Adaptation)
        .count();
    format!("Stages: {runnable}/16 registered ProcessFn cells, one six-scalar chain, one opt-in immutable 1–36 segment chain and a source-shaped seven-mode Sequencer have original Vactrol adaptations; two silent Zero cells are excluded; source serial grouping and Slave remain pending")
}
