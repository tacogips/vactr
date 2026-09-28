//! End-to-end two-output synthesis template tests.

use super::{all_finite, rms, E2e, HostManifest};

#[test]
fn filter_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!("s :filter-voice > note [:a3] > {control} > once"));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: event committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("morph 0.5 > timbre 0.5 > filter-harmonics 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "morph 0.9",
        "timbre 0.9",
        "filter-harmonics 0.9",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("filter-voice")
        .expect("filter-voice editor");
    for name in ["freq", "morph", "timbre", "filter-harmonics"] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"filter-voice")
        .expect("realized filter-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(
        template.has_aux,
        "prelude routes high-pass to auxiliary output"
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 4
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::VaFilter
        )));
}

#[test]
fn phase_pair_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :phase-pair-voice > note [:a3] > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("phase-harmonics 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "phase-harmonics 0.9",
        "timbre 0.9",
        "morph 0.9",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("phase-pair-voice")
        .expect("phase-pair-voice editor");
    for name in ["freq", "phase-harmonics", "timbre", "morph"] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"phase-pair-voice")
        .expect("realized phase-pair-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    assert!(
        crate::dsp::ugen::Template::from_inst(&entry.def, &env)
            .unwrap()
            .has_aux
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 4
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::PhasePair
        )));
}

#[test]
fn fm_pair_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!("s :fm-pair-voice > note [:a3] > {control} > once"));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("fm-harmonics 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "fm-harmonics 0.9",
        "timbre 0.9",
        "morph 0.1",
        "morph 0.9",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("fm-pair-voice")
        .expect("fm-pair-voice editor");
    for name in ["freq", "fm-harmonics", "timbre", "morph"] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"fm-pair-voice")
        .expect("realized fm-pair-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    assert!(
        crate::dsp::ugen::Template::from_inst(&entry.def, &env)
            .unwrap()
            .has_aux
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 4
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::FmPair
        )));
}

#[test]
fn spectrum_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :spectrum-voice > note [:a3] > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("timbre 0.5 > morph 0.5 > spectrum-bumps 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "timbre 0.9",
        "morph 0.9",
        "spectrum-bumps 0.9",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("spectrum-voice")
        .expect("spectrum-voice editor");
    for name in ["freq", "timbre", "morph", "spectrum-bumps"] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"spectrum-voice")
        .expect("realized spectrum-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(template.has_aux);
    assert_eq!(template.mem_total, 96);
    assert!(entry.def.edges.iter().any(|edge| edge.port == 4
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::SpectrumPair
        )));
}

#[test]
fn clock_noise_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :clock-noise-voice > note [:a3] > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("noise-harmonics 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "noise-harmonics 0.9",
        "timbre 0.9",
        "morph 0.9",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("clock-noise-voice")
        .expect("clock-noise-voice editor");
    for name in ["freq", "noise-harmonics", "timbre", "morph"] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"clock-noise-voice")
        .expect("realized clock-noise-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(template.has_aux);
    assert_eq!(
        template.mem_total, 0,
        "node state is inline and preallocated"
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 4
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::ClockNoisePair
        )));
}

#[test]
fn dual_kick_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :dual-kick-voice > note [:a3] > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("kick-harmonics 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "kick-harmonics 0.9",
        "timbre 0.9",
        "morph 0.9",
        "velocity 0.2",
        "kick-sustain 1",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("dual-kick-voice")
        .expect("dual-kick-voice editor");
    for name in [
        "freq",
        "kick-harmonics",
        "timbre",
        "morph",
        "velocity",
        "kick-sustain",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"dual-kick-voice")
        .expect("realized dual-kick-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(template.has_aux);
    assert_eq!(
        template.mem_total, 48,
        "both output states are preallocated"
    );
    let too_small = crate::dsp::ugen::BuildEnv {
        voice_mem: 47,
        ..env
    };
    assert_eq!(
        crate::dsp::ugen::Template::from_inst(&entry.def, &too_small).unwrap_err(),
        crate::dsp::ugen::BuildError::MemExceeded
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 5
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::DualKick
        )));
}

#[test]
fn dual_kick_two_scheduled_hits_retrigger_the_audio_paths() {
    fn run(notes: &str) -> (Vec<f32>, usize) {
        let mut e = E2e::new();
        e.eval(&format!("s :dual-kick-voice > note {notes} > once"));
        let out = e.run_for(1.1);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        (out, e.committed)
    }
    let (single, single_count) = run("[:a3]");
    let (double, double_count) = run("[:a3 :a3]");
    assert_eq!(single_count, 1);
    assert_eq!(double_count, 2);
    let start = 48_000;
    let end = 50_400;
    let single_rms = rms(&single[start..end]);
    let double_rms = rms(&double[start..end]);
    assert!(
        double_rms > single_rms * 1.2,
        "second scheduled hit excites a fresh pulse: {double_rms} vs {single_rms}"
    );
}

#[test]
fn dual_snare_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :dual-snare-voice > note [:a3] > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("snare-harmonics 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "snare-harmonics 0.9",
        "timbre 0.9",
        "morph 0.9",
        "velocity 0.2",
        "snare-sustain 1",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("dual-snare-voice")
        .expect("dual-snare-voice editor");
    for name in [
        "freq",
        "snare-harmonics",
        "timbre",
        "morph",
        "velocity",
        "snare-sustain",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"dual-snare-voice")
        .expect("realized dual-snare-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(template.has_aux);
    assert_eq!(
        template.mem_total, 48,
        "both output states are preallocated"
    );
    let too_small = crate::dsp::ugen::BuildEnv {
        voice_mem: 47,
        ..env
    };
    assert_eq!(
        crate::dsp::ugen::Template::from_inst(&entry.def, &too_small).unwrap_err(),
        crate::dsp::ugen::BuildError::MemExceeded
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 5
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::SnarePair
        )));
}

