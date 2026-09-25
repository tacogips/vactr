//! Sounds, the sound kit, SOUND FIRST, and `load`/`sample`/path/url typing
//! (7.1.4; ME-CHECK required tests).

use crate::types::diag::{DiagCode, Severity};
use crate::types::manifest::HostManifest;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Scheme, Ty};

use super::{assert_clean, assert_diags, assert_has, check_env, check_src, diag_lines, last_type};

#[test]
fn builtin_sound_keywords_are_checked() {
    assert_clean("s :bd > d1");
    assert_clean("s [:bd-haus :sn-dub] > d1");
    assert_diags("s :not-a-sound > d1", &["unknown-keyword@1"]);
    assert_has(
        "s :not-a-sound > d1",
        DiagCode::UnknownKeyword,
        1,
        Severity::Error,
    );
    // lang-reference section 4: the diagnostic is on the unknown step only.
    assert_diags("s [:bd-haus :not-a-sample] > d1", &["unknown-keyword@1"]);
    // Step lists and step constructors hold sound positions too.
    assert_diags("s [:bd {alt :sd :nope}] > d1", &["unknown-keyword@1"]);
    assert_clean("s [:bd :sd [:hh :hh] [:cp :cp] {alt :bd :sd} {maybe :bd} {euclid :bd 3 8} nil]");
    assert_clean("sound {choose :bd-haus :bd-tek} > d1");
    // `inst` names of the document are sounds.
    assert_clean("inst pluck2 amp: float = 0.5:\n\tsaw 440\ns :pluck2 > d1");
}

#[test]
fn sound_first_rejects_a_pattern_before_s() {
    assert_diags("n [0 3] > s :bd > d1", &["sound-not-first@1"]);
    assert_has(
        "n [0 3] > s :bd > d1",
        DiagCode::SoundNotFirst,
        1,
        Severity::Error,
    );
    assert_diags("note [:c] > s :x > d1", &["sound-not-first@1"]);
    // One non-sound pattern is an ordinary mismatch.
    let r = check_src("s {n [0 3]}");
    assert!(
        r.diags.iter().any(|d| d.code == DiagCode::TypeMismatch),
        "{:?}",
        r.diags
    );
    assert!(r.diags.iter().all(|d| d.code != DiagCode::SoundNotFirst));
    // `s` first, then structure-giving steps and sources.
    assert_clean("s :bd > n [0 3] > d1");
    assert_clean("s :pluck > midi-notes channel: 1 > d1");
}

#[test]
fn kit_argument_skips_the_static_check() {
    assert_clean(
        "let tr909 [bd909: {sample ./tr909/bd.wav} sd909: {sample ./tr909/sd.wav}]\ns [:bd909 :sd909] kit: tr909 > d1",
    );
    assert_clean("s :bd909 kit: [bd: {sample ./bd.wav}] > d1");
    assert_diags("s :bd kit: [bd: \"x\"] > d1", &["type-mismatch@1"]);
    assert_diags("s :bd foo: 1", &["type-mismatch@1"]);
}

#[test]
fn a_session_sound_kit_skips_the_static_check() {
    assert_diags(
        "let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]\ns :bd > n [0 3] > d1",
        &["shadows-prelude@1"],
    );
    // Rebinding `sound-kit` changes the key set `s` sees (late-bound), so
    // the checker cannot know the keys any more.
    assert_diags(
        "let sound-kit put default-sound-kit [zz: {sample ./zz.wav}]\ns :zz > d1",
        &["shadows-prelude@1"],
    );
    let mut env = CheckEnv::empty();
    env.globals.insert(
        "sound-kit".into(),
        GlobalInfo {
            kind: BindKind::Let,
            scheme: Some(Scheme::mono(Ty::Dict(
                Box::new(Ty::keyword()),
                Box::new(Ty::Sound),
            ))),
            mask: None,
            span: None,
        },
    );
    // A local `sound-kit` inside a `fn` does not affect `s` (7.1.4).
    assert_diags(
        "fn f sound-kit:\n\ts :nope",
        &["shadows-prelude@1", "unknown-keyword@2"],
    );
    let src = "s :from-a-pack > d1";
    let r = check_env(src, &env, &HostManifest::spec_default());
    assert_eq!(diag_lines(src, &r), Vec::<String>::new());
}

#[test]
fn sound_values_are_used_as_is() {
    assert_clean("let kick sample ./kick.wav\ns kick > d1");
    assert_clean("s {midi 1} > note [:c :e] > d1");
    assert_diags("midi 17", &["type-mismatch@1"]);
    assert_diags("s {midi 0} > d1", &["type-mismatch@1"]);
    assert_eq!(last_type("sample ./kick.wav"), "sound");
}

#[test]
fn a_sample_bank_is_accepted_where_a_sound_is_expected() {
    let pack = "[bd: [{sample ./a.wav} {sample ./b.wav}] sd: {sample ./sd.wav}]";
    assert_eq!(last_type(pack), "[keyword: sound]");
    assert_clean(&format!("let pack {pack}\ns :bd kit: pack > d1"));
    assert_clean("let bank [{sample ./a.wav} {sample ./b.wav}]\ns bank > n 1 > d1");
}

#[test]
fn load_and_sample_take_paths() {
    assert_clean("let my-pack load ./p.vact\nput default-sound-kit my-pack");
    assert_diags("load https://x.org/p.vact", &["type-mismatch@1"]);
    assert_diags("sample https://x.org/a.wav", &["type-mismatch@1"]);
    // Neither converts to or from a string.
    assert_diags("sample \"a.wav\"", &["type-mismatch@1"]);
    assert_diags("concat \"a\" ./b > sample", &["type-mismatch@1"]);
    assert_eq!(last_type("let pack-dir ./soundpack\npack-dir"), "path");
    assert_eq!(
        last_type("let remote https://example.org/packs/x.vact\nremote"),
        "url"
    );
}
