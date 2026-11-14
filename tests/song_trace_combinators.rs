//! Stable producer traversal through combinators; dynamic canonicalization is SONG-04.
use std::collections::BTreeSet;
use std::rc::Rc;
use vactr::ns::namespace::VarSlotRef;
use vactr::pattern::combinators::{control, music, random, region, sound, structure, time};
use vactr::pattern::eval::{InputCells, QueryCtx, QueryVm};
use vactr::pattern::occ::{ProducerKind, ProducerTrace};
use vactr::pattern::pat::{PParam, Pat, PatNode, SliceCuts};
use vactr::pattern::query::{query, query_traced, Event, QueryResult, TimeSpan};
use vactr::pattern::step::{steps, Step};
use vactr::reader::span::NodeId;
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::value::value::{NativeId, Value};
use vactr::vm::fail::{FailCode, Failure, Origin};

#[derive(Default)]
struct Vm {
    output: Vec<(Origin, Rc<str>)>,
}
impl QueryVm for Vm {
    fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure> {
        match f {
            Value::Native(id) if id.get() == 1 => Ok(args[0].clone()),
            Value::Native(id) if id.get() == 3 => Ok(Value::Pattern(Rc::new(time::rev(
                pure(Value::Sound(Rc::new(vactr::value::value::Sound::MidiOut(
                    1,
                )))),
                None,
            )))),
            Value::Native(id) if id.get() == 2 => {
                Ok(Value::list(vec![Value::Int(8), Value::Int(8)]))
            }
            _ => Err(Failure::new(FailCode::DivisionByZero, "fixture failure")),
        }
    }
    fn deref(&mut self, r: &VarSlotRef) -> Result<Value, Failure> {
        Ok(r.get())
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        std::mem::take(&mut self.output)
    }
    fn put_output(&mut self, out: Vec<(Origin, Rc<str>)>) {
        self.output = out;
    }
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        Ok(Value::dict(Default::default()))
    }
    fn inst_sound(&mut self, _: vactr::value::intern::KwId) -> Option<Value> {
        // Resolving into an instrument is independently covered by the legacy sound suite.
        None
    }
}
fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).unwrap()
}
fn span(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(r(a, d), r(b, d)).unwrap()
}
fn run(p: &Pat, w: TimeSpan, traced: bool) -> QueryResult {
    let mut vm = Vm::default();
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 42);
    if traced {
        query_traced(p, w, &mut cx)
    } else {
        query(p, w, &mut cx)
    }
}
fn list(v: Vec<Value>) -> Pat {
    steps(v.into_iter().map(Step::bare).collect(), None)
}
fn source() -> Rc<Pat> {
    Rc::new(list(vec![
        Value::Int(7),
        Value::Int(7),
        Value::Int(7),
        Value::Int(7),
    ]))
}
fn pure(v: Value) -> Rc<Pat> {
    Rc::new(Pat::new(PatNode::Pure(Step::bare(v)), None, false))
}
fn identity() -> Value {
    Value::Native(NativeId::new(1))
}
fn trace(e: &Event) -> &ProducerTrace {
    e.producer.as_ref().unwrap()
}
fn unique(result: &QueryResult) {
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert!(!result.events.is_empty());
    let identities: BTreeSet<_> = result
        .events
        .iter()
        .map(|e| (trace(e), e.anchor()))
        .collect();
    assert_eq!(identities.len(), result.events.len());
}
fn compatible(p: &Pat, w: TimeSpan) {
    let old = run(p, w, false);
    let new = run(p, w, true);
    assert_eq!(old.events.len(), new.events.len());
    assert_eq!(old.output, new.output);
    for (a, b) in old.events.iter().zip(&new.events) {
        assert!(a.producer.is_none());
        assert!(b.producer.is_some());
        assert_eq!(a.occ, b.occ);
        assert_eq!(a.whole, b.whole);
        assert_eq!(a.part, b.part);
        assert_eq!(format!("{:?}", a.value), format!("{:?}", b.value));
        assert_eq!(format!("{:?}", a.controls), format!("{:?}", b.controls));
        assert_eq!(a.src, b.src);
        assert_eq!(a.is_onset(), b.is_onset());
    }
    assert_eq!(old.faults.len(), new.faults.len());
    for (a, b) in old.faults.iter().zip(&new.faults) {
        assert_eq!(a.code, b.code);
        assert_eq!(a.message, b.message);
        assert_eq!(a.origin, b.origin);
    }
}
fn partitions(p: &Pat) {
    let all = run(p, span(0, 2, 1), true);
    unique(&all);
    for w in [
        span(7, 11, 8),
        span(1, 5, 8),
        span(0, 1, 2),
        span(1, 2, 2),
        span(1, 2, 1),
    ] {
        let clipped = run(p, w, true);
        assert!(clipped.faults.is_empty());
        assert!(!clipped.events.is_empty());
        let expected: Vec<_> = all
            .events
            .iter()
            .filter(|e| {
                let s = e.whole.unwrap_or(e.part);
                s.begin < w.end && w.begin < s.end
            })
            .map(|e| (e.whole, e.producer.clone(), e.occ.clone()))
            .collect();
        let actual: Vec<_> = clipped
            .events
            .iter()
            .map(|e| (e.whole, e.producer.clone(), e.occ.clone()))
            .collect();
        assert_eq!(actual, expected, "window {w:?}");
    }
}

