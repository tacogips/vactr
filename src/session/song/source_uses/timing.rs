//! Captures frozen operands only; never invokes a VM or draws randomness.
use super::{operation, shape_key, PreparedShapes};
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::pat::{PParam, Pat, PatNode, SliceCuts};
use crate::reader::span::NodeId;
use crate::song::assets::SongAssetLimits;
use crate::song::source_uses::layout::FrozenUseSlotLayout;
use crate::song::source_uses::timing::{
    admit, overflow, FrozenIndexChild as Child, FrozenIndexDynamicKind as Dynamic,
    FrozenIndexLeaf as Leaf, FrozenIndexNumber as Number, FrozenIndexParameter as Parameter,
    FrozenIndexSlot as Slot, FrozenIndexTiming, FrozenIndexTimingNode as Node,
};
use crate::song::source_uses::{FrozenUseOperation as Operation, FrozenUseTraceTerm as Term};
use crate::value::value::Value;
use crate::value::Ratio64;
use crate::vm::fail::{FailCode, Failure};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
pub(in crate::session::song) fn capture_index_timing(
    pattern: &Rc<Pat>,
    shapes: &PreparedShapes,
    remaining: &mut u32,
    limits: SongAssetLimits,
) -> Result<Rc<FrozenIndexTiming>, Failure> {
    let mut capture = TimingCapture {
        shapes,
        remaining,
        limits,
        nodes: Vec::new(),
        seen: BTreeMap::new(),
        visiting: BTreeSet::new(),
        peak: 0,
        requires_realization: false,
        assembling: false,
    };
    let root = capture.pat(pattern, 0, false)?;
    admit(capture.remaining, 1)?;
    Ok(Rc::new(FrozenIndexTiming::issue(
        root,
        capture.nodes,
        capture.requires_realization,
        capture.remaining,
        limits.max_walk_depth,
    )?))
}
mod capture;

