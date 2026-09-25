//! Forcing-mask tests (vactrol-core.md TASK-005 forcing criterion, design
//! 5.5). Each case counts runs of an effectful thunk through a `var`.

use super::*;
use crate::ns::stage::StagedEffect;
use crate::types::diag::DiagCode;
use crate::types::masks::{chase, mixed_forcing, EffectiveEntry, MaskEntry};

const BUMP: &str = "{upd hits {+ hits 1}}";

fn with_hits(prelude: &str) -> Sess {
    let mut s = Sess::probe();
    s.eval(&format!("var hits 0\n{prelude}")).expect("setup");
    s
}

fn hits(s: &Sess) -> String {
    s.get_str("hits")
}

fn closure_mask(s: &Sess, name: &str) -> crate::types::masks::ForcingMask {
    match s.get(name) {
        Value::Fn(c) => c.mask.clone(),
        other => panic!("`{name}` is {other:?}"),
    }
}

#[test]
fn ignored_effectful_thunk_runs_exactly_once() {
    let mut s = with_hits("fn ignore x:\n\t7");
    assert_eq!(s.show(&format!("ignore {BUMP}")), "7");
    assert_eq!(hits(&s), "1");
}

#[test]
fn doubly_read_value_parameter_forces_once() {
    let mut s = with_hits("fn twice x:\n\t+ x x");
    assert_eq!(s.show(&format!("twice {BUMP}")), "2");
    assert_eq!(hits(&s), "1");
}

#[test]
fn wrapper_forwarding_twice_to_a_numeric_consumer_runs_once() {
    let mut s = with_hits("fn num a:\n\t+ a 0\nfn wrap y:\n\tnum y\n\tnum y");
    assert!(matches!(
        closure_mask(&s, "wrap").0[0],
        MaskEntry::Forward { .. }
    ));
    assert_eq!(s.show(&format!("wrap {BUMP}")), "1");
    assert_eq!(hits(&s), "1");
}

#[test]
fn conditional_wrapper_whose_forward_is_skipped_still_forces_once() {
    let mut s = with_hits("fn num a:\n\t+ a 0\nfn cond-wrap flag y:\n\tif flag {num y} 0");
    assert!(matches!(
        closure_mask(&s, "cond-wrap").0[1],
        MaskEntry::Forward { .. }
    ));
    assert_eq!(s.show(&format!("cond-wrap false {BUMP}")), "0");
    assert_eq!(hits(&s), "1");
}

#[test]
fn boundary_effect_order_is_left_to_right() {
    let mut s = Sess::probe();
    s.eval("var log []\nfn note-it k:\n\tupd log {put log k}\n\t+ k 0")
        .expect("setup");
    s.eval("fn two a b:\n\t+ a b\ntwo {note-it 1} {note-it 2}")
        .expect("two");
    assert_eq!(s.get_str("log"), "[1 2]");
    // Mixed kinds: `a` and `c` are forced at the boundary, `f` is passed
    // and runs later in the body.
    s.eval("upd log []\nfn mix a f c:\n\tf\n\t+ a c\nmix {note-it 1} {note-it 2} {note-it 3}")
        .expect("mix");
    assert_eq!(s.get_str("log"), "[1 3 2]");
}

#[test]
fn wrapper_forwarding_to_at_passes_the_thunk_unevaluated() {
    let mut s = with_hits("fn later body:\n\tat 4 body");
    assert_eq!(s.show(&format!("later {BUMP}")), "nil");
    assert_eq!(hits(&s), "0");
    let staged = s.sink.effects.iter().find_map(|e| match e {
        StagedEffect::OneShot { at, value, .. } => Some((*at, value.clone())),
        _ => None,
    });
    let (at, value) = staged.expect("a one-shot was staged");
    assert_eq!(at.map(|r| r.to_string()), Some("4".into()));
    assert!(matches!(value, Value::Thunk(_)), "{value:?}");
}

#[test]
fn late_masked_domain_parameter_keeps_its_thunk() {
    let mut s = with_hits("");
    let v = s.eval(&format!("late-probe {BUMP}")).expect("late-probe");
    assert!(matches!(v, Value::Thunk(_)), "{v:?}");
    assert_eq!(hits(&s), "0");
}

#[test]
fn fan_out_forward_value_vs_fn_forces_once_and_is_mixed() {
    let mut s = with_hits(
        "fn val-use a:\n\t+ a 0\nfn fn-use g:\n\tif false {g} 1\nfn fan y:\n\tfn-use y\n\tval-use y",
    );
    let mask = closure_mask(&s, "fan");
    let MaskEntry::Forward { links } = &mask.0[0] else {
        panic!("{mask:?}");
    };
    assert_eq!(links.len(), 2);
    let ns = &s.ns;
    assert_eq!(
        chase(links, &|c| ns.callee_mask(c)),
        (EffectiveEntry::Value, true)
    );
    assert_eq!(mixed_forcing(&mask, &|c| ns.callee_mask(c)), vec![0]);
    assert_eq!(s.show(&format!("fan {BUMP}")), "1");
    assert_eq!(hits(&s), "1");
}