#[test]
fn static_children_and_generated_splits_survive_stable_partitions() {
    let s = source();
    let patterns = vec![
        time::fast(s.clone(), PParam::int(2), None),
        time::slow(s.clone(), PParam::int(2), None),
        time::hurry(s.clone(), PParam::int(2), None),
        time::rev(s.clone(), None),
        structure::stack(vec![(*s).clone(), (*s).clone()], None),
        structure::cat(vec![(*s).clone(), (*s).clone()], None),
        structure::fastcat(vec![(*s).clone(), (*s).clone()], None),
        structure::ply(s.clone(), PParam::int(3), None),
        region::chop(s.clone(), PParam::int(3), None),
        structure::repeat(s.clone(), PParam::int(2), None),
        structure::hold(s.clone(), PParam::int(3), None),
        time::iter(s.clone(), PParam::int(2), None),
        time::segment(s.clone(), PParam::int(8), None),
        structure::grid(s.clone(), Rc::new(list(vec![Value::Bool(true); 4])), None),
        structure::euclid(
            s.clone(),
            PParam::int(3),
            PParam::int(4),
            PParam::int(0),
            None,
        ),
        random::choose(vec![(*s).clone(), (*s).clone()], None),
        region::slice(
            s.clone(),
            SliceCuts::Equal(PParam::int(4)),
            Rc::new(list(vec![Value::Int(0); 4])),
            None,
        ),
        region::splice(
            s.clone(),
            SliceCuts::Equal(PParam::int(4)),
            Rc::new(list(vec![Value::Int(0); 4])),
            None,
        ),
        region::fit(s.clone(), None),
        region::loop_at(s, PParam::int(2), None),
    ];
    for p in patterns {
        partitions(&p);
        compatible(&p, span(1, 11, 8));
    }
}

#[test]
fn simultaneous_identical_twins_and_forced_hashes_have_distinct_full_paths() {
    let mut a = (*source()).clone();
    a.id = NodeId::new(0);
    let b = a.clone();
    let mut p = structure::stack(vec![a, b], None);
    p.id = NodeId::new(0);
    let result = run(&p, span(0, 1, 1), true);
    unique(&result);
    assert_eq!(result.events.len(), 8);
    for (a, b) in result.events[..4].iter().zip(&result.events[4..]) {
        assert_eq!(a.anchor(), b.anchor());
        assert_ne!(trace(a), trace(b));
    }
    compatible(&p, span(0, 1, 1));
    partitions(&p);
}

#[test]
fn dynamic_transform_branches_are_explicit_before_selection() {
    let s = source();
    let patterns = vec![
        structure::superimpose(s.clone(), identity(), None),
        structure::off(
            s.clone(),
            PParam::Const(Value::Ratio(r(1, 4))),
            identity(),
            None,
        ),
        structure::jux(s.clone(), identity(), None),
        time::every(s.clone(), PParam::int(2), identity(), None),
        time::whenmod(s.clone(), PParam::int(2), PParam::int(1), identity(), None),
        time::chunk(s.clone(), PParam::int(2), identity(), None),
        random::sometimes_by(s, PParam::Const(Value::Float64(0.5)), identity(), None),
    ];
    for p in patterns {
        let all = run(&p, span(0, 2, 1), true);
        unique(&all);
        assert!(all.events.iter().any(|e| trace(e)
            .steps
            .iter()
            .any(|s| s.kind == ProducerKind::DynamicExpansion)));
        partitions(&p);
        compatible(&p, span(1, 11, 8));
    }
}

