//! Prerequisite contracts; successful opaque issuance belongs to SONG-04.
use std::rc::Rc;
use vactr::ns::namespace::VarSlotRef;
use vactr::pattern::combinators::{control::control, sound::sound, time::slow};
use vactr::pattern::eval::{InputCells, QueryCtx, QueryVm};
use vactr::pattern::pat::{PParam, Pat, PatNode};
use vactr::pattern::query::{query, query_traced, Event, TimeSpan};
use vactr::pattern::step::Step;
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::value::value::{NativeId, Sound, Value};
use vactr::vm::fail::{Failure, Origin};

struct TestVm {
    returned: Value,
}
impl QueryVm for TestVm {
    fn call(&mut self, _: &Value, _: &[Value]) -> Result<Value, Failure> {
        Ok(self.returned.clone())
    }
    fn deref(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        Ok(slot.get())
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        vec![]
    }
    fn put_output(&mut self, _: Vec<(Origin, Rc<str>)>) {}
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        Ok(Value::dict(Default::default()))
    }
}
fn scalar(value: Value) -> Pat {
    Pat::new(PatNode::Pure(Step::bare(value)), None, false)
}
fn builtin() -> Value {
    Value::Sound(Rc::new(Sound::Builtin(intern_kw("piano"))))
}

#[test]
fn fresh_events_have_no_certified_origin() {
    let span = TimeSpan::cycle(0).unwrap();
    let event = Event::new(Some(span), span, builtin(), None);
    assert!(event.song_source.is_none());
    assert!(event.clone().song_source.is_none());
}
#[test]
fn ordinary_queries_do_not_fabricate_song_provenance() {
    let pattern = scalar(builtin());
    for traced in [false, true] {
        let cells = InputCells::new();
        let mut vm = TestVm {
            returned: Value::Nil,
        };
        let mut cx = QueryCtx::new(&mut vm, &cells, 7);
        let result = if traced {
            query_traced(&pattern, TimeSpan::cycle(0).unwrap(), &mut cx)
        } else {
            query(&pattern, TimeSpan::cycle(0).unwrap(), &mut cx)
        };
        assert!(result.faults.is_empty());
        assert_eq!(result.events.len(), 1);
        assert!(result.events[0].song_source.is_none());
    }
}
#[test]
fn callable_sound_keeps_legacy_control_free_sampling() {
    let returned = control(
        intern_kw("note"),
        Rc::new(scalar(Value::Float64(66.75))),
        Rc::new(scalar(builtin())),
        None,
    );
    let pattern = sound(PParam::Fn(Value::Native(NativeId::new(1))), None, None);
    for traced in [false, true] {
        let cells = InputCells::new();
        let mut vm = TestVm {
            returned: Value::Pattern(Rc::new(returned.clone())),
        };
        let mut cx = QueryCtx::new(&mut vm, &cells, 7);
        let result = if traced {
            query_traced(&pattern, TimeSpan::cycle(0).unwrap(), &mut cx)
        } else {
            query(&pattern, TimeSpan::cycle(0).unwrap(), &mut cx)
        };
        assert!(result.faults.is_empty());
        assert_eq!(result.events.len(), 1);
        assert!(result.events[0].controls.is_empty());
        assert!(result.events[0].song_source.is_none());
    }
}
#[test]
fn slow_continuations_keep_absent_origin_and_original_whole() {
    let pattern = slow(
        Rc::new(scalar(builtin())),
        PParam::Const(Value::Int(4)),
        None,
    );
    let mut vm = TestVm {
        returned: Value::Nil,
    };
    let cells = InputCells::new();
    for begin in 0..4 {
        let result = query_traced(
            &pattern,
            TimeSpan::cycle(begin).unwrap(),
            &mut QueryCtx::new(&mut vm, &cells, 7),
        );
        assert!(result.faults.is_empty());
        assert_eq!(result.events.len(), 1);
        let event = &result.events[0];
        assert!(event.song_source.is_none());
        assert_eq!(
            event.whole.unwrap(),
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap()
        );
    }
}
