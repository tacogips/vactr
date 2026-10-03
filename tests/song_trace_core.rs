//! Full producer identity core; canonical arrangement realization is SONG-04.
use std::collections::BTreeSet;
use std::rc::Rc;
use vactr::ns::namespace::VarSlotRef;
use vactr::pattern::eval::{InputCells, QueryCtx, QueryVm};
use vactr::pattern::occ::{ProducerKind, ProducerStep, ProducerTrace};
use vactr::pattern::pat::{PParam, Pat, PatNode};
use vactr::pattern::query::{query, query_traced, Event, QueryResult, TimeSpan};
use vactr::pattern::step::{steps, Step};
use vactr::reader::span::NodeId;
use vactr::value::intern::intern_kw;
use vactr::value::value::{NativeId, Value};
use vactr::vm::fail::{FailCode, Failure, Origin};

#[derive(Default)]
struct TestVm {
    output: Vec<(Origin, Rc<str>)>,
    returned: Option<Value>,
}
impl QueryVm for TestVm {
    fn call(&mut self, f: &Value, _args: &[Value]) -> Result<Value, Failure> {
        if let Some(value) = &self.returned {
            return Ok(value.clone());
        }
        match f {
            Value::Native(id) if id.get() == 1 => {
                Ok(Value::list(vec![Value::Int(7), Value::Int(7)]))
            }
            _ => Err(Failure::new(FailCode::DivisionByZero, "fixture failure")),
        }
    }
    fn deref(&mut self, slot: &VarSlotRef) -> Result<Value, Failure> {
        Ok(slot.get())
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
}
fn span(begin: i64, end: i64, denominator: i64) -> TimeSpan {
    TimeSpan::new(
        vactr::value::ratio::Ratio64::new(begin, denominator).unwrap(),
        vactr::value::ratio::Ratio64::new(end, denominator).unwrap(),
    )
    .unwrap()
}
fn run(p: &Pat, span: TimeSpan, traced: bool) -> QueryResult {
    let mut vm = TestVm::default();
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 42);
    if traced {
        query_traced(p, span, &mut cx)
    } else {
        query(p, span, &mut cx)
    }
}
fn path(event: &Event) -> &[ProducerStep] {
    &event.producer.as_ref().unwrap().steps
}
fn edge(kind: ProducerKind, ordinal: u32) -> ProducerStep {
    ProducerStep { kind, ordinal }
}
fn list(values: Vec<Value>) -> Pat {
    steps(values.into_iter().map(Step::bare).collect(), None)
}

#[test]
fn nested_leaf_paths_are_assigned_before_rests_and_query_clipping() {
    let p = list(vec![
        Value::Nil,
        Value::list(vec![Value::Int(7), Value::Int(7)]),
        Value::Int(9),
    ]);
    let all = run(&p, span(0, 1, 1), true);
    assert!(all.faults.is_empty());
    assert_eq!(all.events.len(), 3);
    assert_eq!(
        path(&all.events[0]),
        &[
            edge(ProducerKind::NestedStep, 1),
            edge(ProducerKind::NestedStep, 0)
        ]
    );
    assert_eq!(
        path(&all.events[1]),
        &[
            edge(ProducerKind::NestedStep, 1),
            edge(ProducerKind::NestedStep, 1)
        ]
    );
    assert_eq!(path(&all.events[2]), &[edge(ProducerKind::NestedStep, 2)]);
    let clipped = run(&p, span(5, 7, 12), true);
    assert_eq!(clipped.events.len(), 2);
    for event in &clipped.events {
        let original = all
            .events
            .iter()
            .find(|original| original.whole == event.whole)
            .unwrap();
        assert_eq!(original.producer, event.producer);
        assert_eq!(original.occ, event.occ);
        assert_eq!(original.whole, event.whole);
        assert!(!event.is_onset() || event.part.begin == event.whole.unwrap().begin);
    }
}

#[test]
fn forced_equal_node_hashes_and_replicated_steps_keep_full_distinct_paths() {
    let mut a = Pat::new(PatNode::Pure(Step::bare(Value::Int(7))), None, false);
    let mut b = a.clone();
    a.id = NodeId::new(0);
    b.id = NodeId::new(0);
    let repeated = Pat::new(PatNode::Repeat(Rc::new(a), PParam::int(2)), None, true);
    let p = list(vec![
        Value::Pattern(Rc::new(repeated)),
        Value::Pattern(Rc::new(b)),
    ]);
    let all = run(&p, span(0, 1, 1), true);
    assert!(all.faults.is_empty());
    assert_eq!(all.events.len(), 3);
    let paths: BTreeSet<_> = all.events.iter().map(|e| e.producer.clone()).collect();
    assert_eq!(paths.len(), 3);
    for (copy, event) in all.events[..2].iter().enumerate() {
        assert_eq!(
            path(event),
            &[
                edge(ProducerKind::NestedStep, 0),
                edge(ProducerKind::GeneratedBranch, copy as u32),
                edge(ProducerKind::Child, 0),
                edge(ProducerKind::Child, 0)
            ]
        );
    }
    assert_eq!(
        path(&all.events[2]),
        &[
            edge(ProducerKind::NestedStep, 1),
            edge(ProducerKind::Child, 0)
        ]
    );
}

