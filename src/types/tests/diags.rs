//! One positive trigger per checker-emitted code of design 7 and 7.1.4,
//! each asserting code, severity and line (ME-CHECK required tests).

use std::collections::BTreeMap;

use crate::types::diag::{DiagCode, Severity};
use crate::types::manifest::HostManifest;
use crate::types::natives::HostCap;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Scheme, Ty};

use super::{assert_clean, assert_diags, assert_has, check_env, diag_lines, line_of};

use DiagCode as C;
use Severity::{Error as E, Hint as H, Warning as W};

/// `(source, code, line, severity)` for every code that needs no special
/// environment.
const TRIGGERS: &[(&str, DiagCode, usize, Severity)] = &[
    ("let x 1\n/ x 0", C::LiteralDivisionByZero, 2, E),
    ("+ 1 \"a\"", C::TypeMismatch, 1, E),
    ("let m: int \"a\"", C::AnnotationMismatch, 1, E),
    ("let d [gain: 0.5]\n+ {d :gain} 1", C::OptionalAsValue, 2, E),
    ("let x: any 5\n+ x 1", C::AnyNotNarrowed, 2, E),
    ("print nope", C::UndefinedName, 1, E),
    ("let a 1\nlet a 2", C::Rebinding, 2, E),
    ("let x 1\nfn f x:\n\tx", C::Shadowing, 2, W),
    ("let scale 2", C::ShadowsPrelude, 1, H),
    ("let a 1\nupd a 2", C::UpdImmutable, 2, E),
    ("s :nope > d1", C::UnknownKeyword, 1, E),
    (
        "enum drum:\n\tkick\n\tsnare\nfn f d:\n\tmatch d:\n\t\tkick -> 1",
        C::MissingVariant,
        5,
        E,
    ),
    ("n [0 3] > s :bd > d1", C::SoundNotFirst, 1, E),
    ("[amp: 1 amp: 2]", C::DuplicateKey, 1, W),
    (
        "s :bd > every 2 {p -> print p} > d1",
        C::EffectInPattern,
        1,
        W,
    ),
    ("for x 0..:\n\tprint x", C::UnboundedSource, 1, W),
    (
        "fn g v:\n\t+ v 1\nfn f b:\n\tat 4 b\n\tg b",
        C::MixedForcing,
        3,
        W,
    ),
    ("fn f h x:\n\th x\nf print {+ 1 2}", C::LatentForcing, 3, W),
    (
        "s :break > slice [0 0.5 0.3] [0] > d1",
        C::BadSlicePoints,
        1,
        E,
    ),
    ("use-clock :link", C::ClockSourceUnavailable, 1, E),
    ("s :piano > chord [:c :maj99] > d1", C::UnknownKeyword, 1, E),
];

#[test]
fn every_checker_code_has_a_trigger() {
    for (src, code, line, severity) in TRIGGERS {
        assert_has(src, *code, *line, *severity);
    }
}

#[test]
fn withdrawn_forms_are_undefined_names() {
    for word in ["while", "break", "when", "unless", "each"] {
        let src = format!("{word} true 1");
        assert_diags(&src, &["undefined-name@1"]);
    }
    // `loop` is the sampler template's header control (M3), not a language form.
    assert_diags("loop true 1", &["type-mismatch@1"]);
}

#[test]
fn clean_counterparts_do_not_trigger() {
    assert_clean("let x 2\n/ x 4");
    assert_clean("use-clock :internal\nuse-clock :midi");
    assert_clean("s :break > slice [0 0.31 0.5 0.8] [2 0] > d1");
    assert_clean("s :break > slice 8 [0 2 4 7] > d1");
    assert_clean("take 0.. 3");
    assert_clean("fn g v:\n\t+ v 1\nfn f b:\n\tg b\n\tg b");
    assert_clean("s :bd > every 4 {p -> fast p 2} > d1");
    assert_clean("enum drum:\n\tkick\n\tsnare\nfn f d:\n\tmatch d:\n\t\tkick -> 1\n\t\t_ -> 2");
}