struct TimingCapture<'a> {
    shapes: &'a PreparedShapes,
    remaining: &'a mut u32,
    limits: SongAssetLimits,
    nodes: Vec<Node>,
    seen: BTreeMap<(usize, bool), (u32, u32)>,
    visiting: BTreeSet<(usize, bool)>,
    peak: u32,
    requires_realization: bool,
    assembling: bool,
}
fn exact(kind: ProducerKind, ordinal: u32) -> Term {
    Term::Exact(ProducerStep { kind, ordinal })
}
fn number(value: &Value) -> Option<Number> {
    match value {
        Value::Int(n) => Some(Number::Int(*n)),
        Value::Int64(n) => Some(Number::Int64(*n)),
        Value::Ratio(n) => Some(Number::Ratio(*n)),
        Value::Float(n) => Some(Number::FloatBits(n.to_bits())),
        Value::Float64(n) => Some(Number::Float64Bits(n.to_bits())),
        _ => None,
    }
}
fn dynamic(value: &Value) -> Option<Dynamic> {
    match value {
        Value::Fn(_) => Some(Dynamic::Function),
        Value::Native(_) => Some(Dynamic::Native),
        Value::VarRef(_) => Some(Dynamic::Late),
        Value::Thunk(_) => Some(Dynamic::Thunk),
        Value::Signal(_) => Some(Dynamic::Signal),
        _ => None,
    }
}
impl TimingCapture<'_> {
    fn enter(&mut self, depth: u32) -> Result<(), Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "index timing capture depth exhausted",
            ));
        }
        admit(self.remaining, 1)?;
        self.peak = self.peak.max(depth + 1);
        Ok(())
    }
    fn empty(issuer: NodeId, operation: Operation, structured: bool) -> Node {
        Node {
            issuer,
            operation,
            structured,
            leaf: None,
            leaf_trace: Vec::new(),
            parameters: Vec::new(),
            slots: Vec::new(),
            children: Vec::new(),
        }
    }
    fn push(&mut self, node: Node) -> Result<u32, Failure> {
        admit(self.remaining, 1)?;
        let index = u32::try_from(self.nodes.len()).map_err(|_| overflow())?;
        self.nodes.push(node);
        Ok(index)
    }
    fn child(
        &mut self,
        node: &mut Node,
        role: u32,
        child: u32,
        trace: &[Term],
    ) -> Result<(), Failure> {
        admit(
            self.remaining,
            trace.len().checked_add(1).ok_or_else(overflow)?,
        )?;
        node.children.push(Child {
            role,
            child,
            trace: trace.to_vec(),
        });
        Ok(())
    }
    fn parameter(&mut self, param: &PParam, depth: u32) -> Result<Parameter, Failure> {
        self.enter(depth)?;
        match param {
            PParam::Const(v) => {
                if let Some(n) = number(v) {
                    return Ok(Parameter::Number(n));
                }
                if let Value::Pattern(p) = v {
                    return Ok(Parameter::Pattern {
                        child: self.pat(p, depth + 1, false)?,
                    });
                }
                if let Some(kind) = dynamic(v) {
                    self.requires_realization = true;
                    Ok(Parameter::Dynamic(kind))
                } else {
                    Ok(Parameter::Literal(self.atomic_leaf(v, depth + 1)?))
                }
            }
            PParam::Pat(p) => Ok(Parameter::Pattern {
                child: self.pat(p, depth + 1, false)?,
            }),
            PParam::Late(_) => {
                self.requires_realization = true;
                Ok(Parameter::Dynamic(Dynamic::Late))
            }
            PParam::Fn(v) => {
                self.requires_realization = true;
                Ok(Parameter::Dynamic(dynamic(v).unwrap_or(Dynamic::Function)))
            }
        }
    }
    fn param(&mut self, node: &mut Node, param: &PParam, depth: u32) -> Result<(), Failure> {
        admit(self.remaining, 1)?;
        node.parameters.push(self.parameter(param, depth)?);
        Ok(())
    }
    fn fixed_value(&mut self, node: &mut Node, value: &Value, depth: u32) -> Result<bool, Failure> {
        let shapes = self.shapes;
        let Some(result) = shapes.fixed_pattern(value, depth, self.remaining, self.limits)? else {
            return Ok(false);
        };
        let child = self.pat(result, depth + 1, false)?;
        self.child(
            node,
            0,
            child,
            &[
                exact(ProducerKind::DynamicExpansion, 0),
                exact(ProducerKind::Child, 0),
            ],
        )?;
        Ok(true)
    }
    fn atomic_leaf(&mut self, value: &Value, depth: u32) -> Result<Leaf, Failure> {
        self.enter(depth)?;
        match value {
            Value::Nil => Ok(Leaf::Rest),
            Value::List(list) => {
                admit(self.remaining, list.items.len())?;
                let mut elements = Vec::with_capacity(list.items.len());
                for value in &list.items {
                    elements.push(self.atomic_leaf(value, depth + 1)?);
                }
                Ok(Leaf::AtomicList { elements })
            }
            Value::Pattern(p) => Ok(Leaf::Pattern {
                child: self.pat(p, depth + 1, false)?,
            }),
            v if dynamic(v).is_some() => {
                self.requires_realization = true;
                Ok(Leaf::Dynamic(dynamic(v).ok_or_else(overflow)?))
            }
            v => Ok(Leaf::Scalar { number: number(v) }),
        }
    }
    fn value_node(&mut self, value: &Value, issuer: NodeId, depth: u32) -> Result<u32, Failure> {
        self.enter(depth)?;
        match value {
            Value::Pattern(p) => self.pat(p, depth + 1, true),
            Value::List(list) => {
                admit(self.remaining, list.items.len())?;
                let values: Vec<_> = list.items.iter().collect();
                let mut node = Self::empty(issuer, Operation::Steps, true);
                self.steps(&mut node, &values, depth + 1)?;
                self.push(node)
            }
            _ => {
                let mut node = Self::empty(issuer, Operation::Pure, false);
                if self.fixed_value(&mut node, value, depth + 1)? {
                    return self.push(node);
                }
                if dynamic(value).is_some() {
                    admit(self.remaining, 1)?;
                    node.leaf_trace
                        .push(exact(ProducerKind::DynamicExpansion, 0));
                }
                node.leaf = Some(self.atomic_leaf(value, depth + 1)?);
                self.push(node)
            }
        }
    }
    fn steps(&mut self, node: &mut Node, values: &[&Value], depth: u32) -> Result<(), Failure> {
        self.enter(depth)?;
        admit(
            self.remaining,
            values.len().checked_mul(3).ok_or_else(overflow)?,
        )?;
        let mut weights = Vec::with_capacity(values.len());
        let mut total = Some(Ratio64::ZERO);
        for value in values {
            let (weight, copies, raw_weight, raw_count) = match value {
                Value::Pattern(p) => match &p.node {
                    PatNode::Hold(_, w) => {
                        let weight = super::numeric(w).filter(|n| *n > Ratio64::ZERO);
                        (weight, Some(1), self.parameter(w, depth + 1)?, None)
                    }
                    PatNode::Repeat(_, n) => {
                        let copies = match n {
                            PParam::Const(v) => crate::pattern::eval::int_of(v)
                                .and_then(|n| u32::try_from(n.clamp(0, 4096)).ok()),
                            _ => None,
                        };
                        (
                            Some(Ratio64::ONE),
                            copies,
                            Parameter::Number(Number::Int(1)),
                            Some(self.parameter(n, depth + 1)?),
                        )
                    }
                    _ => (
                        Some(Ratio64::ONE),
                        Some(1),
                        Parameter::Number(Number::Int(1)),
                        None,
                    ),
                },
                _ => (
                    Some(Ratio64::ONE),
                    Some(1),
                    Parameter::Number(Number::Int(1)),
                    None,
                ),
            };
            total = match (total, weight, copies) {
                (Some(t), Some(w), Some(c)) => {
                    Some(t.checked_add(w.checked_mul(Ratio64::from_int(i64::from(c)))?)?)
                }
                _ => None,
            };
            weights.push((weight, copies, raw_weight, raw_count));
        }
        let total = total.filter(|n| *n > Ratio64::ZERO);
        let mut prefix = Ratio64::ZERO;
        for (ordinal, (value, (weight, copies, raw_weight, raw_count))) in
            values.iter().zip(weights).enumerate()
        {
            let ordinal = u32::try_from(ordinal).map_err(|_| overflow())?;
            let child = self.value_node(value, node.issuer, depth + 1)?;
            let repeated =
                matches!(value,Value::Pattern(p) if matches!(p.node,PatNode::Repeat(..)));
            let geometry = match (total, weight, copies) {
                (Some(total), Some(weight), Some(copies)) => {
                    let g = FrozenUseSlotLayout {
                        prefix: prefix.checked_div(total)?,
                        width: weight.checked_div(total)?,
                        copy_trace_term: if repeated && copies > 0 {
                            Some(1)
                        } else {
                            None
                        },
                    };
                    prefix = prefix
                        .checked_add(weight.checked_mul(Ratio64::from_int(i64::from(copies)))?)?;
                    Some(g)
                }
                _ => None,
            };
            // A zero-copy slot still exists in the operand, but emits no path.
            let mut trace = [
                exact(ProducerKind::NestedStep, ordinal),
                exact(ProducerKind::Child, 0),
                exact(ProducerKind::Child, 0),
            ];
            let terms = if repeated {
                if let Some(count) = copies {
                    trace[1] = Term::Copies {
                        kind: ProducerKind::GeneratedBranch,
                        count,
                    };
                    &trace[..3]
                } else {
                    self.requires_realization = true;
                    &trace[..1]
                }
            } else if matches!(value, Value::Pattern(_)) {
                &trace[..2]
            } else {
                &trace[..1]
            };
            self.child(node, ordinal, child, terms)?;
            admit(self.remaining, 5)?;
            node.slots.push(Slot {
                ordinal,
                geometry,
                copies,
                weight: raw_weight,
                count: raw_count,
                child,
            });
        }
        Ok(())
    }
    fn pat(&mut self, pat: &Pat, depth: u32, step_wrapper: bool) -> Result<u32, Failure> {
        capture::capture_pat(self, pat, depth, step_wrapper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::eval::{InputCells, QueryCtx, QueryVm};
    use crate::pattern::query::{QueryResult, TimeSpan};
    use crate::value::value::{ListVal, NativeId};
    fn limits() -> SongAssetLimits {
        SongAssetLimits {
            max_resources: 32,
            max_pcm_bytes: 100000,
            max_source_files: 8,
            max_source_bytes: 100000,
            max_banks: 8,
            max_walk_nodes: 100000,
            max_walk_depth: 256,
        }
    }
    fn capture(
        pattern: &Rc<Pat>,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<Rc<FrozenIndexTiming>, Failure> {
        let mut limit = limits();
        limit.max_walk_depth = depth;
        capture_index_timing(pattern, &PreparedShapes::default(), remaining, limit)
    }
    fn actual_query(pattern: &Pat, window: TimeSpan) -> QueryResult {
        let ns = crate::ns::namespace::Namespace::new(crate::ns::namespace::Prelude::core());
        let mut vm = crate::vm::vm::Vm::new();
        let mut query = crate::vm::query_vm::VmQuery::new(&mut vm, &ns);
        let cells = InputCells::new();
        crate::pattern::query::query_traced(
            pattern,
            window,
            &mut QueryCtx::new(&mut query, &cells, 1),
        )
    }
    fn scalar(n: i32) -> Rc<Pat> {
        Rc::new(crate::pattern::build::pure(Value::Int(n), None))
    }
    #[test]
    fn actual_vm_thunk_survives_freeze_capture_and_query() {
        use crate::ns::stage::StagedEffect;
        use crate::song::assets::{DecodedSongAssetFactory, SongAssetFactory, SongSourceFile};
        let mut session = crate::vm::tests::Sess::new();
        session.eval("at 4 {0}").unwrap();
        let thunk = session
            .sink
            .effects
            .iter()
            .find_map(|effect| match effect {
                StagedEffect::OneShot { value, .. } => Some(value.clone()),
                _ => None,
            })
            .expect("actual VM-staged unevaluated block");
        assert!(matches!(&thunk, Value::Thunk(_)), "{thunk:?}");
        let assets =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
                .begin(
                    SongSourceFile {
                        file: crate::reader::span::FileId::new(0),
                        path: crate::value::value::PathVal {
                            text: "thunk.vact".into(),
                            file: None,
                        },
                    },
                    limits(),
                )
                .unwrap()
                .close()
                .unwrap();
        let mut freeze = crate::session::song::freeze::Freeze::new(&assets, limits());
        let frozen = freeze.value(&thunk).unwrap();
        match (&thunk, &frozen) {
            (Value::Thunk(original), Value::Thunk(copied)) => {
                assert!(!Rc::ptr_eq(original, copied));
            }
            other => panic!("frozen closure kind changed: {other:?}"),
        }
        let pattern = Rc::new(crate::pattern::build::value_steps(&[frozen]));
        let recipe = capture(&pattern, &mut 100000, 256).unwrap();
        assert!(recipe.requires_realization());
        assert!(recipe
            .nodes()
            .iter()
            .any(|node| node.leaf() == Some(&Leaf::Dynamic(Dynamic::Thunk))));
        let actual = actual_query(&pattern, TimeSpan::cycle(0).unwrap());
        assert!(actual.faults.is_empty(), "{:?}", actual.faults);
        assert_eq!(actual.events.len(), 1);
        assert!(matches!(actual.events[0].value, Value::Int(0)));
        assert_eq!(actual.events[0].whole, Some(TimeSpan::cycle(0).unwrap()));
    }
    #[test]
    fn scalar_rest_order_and_atomic_lists_survive_actual_query() {
        for values in [
            vec![Value::Int(0), Value::Nil],
            vec![Value::Nil, Value::Int(0)],
        ] {
            let pattern = Rc::new(crate::pattern::build::value_steps(&values));
            let recipe = capture(&pattern, &mut 100000, 256).unwrap();
            let root = &recipe.nodes()[recipe.root() as usize];
            assert_eq!(root.slots().len(), 2);
            assert!(root.structured());
            for (i, value) in values.iter().enumerate() {
                let leaf = recipe.nodes()[root.slots()[i].child() as usize]
                    .leaf()
                    .unwrap();
                assert_eq!(matches!(leaf, Leaf::Rest), matches!(value, Value::Nil));
            }
            let actual = actual_query(&pattern, TimeSpan::cycle(0).unwrap());
            assert!(actual.faults.is_empty());
            assert_eq!(actual.events.len(), 1);
            let ordinal = if matches!(values[0], Value::Nil) {
                1
            } else {
                0
            };
            let slot = &root.slots()[ordinal];
            let g = slot.geometry().unwrap();
            assert_eq!(
                actual.events[0].whole,
                Some(TimeSpan::new(g.prefix, g.prefix.checked_add(g.width).unwrap()).unwrap())
            );
            assert!(actual.events[0]
                .producer
                .as_ref()
                .unwrap()
                .steps
                .contains(&ProducerStep {
                    kind: ProducerKind::NestedStep,
                    ordinal: ordinal as u32
                }));
        }
        let value = Value::List(Rc::new(ListVal {
            items: vec![Value::Int(0), Value::Nil].into(),
            prov: None,
        }));
        let atomic = Rc::new(crate::pattern::build::pure(value.clone(), None));
        let divided = Rc::new(crate::pattern::build::value_steps(&[value]));
        let a = capture(&atomic, &mut 100000, 256).unwrap();
        let b = capture(&divided, &mut 100000, 256).unwrap();
        assert!(
            matches!(a.nodes()[a.root() as usize].leaf(),Some(Leaf::AtomicList{elements}) if elements.len()==2)
        );
        assert!(!a.nodes()[a.root() as usize].structured());
        assert!(b.nodes()[b.root() as usize].structured());
        let qa = actual_query(&atomic, TimeSpan::cycle(0).unwrap());
        let qb = actual_query(&divided, TimeSpan::cycle(0).unwrap());
        assert!(qa.faults.is_empty());
        assert!(qb.faults.is_empty());
        assert_eq!(qa.events[0].whole, Some(TimeSpan::cycle(0).unwrap()));
        assert_eq!(qb.events[0].whole.unwrap().end, Ratio64::new(1, 2).unwrap());
    }
    #[test]
    fn weighted_repeat_zero_and_huge_copies_are_symbolic_and_match_producers() {
        for count in [0, 2, 5000] {
            let repeat = Rc::new(Pat::new(
                PatNode::Repeat(scalar(7), PParam::Const(Value::Int(count))),
                None,
                true,
            ));
            let hold = Rc::new(Pat::new(
                PatNode::Hold(
                    scalar(9),
                    PParam::Const(Value::Ratio(Ratio64::new(3, 2).unwrap())),
                ),
                None,
                true,
            ));
            let pattern = Rc::new(crate::pattern::build::value_steps(&[
                Value::Pattern(repeat),
                Value::Pattern(hold),
                Value::Nil,
            ]));
            let recipe = capture(&pattern, &mut 100000, 256).unwrap();
            let root = &recipe.nodes()[recipe.root() as usize];
            assert_eq!(root.slots()[0].copies(), Some((count as u32).min(4096)));
            assert_eq!(root.slots().len(), 3);
            assert!(recipe.nodes().len() < 10);
            assert_eq!(
                root.slots()[1].weight(),
                &Parameter::Number(Number::Ratio(Ratio64::new(3, 2).unwrap()))
            );
            if count <= 2 {
                let actual = actual_query(&pattern, TimeSpan::cycle(0).unwrap());
                assert!(actual.faults.is_empty());
                assert_eq!(actual.events.len(), count as usize + 1);
                for event in &actual.events {
                    let trace = &event.producer.as_ref().unwrap().steps;
                    let ordinal = trace
                        .iter()
                        .find(|step| step.kind == ProducerKind::NestedStep)
                        .unwrap()
                        .ordinal;
                    let edge = &root.children()[ordinal as usize];
                    let mut cursor = 0;
                    for term in edge.trace() {
                        let found = &trace[cursor];
                        match term {
                            Term::Exact(step) => assert_eq!(found, step),
                            Term::Copies { kind, count } => {
                                assert_eq!(found.kind, *kind);
                                assert!(found.ordinal < *count);
                            }
                        }
                        cursor += 1;
                    }
                    let child = &recipe.nodes()[edge.child() as usize];
                    assert_eq!(
                        trace[cursor],
                        ProducerStep {
                            kind: ProducerKind::Child,
                            ordinal: 0
                        }
                    );
                    assert_eq!(
                        child.children()[0].trace(),
                        &[exact(ProducerKind::Child, 0)]
                    );
                }
            }
        }
    }
    #[test]
    fn static_operations_and_unresolved_operands_retain_exact_raw_data() {
        let pattern = Rc::new(Pat::new(
            PatNode::Euclid(
                Rc::new(Pat::new(
                    PatNode::Rev(Rc::new(Pat::new(
                        PatNode::Fast(
                            scalar(0),
                            PParam::Const(Value::Ratio(Ratio64::new(3, 2).unwrap())),
                        ),
                        None,
                        false,
                    ))),
                    None,
                    false,
                )),
                PParam::Const(Value::Int64(-2)),
                PParam::Const(Value::Int(7)),
                PParam::Const(Value::Int(3)),
            ),
            None,
            false,
        ));
        let recipe = capture(&pattern, &mut 100000, 256).unwrap();
        let root = &recipe.nodes()[recipe.root() as usize];
        assert_eq!(root.operation(), Operation::Euclid);
        assert_eq!(
            root.parameters(),
            &[
                Parameter::Number(Number::Int64(-2)),
                Parameter::Number(Number::Int(7)),
                Parameter::Number(Number::Int(3))
            ]
        );
        assert!(!recipe.requires_realization());
        let slot = crate::ns::namespace::VarSlotRef::new(
            crate::value::intern::intern_sym("late-index"),
            crate::ns::namespace::SlotKind::Var,
            Value::Int(1),
        );
        for (value, kind) in [
            (Value::VarRef(slot), Dynamic::Late),
            (Value::Native(NativeId::new(0)), Dynamic::Native),
            (
                Value::Signal(Rc::new(crate::pattern::signal::Sig::Phase)),
                Dynamic::Signal,
            ),
        ] {
            let p = Rc::new(crate::pattern::build::value_steps(&[value]));
            let r = capture(&p, &mut 100000, 256).unwrap();
            let root = &r.nodes()[r.root() as usize];
            let leaf = &r.nodes()[root.slots()[0].child() as usize];
            assert_eq!(leaf.leaf(), Some(&Leaf::Dynamic(kind)));
            assert_eq!(
                leaf.leaf_trace(),
                &[exact(ProducerKind::DynamicExpansion, 0)]
            );
            assert!(r.requires_realization());
        }
        for v in [Value::Float(f32::INFINITY), Value::Float64(1.25)] {
            let p = Rc::new(crate::pattern::build::pure(v.clone(), None));
            let r = capture(&p, &mut 100000, 256).unwrap();
            assert_eq!(
                r.nodes()[r.root() as usize].leaf(),
                Some(&Leaf::Scalar { number: number(&v) })
            );
        }
    }
    #[test]
    fn actual_retained_native_transform_is_used_only_when_present() {
        let p = scalar(0);
        let (id, _) = crate::types::natives::NativeTable::global()
            .get("rev")
            .unwrap();
        let f = Value::Native(id);
        let mut prelude = crate::ns::namespace::Prelude::core();
        crate::vm::natives::register_domain(&mut prelude);
        let ns = crate::ns::namespace::Namespace::new(prelude);
        let mut vm = crate::vm::vm::Vm::new();
        let mut query = crate::vm::query_vm::VmQuery::new(&mut vm, &ns);
        let Value::Pattern(output) = query.call(&f, &[Value::Pattern(p.clone())]).unwrap() else {
            panic!("actual rev native output")
        };
        let input = Rc::new(Pat::new(
            PatNode::Every(PParam::Const(Value::Int(2)), f.clone(), p.clone()),
            None,
            false,
        ));
        let mut shapes = PreparedShapes::default();
        shapes
            .patterns
            .insert(shape_key(&f, &p).unwrap(), output.clone());
        let recipe = capture_index_timing(&input, &shapes, &mut 100000, limits()).unwrap();
        let root = &recipe.nodes()[recipe.root() as usize];
        assert_eq!(root.children().len(), 2);
        assert_eq!(
            recipe.nodes()[root.children()[1].child() as usize].issuer(),
            output.id
        );
        assert!(!recipe.requires_realization());
        assert_eq!(
            root.children()[1].trace(),
            &[
                exact(ProducerKind::DynamicExpansion, 1),
                exact(ProducerKind::Child, 1)
            ]
        );
        let absent = capture(&input, &mut 100000, 256).unwrap();
        assert!(absent.requires_realization());
        assert_eq!(absent.nodes()[absent.root() as usize].children().len(), 1);
    }
    #[test]
    fn exact_shared_quota_cache_height_and_preallocation_refusal() {
        let p = scalar(0);
        let root = Rc::new(Pat::new(
            PatNode::Control(
                crate::value::intern::intern_kw("gain"),
                p.clone(),
                Rc::new(Pat::new(
                    PatNode::Fast(p.clone(), PParam::Const(Value::Int(2))),
                    None,
                    false,
                )),
            ),
            None,
            false,
        ));
        let mut remaining = 100000;
        let recipe = capture(&root, &mut remaining, 256).unwrap();
        let cost = 100000 - remaining;
        assert!(cost > 20);
        assert_eq!(recipe.nodes().len(), 3);
        let mut exact = cost;
        capture(&root, &mut exact, 256).unwrap();
        assert_eq!(exact, 0);
        assert_eq!(
            capture(&root, &mut (cost - 1), 256).unwrap_err().code,
            FailCode::FuelExhausted
        );
        let mut twice = cost * 2;
        capture(&root, &mut twice, 256).unwrap();
        capture(&root, &mut twice, 256).unwrap();
        assert_eq!(twice, 0);
        let minimum = (1..64)
            .find(|d| capture(&root, &mut 100000, *d).is_ok())
            .unwrap();
        assert_eq!(
            capture(&root, &mut 100000, minimum - 1).unwrap_err().code,
            FailCode::DepthExceeded
        );
        assert_eq!(
            capture(
                &Rc::new(crate::pattern::build::value_steps(&vec![
                    Value::Int(0);
                    4096
                ])),
                &mut 4,
                256
            )
            .unwrap_err()
            .code,
            FailCode::FuelExhausted
        );
    }
}