#[test]
fn timing_and_content_merges_are_role_and_length_framed() {
    let subjects = Rc::new(structure::stack(
        vec![(*pure(Value::Int(9))).clone(); 2],
        None,
    ));
    let mut unstructured = (*subjects).clone();
    unstructured.structured = false;
    let unstructured = Rc::new(unstructured);
    let timing = source();
    let patterns = vec![
        control::control(
            intern_kw("gain"),
            timing.clone(),
            unstructured.clone(),
            None,
        ),
        structure::grid(
            unstructured.clone(),
            Rc::new(list(vec![Value::Bool(true); 4])),
            None,
        ),
        region::slice(
            unstructured.clone(),
            SliceCuts::Equal(PParam::int(4)),
            Rc::new(list(vec![Value::Int(0); 4])),
            None,
        ),
        region::splice(
            unstructured,
            SliceCuts::Equal(PParam::int(4)),
            Rc::new(list(vec![Value::Int(0); 4])),
            None,
        ),
    ];
    for p in patterns {
        let all = run(&p, span(0, 1, 1), true);
        unique(&all);
        assert_eq!(all.events.len(), 8);
        for e in &all.events {
            let steps = &trace(e).steps;
            assert_eq!(steps[0].kind, ProducerKind::TimingSource);
            let len = steps[0].ordinal as usize;
            assert_eq!(steps[len + 2].kind, ProducerKind::ContentSource);
            assert_eq!(steps.len(), len + 4 + steps[len + 2].ordinal as usize);
        }
        partitions(&p);
        compatible(&p, span(1, 7, 8));
    }
}

#[test]
fn chord_arp_and_nested_dynamic_sources_preserve_branches() {
    let chord = pure(Value::list(vec![Value::kw("c"), Value::kw("major")]));
    let p = music::arp(
        Rc::new(music::chord(chord, source(), None)),
        PParam::Const(Value::kw("up")),
        None,
    );
    let all = run(&p, span(0, 1, 1), true);
    unique(&all);
    assert_eq!(all.events.len(), 12);
    assert!(all
        .events
        .iter()
        .all(|e| trace(e).steps.last().unwrap().kind == ProducerKind::GeneratedBranch));
    partitions(&p);
    compatible(&p, span(1, 11, 8));
    let dynamic = Rc::new(list(vec![Value::Native(NativeId::new(2))]));
    let p = structure::ply(dynamic, PParam::int(2), None);
    let all = run(&p, span(0, 1, 1), true);
    unique(&all);
    assert_eq!(all.events.len(), 4);
    assert!(all.events.iter().all(|e| trace(e)
        .steps
        .iter()
        .any(|s| s.kind == ProducerKind::DynamicExpansion)));
    partitions(&p);
}

#[test]
fn striate_ranks_by_full_paths_and_retains_widening_fault_evidence() {
    let p = region::striate(
        Rc::new(structure::stack(vec![(*source()).clone(); 2], None)),
        PParam::int(8),
        None,
    );
    let all = run(&p, span(0, 1, 1), true);
    unique(&all);
    assert_eq!(all.events.len(), 8);
    let starts: BTreeSet<_> = all
        .events
        .iter()
        .map(|e| format!("{:?}", e.controls[&intern_kw("begin")]))
        .collect();
    assert_eq!(starts.len(), 8);
    partitions(&p);
    compatible(&p, span(1, 11, 8));
    let inner = Rc::new(list(vec![Value::Int(1), Value::Native(NativeId::new(99))]));
    let p = region::striate(inner, PParam::int(2), None);
    let traced = run(&p, span(0, 1, 4), true);
    let legacy = run(&p, span(0, 1, 4), false);
    assert_eq!(traced.events.len(), 1);
    assert_eq!(legacy.events.len(), 1);
    assert!(legacy.faults.is_empty());
    assert_eq!(traced.faults.len(), 1);
    assert_eq!(traced.faults[0].code, FailCode::DivisionByZero);
}

#[test]
fn failing_dynamic_siblings_restore_paths_and_keep_legacy_faults() {
    let p = structure::superimpose(source(), Value::Native(NativeId::new(99)), None);
    let result = run(&p, span(0, 1, 1), true);
    assert_eq!(result.events.len(), 4);
    assert_eq!(result.faults.len(), 1);
    assert!(result
        .events
        .iter()
        .all(|e| trace(e).steps[0].kind == ProducerKind::Child && trace(e).steps[0].ordinal == 0));
    compatible(&p, span(0, 1, 1));
    for p in [
        random::degrade_by(source(), PParam::Const(Value::Float64(0.4)), None),
        random::maybe(source(), None, None),
    ] {
        let result = run(&p, span(0, 4, 1), true);
        unique(&result);
        compatible(&p, span(0, 4, 1));
    }
}

