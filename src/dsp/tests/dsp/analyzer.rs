//! TASK-008 criterion 5 (render half): analyzers are audio-transparent —
//! output with and without an analyzer is bit-identical — they write their
//! cells, and an analyzer over the cell budget clamps and counts while the
//! audio continues. (The diagnostic half is BE-SCHED's.)

use super::{bus_def, chain, ctl, event, NativeRig};
use crate::dsp::effects::analyzer::{cells, BANDS};
use crate::dsp::effects::catalog::spec;
use crate::dsp::graph::{AnalyzerKind, EffectKind, UGenSpec};
use crate::host::wire::{Ctl, HostMsg};

fn source(rig: &mut NativeRig, node: UGenSpec, freq: f32) {
    rig.install(&chain(1, vec![node]));
    let _ = rig.step();
    let t = rig.engine.now();
    rig.send(event(
        1,
        t,
        &[(ctl::FREQ, freq), (ctl::LEGATO, 10.0), (ctl::PAN, 0.3)],
    ));
}

fn render(
    analyzer: Option<(AnalyzerKind, f32)>,
    node: UGenSpec,
    freq: f32,
) -> (NativeRig, Vec<f32>) {
    let mut rig = NativeRig::native();
    if let Some((kind, id)) = analyzer {
        let mut named = vec![("id", Ctl::Const(id))];
        if kind == AnalyzerKind::Level {
            named.push(("window", Ctl::Const(0.01)));
        }
        let s = spec(EffectKind::Analyzer(kind), &named).unwrap();
        rig.install_bus(&bus_def(0, vec![s]), true);
    }
    source(&mut rig, node, freq);
    let (l, r) = rig.run(40);
    let mut both = l;
    both.extend(r);
    (rig, both)
}

#[test]
fn every_analyzer_is_bit_transparent() {
    let (_, plain) = render(None, UGenSpec::WhiteNoise, 440.0);
    for &kind in AnalyzerKind::ALL {
        let (rig, tapped) = render(Some((kind, 100.0)), UGenSpec::WhiteNoise, 440.0);
        assert!(plain.iter().any(|v| *v != 0.0));
        assert_eq!(plain, tapped, "{} altered the audio", kind.name());
        let n = cells(kind);
        let written = &rig.engine.analysis()[100..100 + n];
        assert!(
            written.iter().any(|v| *v != 0.0),
            "{} wrote its cells",
            kind.name()
        );
    }
}

#[test]
fn level_and_pitch_read_the_signal() {
    let (rig, _) = render(Some((AnalyzerKind::Level, 0.0)), UGenSpec::Const(1.0), 0.0);
    let (g, _) = crate::dsp::effects::prim::pan_gains(0.3);
    let rms = rig.engine.analysis()[0];
    let want = 0.5 * (g + (0.3f32 * std::f32::consts::FRAC_PI_2).sin());
    assert!((rms - want).abs() < 0.02, "rms {rms} vs {want}");
    let (rig, _) = render(
        Some((AnalyzerKind::PitchMeter, 0.0)),
        UGenSpec::SinOsc,
        1000.0,
    );
    let f = rig.engine.analysis()[0];
    assert!((f - 1000.0).abs() < 50.0, "pitch {f}");
    let (rig, _) = render(
        Some((AnalyzerKind::Spectrum, 0.0)),
        UGenSpec::SinOsc,
        1000.0,
    );
    let bands = &rig.engine.analysis()[..BANDS];
    let loudest = bands
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
        .unwrap();
    // 1 kHz is bin 10.7 of 256; band b spans bins 256^(b/32)..256^((b+1)/32).
    assert!(
        (12..=15).contains(&loudest),
        "1 kHz lands in band 13-14 ({loudest})"
    );
}

#[test]
fn analyzer_over_the_cell_budget_clamps_counts_and_keeps_the_audio() {
    let (_, plain) = render(None, UGenSpec::WhiteNoise, 440.0);
    let (rig, tapped) = render(
        Some((AnalyzerKind::Spectrogram, 1000.0)),
        UGenSpec::WhiteNoise,
        440.0,
    );
    assert_eq!(plain, tapped, "no dropout, no change");
    assert!(rig.engine.counters().fx.clamped > 0, "counted");
}

#[test]
fn analysis_cells_are_published() {
    let (mut rig, _) = render(Some((AnalyzerKind::Level, 5.0)), UGenSpec::SinOsc, 220.0);
    // Cells are published 16 per process, cycling through the table.
    let _ = rig.run(80);
    let acks = rig.acks();
    assert!(acks
        .iter()
        .any(|m| matches!(m, HostMsg::AnalysisCell { id: 5, value } if *value > 0.0)));
    assert!(rig.engine.host_sigs().amp > 0.0);
    assert!(rig.engine.host_sigs().fft.iter().any(|b| *b > 0.0));
}
