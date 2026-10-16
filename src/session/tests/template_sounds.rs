//! Regression coverage for realized template sounds (design-browser-tier-voices.md,
//! "Validation").

use std::collections::BTreeSet;
use std::rc::Rc;

use super::support::{Rig, DOC};
use crate::dsp::caps::CapabilitySet;
use crate::ns::insts::{InstRegistry, TEMPLATES, TEMPLATE_FILE};
use crate::ns::load::read_forms;
use crate::session::session::SessionConfig;
use crate::types::check::def_head;
use crate::types::manifest::HostManifest;
use crate::types::ty::KeySet;

fn template_names() -> BTreeSet<String> {
    let forms = read_forms(TEMPLATES, TEMPLATE_FILE).expect("prelude template forms");
    let names: BTreeSet<_> = forms
        .iter()
        .filter(|form| def_head(form) == Some("inst"))
        .filter_map(|form| form.children.get(1).and_then(|name| name.sym_name()))
        .map(|name| name.to_string())
        .collect();
    assert!(!names.is_empty(), "templates.vact has no inst forms");
    names
}

fn native_rig() -> Rig {
    Rig::with(SessionConfig::new(CapabilitySet::native()))
}

fn wasm_host_rig() -> Rig {
    // Mirror the browser-tier registry setup in host/wasm/session_half.rs.
    let registry = InstRegistry::shared();
    registry.borrow_mut().caps = CapabilitySet::browser();
    let mut config = SessionConfig::new(CapabilitySet::browser());
    config.insts = Some(Rc::clone(&registry));
    Rig::with(config)
}

fn unknown_keyword_count(diagnostics: &[crate::session::protocol::WireDiag]) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "unknown-keyword")
        .count()
}

#[test]
fn template_sounds_are_accepted_only_when_realized_in_each_tier() {
    let names = template_names();
    for (tier, make_rig) in [
        ("native", native_rig as fn() -> Rig),
        ("wasm-host", wasm_host_rig as fn() -> Rig),
    ] {
        let mut rig = make_rig();
        let registry = rig.s.evaluator().insts().expect("session registry");
        let registry = registry.borrow();
        assert!(
            registry.template_errors().is_empty(),
            "{tier}: {:?}",
            registry.template_errors()
        );
        let realized: BTreeSet<_> = registry
            .template_sounds_rc()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(realized, names, "{tier} realized template names");
        drop(registry);

        let mut revision = 1;
        for name in &names {
            let source = format!("s :{name} > d1");
            let (result, _) = rig.eval(&source, revision, 0);
            assert_eq!(
                unknown_keyword_count(&result.diagnostics),
                0,
                "{tier} eval rejected template {name}: {:?}",
                result.diagnostics
            );
            assert_eq!(
                unknown_keyword_count(&rig.s.check(DOC, &source)),
                0,
                "{tier} static check rejected template {name}"
            );
            revision += 1;
        }

        let unknown = "s :not-a-template-voice > d1";
        let (result, _) = rig.eval(unknown, revision, 0);
        assert_eq!(unknown_keyword_count(&result.diagnostics), 1, "{tier} eval");
        assert_eq!(
            unknown_keyword_count(&rig.s.check(DOC, unknown)),
            1,
            "{tier} static check"
        );

        for (name, source) in [
            (
                "acid-techno",
                include_str!("../../../examples/acid-techno.vact"),
            ),
            (
                "offbeat-bass-techno",
                include_str!("../../../examples/offbeat-bass-techno.vact"),
            ),
            (
                "rumble-techno",
                include_str!("../../../examples/rumble-techno.vact"),
            ),
            (
                "wobble-techno",
                include_str!("../../../examples/wobble-techno.vact"),
            ),
            (
                "bass-presets",
                include_str!("../../../examples/bass-presets.vact"),
            ),
            (
                "experimental-oscillators",
                include_str!("../../../examples/experimental-oscillators.vact"),
            ),
            (
                "hurdy-gurdy",
                include_str!("../../../examples/hurdy-gurdy.vact"),
            ),
            ("kalimba", include_str!("../../../examples/kalimba.vact")),
            (
                "tonewheel-organ",
                include_str!("../../../examples/tonewheel-organ.vact"),
            ),
        ] {
            let errors: Vec<_> = rig
                .s
                .check(DOC, source)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == "error")
                .collect();
            assert!(errors.is_empty(), "{tier} {name} static errors: {errors:?}");
        }
    }
}

#[test]
fn templates_stay_out_of_the_default_sound_kit() {
    let manifest = HostManifest::spec_default();
    assert_eq!(manifest.templates, KeySet::of([] as [&str; 0]));
    let KeySet::Of(keys) = manifest.sound_kit_keys() else {
        panic!("default sound kit should be closed");
    };
    assert_eq!(keys.len(), 20);
}
