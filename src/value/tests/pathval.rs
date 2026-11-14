//! Path, url and sound values (design 6.5.8).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::reader::span::FileId;
use crate::value::value::{PathVal, Sound};
use crate::value::{deep_eq, get, index, intern_kw, len, put, truthy, Key, Value};
use crate::vm::fail::FailCode;

const FILE: FileId = FileId::new(1);
const OTHER: FileId = FileId::new(2);

fn sound(s: Sound) -> Value {
    Value::Sound(Rc::new(s))
}

fn sample(text: &str, file: Option<FileId>) -> Sound {
    Sound::Sample(PathVal {
        text: Rc::from(text),
        file,
    })
}

fn eq(a: &Value, b: &Value) -> bool {
    deep_eq(a, b).expect("comparable")
}

#[test]
fn path_equality_uses_text_and_file() {
    let a = Value::path("./bd/1.wav", Some(FILE));
    assert!(eq(&a, &Value::path("./bd/1.wav", Some(FILE))));
    assert!(!eq(&a, &Value::path("./bd/1.wav", Some(OTHER))));
    assert!(!eq(&a, &Value::path("./bd/2.wav", Some(FILE))));
    let abs = Value::path("/abs", None);
    assert!(eq(&abs, &abs));
    assert!(eq(&abs, &Value::path("/abs", None)));
    // No normalization: `./a/../b` is not `./b`.
    assert!(!eq(
        &Value::path("./a/../b", Some(FILE)),
        &Value::path("./b", Some(FILE))
    ));
}

#[test]
fn url_and_sound_equality() {
    let u = Value::url("https://x.org/a");
    assert!(eq(&u, &Value::url("https://x.org/a")));
    assert!(!eq(&u, &Value::url("https://x.org/b")));
    let bd = sound(Sound::Builtin(intern_kw("bd")));
    assert!(eq(&bd, &sound(Sound::Builtin(intern_kw("bd")))));
    assert!(!eq(&bd, &sound(Sound::Builtin(intern_kw("sd")))));
    assert!(eq(&sound(Sound::MidiOut(1)), &sound(Sound::MidiOut(1))));
    assert!(!eq(&sound(Sound::MidiOut(1)), &sound(Sound::MidiOut(2))));
    assert!(eq(
        &sound(sample("./bd/1.wav", Some(FILE))),
        &sound(sample("./bd/1.wav", Some(FILE)))
    ));
    assert!(!eq(
        &sound(sample("./bd/1.wav", Some(FILE))),
        &sound(sample("./bd/1.wav", Some(OTHER)))
    ));
}

#[test]
fn cross_kind_values_are_never_equal() {
    let p = Value::path("/a", None);
    let u = Value::url("https://a");
    let s = sound(sample("/a", None));
    assert!(!eq(&p, &u));
    assert!(!eq(&p, &s));
    assert!(!eq(&u, &s));
    assert!(!eq(&p, &Value::str("/a")));
    assert!(!eq(&u, &Value::str("https://a")));
    assert!(!eq(
        &sound(Sound::Builtin(intern_kw("bd"))),
        &Value::kw("bd")
    ));
    assert!(!eq(&p, &Value::Nil));
}

#[test]
fn print_forms() {
    assert_eq!(
        Value::path("./soundpack/bd/1.wav", Some(FILE)).to_string(),
        "./soundpack/bd/1.wav"
    );
    assert_eq!(Value::path("~/kits/a", None).to_string(), "~/kits/a");
    assert_eq!(
        Value::url("https://example.org/packs/x.vact").to_string(),
        "https://example.org/packs/x.vact"
    );
    assert_eq!(
        sound(Sound::Builtin(intern_kw("bd"))).to_string(),
        "(sound :bd)"
    );
    assert_eq!(
        sound(sample("./bd/1.wav", Some(FILE))).to_string(),
        "(sound ./bd/1.wav)"
    );
    assert_eq!(sound(Sound::MidiOut(1)).to_string(), "(sound midi 1)");
    let l = Value::list(vec![
        Value::path("./a", Some(FILE)),
        Value::url("https://x"),
        sound(Sound::MidiOut(16)),
    ]);
    assert_eq!(l.to_string(), "[./a https://x (sound midi 16)]");
}

#[test]
fn accessors_treat_them_as_scalars() {
    let values = [
        Value::path("./a", Some(FILE)),
        Value::url("https://x.org"),
        sound(Sound::MidiOut(1)),
    ];
    for v in &values {
        assert!(truthy(v), "{v}");
        assert_eq!(len(v).unwrap_err().code, FailCode::Type, "{v}");
        assert_eq!(
            index(v, &Value::Int(0)).unwrap_err().code,
            FailCode::Type,
            "{v}"
        );
        assert_eq!(
            get(v, &Value::kw("k")).unwrap_err().code,
            FailCode::Type,
            "{v}"
        );
    }
}

#[test]
fn none_of_them_is_a_dict_key() {
    let values = [
        Value::path("./a", Some(FILE)),
        Value::url("https://x.org"),
        sound(Sound::Builtin(intern_kw("bd"))),
    ];
    for v in &values {
        assert_eq!(Key::from_value(v).unwrap_err().code, FailCode::Type, "{v}");
        let d = Value::dict(BTreeMap::new());
        assert_eq!(
            put(&d, &[Value::list(vec![v.clone(), Value::Int(1)])])
                .unwrap_err()
                .code,
            FailCode::Type,
            "{v}"
        );
    }
}
