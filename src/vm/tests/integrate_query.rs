//! INTEGRATE: `QueryVm` for the VM (design 7.1.3, 10.4) and the shared
//! evaluator harness of the `integrate_*` tests (read -> expand -> check ->
//! compile -> run through `Evaluator`, a recording sink, and queries through
//! `VmQuery`).

use std::cell::RefCell;
use std::rc::Rc;

use crate::ns::evaluator::{Evaluator, FormOutcome};
use crate::ns::load::{NoopHost, SourceLoader};
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, RecordingSink, SlotKey, StagedEffect};
use crate::pattern::eval::{InputCells, QueryCtx, QueryVm};
use crate::pattern::query::{query, QueryResult, TimeSpan};
use crate::types::diag::{DiagCode, Severity};
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::query_vm::VmQuery;
use crate::vm::tests::FILE;

/// A sink the test keeps a handle to.
struct Shared(Rc<RefCell<RecordingSink>>);

impl EffectSink for Shared {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().apply(effect);
    }
}

/// An evaluator over the core prelude (the evaluator adds the domain) with
/// `NoopHost` and a recording sink.
pub(super) struct Ev {
    pub ev: Evaluator,
    pub sink: Rc<RefCell<RecordingSink>>,
    pub cells: InputCells,
}

impl Ev {
    pub(super) fn new() -> Ev {
        Ev::with_prelude(Prelude::core())
    }

    pub(super) fn with_prelude(p: Prelude) -> Ev {
        Ev::with_loader(p, Box::new(NoopHost))
    }

    pub(super) fn with_loader(p: Prelude, loader: Box<dyn SourceLoader>) -> Ev {
        let sink = Rc::new(RefCell::new(RecordingSink::default()));
        let ev = Evaluator::new(p, loader, Box::new(Shared(Rc::clone(&sink))));
        Ev {
            ev,
            sink,
            cells: InputCells::new(),
        }
    }

    /// Evaluates every form of `src` (which must read and expand clean).
    pub(super) fn run(&mut self, src: &str) -> Vec<FormOutcome> {
        self.ev
            .eval_str(src, FILE)
            .unwrap_or_else(|d| panic!("{src:?}: {d}"))
    }

    /// The value of the last form of `src`.
    pub(super) fn last(&mut self, src: &str) -> Result<Value, crate::vm::fail::Failure> {
        self.run(src).pop().expect("a form").value
    }

    /// The codes of the error and warning diagnostics of `src`'s forms.
    pub(super) fn codes(outcomes: &[FormOutcome]) -> Vec<DiagCode> {
        outcomes
            .iter()
            .flat_map(|o| o.diags.iter())
            .filter(|d| d.severity != Severity::Hint)
            .map(|d| d.code)
            .collect()
    }

    /// The value most recently bound to `slot` and released.
    pub(super) fn bound(&self, slot: SlotKey) -> Option<Value> {
        self.sink
            .borrow()
            .effects
            .iter()
            .rev()
            .find_map(|e| match e {
                StagedEffect::SlotBind { slot: s, value } if *s == slot => Some(value.clone()),
                _ => None,
            })
    }

    /// The number of released slot binds.
    pub(super) fn bind_count(&self) -> usize {
        self.sink
            .borrow()
            .effects
            .iter()
            .filter(|e| matches!(e, StagedEffect::SlotBind { .. }))
            .count()
    }

    /// The session value `name`.
    pub(super) fn get(&self, name: &str) -> Value {
        self.ev
            .ns()
            .session_value(name)
            .unwrap_or_else(|| panic!("`{name}` is not bound"))
    }

    /// Queries a pattern value over cycle `c` through the real VM.
    pub(super) fn query(&mut self, v: &Value, c: i64) -> QueryResult {
        let Value::Pattern(p) = v else {
            panic!("not a pattern: {v}");
        };
        let (vm, ns) = self.ev.vm_and_ns();
        let mut handle = VmQuery::new(vm, ns);
        let mut cx = QueryCtx::new(&mut handle, &self.cells, 7);
        query(p, TimeSpan::cycle(c).expect("cycle"), &mut cx)
    }

    /// The events' values of one cycle, printed.
    pub(super) fn values(&mut self, v: &Value, c: i64) -> Vec<String> {
        let r = self.query(v, c);
        r.events.iter().map(|e| e.value.to_string()).collect()
    }

