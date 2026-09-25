//! Sound resolution (12.8.6): the kit first, then the instrument registry,
//! else `unknown-sound`; the `InstResolver` routes (instruments, banks and
//! sample files through `sampler`, MIDI, OSC); `osc "/addr"`.

use std::rc::Rc;

use crate::host::caps::{InstResolver, Route, SampleSrc};
use crate::ns::insts::InstRegistry;
use crate::types::diag::DiagCode;
use crate::value::intern::intern_kw;
use crate::value::value::{PathVal, Sound, Value};
use crate::vm::fail::FailCode;

use super::super::last_type;
use super::Session;

fn sound_of(v: &Value) -> Sound {
    match v {
        Value::Sound(s) => (**s).clone(),
        other => panic!("not a sound: {other:?}"),
    }
}

#[test]
fn the_kit_first_then_the_registry_then_unknown_sound() {
    let mut s = Session::new();
    s.ok("inst drum:\n\tsaw freq > * {env-perc 0.01 0.2}");
    let id = s.reg.borrow().id_of(intern_kw("drum")).expect("drum");
    // `:drum` is not in the kit: the registry has it.
    let p = s.ok("s :drum");
    let q = s.query(&p, 0);
    assert_eq!(q.events.len(), 1, "{:?}", q.faults);
    assert_eq!(sound_of(&q.events[0].value), Sound::Inst(id));
    // A kit key resolves in the kit.
    let p = s.ok("s :bd");
    let q = s.query(&p, 0);
    assert_eq!(
        sound_of(&q.events[0].value),
        Sound::Builtin(intern_kw("bd"))
    );
    // Neither: an event-local failure.
    let p = s.ok("s :nope");
    let q = s.query(&p, 0);
    assert!(q.events.is_empty());
    assert_eq!(q.faults[0].code, FailCode::UnknownSound);
}

#[test]
fn routes_follow_the_sound() {
    let s = Session::new();
    let r = s.reg.borrow();
    let sampler = r.id_of(intern_kw("sampler")).expect("sampler template");
    let analog = r.id_of(intern_kw("analog")).expect("analog template");
    let bank = intern_kw("bd-haus");
    assert_eq!(
        r.route(&Sound::Builtin(bank)),
        Ok(Route::Audio {
            inst: sampler,
            sample: Some(SampleSrc::Bank { kw: bank, index: 0 })
        })
    );
    // `s :analog` resolves in the kit to the builtin key, which names the
    // template.
    assert_eq!(
        r.route(&Sound::Builtin(intern_kw("analog"))),
        Ok(Route::Audio {
            inst: analog,
            sample: None
        })
    );
    let path = PathVal {
        text: Rc::from("./kick.wav"),
        file: None,
    };
    assert_eq!(
        r.route(&Sound::Sample(path.clone())),
        Ok(Route::Audio {
            inst: sampler,
            sample: Some(SampleSrc::Path(path))
        })
    );
    assert_eq!(r.route(&Sound::MidiOut(3)), Ok(Route::Midi { ch: 3 }));
    assert_eq!(
        r.route(&Sound::Osc(Rc::from("/x"))),
        Ok(Route::Osc {
            addr: Rc::from("/x")
        })
    );
    let missing = r.route(&Sound::Inst(crate::dsp::graph::InstId::new(999)));
    assert_eq!(missing.map_err(|f| f.code), Err(FailCode::UnknownSound));
    // With no `sampler` a bank is unknown.
    let empty = InstRegistry::new();
    let e = empty.route(&Sound::Builtin(bank));
    assert_eq!(e.map_err(|f| f.code), Err(FailCode::UnknownSound));
}

#[test]
fn a_session_inst_shadows_a_builtin_key() {
    let mut s = Session::new();
    s.ok("inst pluck:\n\tsaw freq");
    let r = s.reg.borrow();
    let id = r.id_of(intern_kw("pluck")).expect("pluck");
    assert_eq!(
        r.route(&Sound::Builtin(intern_kw("pluck"))),
        Ok(Route::Audio {
            inst: id,
            sample: None
        })
    );
}

#[test]
fn osc_with_a_string_is_a_sound_and_with_a_number_the_visual() {
    let mut s = Session::new();
    assert_eq!(
        sound_of(&s.ok("osc \"/synth\"")),
        Sound::Osc(Rc::from("/synth"))
    );
    assert!(matches!(s.ok("osc 20"), Value::Tex(_)));
    assert_eq!(last_type("osc \"/synth\""), "sound");
    assert_eq!(last_type("osc 20"), "tex");
    let p = s.ok("s {osc \"/synth\"}");
    let q = s.query(&p, 0);
    assert_eq!(sound_of(&q.events[0].value), Sound::Osc(Rc::from("/synth")));
}

#[test]
fn a_session_inst_name_is_a_known_sound_keyword() {
    let mut s = Session::new();
    s.ok("inst drum:\n\tsaw freq");
    let out = s.eval("s :drum > d1");
    let o = out.last().expect("a form");
    assert!(
        o.diags.iter().all(|d| d.code != DiagCode::UnknownKeyword),
        "{:?}",
        o.diags
    );
}