#[test]
fn chord_qualities_are_the_letter_first_set() {
    assert_clean("s :piano > chord {alt [:c :maj7] [:d :m7] [:g :dom7] [:a :sus4]} > voicing > d1");
    assert_clean("s :piano > chord [:c :m7] > arp :up > d1");
    // The full vocabulary (2026-09-27): triads, sixths, sevenths, ninths,
    // elevenths, thirteenths, sus and added tones.
    assert_clean("s :piano > chord {alt [:c :maj] [:a :m] [:f :maj9] [:g :dom13] [:b :m7f5] [:e :dom7s9] [:d :sus2] [:c :add9]} > voicing > d1");
    assert_diags("s :piano > chord [:c :maj99] > d1", &["unknown-keyword@1"]);
    assert_diags(
        "s :piano > chord {alt [:c :maj7] [:g :seven]} > d1",
        &["unknown-keyword@1"],
    );
}

#[test]
fn bare_variant_binding_on_both_forms() {
    let enums = "enum opt:\n\tnone\n\tsome v\nlet s2 none\n";
    // The `if S P -> T` desugaring.
    let src = format!("{enums}if s2 none -> 1");
    assert_diags(&src, &["bare-variant-binding@5"]);
    assert_has(&src, C::BareVariantBinding, 5, E);
    // The hand-written identical match.
    let src = format!("{enums}match s2:\n\tfalse | nil -> 0\n\tnone -> 1");
    assert_diags(&src, &["bare-variant-binding@7"]);
    // A variant with fields binds, and a plain name binds: no diagnostic.
    assert_clean(&format!("{enums}if s2 some v -> v"));
    assert_clean(&format!("{enums}if s2 x -> x"));
    assert_clean(&format!("{enums}match s2:\n\tnone -> 0\n\t_ -> 1"));
}

#[test]
fn beyond_capability_follows_the_manifest() {
    let mut m = HostManifest::spec_default();
    m.caps.remove(&HostCap::MidiIn);
    let src = "cc 74";
    let r = check_env(src, &CheckEnv::empty(), &m);
    assert_eq!(diag_lines(src, &r), ["beyond-capability@1"]);
    assert_eq!(r.diags[0].severity, Severity::Error);
    let src = "s :bd > lpf {range {cc 74} 200 2000} > d1";
    let r = check_env(src, &CheckEnv::empty(), &HostManifest::spec_default());
    assert_eq!(diag_lines(src, &r), Vec::<String>::new());
}

#[test]
fn import_collision_warns_on_prelude_and_open_overlap() {
    let info = GlobalInfo {
        kind: BindKind::Fn,
        scheme: Some(Scheme::mono(Ty::Any)),
        mask: None,
        span: None,
    };
    let mut env = CheckEnv::empty();
    let pkg: BTreeMap<_, _> = [
        ("scale".into(), info.clone()),
        ("warm".into(), info.clone()),
    ]
    .into();
    env.qualified.insert("pads".into(), pkg);
    let other: BTreeMap<_, _> = [("warm".into(), info)].into();
    env.qualified.insert("keys".into(), other);
    let m = HostManifest::spec_default();
    let src = "import github.com/someone/vactrol-pads open";
    let r = check_env(src, &env, &m);
    assert_eq!(diag_lines(src, &r), ["import-collision@1"]);
    assert_eq!(r.diags[0].severity, Severity::Warning);
    let src =
        "import github.com/someone/vactrol-pads open\nimport github.com/someone/vactrol-keys open";
    let r = check_env(src, &env, &m);
    assert_eq!(
        diag_lines(src, &r),
        ["import-collision@1", "import-collision@2"]
    );
    assert_eq!(line_of(src, r.diags[1].span.start), 2);
    let src = "import github.com/someone/vactrol-pads\npads.warm\npads.nope";
    let r = check_env(src, &env, &m);
    assert_eq!(diag_lines(src, &r), ["undefined-name@3"]);
}