#[test]
fn dual_snare_two_scheduled_hits_retrigger_the_audio_paths() {
    fn run(notes: &str) -> (Vec<f32>, usize) {
        let mut e = E2e::new();
        e.eval(&format!("s :dual-snare-voice > note {notes} > once"));
        let out = e.run_for(1.1);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        (out, e.committed)
    }
    let (single, single_count) = run("[:a3]");
    let (double, double_count) = run("[:a3 :a3]");
    assert_eq!(single_count, 1);
    assert_eq!(double_count, 2);
    let start = 48_000;
    let end = 50_400;
    let single_rms = rms(&single[start..end]);
    let double_rms = rms(&double[start..end]);
    assert!(
        double_rms > single_rms * 1.2,
        "second scheduled hit excites a fresh pulse: {double_rms} vs {single_rms}"
    );
}

#[test]
fn dual_hat_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :dual-hat-voice > note [:a3] > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("hat-harmonics 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "hat-harmonics 0.9",
        "timbre 0.9",
        "morph 0.9",
        "velocity 0.2",
        "hat-sustain 1",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("dual-hat-voice")
        .expect("dual-hat-voice editor");
    for name in [
        "freq",
        "hat-harmonics",
        "timbre",
        "morph",
        "velocity",
        "hat-sustain",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"dual-hat-voice")
        .expect("realized dual-hat-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(template.has_aux);
    assert_eq!(
        template.mem_total, 32,
        "both output states are preallocated"
    );
    let too_small = crate::dsp::ugen::BuildEnv {
        voice_mem: 31,
        ..env
    };
    assert_eq!(
        crate::dsp::ugen::Template::from_inst(&entry.def, &too_small).unwrap_err(),
        crate::dsp::ugen::BuildError::MemExceeded
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 5
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::HatPair
        )));
}

#[test]
fn dual_hat_two_scheduled_hits_retrigger_the_audio_paths() {
    fn run(notes: &str) -> (Vec<f32>, usize) {
        let mut e = E2e::new();
        e.eval(&format!("s :dual-hat-voice > note {notes} > once"));
        let out = e.run_for(1.1);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        (out, e.committed)
    }
    let (single, single_count) = run("[:a3]");
    let (double, double_count) = run("[:a3 :a3]");
    assert_eq!(single_count, 1);
    assert_eq!(double_count, 2);
    let start = 48_000;
    let end = 50_400;
    let single_rms = rms(&single[start..end]);
    let double_rms = rms(&double[start..end]);
    assert!(
        double_rms > single_rms * 1.2,
        "second scheduled hit excites a fresh pulse: {double_rms} vs {single_rms}"
    );
}

#[test]
fn swarm_voice_controls_and_note_reach_audio_and_editor() {
    fn render(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!("s :swarm-voice > note [:a3] > {control} > once"));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{control}: audible");
        output
    }

    let base = render("swarm-spread 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "swarm-spread 0.9",
        "timbre 0.9",
        "morph 0.9",
        "swarm-continuous 1",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{control}: response {difference}");
    }

    let editor = HostManifest::spec_default()
        .editor_decl("swarm-voice")
        .expect("swarm-voice editor");
    for name in [
        "freq",
        "swarm-spread",
        "timbre",
        "morph",
        "swarm-continuous",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }

    let e = E2e::new();
    let reg = e.reg.borrow();
    let entry = reg
        .entries()
        .find(|entry| *crate::value::intern::name_of_kw(entry.name) == *"swarm-voice")
        .expect("realized swarm-voice");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: crate::dsp::caps::CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = crate::dsp::ugen::Template::from_inst(&entry.def, &env).unwrap();
    assert!(template.has_aux);
    assert_eq!(
        template.mem_total, 224,
        "both output states are preallocated"
    );
    let too_small = crate::dsp::ugen::BuildEnv {
        voice_mem: 223,
        ..env
    };
    assert_eq!(
        crate::dsp::ugen::Template::from_inst(&entry.def, &too_small).unwrap_err(),
        crate::dsp::ugen::BuildError::MemExceeded
    );
    assert!(entry.def.edges.iter().any(|edge| edge.port == 4
        && matches!(
            entry.def.nodes[usize::from(edge.from)],
            crate::dsp::graph::UGenSpec::Const(1.0)
        )
        && matches!(
            entry.def.nodes[usize::from(edge.to)],
            crate::dsp::graph::UGenSpec::SwarmPair
        )));
}

#[test]
fn swarm_two_scheduled_hits_retrigger_the_audio_paths() {
    fn run(notes: &str) -> (Vec<f32>, usize) {
        let mut e = E2e::new();
        e.eval(&format!("s :swarm-voice > note {notes} > once"));
        let out = e.run_for(1.1);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        (out, e.committed)
    }
    let (single, single_count) = run("[:a3]");
    let (double, double_count) = run("[:a3 :a3]");
    assert_eq!(single_count, 1);
    assert_eq!(double_count, 2);
    let start = 48_000;
    let end = 50_400;
    let single_rms = rms(&single[start..end]);
    let double_rms = rms(&double[start..end]);
    assert!(
        double_rms > single_rms * 1.2,
        "second scheduled hit excites a fresh pulse: {double_rms} vs {single_rms}"
    );
}
