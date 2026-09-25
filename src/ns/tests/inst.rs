//! Evaluator wiring of instruments (12.8.3, 12.8.6): the shared registry,
//! template installs at construction, and reactive rebuilds of an `inst`
//! whose body reads a session binding.

use std::cell::RefCell;
use std::rc::Rc;

use crate::dsp::graph::UGenSpec;
use crate::host::caps::GraphHandle;
use crate::ns::evaluator::Evaluator;
use crate::ns::insts::{InstRegistry, TEMPLATE_NAMES};
use crate::ns::load::NoopHost;
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, StagedEffect};
use crate::reader::span::FileId;
use crate::value::intern::intern_kw;

struct Rec(Rc<RefCell<Vec<StagedEffect>>>);

impl EffectSink for Rec {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().push(effect);
    }
}

fn evaluator(reg: &Rc<RefCell<InstRegistry>>) -> (Evaluator, Rc<RefCell<Vec<StagedEffect>>>) {
    let fx = Rc::new(RefCell::new(Vec::new()));
    let ev = Evaluator::with_insts(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(Rec(Rc::clone(&fx))),
        Rc::clone(reg),
    );
    (ev, fx)
}

fn inst_installs(fx: &RefCell<Vec<StagedEffect>>) -> usize {
    fx.borrow()
        .iter()
        .filter(|e| matches!(e, StagedEffect::Install(GraphHandle::Inst { .. })))
        .count()
}

#[test]
fn the_registry_is_shared_and_the_templates_are_installed_at_construction() {
    let reg = InstRegistry::shared();
    let (ev, fx) = evaluator(&reg);
    assert!(Rc::ptr_eq(&ev.insts().expect("registry"), &reg));
    assert_eq!(inst_installs(&fx), TEMPLATE_NAMES.len());
    for name in TEMPLATE_NAMES {
        assert!(reg.borrow().id_of(intern_kw(name)).is_some(), "{name}");
    }
    // `Evaluator::new` builds its own.
    let ev = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(crate::ns::stage::RecordingSink::default()),
    );
    let own = ev.insts().expect("registry");
    assert!(!Rc::ptr_eq(&own, &reg));
    assert!(own.borrow().id_of(intern_kw("sampler")).is_some());
}

#[test]
fn an_inst_reading_a_session_binding_is_rebuilt_when_it_changes() {
    let reg = InstRegistry::shared();
    let (mut ev, fx) = evaluator(&reg);
    let run = |ev: &mut Evaluator, src: &str| {
        for o in ev.eval_str(src, FileId::new(1)).expect("reads") {
            assert!(o.value.is_ok(), "{src:?}: {:?}", o.value);
        }
    };
    run(&mut ev, "let base 220\ninst k:\n\tsaw base");
    let konst = |reg: &Rc<RefCell<InstRegistry>>| {
        let r = reg.borrow();
        let id = r.id_of(intern_kw("k")).expect("k");
        let nodes = r.entry(id).expect("entry").def.nodes.clone();
        nodes.iter().find_map(|n| match n {
            UGenSpec::Const(v) => Some(*v),
            _ => None,
        })
    };
    assert_eq!(konst(&reg), Some(220.0));
    fx.borrow_mut().clear();
    run(&mut ev, "let base 330");
    assert_eq!(konst(&reg), Some(330.0));
    assert_eq!(inst_installs(&fx), 1, "the rebuilt inst is installed again");
    assert_eq!(ev.edges("k"), ["base"]);
}
