//! Reactive propagation (vactr-core.md TASK-005 reactive criterion, design
//! 5.6 revised), and the evaluator harness the other reactive tests share.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ns::depgraph::FormState;
use crate::ns::evaluator::{Evaluator, FormOutcome, PassEvent};
use crate::ns::load::SourceLoader;
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, RecordingSink, SlotKey, StagedEffect};
use crate::reader::span::FileId;
use crate::value::intern::{intern_sym, name_of_sym};
use crate::value::value::{PathVal, Value};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::tests::probe_prelude;

/// The editor buffer's file.
pub(crate) const FILE: FileId = FileId::new(1);

/// A sink the test can read while the evaluator owns it.
pub(crate) struct SharedSink(pub Rc<RefCell<RecordingSink>>);

impl EffectSink for SharedSink {
    fn apply(&mut self, effect: StagedEffect) {
        self.0.borrow_mut().apply(effect);
    }
}

/// An in-memory loader: path text (without a leading `./`) -> source.
#[derive(Default)]
pub(crate) struct MapLoader {
    files: BTreeMap<String, (FileId, Rc<str>)>,
    pub reads: Rc<RefCell<Vec<String>>>,
}

impl MapLoader {
    pub(crate) fn with(files: &[(&str, &str)]) -> MapLoader {
        let mut m = MapLoader::default();
        for (k, (name, text)) in files.iter().enumerate() {
            let id = FileId::new(u32::try_from(k).unwrap_or(0) + 10);
            m.files.insert((*name).to_string(), (id, Rc::from(*text)));
        }
        m
    }
}

impl SourceLoader for MapLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        let key = path.text.trim_start_matches("./").to_string();
        self.reads.borrow_mut().push(key.clone());
        self.files
            .get(&key)
            .cloned()
            .ok_or_else(|| Failure::new(FailCode::LoadFailed, format!("no file `{key}`")))
    }
}

/// An evaluator over the probe prelude with a readable sink.
pub(crate) struct Harness {
    pub ev: Evaluator,
    pub sink: Rc<RefCell<RecordingSink>>,
}

impl Harness {
    pub(crate) fn new() -> Harness {
        Harness::with(probe_prelude(), Box::new(MapLoader::default()))
    }

    pub(crate) fn with(prelude: Prelude, loader: Box<dyn SourceLoader>) -> Harness {
        let sink = Rc::new(RefCell::new(RecordingSink::default()));
        let ev = Evaluator::new(prelude, loader, Box::new(SharedSink(Rc::clone(&sink))));
        Harness { ev, sink }
    }

    /// Evaluates every form of `src`; the source must read and expand clean.
    pub(crate) fn run(&mut self, src: &str) -> Vec<FormOutcome> {
        self.ev
            .eval_str(src, FILE)
            .unwrap_or_else(|d| panic!("{src:?}: {d}"))
    }

    /// Evaluates `src` and returns its last value, failing the test on a
    /// failure.
    pub(crate) fn ok(&mut self, src: &str) -> Value {
        let out = self.run(src);
        for o in &out {
            if let Err(e) = &o.value {
                panic!("{src:?}: {e}");
            }
        }
        out.last()
            .and_then(|o| o.value.clone().ok())
            .unwrap_or(Value::Nil)
    }

    /// The printed value of the last form, or `fail: CODE`.
    pub(crate) fn show(&mut self, src: &str) -> String {
        match self.run(src).pop().map(|o| o.value) {
            Some(Ok(v)) => v.to_string(),
            Some(Err(e)) => format!("fail: {}", e.code),
            None => "nothing".to_string(),
        }
    }

    /// The printed session value of `name`.
    pub(crate) fn get(&self, name: &str) -> String {
        self.ev
            .ns()
            .session_value(name)
            .map_or_else(|| format!("<unbound {name}>"), |v| v.to_string())
    }

    pub(crate) fn state(&self, name: &str) -> FormState {
        self.ev
            .form_state(name)
            .unwrap_or_else(|| panic!("`{name}` has no form"))
    }

    pub(crate) fn blocked_on(&self, name: &str) -> Option<String> {
        match self.state(name) {
            FormState::Blocked { on } => Some(name_of_sym(on).to_string()),
            _ => None,
        }
    }

    pub(crate) fn failed(&self, name: &str) -> Option<FailCode> {
        match self.state(name) {
            FormState::Failed(e) => Some(e.code),
            _ => None,
        }
    }

    /// Takes the released effects.
    pub(crate) fn take(&self) -> Vec<StagedEffect> {
        std::mem::take(&mut self.sink.borrow_mut().effects)
    }

