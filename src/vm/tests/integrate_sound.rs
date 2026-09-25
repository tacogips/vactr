//! INTEGRATE: sounds and the sound kit through the whole pipeline (design
//! 7.1.4, 10.1): the late-bound session `sound-kit`, `kit:`, sound values,
//! MIDI out, SOUND FIRST, and the `scale`/`shape` subject overloads.

use crate::ns::stage::SlotKey;
use crate::pattern::eval::QueryVm;
use crate::types::diag::{DiagCode, Severity};
use crate::value::intern::name_of_kw;
use crate::value::value::{Sound, Value};
use crate::vm::fail::FailCode;
use crate::vm::tests::integrate_query::Ev;

/// A sound value as `builtin :bd`, `sample ./x.wav` or `midi 1`.
fn sound_str(v: &Value) -> String {
    match v {
        Value::Sound(s) => match &**s {
            Sound::Builtin(k) => format!("builtin :{}", name_of_kw(*k)),
            Sound::Sample(p) => format!("sample {}", p.text),
            Sound::MidiOut(c) => format!("midi {c}"),
        },
        other => format!("not a sound: {other}"),
    }
}

/// The sounds of the events of cycle 0 of the pattern bound to `d1`.
fn d1_sounds(h: &mut Ev) -> Vec<String> {
    let p = h.bound(SlotKey::D(1)).expect("d1 is bound");
    h.query(&p, 0)
        .events
        .iter()
        .map(|e| sound_str(&e.value))
        .collect()
}

#[test]
fn a_session_sound_kit_is_heard_at_the_next_query() {
    let mut h = Ev::new();
    let out = h.run("s :bd > d1");
    assert!(Ev::codes(&out).is_empty(), "{:?}", out[0].diags);
    let p = h.bound(SlotKey::D(1)).expect("d1");
    assert_eq!(d1_sounds(&mut h), ["builtin :bd"]);
    // The design-music spelling: `put` merges a dict element (later keys win).
    let out = h.run("let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]");
    assert!(out[0].value.is_ok(), "{:?}", out[0].value);
    let codes: Vec<(DiagCode, Severity)> =
        out[0].diags.iter().map(|d| (d.code, d.severity)).collect();
    assert_eq!(codes, [(DiagCode::ShadowsPrelude, Severity::Hint)]);
    // The pattern bound before the override resolves in the new kit.
    let events = h.query(&p, 0).events;
    assert_eq!(sound_str(&events[0].value), "sample ./bd/909.wav");
    // `default-sound-kit` is unchanged.
    let kit = h.with_handle(|q| q.sound_kit()).expect("kit");
    assert!(kit.to_string().contains("909"), "{kit}");
    let prelude = h.last("default-sound-kit :bd").expect("value");
    assert_eq!(sound_str(&prelude), "builtin :bd");
}

#[test]
fn a_kit_argument_resolves_keys_outside_the_default_kit() {
    let mut h = Ev::new();
    let out = h.run(
        "let tr909 [bd909: {sample ./tr909/bd.wav} sd909: {sample ./tr909/sd.wav}]\n\
         s [:bd909 :sd909] kit: tr909 > d1",
    );
    assert!(Ev::codes(&out).is_empty(), "{out:?}");
    assert!(out.iter().all(|o| o.value.is_ok()));
    assert_eq!(
        d1_sounds(&mut h),
        ["sample ./tr909/bd.wav", "sample ./tr909/sd.wav"]
    );
}

#[test]
fn a_var_kit_changed_with_upd_is_heard_at_the_next_query() {
    let mut h = Ev::new();
    h.run("var my-kit [bd: {sample ./a.wav}]\ns :bd kit: my-kit > d1");
    assert_eq!(d1_sounds(&mut h), ["sample ./a.wav"]);
    let out = h.run("upd my-kit [bd: {sample ./b.wav}]");
    assert!(out[0].value.is_ok());
    assert_eq!(d1_sounds(&mut h), ["sample ./b.wav"]);
}

#[test]
fn a_sound_value_needs_no_kit_lookup() {
    let mut h = Ev::new();
    let out = h.run("let kick sample ./kick.wav\ns kick > d1");
    assert!(Ev::codes(&out).is_empty(), "{out:?}");
    assert_eq!(d1_sounds(&mut h), ["sample ./kick.wav"]);
    // Even under a kit without the key: a value is used as is.
    h.run("s kick kit: [bd: {sample ./bd.wav}] > d2");
    let p = h.bound(SlotKey::D(2)).expect("d2");
    let r = h.query(&p, 0);
    assert!(r.faults.is_empty(), "{:?}", r.faults);
    assert_eq!(sound_str(&r.events[0].value), "sample ./kick.wav");
}

#[test]
fn a_key_missing_from_the_kit_argument_is_event_local_unknown_sound() {
    let mut h = Ev::new();
    let out = h.run("s :bd909 kit: [bd: {sample ./bd.wav}] > d1");
    assert!(Ev::codes(&out).is_empty(), "{out:?}");
    assert!(out[0].value.is_ok());
    let p = h.bound(SlotKey::D(1)).expect("d1");
    let r = h.query(&p, 0);
    assert!(r.events.is_empty());
    assert_eq!(r.faults[0].code, FailCode::UnknownSound);
}

#[test]
fn an_unknown_keyword_is_a_check_error_without_a_kit() {
    let mut h = Ev::new();
    let out = h.run("s :not-a-sound > d1");
    assert_eq!(Ev::codes(&out), [DiagCode::UnknownKeyword]);
    // Check never gates run: the bind happens, the query fails per event.
    let p = h.bound(SlotKey::D(1)).expect("d1");
    assert_eq!(h.query(&p, 0).faults[0].code, FailCode::UnknownSound);
}

