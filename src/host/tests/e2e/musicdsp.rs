//! New MusicDSP families through actual Vactr bus and instrument definitions.
use super::{rms, E2e};
use crate::dsp::graph::EffectKind;
use crate::types::manifest::HostManifest;

#[test]
fn musicdsp_all_effect_names_and_parameters_have_metadata_and_evaluate_on_buses() {
    let names = [
        "svf-filter",
        "butterworth-filter",
        "chebyshev-filter",
        "ladder-filter",
        "allpass-filter",
        "parametric-resonator",
        "feedback-resonator",
        "foldback",
        "variable-clip",
        "alien-wah",
        "dynamic-convolution",
        "early-reflections",
        "schroeder-reverb",
        "spring-reverb",
        "space-reverb",
        "shimmer-reverb",
        "tape-delay",
        "diffusion-delay",
    ];
    for name in names {
        let kind = EffectKind::from_name(name).unwrap();
        let manifest = HostManifest::spec_default();
        let editor = manifest.editor_decl(name).unwrap();
        for param in crate::dsp::effects::params(kind) {
            let declaration = editor.params.iter().find(|p| p.name == param.name).unwrap();
            assert_eq!(declaration.range, (param.min, param.max));
            let expected_unit = match param.unit {
                "dB" => crate::dsp::meta::Unit::Db,
                "Hz" => crate::dsp::meta::Unit::Hz,
                "s" => crate::dsp::meta::Unit::Seconds,
                "semitones" => crate::dsp::meta::Unit::Semitones,
                "m" => crate::dsp::meta::Unit::Metres,
                _ => crate::dsp::meta::Unit::None,
            };
            assert_eq!(
                declaration.unit, expected_unit,
                "{name}/{} typed unit",
                param.name
            );
        }
        let mut rig = E2e::new();
        rig.eval(&format!(
            "bus :lab:\n\t{name}\ns :analog > note [:c4] > gain 0.15 > bus :lab > once"
        ));
        let (l, r) = rig.run_stereo_for(0.55);
        assert!(rig.faults.is_empty(), "{name}: {:?}", rig.faults);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()), "{name}");
        assert!(rms(&l) > 1e-5, "{name} audible");
    }
}
#[test]
fn musicdsp_four_new_algorithms_evaluate_in_instruments() {
    for name in [
        "foldback",
        "variable-clip",
        "alien-wah",
        "dynamic-convolution",
    ] {
        let mut rig = E2e::new();
        rig.eval(&format!(
            "inst effect-voice:\n\tsin-osc 440 > {name}\ns :effect-voice > gain 0.15 > once"
        ));
        let audio = rig.run_for(0.4);
        assert!(rig.faults.is_empty(), "{name}: {:?}", rig.faults);
        assert!(rms(&audio) > 1e-5, "{name}");
    }
}

#[test]
fn musicdsp_all_labs_evaluate_install_and_render_meaningful_audio() {
    let mut paths = vec![
        "filter-lab.vact".to_owned(),
        "resonator-lab.vact".to_owned(),
        "reverb-delay-lab.vact".to_owned(),
    ];
    for name in [
        "svf-filter",
        "butterworth-filter",
        "chebyshev-filter",
        "ladder-filter",
        "allpass-filter",
        "parametric-resonator",
        "feedback-resonator",
        "foldback",
        "variable-clip",
        "alien-wah",
        "dynamic-convolution",
        "early-reflections",
        "schroeder-reverb",
        "spring-reverb",
        "space-reverb",
        "shimmer-reverb",
        "tape-delay",
        "diffusion-delay",
        "comb-feedback",
        "comb-feedforward",
        "comb-allpass",
    ] {
        paths.push(format!("effect-labs/{name}.vact"));
    }
    for path in paths {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples")
                .join(&path),
        )
        .unwrap();
        let mut rig = E2e::new();
        rig.eval(&source);
        let (l, r) = rig.run_stereo_for(1.1);
        assert!(rig.faults.is_empty(), "{path}: {:?}", rig.faults);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()), "{path}");
        assert!(
            rms(&l) > 1e-5 && rms(&r) > 1e-5,
            "{path} meaningful stereo output"
        );
        assert!(
            rig.reg
                .borrow()
                .bus(crate::value::intern::intern_kw("audition"))
                .is_some(),
            "{path} installed effect bus"
        );
    }
}