    /// The slot binds among `effects`, printed.
    pub(crate) fn binds(effects: &[StagedEffect]) -> Vec<(String, String)> {
        effects
            .iter()
            .filter_map(|e| match e {
                StagedEffect::SlotBind { slot, value } => Some((slot.name(), value.to_string())),
                _ => None,
            })
            .collect()
    }

    /// The `bindings` batches among `effects`, printed.
    pub(crate) fn batches(effects: &[StagedEffect]) -> Vec<Vec<(String, String)>> {
        effects
            .iter()
            .filter_map(|e| match e {
                StagedEffect::Bindings(b) => Some(
                    b.iter()
                        .map(|(n, v)| (name_of_sym(*n).to_string(), v.to_string()))
                        .collect(),
                ),
                _ => None,
            })
            .collect()
    }

    /// Every value any released effect carries, printed (to assert that a
    /// provisional value reached no host-visible surface).
    pub(crate) fn visible(effects: &[StagedEffect]) -> Vec<String> {
        let mut out = Vec::new();
        for e in effects {
            match e {
                StagedEffect::SlotBind { value, .. } | StagedEffect::CellUpdate { value, .. } => {
                    out.push(value.to_string());
                }
                StagedEffect::Bindings(b) => out.extend(b.iter().map(|(_, v)| v.to_string())),
                _ => {}
            }
        }
        out
    }

    /// The events of the last pass.
    pub(crate) fn events(&self) -> Vec<PassEvent> {
        self.ev
            .last_pass()
            .map_or_else(Vec::new, |p| p.events.clone())
    }

    /// The name a form defines (its first), for event assertions.
    pub(crate) fn form_name(&self, f: crate::ns::depgraph::FormId) -> String {
        self.ev
            .graph()
            .get(f)
            .and_then(|r| r.names().next().map(|s| name_of_sym(s.name()).to_string()))
            .unwrap_or_else(|| "?".to_string())
    }

    /// The last pass's events, compact: `commit x`, `dirty x<-y`,
    /// `blocked x@y`, `fail x`, `stale x`, `cycle x,y`, `restore x`.
    pub(crate) fn trace(&self) -> Vec<String> {
        self.events()
            .iter()
            .map(|e| match e {
                PassEvent::Commit { form, .. } => format!("commit {}", self.form_name(*form)),
                PassEvent::DirtyAbort { form, owner } => {
                    format!(
                        "dirty {}<-{}",
                        self.form_name(*form),
                        self.form_name(*owner)
                    )
                }
                PassEvent::BlockedAbort { form, on } => {
                    format!("blocked {}@{}", self.form_name(*form), name_of_sym(*on))
                }
                PassEvent::Failed { form, .. } => format!("fail {}", self.form_name(*form)),
                PassEvent::Stale { form } => format!("stale {}", self.form_name(*form)),
                PassEvent::Cycle { forms } => format!(
                    "cycle {}",
                    forms
                        .iter()
                        .map(|f| self.form_name(*f))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                PassEvent::Restore(r) => format!("restore {}", name_of_sym(r.name)),
            })
            .collect()
    }

    /// The provisional commits of `name` in the last pass, printed.
    pub(crate) fn provisional(&self, name: &str) -> Vec<String> {
        let sym = intern_sym(name);
        self.events()
            .iter()
            .filter_map(|e| match e {
                PassEvent::Commit { values, .. } => values
                    .iter()
                    .find(|(n, _)| *n == sym)
                    .map(|(_, v)| v.to_string()),
                _ => None,
            })
            .collect()
    }
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
        .collect()
}

#[test]
fn upd_root_recomputes_raised_and_rebinds_d1_with_one_display_batch() {
    let mut h = Harness::new();
    h.ok("var root 60\nlet raised + root 7\nd1 raised");
    assert_eq!(h.get("raised"), "67");
    h.take();
    assert_eq!(h.show("upd root 62"), "62");
    assert_eq!(h.get("raised"), "69");
    let fx = h.take();
    assert_eq!(Harness::binds(&fx), pairs(&[("d1", "69")]));
    // ONE bindings batch, after everything else, naming the changed names.
    let batches = Harness::batches(&fx);
    assert_eq!(batches, vec![pairs(&[("root", "62"), ("raised", "69")])]);
    assert!(matches!(fx.last(), Some(StagedEffect::Bindings(_))));
    assert!(fx.iter().any(|e| matches!(
        e,
        StagedEffect::CellUpdate {
            value: Value::Int(62),
            ..
        }
    )));
    assert_eq!(h.state("raised"), FormState::Recomputed);
    assert_eq!(h.ev.edges("raised"), vec!["root"]);
    let d1 = h.ev.form_of_bind(SlotKey::D(1)).expect("d1 owner");
    assert_eq!(h.ev.graph().get(d1).map(|r| r.runs), Some(2));
}

#[test]
fn a_transitive_chain_propagates_once_per_node_in_order() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet a + root 1\nlet b * a 2\nlet c + b 1");
    h.ok("upd root 5");
    assert_eq!(
        (h.get("a"), h.get("b"), h.get("c")),
        ("6".into(), "12".into(), "13".into())
    );
    assert_eq!(h.trace(), vec!["commit a", "commit b", "commit c"]);
    for n in ["a", "b", "c"] {
        assert_eq!(h.ev.runs(n), 2, "{n}");
    }
}