fn callable_fixture(text: &str) -> Rc<vactr::compile::proto::Closure> {
    use vactr::compile::compiler::{compile, CompileCx};
    use vactr::ns::namespace::{FormGen, Namespace, Prelude};
    use vactr::reader::{read, AliasEnv};
    let source = read(text, vactr::reader::span::FileId::new(1), &AliasEnv::new());
    let form = vactr::expand::expand(
        &source.nodes[0],
        &mut vactr::expand::ExpandCx::new(source.next_node_id()),
    )
    .unwrap();
    let ns = Namespace::new(Prelude::empty());
    let proto = compile(&form, &mut CompileCx::new(&ns, FormGen::new(1))).unwrap();
    Rc::new(vactr::compile::proto::Closure {
        proto: proto.protos.first().cloned().unwrap_or(proto),
        captures: Box::new([]),
        mask: Default::default(),
        memo: None,
    })
}

#[test]
fn dynamically_resolved_finite_values_fault_without_eager_forcing() {
    use vactr::ns::namespace::{SlotKind, VarSlotRef};
    use vactr::pattern::build::{param_of, pattern_of};
    use vactr::song::{capture_part, Song, SongSettings};
    use vactr::value::intern::intern_sym;
    let part =
        Rc::new(capture_part(Default::default(), vactr::value::ratio::Ratio64::ONE).unwrap());
    let song = Rc::new(Song::new(Rc::clone(&part), SongSettings::default()).unwrap());
    for finite in [Value::Part(part), Value::Song(song)] {
        for returned in [finite.clone(), Value::list(vec![finite.clone()])] {
            let reference = Value::VarRef(VarSlotRef::new(
                intern_sym("finite"),
                SlotKind::Var,
                returned.clone(),
            ));
            for source in [
                reference,
                Value::Fn(callable_fixture("{t -> 0}")),
                Value::Thunk(callable_fixture("{0}")),
            ] {
                // Lazy constructors do not read the ref or invoke its callable.
                let pattern = pattern_of(&source, None).unwrap();
                let parameter = param_of(&source).unwrap();
                let mut vm = TestVm {
                    returned: Some(returned.clone()),
                    ..Default::default()
                };
                let cells = InputCells::new();
                let mut cx = QueryCtx::new(&mut vm, &cells, 42);
                let result = query_traced(&pattern, span(0, 1, 1), &mut cx);
                assert!(result.events.is_empty());
                assert!(result
                    .faults
                    .iter()
                    .any(|f| f.code == FailCode::Type && f.message.contains("finite song")));
                let siblings = list(vec![Value::Pattern(Rc::clone(&pattern)), Value::Int(42)]);
                let result = query_traced(&siblings, span(0, 1, 1), &mut cx);
                assert_eq!(result.events.len(), 1);
                assert!(matches!(result.events[0].value, Value::Int(42)));
                assert_eq!(result.faults.len(), 1);
                assert_eq!(result.faults[0].code, FailCode::Type);
                let fast = Pat::new(
                    PatNode::Fast(Rc::new(list(vec![Value::Int(1)])), parameter),
                    None,
                    true,
                );
                let result = query_traced(&fast, span(0, 1, 1), &mut cx);
                assert!(result.events.is_empty());
                assert!(result
                    .faults
                    .iter()
                    .any(|f| f.code == FailCode::Type && f.message.contains("finite song")));
            }
        }
    }
}

#[test]
fn embedded_hold_child_trace_keeps_every_wrapper_edge() {
    let leaf = Rc::new(Pat::new(
        PatNode::Pure(Step::bare(Value::Int(1))),
        None,
        false,
    ));
    let wrapped = Pat::new(PatNode::Hold(leaf, PParam::int(2)), None, false);
    let p = list(vec![Value::Pattern(Rc::new(wrapped)), Value::Int(2)]);
    let result = run(&p, span(0, 1, 1), true);
    assert!(result.faults.is_empty());
    assert_eq!(
        path(&result.events[0]),
        &[
            edge(ProducerKind::NestedStep, 0),
            edge(ProducerKind::Child, 0),
            edge(ProducerKind::Child, 0)
        ]
    );
}