#[test]
fn sound_source_failures_keep_trace_scope_and_siblings() {
    let bad = sound::sound(
        PParam::Pat(Rc::new(list(vec![Value::kw("missing")]))),
        None,
        None,
    );
    let p = structure::stack(vec![bad, (*source()).clone()], None);
    let result = run(&p, span(0, 1, 1), true);
    assert_eq!(result.events.len(), 4);
    assert_eq!(result.faults.len(), 1);
    assert_eq!(result.faults[0].code, FailCode::UnknownSound);
    assert!(result.events.iter().all(|e| trace(e).steps[0].ordinal == 1));
    compatible(&p, span(0, 1, 1));
}

#[test]
fn sound_static_pattern_and_bank_sources_preserve_complete_paths() {
    use vactr::value::value::Sound;
    let snd = Value::Sound(Rc::new(Sound::MidiOut(1)));
    let nested = Rc::new(time::rev(pure(snd.clone()), None));
    let patterns = [
        sound::sound(PParam::Pat(nested), None, None),
        sound::sound(PParam::Pat(Rc::new(list(vec![snd.clone(); 4]))), None, None),
        control::control(
            intern_kw("n"),
            pure(Value::Int(1)),
            Rc::new(sound::sound(
                PParam::Const(Value::list(vec![snd.clone(), snd])),
                None,
                None,
            )),
            None,
        ),
    ];
    for p in patterns {
        partitions(&p);
        compatible(&p, span(1, 11, 8));
    }
    let p = &patterns_for_nested_sound();
    let result = run(p, span(0, 1, 1), true);
    assert_eq!(trace(&result.events[0]).steps.len(), 2);
}
fn patterns_for_nested_sound() -> Pat {
    use vactr::value::value::Sound;
    sound::sound(
        PParam::Pat(Rc::new(time::rev(
            pure(Value::Sound(Rc::new(Sound::MidiOut(1)))),
            None,
        ))),
        None,
        None,
    )
}

#[test]
fn deep_combinator_chains_keep_the_existing_depth_bound() {
    for (depth, succeeds) in [(200, true), (300, false)] {
        let mut p = source();
        for _ in 0..depth {
            p = Rc::new(time::rev(p, None));
        }
        for traced in [false, true] {
            eprintln!("adversarial reverse depth={depth} traced={traced}");
            let result = run(&p, span(0, 1, 1), traced);
            if succeeds {
                assert_eq!(result.events.len(), 4);
                assert!(result.faults.is_empty());
            } else {
                assert!(result.events.is_empty());
                assert!(result
                    .faults
                    .iter()
                    .any(|f| f.code == FailCode::DepthExceeded));
            }
        }
    }
}

#[test]
fn callable_sound_pattern_retains_dynamic_and_static_leaf_edges() {
    let p = sound::sound(PParam::Fn(Value::Native(NativeId::new(3))), None, None);
    let result = run(&p, span(0, 1, 1), true);
    unique(&result);
    assert_eq!(result.events.len(), 1);
    let steps = &trace(&result.events[0]).steps;
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].kind, ProducerKind::DynamicExpansion);
    assert!(steps[1..].iter().all(|s| s.kind == ProducerKind::Child));
    compatible(&p, span(0, 1, 1));
    partitions(&p);
}

#[test]
fn deep_fast_slow_chains_keep_the_existing_depth_bound() {
    for (depth, succeeds) in [(200, true), (300, false)] {
        let mut p = source();
        for i in 0..depth {
            p = Rc::new(if i % 2 == 0 {
                time::fast(p, PParam::int(1), None)
            } else {
                time::slow(p, PParam::int(1), None)
            });
        }
        for traced in [false, true] {
            eprintln!("adversarial fast/slow depth={depth} traced={traced}");
            let result = run(&p, span(0, 1, 1), traced);
            if succeeds {
                assert_eq!(result.events.len(), 4);
                assert!(result.faults.is_empty());
            } else {
                assert!(result.events.is_empty());
                assert!(result
                    .faults
                    .iter()
                    .any(|f| f.code == FailCode::DepthExceeded));
            }
        }
    }
}