    /// A handle for direct `QueryVm` calls.
    pub(super) fn with_handle<T>(&mut self, f: impl FnOnce(&mut VmQuery<'_>) -> T) -> T {
        let (vm, ns) = self.ev.vm_and_ns();
        let mut handle = VmQuery::new(vm, ns);
        f(&mut handle)
    }
}

#[test]
fn late_param_rereads_a_var_per_query() {
    let mut h = Ev::new();
    h.run("var k 2\nlet p fast [1 2] k");
    let p = h.get("p");
    assert_eq!(h.values(&p, 0), ["1", "2", "1", "2"]);
    h.run("upd k 1");
    // The same pattern object: the `var` is read per query (`PParam::Late`).
    assert_eq!(h.values(&p, 0), ["1", "2"]);
}

#[test]
fn fn_param_calls_back_in_query_mode() {
    let mut h = Ev::new();
    h.run("let p fast [1 2] {print 2}");
    let before = h.sink.borrow().console().len();
    let p = h.get("p");
    let r = h.query(&p, 0);
    assert_eq!(r.events.len(), 4, "{:?}", r.faults);
    // `print` inside the query is captured, never staged.
    assert!(r.output.iter().all(|(_, t)| &**t == "2"));
    assert!(!r.output.is_empty());
    assert_eq!(h.sink.borrow().console().len(), before);
}

#[test]
fn global_upd_inside_a_query_is_effect_in_query() {
    let mut h = Ev::new();
    h.run("var g 0\nlet p fast [1 2] {upd g 2}");
    let p = h.get("p");
    let r = h.query(&p, 0);
    assert!(r.events.is_empty());
    assert_eq!(r.faults[0].code, FailCode::EffectInQuery, "{:?}", r.faults);
    assert_eq!(h.get("g").to_string(), "0");
}

#[test]
fn each_call_gets_its_own_fuel_and_restores_the_callers() {
    let mut h = Ev::new();
    h.run("fn cheap:\n\t+ 1 2\nfn costly:\n\tlen {take 0.. 5000}");
    let cheap = h.get("cheap");
    let costly = h.get("costly");
    h.ev.vm_mut().fuel_limit = 2000;
    h.ev.vm_mut().set_fuel(0);
    let r = h.with_handle(|q| {
        let a = q.call(&cheap, &[]);
        let b = q.call(&cheap, &[]);
        let c = q.call(&costly, &[]);
        (a, b, c)
    });
    assert_eq!(r.0.expect("a fresh budget").to_string(), "3");
    assert_eq!(r.1.expect("a fresh budget again").to_string(), "3");
    assert_eq!(r.2.expect_err("over budget").code, FailCode::FuelExhausted);
    assert_eq!(h.ev.vm_mut().fuel(), 0, "the caller's budget is restored");
}

#[test]
fn each_call_counts_as_a_reentry() {
    let mut h = Ev::new();
    h.run("fn cheap:\n\t+ 1 2");
    let cheap = h.get("cheap");
    h.ev.vm_mut().reentry_limit = 0;
    let r = h.with_handle(|q| q.call(&cheap, &[]));
    assert_eq!(
        r.expect_err("no re-entry left").code,
        FailCode::DepthExceeded
    );
}

#[test]
fn query_mode_is_restored_after_a_failing_call() {
    let mut h = Ev::new();
    h.run("var g 0\nfn bad:\n\tupd g 1");
    let bad = h.get("bad");
    let r = h.with_handle(|q| q.call(&bad, &[]));
    assert_eq!(r.expect_err("query mode").code, FailCode::EffectInQuery);
    // Back in Normal mode: the same write works at the top level.
    assert!(h.last("upd g 3").is_ok());
    assert_eq!(h.get("g").to_string(), "3");
}

#[test]
fn deref_reads_the_current_value() {
    let mut h = Ev::new();
    h.run("var k 5");
    let slot =
        h.ev.ns()
            .session_slot(crate::value::intern::intern_sym("k"))
            .expect("k");
    assert_eq!(
        h.with_handle(|q| q.deref(&slot))
            .expect("bound")
            .to_string(),
        "5"
    );
    h.run("upd k 6");
    assert_eq!(
        h.with_handle(|q| q.deref(&slot))
            .expect("bound")
            .to_string(),
        "6"
    );
}
