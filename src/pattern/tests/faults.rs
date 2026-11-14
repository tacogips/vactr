//! Failure granularity (design 10.3): event-local and subtree-local faults
//! with origins; `query` never fails as a whole.

use std::rc::Rc;

use super::*;
use crate::ns::namespace::FormGen;
use crate::pattern::combinators::random::degrade_by;
use crate::pattern::combinators::sound::sound;
use crate::pattern::combinators::structure::stack;
use crate::pattern::combinators::time::{every, fast};
use crate::pattern::pat::PParam;
use crate::pattern::step::{steps, steps_of_list};
use crate::reader::span::{FileId, Span};
use crate::value::value::{ListProv, ListVal};
use crate::vm::fail::{FailCode, Failure, Origin};

fn provenanced(items: Vec<Value>, file: FileId) -> Value {
    let elems: Vec<Span> = (0..items.len())
        .map(|i| {
            let start = 10 * u32::try_from(i).unwrap();
            Span::new(file, start, start + 4)
        })
        .collect();
    Value::List(Rc::new(ListVal {
        items: items.into_boxed_slice(),
        prov: Some(Rc::new(ListProv {
            form_gen: FormGen::new(1),
            doc_revision: 1,
            elems: elems.into_boxed_slice(),
        })),
    }))
}

#[test]
fn mixed_valid_and_failing_events_keep_siblings() {
    let file = FileId::new(5);
    let Value::List(l) = provenanced(vec![kw("bd"), kw("nope"), kw("sd"), kw("gone")], file) else {
        unreachable!()
    };
    let src = Rc::new(steps(steps_of_list(&l), None));
    let node_span = Some(Span::new(file, 100, 120));
    let p = sound(PParam::Pat(src), None, node_span);
    let res = run(&p, cycle(2));
    assert_eq!(
        names(&res.events),
        vec![(r(2, 1), "bd".into()), (r(5, 2), "sd".into())]
    );
    assert_eq!(res.faults.len(), 2, "one fault per failure");
    // Each fault carries its element's span and its beat.
    let origins: Vec<Origin> = res.faults.iter().map(|f| f.origin.clone()).collect();
    assert_eq!(origins[0].span, Some(Span::new(file, 10, 14)));
    assert_eq!(origins[0].beat, Some(r(9, 1)));
    assert_eq!(origins[1].span, Some(Span::new(file, 30, 34)));
    assert_eq!(origins[1].beat, Some(r(11, 1)));
    assert!(res.faults.iter().all(|f| f.code == FailCode::UnknownSound));
}

#[test]
fn a_failing_transform_is_subtree_local() {
    let mut vm = StubVm::new();
    let bad = vm.define(20, |_| Ok(Value::Int(3)));
    let span_every = Some(Span::new(FileId::new(1), 0, 9));
    let failing = every(pat(list(vec![kw("bd")])), PParam::int(1), bad, span_every);
    let ok = (*pat(list(vec![kw("hh"), kw("hh")]))).clone();
    let p = stack(vec![failing, ok], None);
    let res = run_with(&p, cycle(0), &mut vm);
    assert_eq!(res.events.len(), 2, "the sibling branch still plays");
    assert_eq!(res.faults.len(), 1);
    assert_eq!(res.faults[0].code, FailCode::Type);
    assert_eq!(res.faults[0].origin.span, span_every);
}

#[test]
fn a_failing_per_event_parameter_is_event_local() {
    let mut vm = StubVm::new();
    // A probability function that fails in the second half of the cycle.
    let prob = vm.define(21, |args| match args.first() {
        Some(Value::Ratio(t)) if *t >= Ratio64::new(1, 2).unwrap() => {
            Err(Failure::new(FailCode::DivisionByZero, "boom"))
        }
        _ => Ok(Value::Int(0)),
    });
    let p = degrade_by(
        pat(list(vec![kw("a"), kw("b"), kw("c"), kw("d")])),
        PParam::Fn(prob),
        None,
    );
    let res = run_with(&p, cycle(0), &mut vm);
    assert_eq!(
        names(&res.events),
        vec![(r(0, 1), "a".into()), (r(1, 4), "b".into())]
    );
    assert_eq!(res.faults.len(), 2);
    assert_eq!(res.faults[0].origin.beat, Some(r(2, 1)));
    assert_eq!(res.faults[1].origin.beat, Some(r(3, 1)));
}

#[test]
fn a_self_sampling_pattern_ends_in_depth_exceeded() {
    let mut vm = StubVm::new();
    // f(t) returns a pattern whose factor is f again: unbounded without the
    // depth bound.
    let me = Value::Native(crate::value::value::NativeId::new(22));
    let again = me.clone();
    vm.define(22, move |_| {
        Ok(Value::Pattern(Rc::new(fast(
            konst(kw("bd")),
            PParam::Fn(again.clone()),
            None,
        ))))
    });
    let p = fast(konst(kw("bd")), PParam::Fn(me), None);
    let res = run_with(&p, cycle(0), &mut vm);
    assert!(res.events.is_empty());
    assert!(res.faults.iter().any(|f| f.code == FailCode::DepthExceeded));
}

#[test]
fn an_enormous_pattern_exhausts_the_budget_without_hanging() {
    let p = fast(
        konst(kw("hh")),
        PParam::Const(Value::Int64(1_000_000_000)),
        None,
    );
    let res = run(&p, cycle(0));
    assert!(res.events.is_empty());
    assert_eq!(res.faults.len(), 1);
    assert_eq!(res.faults[0].code, FailCode::FuelExhausted);
    // Ratio overflow is a fault, never a panic.
    let huge = fast(konst(kw("hh")), PParam::Const(Value::Int64(i64::MAX)), None);
    let res = run(&huge, span(r(1, 1), r(2, 1)));
    assert!(res.events.is_empty());
    assert_eq!(res.faults[0].code, FailCode::Overflow);
}

#[test]
fn captured_print_output_is_returned() {
    let mut vm = StubVm::new();
    vm.output
        .push((Origin::none(), Rc::from("printed in query")));
    let res = run_with(&s(kw("bd")), cycle(0), &mut vm);
    assert_eq!(res.output.len(), 1);
    assert_eq!(&*res.output[0].1, "printed in query");
    assert!(vm.output.is_empty());
}

#[test]
fn deep_chains_work_and_too_deep_chains_fault() {
    use crate::pattern::combinators::time::rev;
    let chain = |n: usize| {
        let mut p = pat(list(vec![kw("a"), kw("b")]));
        for _ in 0..n {
            p = Rc::new(rev(p, None));
        }
        p
    };
    // 200 reversals: an even count restores the order.
    let res = run(&chain(200), cycle(0));
    assert!(res.faults.is_empty());
    assert_eq!(
        names(&res.events),
        vec![(r(0, 1), "a".into()), (r(1, 2), "b".into())]
    );
    let res = run(&chain(300), cycle(0));
    assert!(res.events.is_empty());
    assert_eq!(res.faults[0].code, FailCode::DepthExceeded);
}
