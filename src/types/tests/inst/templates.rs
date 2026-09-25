//! The seven prelude templates are `.vact` source realized to `InstDef`s at
//! session start; their parameters are controls (design 12.4, 12.8.6,
//! TASK-008 criterion 2, compile half).

use crate::dsp::controls;
use crate::dsp::graph::UGenSpec;
use crate::host::caps::GraphHandle;
use crate::host::wire::Ctl;
use crate::ns::insts::{TEMPLATES, TEMPLATE_NAMES};
use crate::ns::stage::StagedEffect;
use crate::value::intern::intern_kw;

use super::{kinds, param, Session};

#[test]
fn seven_templates_realize_at_session_start() {
    let s = Session::new();
    assert!(
        s.reg.borrow().template_errors().is_empty(),
        "{:?}",
        s.reg.borrow().template_errors()
    );
    for name in TEMPLATE_NAMES {
        assert!(
            TEMPLATES.contains(&format!("inst {name} ")),
            "{name} is prelude source"
        );
        let d = s.def(name);
        assert_eq!(kinds(&d).last().map(String::as_str), Some("mul"), "{name}");
    }
    let installs = s
        .effects
        .borrow()
        .iter()
        .filter(|e| matches!(e, StagedEffect::Install(GraphHandle::Inst { .. })))
        .count();
    assert_eq!(installs, TEMPLATE_NAMES.len());
}

#[test]
fn each_template_is_built_from_its_model_ugens() {
    let s = Session::new();
    let has = |name: &str, want: &[&str]| {
        let k = kinds(&s.def(name));
        for w in want {
            assert!(k.contains(&(*w).to_string()), "{name}: {w} not in {k:?}");
        }
    };
    // `n` (like `bank`) is a voice-level resource-selection argument of
    // `sample-play` (R2a): skipped before it becomes a node, so it is
    // never a `param:n` node — the scheduler selects it directly.
    has("sampler", &["sampleplay", "envperc", "param:speed"]);
    assert!(
        !kinds(&s.def("sampler")).contains(&"param:n".to_string()),
        "n is a voice-level argument, not a node"
    );
    has(
        "analog",
        &["vco", "subosc", "ladder", "envadsr", "param:freq"],
    );
    has("fm", &["fmop", "fmmod", "envperc", "envadsr"]);
    has("pd", &["phasedistortion", "envperc"]);
    has("additive", &["additive", "envadsr"]);
    has("wavetable", &["wavetable", "svf", "envadsr"]);
    has("granular", &["granular", "envadsr", "param:pitch"]);
    let analog = s.def("analog");
    assert!(analog
        .nodes
        .iter()
        .any(|n| matches!(n, UGenSpec::Vco { unison_max: 16 })));
    let additive = s.def("additive");
    assert!(additive
        .nodes
        .iter()
        .any(|n| matches!(n, UGenSpec::Additive { partials_max: 5 })));
    assert_eq!(
        additive.node_params.len(),
        5,
        "the partials are node constants"
    );
}

#[test]
fn template_parameters_are_controls() {
    let s = Session::new();
    let analog = s.def("analog");
    assert_eq!(param(&analog, "cutoff"), Some(Ctl::Const(1200.0)));
    assert_eq!(param(&analog, "res"), Some(Ctl::Const(0.3)));
    assert_eq!(param(&analog, "unison"), Some(Ctl::Const(1.0)));
    assert_eq!(param(&analog, "detune"), Some(Ctl::Const(0.1)));
    assert_eq!(param(&analog, "wave"), Some(Ctl::Const(0.0)));
    let granular = s.def("granular");
    assert_eq!(param(&granular, "density"), Some(Ctl::Const(24.0)));
    assert_eq!(param(&granular, "envelope"), Some(Ctl::Const(0.0)));
    assert_eq!(param(&granular, "freeze"), Some(Ctl::Const(0.0)));
    // Every header parameter of every template is a control-table row, so
    // a pattern control of the same name reaches it (12.8.7).
    for name in TEMPLATE_NAMES {
        for (c, _) in s.def(name).params.iter() {
            assert!(controls::row_by_id(*c).is_some(), "{name}: {c:?}");
        }
    }
    assert!(s.reg.borrow().id_of(intern_kw("pd")).is_some());
}