#[test]
fn a_diamond_rebuilds_the_shared_descendant_once_after_both_parents() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet l + root 1\nlet r + root 2\nlet t + l r");
    h.ok("upd root 10");
    assert_eq!(h.get("t"), "23");
    assert_eq!(h.trace(), vec!["commit l", "commit r", "commit t"]);
    assert_eq!(h.ev.runs("t"), 2);
    assert!(h.ev.last_pass().is_some_and(|p| p.diags.is_empty()));
}

#[test]
fn equality_cutoff_stops_at_an_unchanged_value_and_leaves_unrelated_branches() {
    let mut h = Harness::new();
    h.ok("var root 5\nlet parity mod root 2\nlet label + parity 10\nvar z 1\nlet zz + z 1");
    h.ok("upd root 7");
    assert_eq!(h.get("parity"), "1");
    assert_eq!(h.ev.runs("parity"), 2);
    assert_eq!(
        h.ev.runs("label"),
        1,
        "no dependent of an unchanged value runs"
    );
    assert_eq!(h.ev.runs("zz"), 1, "an unrelated branch is untouched");
    assert_eq!(h.trace(), vec!["commit parity"]);
    h.ok("upd root 8");
    assert_eq!(h.get("label"), "10");
    assert_eq!(h.ev.runs("label"), 2);
}

#[test]
fn a_var_read_only_through_a_late_capture_triggers_no_rebuild() {
    let mut h = Harness::new();
    h.ok("var v 1\nlet p ctl 0 v");
    assert!(
        h.ev.edges("p").is_empty(),
        "no eager edge: {:?}",
        h.ev.edges("p")
    );
    h.take();
    h.ok("upd v 2");
    assert_eq!(h.ev.runs("p"), 1);
    assert!(h.trace().is_empty(), "{:?}", h.trace());
    // The late read hears the new value through the ref.
    let fx = h.take();
    assert!(Harness::binds(&fx).is_empty());
    assert_eq!(h.show("+ {first p} 0"), "2");
}

#[test]
fn a_failed_recomputation_keeps_the_previous_value_and_binding_with_origin() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet inv / 1 root\nd1 inv");
    h.take();
    h.ok("upd root 0");
    assert_eq!(h.get("inv"), "1");
    match h.state("inv") {
        FormState::Failed(e) => {
            assert_eq!(e.code, FailCode::DivisionByZero);
            assert!(e.origin.span.is_some(), "origin reported");
        }
        other => panic!("{other:?}"),
    }
    let fx = h.take();
    assert!(
        Harness::binds(&fx).is_empty(),
        "the old d1 binding keeps playing"
    );
    let d1 = h.ev.form_of_bind(SlotKey::D(1)).expect("d1");
    assert!(matches!(
        h.ev.graph().get(d1).map(|r| r.state.clone()),
        Some(FormState::Blocked { .. })
    ));
}

#[test]
fn controller_rate_upds_coalesce_latest_wins_into_one_pass() {
    let mut h = Harness::new();
    h.ok("var root 60\nlet raised + root 7\nd1 raised");
    h.take();
    for v in [61, 62, 63] {
        h.ev.queue_upd("root", Value::Int(v)).expect("queue");
    }
    let report = h.ev.run_pass();
    assert_eq!(h.get("root"), "63");
    assert_eq!(h.get("raised"), "70");
    assert_eq!(
        h.ev.runs("raised"),
        2,
        "one recomputation for three updates"
    );
    let fx = h.take();
    assert_eq!(Harness::binds(&fx), pairs(&[("d1", "70")]));
    assert_eq!(Harness::batches(&fx).len(), 1);
    assert_eq!(report.bindings.len(), 2);
    assert_eq!(
        h.ev.queue_upd("raised", Value::Int(1)).unwrap_err().code,
        FailCode::UpdImmutable
    );
}
