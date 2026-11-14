//! `load` (design 7.1.3 "Source loading", lang-reference section 4): a fresh
//! scope over the prelude, the last expression, and the failure cases.

use super::reactive_basic::{Harness, MapLoader};
use crate::ns::load::NoopHost;
use crate::reader::span::FileId;
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::tests::probe_prelude;
use crate::vm::vm::EffectMode;

fn with_files(files: &[(&str, &str)]) -> Harness {
    Harness::with(probe_prelude(), Box::new(MapLoader::with(files)))
}

fn fail_code(h: &mut Harness, src: &str) -> FailCode {
    match h.run(src).pop().map(|o| o.value) {
        Some(Err(e)) => e.code,
        other => panic!("{src:?}: expected a failure, got {other:?}"),
    }
}

#[test]
fn load_returns_the_value_of_the_last_top_level_form() {
    let mut h = with_files(&[(
        "kit.vact",
        "let base 60\nlet up + base 7\n[up: up base: base]",
    )]);
    assert_eq!(h.show("let kit load ./kit.vact"), "[base: 60 up: 67]");
    assert_eq!(h.get("kit"), "[base: 60 up: 67]");
}

#[test]
fn the_loaded_file_sees_the_prelude_but_not_the_callers_session() {
    let mut h = with_files(&[
        ("uses-prelude.vact", "len [1 2 3]"),
        ("uses-session.vact", "+ secret 1"),
    ]);
    assert_eq!(h.show("load ./uses-prelude.vact"), "3");
    h.ok("let secret 41");
    let out = h.run("load ./uses-session.vact");
    let e = out[0]
        .value
        .clone()
        .expect_err("undefined in the loaded scope");
    assert_eq!(e.code, FailCode::LoadFailed);
    assert!(e.message.contains("undefined-name"), "the first cause: {e}");
}

#[test]
fn the_caller_cannot_see_the_loaded_files_bindings() {
    let mut h = with_files(&[("defs.vact", "let hidden 5\nhidden")]);
    assert_eq!(h.show("load ./defs.vact"), "5");
    assert!(h.ev.ns().session_value("hidden").is_none());
    assert_eq!(h.show("hidden"), "fail: undefined-name");
}

#[test]
fn a_relative_path_inside_the_loaded_file_carries_its_file_id() {
    let mut h = with_files(&[("pack.vact", "./bd.wav")]);
    let v = h.ok("load ./pack.vact");
    match v {
        Value::Path(p) => {
            assert_eq!(&*p.text, "./bd.wav");
            assert_eq!(
                p.file,
                Some(FileId::new(10)),
                "the loader's id, not the caller's"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_empty_file_loads_as_nil() {
    let mut h = with_files(&[("empty.vact", "")]);
    assert_eq!(h.show("load ./empty.vact"), "nil");
}

#[test]
fn a_reader_error_is_load_failed() {
    let mut h = with_files(&[("bad.vact", "f .//x")]);
    assert_eq!(fail_code(&mut h, "load ./bad.vact"), FailCode::LoadFailed);
    let e = h
        .run("load ./bad.vact")
        .pop()
        .expect("o")
        .value
        .expect_err("e");
    assert!(e.message.contains("bad-path"), "{e}");
}

#[test]
fn a_run_failure_is_load_failed_with_the_cause() {
    let mut h = with_files(&[("div.vact", "let x 1\n/ x 0")]);
    let e = h
        .run("load ./div.vact")
        .pop()
        .expect("o")
        .value
        .expect_err("e");
    assert_eq!(e.code, FailCode::LoadFailed);
    assert!(e.message.contains("division-by-zero"), "{e}");
    assert_eq!(h.show("+ 1 1"), "2", "the session stays usable");
}

#[test]
fn a_load_cycle_ends_in_depth_exceeded_without_overflow() {
    let mut h = with_files(&[("a.vact", "load ./b.vact"), ("b.vact", "load ./a.vact")]);
    assert_eq!(fail_code(&mut h, "load ./a.vact"), FailCode::DepthExceeded);
    assert_eq!(h.show("+ 1 1"), "2", "the session stays usable");
}

#[test]
fn noop_host_is_host_unavailable() {
    let mut h = Harness::with(probe_prelude(), Box::new(NoopHost));
    assert_eq!(
        fail_code(&mut h, "load ./a.vact"),
        FailCode::HostUnavailable
    );
}

#[test]
fn a_url_or_a_string_is_not_a_loadable_path() {
    let mut h = with_files(&[]);
    assert_eq!(fail_code(&mut h, "load \"a.vact\""), FailCode::Type);
}

#[test]
fn load_inside_a_query_is_effect_in_query() {
    let mut h = with_files(&[("a.vact", "1")]);
    h.ok("fn get-a k:\n\tload ./a.vact");
    assert_eq!(h.show("get-a 0"), "1");
    let f = h.ev.ns().session_value("get-a").expect("get-a");
    let (vm, ns) = h.ev.vm_and_ns();
    let r = vm.with_effect_mode(EffectMode::Query, |vm| {
        vm.call_value(ns, &f, vec![Value::Int(0)], Vec::new())
    });
    assert_eq!(r.map_err(|e| e.code).err(), Some(FailCode::EffectInQuery));
    assert_eq!(
        vm.effect_mode(),
        EffectMode::Normal,
        "the scope guard restored Normal"
    );
}
