//! Pattern engine tests (TASK-006). Goldens are exact `Ratio64`s.

mod combinators;
mod cover;
mod faults;
mod input;
mod random;
mod region;
mod signals;
mod sound;
mod steps;
pub(crate) mod stub_vm;

use std::rc::Rc;

use crate::pattern::build::{pattern_of, pure};
use crate::pattern::combinators::control::control;
use crate::pattern::combinators::sound::sound;
use crate::pattern::eval::{InputCells, QueryCtx};
use crate::pattern::pat::{PParam, Pat};
use crate::pattern::query::{query, Event, QueryResult, TimeSpan};
use crate::value::intern::{intern_kw, name_of_kw};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

pub(crate) use stub_vm::{builtin, StubVm};

pub(crate) fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}

pub(crate) fn span(b: Ratio64, e: Ratio64) -> TimeSpan {
    TimeSpan::new(b, e).unwrap()
}

pub(crate) fn cycle(c: i64) -> TimeSpan {
    TimeSpan::cycle(c).unwrap()
}

pub(crate) fn kw(name: &str) -> Value {
    Value::kw(name)
}

pub(crate) fn int(i: i32) -> Value {
    Value::Int(i)
}

/// A list value (a step list where a pattern is expected).
pub(crate) fn list(items: Vec<Value>) -> Value {
    Value::list(items)
}

/// The pattern of a value.
pub(crate) fn pat(v: Value) -> Rc<Pat> {
    pattern_of(&v, None).unwrap()
}

/// `s src` for a keyword or a list.
pub(crate) fn s(src: Value) -> Rc<Pat> {
    let src = match &src {
        Value::List(_) | Value::Pattern(_) => PParam::Pat(pat(src)),
        _ => PParam::Const(src),
    };
    Rc::new(sound(src, None, None))
}

/// `subject > name value`.
pub(crate) fn ctl(subject: Rc<Pat>, name: &str, value: Value) -> Rc<Pat> {
    Rc::new(control(intern_kw(name), pat(value), subject, None))
}

/// A pure constant pattern.
pub(crate) fn konst(v: Value) -> Rc<Pat> {
    Rc::new(pure(v, None))
}

/// Queries with a fresh stub VM, empty cells and seed 42.
pub(crate) fn run(p: &Pat, sp: TimeSpan) -> QueryResult {
    let mut vm = StubVm::new();
    run_with(p, sp, &mut vm)
}

pub(crate) fn run_with(p: &Pat, sp: TimeSpan, vm: &mut StubVm) -> QueryResult {
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(vm, &cells, 42);
    query(p, sp, &mut cx)
}

/// `(onset, whole length)` of each event, sorted.
pub(crate) fn timing(events: &[Event]) -> Vec<(Ratio64, Ratio64)> {
    let mut v: Vec<(Ratio64, Ratio64)> = events
        .iter()
        .map(|e| {
            let w = e.whole.unwrap();
            (w.begin, w.duration().unwrap())
        })
        .collect();
    v.sort();
    v
}

/// Events sorted by onset.
pub(crate) fn sorted(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by(|a, b| a.anchor().cmp(&b.anchor()).then(a.occ.cmp(&b.occ)));
    events
}

/// The printed name of a keyword or builtin-sound value.
pub(crate) fn name(v: &Value) -> String {
    match v {
        Value::Keyword(k) => name_of_kw(*k).to_string(),
        Value::Sound(snd) => match &**snd {
            crate::value::value::Sound::Builtin(k) => name_of_kw(*k).to_string(),
            other => format!("{other:?}"),
        },
        other => format!("{other:?}"),
    }
}

/// A control of an event.
pub(crate) fn ctl_of<'a>(e: &'a Event, name: &str) -> Option<&'a Value> {
    e.controls.get(&intern_kw(name))
}

/// Stub natives: 1 = `{p -> fast p 2}`, 2 = `rev`, 3 = identity,
/// 4 = `{p -> hurry p 2}`.
pub(crate) fn transforms(vm: &mut StubVm) -> [Value; 4] {
    use crate::pattern::combinators::pattern_value;
    use crate::pattern::combinators::time::{fast, hurry, rev};
    fn arg(args: &[Value]) -> Result<Rc<Pat>, crate::vm::fail::Failure> {
        match args.first() {
            Some(Value::Pattern(p)) => Ok(Rc::clone(p)),
            _ => Err(crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::Type,
                "stub: expected a pattern",
            )),
        }
    }
    [
        vm.define(1, |a| {
            Ok(pattern_value(fast(arg(a)?, PParam::int(2), None)))
        }),
        vm.define(2, |a| Ok(pattern_value(rev(arg(a)?, None)))),
        vm.define(3, |a| Ok(Value::Pattern(arg(a)?))),
        vm.define(4, |a| {
            Ok(pattern_value(hurry(arg(a)?, PParam::int(2), None)))
        }),
    ]
}

/// Names by onset.
pub(crate) fn names(events: &[Event]) -> Vec<(Ratio64, String)> {
    let mut v: Vec<(Ratio64, String)> = events
        .iter()
        .map(|e| (e.anchor(), name(&e.value)))
        .collect();
    v.sort();
    v
}

/// Onsets only (`part.begin == whole.begin`), sorted.
pub(crate) fn onsets(events: &[Event]) -> Vec<Ratio64> {
    let mut v: Vec<Ratio64> = events
        .iter()
        .filter(|e| e.is_onset())
        .map(Event::anchor)
        .collect();
    v.sort();
    v
}

/// A value's debug text; `Value` has no `PartialEq` (deep equality is
/// fallible), so tests compare this.
pub(crate) fn show(v: &Value) -> String {
    format!("{v:?}")
}

/// The debug texts of values.
pub(crate) fn shows(vs: &[Value]) -> Vec<String> {
    vs.iter().map(show).collect()
}
