//! `Ty::UGen`, the ugen input coercion, and the catalog: every ugen and
//! effect name has one table entry and one implementation (12.8.6).

use std::collections::BTreeSet;

use crate::dsp::build::{DSP_KEYWORDS, EXTRA_PARAMS, UGENS};
use crate::dsp::controls;
use crate::dsp::graph::EffectKind;
use crate::types::natives::NativeTable;
use crate::types::ty::{Scheme, Ty};
use crate::value::intern::intern_sym;
use crate::vm::natives::full_prelude;

use super::super::{assert_clean, check_src, last_type};

#[test]
fn ugen_type_notation_and_display() {
    let s = Scheme::parse("fn ugen -> ugen").expect("parses");
    assert_eq!(s.ty, Ty::func(vec![Ty::UGen], Ty::UGen));
    assert_eq!(Ty::UGen.to_string(), "ugen");
    assert_eq!(last_type("sin-osc 440"), "ugen");
    assert_eq!(last_type("env-adsr 0.01 0.1 0.8 0.3"), "ugen");
}

#[test]
fn a_ugen_input_takes_numbers_signals_and_ugens() {
    assert_clean("let a sin-osc 440");
    assert_clean("let b sin-osc 440.5");
    assert_clean("let c sin-osc sine");
    assert_clean("let d sin-osc {sin-osc 3}");
    let r = check_src("let e sin-osc \"x\"");
    assert!(!r.diags.is_empty(), "a string is not a ugen input");
}

#[test]
fn every_ugen_and_effect_has_one_entry_and_one_implementation() {
    let table = NativeTable::global();
    let p = full_prelude();
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let names = UGENS
        .iter()
        .map(|(n, _)| *n)
        .chain(EffectKind::ALL.iter().map(|k| k.name()))
        .chain(["bus", "master"]);
    for name in names {
        assert!(table.get(name).is_some(), "`{name}` has no table entry");
        assert!(
            p.slot(intern_sym(name)).is_some(),
            "`{name}` is not registered"
        );
    }
    assert_eq!(table.len(), table.name_count(), "names are unique");
}

#[test]
fn dsp_keywords_are_the_control_rows_and_the_extra_parameters() {
    let kws: BTreeSet<&str> = DSP_KEYWORDS.iter().copied().collect();
    assert_eq!(kws.len(), DSP_KEYWORDS.len(), "no duplicates");
    for p in EXTRA_PARAMS {
        assert!(kws.contains(p), "{p}");
        assert!(controls::row(p).is_none(), "{p} is a control row");
    }
    for k in &kws {
        assert!(
            controls::row(k).is_some() || EXTRA_PARAMS.contains(k),
            "{k} has no id"
        );
    }
}

#[test]
fn output_selection_has_ugen_type_and_checks_selector_arguments() {
    let selection = "let p {sin-osc 440}\n{p :main}";
    assert_clean(selection);
    assert_eq!(last_type(selection), "ugen");

    for source in [
        "let p {sin-osc 440}\n{p \"x\"}",
        "let p {sin-osc 440}\n{p :main :aux}",
    ] {
        let result = check_src(source);
        assert!(
            result
                .diags
                .iter()
                .any(|diag| diag.code == crate::types::diag::DiagCode::TypeMismatch),
            "{source}: {:?}",
            result.diags
        );
    }
}

#[test]
fn output_selection_accepts_an_unresolved_selector_parameter() {
    assert_clean("let p {sin-osc 440}\nfn select-output selector:\n\t{p selector}");
}