#[test]
fn undetermined_parameter_runs_at_most_once() {
    let mut s = with_hits("fn und f x:\n\tf x\n\tf x");
    assert_eq!(closure_mask(&s, "und").0[1], MaskEntry::Undetermined);
    assert!(s.diags.iter().any(|d| d.code == DiagCode::LatentForcing));
    assert_eq!(s.show(&format!("und {{y -> + y 0}} {BUMP}")), "1");
    assert_eq!(hits(&s), "1");
    // Never demanded: never run (the callee's `g` is an `Fn` parameter it
    // does not call).
    s.eval("fn und2 f x:\n\tf 1 x").expect("und2");
    assert_eq!(
        s.show(&format!("und2 {{a g -> if false {{g}} a}} {BUMP}")),
        "1"
    );
    assert_eq!(hits(&s), "1");
}

#[test]
fn masks_are_identical_with_diagnostics_off() {
    use crate::expand::{expand, ExpandCx};
    use crate::types::masks::infer_masks;
    let srcs = [
        "fn later body:\n\tat 4 body",
        "fn twice x:\n\t+ x x",
        "fn und f x:\n\tf x",
        "fn maybe-do body:\n\tbody",
    ];
    for src in srcs {
        let mut s = Sess::probe();
        let r = read(src, FILE, &AliasEnv::new());
        let node = expand(&r.nodes[0], &mut ExpandCx::new(r.next_node_id())).expect("expand");
        let direct = infer_masks(&node, &s.ns.check_env());
        s.eval(src).expect("define");
        let name = src.split_whitespace().nth(1).expect("name");
        assert_eq!(closure_mask(&s, name), direct, "{src}");
    }
}

#[test]
fn redefining_the_directly_called_callee_switches_at_the_next_call() {
    let mut s = with_hits(&format!("fn target x:\n\t+ x 0\nfn go k:\n\ttarget {BUMP}"));
    s.eval("go 0").ok();
    assert_eq!(hits(&s), "1");
    // Now `target` calls its parameter only conditionally: `Fn`.
    s.eval("fn target x:\n\tif false {x} 0").expect("redefine");
    s.eval("go 0").ok();
    assert_eq!(hits(&s), "1");
    s.eval("fn target x:\n\t+ x 0").expect("redefine back");
    s.eval("go 0").ok();
    assert_eq!(hits(&s), "2");
}

#[test]
fn redefining_only_a_downstream_callee_switches_the_unchanged_wrapper() {
    let mut s = with_hits("fn later body:\n\tat 4 body");
    let before = closure_mask(&s, "later");
    s.eval(&format!("later {BUMP}")).expect("prelude at: Fn");
    assert_eq!(hits(&s), "0");
    s.eval("let at value-at").expect("at is Value");
    s.eval(&format!("later {BUMP}")).expect("value");
    assert_eq!(hits(&s), "1");
    s.eval("let at late-at").expect("at is Late");
    s.eval(&format!("later {BUMP}")).expect("late");
    assert_eq!(hits(&s), "1");
    s.eval("let at fn-at").expect("at is Fn");
    s.eval(&format!("later {BUMP}")).expect("fn");
    assert_eq!(hits(&s), "1");
    s.eval("let at value-at").expect("at is Value again");
    s.eval(&format!("later {BUMP}")).expect("value again");
    assert_eq!(hits(&s), "2");
    assert_eq!(closure_mask(&s, "later"), before, "`later` is untouched");
}

/// Records the names of the top-level slots read eagerly (5.6 `Deref`
/// instrumentation, the seam ME-REACTIVE builds on).
struct Reads(std::rc::Rc<std::cell::RefCell<Vec<String>>>);

impl crate::vm::vm::ReadObserver for Reads {
    fn on_read(&mut self, slot: &crate::ns::namespace::VarSlotRef) -> Result<(), Failure> {
        let name = crate::value::intern::name_of_sym(slot.name()).to_string();
        self.0.borrow_mut().push(name);
        Ok(())
    }
}

#[test]
fn eager_reads_are_observed_and_late_captures_are_not() {
    let mut s = Sess::probe();
    s.eval("let a 1\nvar v 2\nfn f k:\n\t+ k 0").expect("setup");
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    s.vm.set_read_observer(Some(Box::new(Reads(std::rc::Rc::clone(&log)))));
    s.eval("+ a v").expect("eager");
    assert_eq!(*log.borrow(), ["a", "v"]);
    log.borrow_mut().clear();
    // A var captured by a `Late` parameter is not an eager read; the
    // prelude and frame locals are never reported.
    s.eval("ctl :x v").expect("late");
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
    s.eval("fn g k:\n\tvar n k\n\tupd n {+ n 1}\n\tn")
        .expect("g");
    log.borrow_mut().clear();
    s.eval("g 1").expect("locals");
    assert_eq!(*log.borrow(), ["g"], "only the callee's own slot");
}

#[test]
fn an_observer_failure_aborts_the_read() {
    struct Abort;
    impl crate::vm::vm::ReadObserver for Abort {
        fn on_read(&mut self, _: &crate::ns::namespace::VarSlotRef) -> Result<(), Failure> {
            Err(Failure::new(
                crate::vm::fail::FailCode::Blocked,
                "blocked-on: a",
            ))
        }
    }
    let mut s = Sess::probe();
    s.eval("let a 1").expect("setup");
    s.vm.set_read_observer(Some(Box::new(Abort)));
    assert_eq!(s.show("+ a 1"), "fail: blocked");
    s.vm.set_read_observer(None);
    assert_eq!(s.show("+ a 1"), "2");
}