#[test]
fn midi_out_is_an_instrument() {
    let mut h = Ev::new();
    let out = h.run("s {midi 1} > note [:c :e :g] > d1");
    assert!(Ev::codes(&out).is_empty(), "{out:?}");
    assert_eq!(d1_sounds(&mut h), ["midi 1", "midi 1", "midi 1"]);
    assert_eq!(
        h.last("midi 17").expect_err("out of range").code,
        FailCode::Type
    );
}

#[test]
fn a_pattern_before_s_is_sound_not_first() {
    let mut h = Ev::new();
    for src in ["n [0 3] > s :bd > d1", "note [:c] > s :x > d1"] {
        let out = h.run(src);
        assert!(
            Ev::codes(&out).contains(&DiagCode::SoundNotFirst),
            "{src}: {:?}",
            out[0].diags
        );
        assert_eq!(out[0].value.as_ref().expect_err(src).code, FailCode::Arity);
    }
    assert_eq!(h.bind_count(), 0, "a failed form stages nothing");
}

#[test]
fn a_fn_local_sound_kit_does_not_affect_s() {
    let mut h = Ev::new();
    h.run("fn drums x:\n\tlet sound-kit [bd: {midi x}]\n\ts :bd\nlet p drums 2");
    let p = h.get("p");
    let r = h.query(&p, 0);
    assert_eq!(sound_str(&r.events[0].value), "builtin :bd");
}

#[test]
fn sample_and_midi_values() {
    let mut h = Ev::new();
    assert_eq!(
        sound_str(&h.last("sample ./x.wav").expect("sound")),
        "sample ./x.wav"
    );
    assert_eq!(sound_str(&h.last("midi 16").expect("sound")), "midi 16");
    let out = h.run("sample https://example.org/x.wav");
    assert!(Ev::codes(&out).contains(&DiagCode::TypeMismatch));
    assert_eq!(out[0].value.as_ref().expect_err("url").code, FailCode::Type);
}

#[test]
fn scale_and_shape_dispatch_on_the_subject() {
    let mut h = Ev::new();
    let notes = h
        .last("s :pluck > n [0 2 4] > scale :c :minor")
        .expect("pattern");
    let r = h.query(&notes, 0);
    assert_eq!(r.events.len(), 3, "{:?}", r.faults);
    assert!(matches!(h.last("osc 10 > scale 2"), Ok(Value::Tex(_))));
    assert!(matches!(h.last("shape 3"), Ok(Value::Tex(_))));
    assert!(matches!(h.last("s :bd > shape 0.5"), Ok(Value::Pattern(_))));
    for bad in ["scale 5 :c :minor", "shape \"x\""] {
        assert_eq!(h.last(bad).expect_err(bad).code, FailCode::Type, "{bad}");
    }
}

/// An in-memory source loader keyed by path text.
struct MapLoader(Vec<(&'static str, &'static str)>);

impl crate::ns::load::SourceLoader for MapLoader {
    fn read(
        &mut self,
        path: &crate::value::value::PathVal,
    ) -> Result<(crate::reader::span::FileId, std::rc::Rc<str>), crate::vm::fail::Failure> {
        self.0
            .iter()
            .position(|(p, _)| **p == *path.text)
            .map(|k| {
                let id = crate::reader::span::FileId::new(u32::try_from(k).unwrap_or(0) + 100);
                (id, std::rc::Rc::from(self.0[k].1))
            })
            .ok_or_else(|| crate::vm::fail::Failure::new(FailCode::LoadFailed, "missing"))
    }
}

#[test]
fn a_loaded_sound_pack_merges_into_the_session_kit() {
    let pack = "let one sample ./bd/1.wav\n[bd: [one {sample ./bd/2.wav}] sd: {sample ./sd.wav}]\n";
    let loader = MapLoader(vec![("./soundpack/sound-pack.vact", pack)]);
    let mut h = Ev::with_loader(crate::ns::namespace::Prelude::core(), Box::new(loader));
    let out = h.run(
        "let my-pack load ./soundpack/sound-pack.vact\n\
         let sound-kit put default-sound-kit & my-pack\n\
         s :bd > n [0 1] > d1",
    );
    assert!(out.iter().all(|o| o.value.is_ok()), "{out:?}");
    assert!(Ev::codes(&out).is_empty(), "{out:?}");
    // `n i` picks the i-th sound of the bank.
    assert_eq!(
        d1_sounds(&mut h),
        ["sample ./bd/1.wav", "sample ./bd/2.wav"]
    );
}

#[test]
fn check_diagnostics_of_a_loaded_file_keep_their_spans_and_never_fail_the_load() {
    let pack = "let a 1\nlet a 2\n[bd: {sample ./x.wav}]\n";
    let loader = MapLoader(vec![("./p.vact", pack)]);
    let mut h = Ev::with_loader(crate::ns::namespace::Prelude::core(), Box::new(loader));
    let out = h.run("let p load ./p.vact");
    assert!(out[0].value.is_ok(), "{:?}", out[0].value);
    let d = out[0]
        .diags
        .iter()
        .find(|d| d.code == DiagCode::Rebinding)
        .expect("the loaded file's rebinding");
    assert_eq!(d.span.file, crate::reader::span::FileId::new(100));
    assert!(h.get("p").to_string().contains("x.wav"));
}
