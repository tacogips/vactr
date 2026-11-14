//! BE-INST required tests (design 12.8.6, B2, B3): realization of `inst`
//! to `InstDef`, ugen typing and the DSP overloads, the prelude templates,
//! effects in three positions, capability checks, implicit control names,
//! buses and sound resolution. The shared session harness is here.

mod buses;
mod capability;
mod effects;
mod implicit;
mod instdef;
mod overloads;
mod resolve;
mod templates;
mod ugens;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::controls;
use crate::dsp::graph::{InstDef, UGenSpec};
use crate::host::wire::Ctl;
use crate::ns::evaluator::{Evaluator, FormOutcome};
use crate::ns::insts::InstRegistry;
use crate::ns::load::NoopHost;
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, StagedEffect};
use crate::pattern::query::{QueryResult, TimeSpan};
use crate::pattern::{query, InputCells, QueryCtx};
use crate::reader::span::FileId;
use crate::sched::slots::CtlId;
use crate::value::intern::intern_kw;
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::VmQuery;

/// Records every released effect.
pub(super) struct Rec(pub Rc<RefCell<Vec<StagedEffect>>>);

impl EffectSink for Rec {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().push(effect);
    }
}

/// An evaluator over the core prelude with a shared registry and a
/// recording sink.
pub(super) struct Session {
    pub ev: Evaluator,
    pub effects: Rc<RefCell<Vec<StagedEffect>>>,
    pub reg: Rc<RefCell<InstRegistry>>,
}

impl Session {
    pub fn new() -> Self {
        Self::with(InstRegistry::shared())
    }

    pub fn with(reg: Rc<RefCell<InstRegistry>>) -> Self {
        let effects = Rc::new(RefCell::new(Vec::new()));
        let ev = Evaluator::with_insts(
            Prelude::core(),
            Box::new(NoopHost),
            Box::new(Rec(Rc::clone(&effects))),
            Rc::clone(&reg),
        );
        Self { ev, effects, reg }
    }

    /// Every form's outcome.
    pub fn eval(&mut self, src: &str) -> Vec<FormOutcome> {
        self.ev
            .eval_str(src, FileId::new(1))
            .unwrap_or_else(|d| panic!("{src:?}: {d}"))
    }

    /// The last form's value; every form must succeed.
    pub fn ok(&mut self, src: &str) -> Value {
        let out = self.eval(src);
        for o in &out {
            if let Err(f) = &o.value {
                panic!("{src:?}: {f}");
            }
        }
        out.last()
            .and_then(|o| o.value.clone().ok())
            .unwrap_or(Value::Nil)
    }

    /// The failure code of the last form.
    pub fn fails(&mut self, src: &str) -> FailCode {
        let out = self.eval(src);
        match out.last().map(|o| &o.value) {
            Some(Err(f)) => f.code,
            other => panic!("{src:?}: expected a failure, got {other:?}"),
        }
    }

    /// The installed template of `name`.
    pub fn def(&self, name: &str) -> Arc<InstDef> {
        let r = self.reg.borrow();
        let id = r
            .id_of(intern_kw(name))
            .unwrap_or_else(|| panic!("no inst {name}"));
        Arc::clone(&r.entry(id).expect("entry").def)
    }

    /// How many `Install` effects were released since the last `clear`.
    pub fn installs(&self) -> usize {
        self.effects
            .borrow()
            .iter()
            .filter(|e| matches!(e, StagedEffect::Install(_)))
            .count()
    }

    pub fn clear(&self) {
        self.effects.borrow_mut().clear();
    }

    /// Queries cycle `c` of a pattern value.
    pub fn query(&mut self, v: &Value, c: i64) -> QueryResult {
        let Value::Pattern(p) = v else {
            panic!("not a pattern: {v:?}");
        };
        let cells = InputCells::new();
        let (vm, ns) = self.ev.vm_and_ns();
        let mut handle = VmQuery::new(vm, ns);
        let mut cx = QueryCtx::new(&mut handle, &cells, 1);
        query(p, TimeSpan::cycle(c).expect("cycle"), &mut cx)
    }
}

/// The wire id of a control-table name.
pub(super) fn ctl(name: &str) -> CtlId {
    controls::row(name).expect("a control").ctl
}

/// A short name for a node.
pub(super) fn spec_name(s: &UGenSpec) -> String {
    match s {
        UGenSpec::Param(c) => format!(
            "param:{}",
            controls::row_by_id(*c).map_or_else(|| c.get().to_string(), |r| r.name.to_string())
        ),
        UGenSpec::Const(v) => format!("const:{v}"),
        UGenSpec::Effect(e) => format!("fx:{}", e.kind.name()),
        other => format!("{other:?}")
            .split(['(', ' ', '{'])
            .next()
            .unwrap_or("")
            .to_lowercase(),
    }
}

/// The node kinds of a template, in order.
pub(super) fn kinds(def: &InstDef) -> Vec<String> {
    def.nodes.iter().map(spec_name).collect()
}

/// The node feeding port `port` of node `to`.
pub(super) fn input(def: &InstDef, to: u16, port: u8) -> Option<u16> {
    def.edges
        .iter()
        .find(|e| e.to == to && e.port == port)
        .map(|e| e.from)
}

/// The output node.
pub(super) fn out(def: &InstDef) -> u16 {
    u16::try_from(def.nodes.len() - 1).expect("small")
}

/// The name of node `i`.
pub(super) fn name_at(def: &InstDef, i: Option<u16>) -> String {
    i.and_then(|i| def.nodes.get(usize::from(i)))
        .map(spec_name)
        .unwrap_or_default()
}

/// The default of header parameter `name`.
pub(super) fn param(def: &InstDef, name: &str) -> Option<Ctl> {
    let c = ctl(name);
    def.params.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}
