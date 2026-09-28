//! Internal Elements-role instrument: all Patch controls are codeable.

use super::{all_finite, rms, E2e};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{elements_internal, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

const PATCH: [&str; 20] = [
    "el-env-shape",
    "el-bow-level",
    "el-bow-timbre",
    "el-blow-level",
    "el-blow-meta",
    "el-blow-timbre",
    "el-strike-level",
    "el-strike-meta",
    "el-strike-timbre",
    "el-signature",
    "el-geometry",
    "el-brightness",
    "el-damping",
    "el-position",
    "el-res-mod-frequency",
    "el-res-mod-offset",
    "el-reverb-diffusion",
    "el-reverb-lp",
    "el-space",
    "el-modulation-frequency",
];

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :elements-voice > note [:a3] > el-bow-level 0.3 > el-blow-level 0.3 > el-strike-level 0.6 > el-modulation 0.4 > {extra} > once"));
    let output = e.run_for(0.3);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0, "{extra}");
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-6, "{extra}");
    output
}

#[test]
fn all_patch_controls_and_three_models_change_audio() {
    let base = render("el-model 0");
    for model in 1..=2 {
        let output = render(&format!("el-model {model}"));
        assert_ne!(base, output, "model {model}");
    }
    for control in PATCH {
        let output = render(&format!("el-model 0 > {control} 0.9"));
        let difference: f32 = base.iter().zip(&output).map(|(a, b)| (a - b).abs()).sum();
        assert!(difference > 1.0e-6, "{control}: {difference}");
    }
    for (name, value) in [
        ("el-gate", 0.0),
        ("el-note", 12.0),
        ("el-modulation", 0.9),
        ("el-strength", 0.2),
    ] {
        let output = render(&format!("el-model 0 > {name} {value}"));
        assert_ne!(base, output, "{name}");
    }
}

#[test]
fn alternate_and_extended_space_are_codeable() {
    let normal = render("el-alternate 0");
    let alternate = render("el-alternate 1");
    assert_ne!(normal, alternate);
    let frozen = render("el-alternate 1 > el-space 1.9");
    assert_ne!(alternate, frozen);
}

#[test]
fn editor_exposes_last_port_and_install_budget_is_explicit() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"elements-voice")
        .unwrap();
    let editor = instrument_decls(&registry)
        .into_iter()
        .find(|decl| decl.name == "elements-voice")
        .unwrap();
    for name in PATCH.into_iter().chain([
        "el-gate",
        "el-note",
        "el-modulation",
        "el-strength",
        "el-model",
        "el-alternate",
    ]) {
        assert!(
            editor
                .params
                .iter()
                .any(|p| p.name == name && p.ctl.is_some()),
            "{name}"
        );
    }
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let memory = 2 * elements_internal::mem_len(sr);
        let env = BuildEnv {
            sr,
            caps: CapabilitySet::native(),
            voice_mem: memory,
        };
        let template = Template::from_inst(&entry.def, &env).unwrap();
        assert_eq!(template.mem_total, memory);
        assert!(template.has_aux);
        let too_small = BuildEnv {
            voice_mem: memory - 1,
            ..env
        };
        assert!(matches!(
            Template::from_inst(&entry.def, &too_small),
            Err(BuildError::MemExceeded)
        ));
    }
}