#[test]
fn dynamic_expansion_and_failed_sibling_restore_the_parent_trace() {
    let p = list(vec![
        Value::Pattern(Rc::new(Pat::new(
            PatNode::Pure(Step::bare(Value::Native(NativeId::new(2)))),
            None,
            false,
        ))),
        Value::Native(NativeId::new(1)),
        Value::Int(9),
    ]);
    let all = run(&p, span(0, 1, 1), true);
    assert_eq!(all.faults.len(), 1);
    assert_eq!(all.faults[0].code, FailCode::DivisionByZero);
    assert_eq!(all.events.len(), 3);
    assert_eq!(
        path(&all.events[0]),
        &[
            edge(ProducerKind::NestedStep, 1),
            edge(ProducerKind::DynamicExpansion, 0),
            edge(ProducerKind::NestedStep, 0)
        ]
    );
    assert_eq!(
        path(&all.events[1]),
        &[
            edge(ProducerKind::NestedStep, 1),
            edge(ProducerKind::DynamicExpansion, 0),
            edge(ProducerKind::NestedStep, 1)
        ]
    );
    assert_eq!(path(&all.events[2]), &[edge(ProducerKind::NestedStep, 2)]);
}

#[test]
fn tracing_does_not_change_legacy_events_keys_faults_or_continuations() {
    let p = list(vec![
        Value::Int(3),
        Value::Native(NativeId::new(2)),
        Value::list(vec![Value::Int(4), Value::Int(5)]),
    ]);
    for window in [span(0, 2, 1), span(1, 11, 12), span(3, 7, 6)] {
        let legacy = run(&p, window, false);
        let traced = run(&p, window, true);
        assert_eq!(legacy.events.len(), traced.events.len());
        assert_eq!(legacy.faults.len(), traced.faults.len());
        for (a, b) in legacy.events.iter().zip(&traced.events) {
            assert!(a.producer.is_none());
            assert!(b.producer.is_some());
            assert_eq!(a.whole, b.whole);
            assert_eq!(a.part, b.part);
            assert_eq!(a.occ, b.occ);
            assert_eq!(format!("{:?}", a.value), format!("{:?}", b.value));
            assert_eq!(a.is_onset(), b.is_onset());
        }
        for (a, b) in legacy.faults.iter().zip(&traced.faults) {
            assert_eq!(a.code, b.code);
            assert_eq!(a.origin, b.origin);
            assert_eq!(a.message, b.message);
        }
    }
}

#[test]
fn typed_edge_encoding_preserves_kind_and_length_without_hashes() {
    let a = ProducerTrace {
        steps: vec![
            edge(ProducerKind::Child, 2),
            edge(ProducerKind::NestedStep, 3),
        ],
    };
    let b = ProducerTrace {
        steps: vec![
            edge(ProducerKind::NestedStep, 2),
            edge(ProducerKind::Child, 3),
        ],
    };
    assert_eq!(a.ordinals(), vec![0, 2, 1, 3]);
    assert_ne!(a, b);
    assert_ne!(a.ordinals(), b.ordinals());
    assert_ne!(
        a.ordinals(),
        ProducerTrace {
            steps: vec![edge(ProducerKind::Child, 2)]
        }
        .ordinals()
    );
}

#[test]
fn traced_entry_retains_depth_reentry_and_million_work_faults() {
    let leaf = Rc::new(Pat::new(
        PatNode::Pure(Step::bare(Value::Int(1))),
        None,
        false,
    ));
    let mut deep = Rc::clone(&leaf);
    for _ in 0..270 {
        deep = Rc::new(Pat::new(PatNode::Hold(deep, PParam::int(1)), None, true));
    }
    for traced in [false, true] {
        let result = run(&deep, span(0, 1, 1), traced);
        assert!(result.events.is_empty());
        assert!(result
            .faults
            .iter()
            .any(|f| f.code == FailCode::DepthExceeded));
    }
    let structured_leaf = Rc::new(list(vec![Value::Int(1)]));
    let mut reentry = Rc::clone(&structured_leaf);
    for _ in 0..70 {
        reentry = Rc::new(Pat::new(
            PatNode::Control(intern_kw("gain"), reentry, Rc::clone(&structured_leaf)),
            None,
            true,
        ));
    }
    let result = run(&reentry, span(0, 1, 1), true);
    assert!(result
        .faults
        .iter()
        .any(|f| f.code == FailCode::DepthExceeded && f.message.contains("re-entries")));
    let exhausted = run(&list(vec![Value::Int(1)]), span(0, 1_000_001, 1), true);
    assert!(exhausted.events.is_empty());
    assert!(exhausted
        .faults
        .iter()
        .any(|f| f.code == FailCode::FuelExhausted));
}

#[test]
fn programmatic_nested_lists_cannot_bypass_the_query_depth_bound() {
    let mut value = Value::Int(1);
    for _ in 0..300 {
        value = Value::list(vec![value]);
    }
    let pattern = list(vec![value]);
    for traced in [false, true] {
        let result = run(&pattern, span(0, 1, 1), traced);
        assert!(result.events.is_empty());
        assert_eq!(result.faults.len(), 1);
        assert_eq!(result.faults[0].code, FailCode::DepthExceeded);
    }
}
