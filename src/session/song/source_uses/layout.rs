//! Exact static Steps geometry, including slots that emit no selected source.
use super::*;
use crate::song::source_uses::layout::FrozenUseSlotLayout;

impl Builder<'_> {
    pub(super) fn atomic_step(
        &mut self,
        value: &Value,
        depth: u32,
        edges: &mut Vec<FrozenSourceUseEdge>,
    ) -> Result<M, Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source-use step depth exhausted",
            ));
        }
        self.admit(1)?;
        match value {
            Value::Pattern(p) => {
                self.admit(5)?;
                if let PatNode::Hold(inner, _) | PatNode::Repeat(inner, _) = &p.node {
                    let mut e = edge(self.pat(inner, depth + 1)?, 0);
                    e.trace.insert(
                        0,
                        T::Exact(ProducerStep {
                            kind: ProducerKind::Child,
                            ordinal: 0,
                        }),
                    );
                    edges.push(e);
                } else {
                    edges.push(edge(self.pat(p, depth + 1)?, 0));
                }
                Ok(M::Preserve)
            }
            Value::Fn(_) => {
                let shapes = self.shapes;
                let Some(result) =
                    shapes.fixed_pattern(value, depth, self.remaining, self.limits)?
                else {
                    return Ok(M::Uncertifiable(R::DynamicSource));
                };
                self.admit(3)?;
                let child = self.pat(result, depth + 1)?;
                edges.push(FrozenSourceUseEdge {
                    child,
                    layout: Vec::new(),
                    trace: vec![
                        T::Exact(ProducerStep {
                            kind: ProducerKind::DynamicExpansion,
                            ordinal: 0,
                        }),
                        T::Exact(ProducerStep {
                            kind: ProducerKind::Child,
                            ordinal: 0,
                        }),
                    ],
                });
                Ok(M::Preserve)
            }
            Value::Native(_) | Value::VarRef(_) | Value::Thunk(_) | Value::Signal(_) => {
                Ok(M::Uncertifiable(R::DynamicSource))
            }
            _ => Ok(M::Empty), // Pure lists are atomic, never subdivided.
        }
    }
    pub(super) fn weighted_steps(
        &mut self,
        values: &[&Value],
        depth: u32,
        edges: &mut Vec<FrozenSourceUseEdge>,
    ) -> Result<M, Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "weighted step depth exhausted",
            ));
        }
        self.admit(
            values
                .len()
                .checked_mul(2)
                .ok_or_else(|| Failure::new(FailCode::Overflow, "layout admission overflow"))?,
        )?;
        let mut weights = Vec::with_capacity(values.len());
        let mut total = Ratio64::ZERO;
        for value in values {
            let (weight, copies) = match value {
                Value::Pattern(p) => match &p.node {
                    PatNode::Hold(_, w) => match numeric(w) {
                        Some(w) if w > Ratio64::ZERO => (w, 1),
                        Some(_) => {
                            return Err(Failure::new(
                                FailCode::Type,
                                "hold weight must be positive",
                            ))
                        }
                        None => return Ok(M::Uncertifiable(R::DynamicTiming)),
                    },
                    PatNode::Repeat(_, n) => {
                        let PParam::Const(v) = n else {
                            return Ok(M::Uncertifiable(R::DynamicCount));
                        };
                        let Some(n) = crate::pattern::eval::int_of(v) else {
                            return Ok(M::Uncertifiable(R::DynamicCount));
                        };
                        (
                            Ratio64::ONE,
                            u32::try_from(n.clamp(0, 4096)).map_err(|_| {
                                Failure::new(FailCode::Overflow, "repeat count overflow")
                            })?,
                        )
                    }
                    _ => (Ratio64::ONE, 1),
                },
                _ => (Ratio64::ONE, 1),
            };
            total = total.checked_add(weight.checked_mul(Ratio64::from_int(i64::from(copies)))?)?;
            weights.push((weight, copies));
        }
        if total == Ratio64::ZERO {
            return Ok(M::Empty);
        }
        let mut prefix = Ratio64::ZERO;
        for (ordinal, (value, (weight, copies))) in values.iter().zip(weights).enumerate() {
            if copies == 0 {
                continue;
            }
            let first = edges.len();
            let child_mapping = if let Value::List(l) = value {
                self.admit(l.items.len())?;
                let nested: Vec<_> = l.items.iter().collect();
                self.weighted_steps(&nested, depth + 1, edges)?
            } else {
                self.atomic_step(value, depth + 1, edges)?
            };
            if matches!(child_mapping, M::Uncertifiable(_)) {
                return Ok(child_mapping);
            }
            let repeated =
                matches!(value,Value::Pattern(p) if matches!(p.node,PatNode::Repeat(..)));
            let trace_count = if repeated { 2 } else { 1 };
            let added = edges.len() - first;
            let cost = edges[first..].iter().try_fold(
                added
                    .checked_mul(trace_count + 1)
                    .ok_or_else(|| Failure::new(FailCode::Overflow, "layout prefix overflow"))?,
                |cost, e| {
                    cost.checked_add(e.layout.len()).ok_or_else(|| {
                        Failure::new(FailCode::Overflow, "nested layout admission overflow")
                    })
                },
            )?;
            self.admit(cost)?;
            let normalized = prefix.checked_div(total)?;
            let width = weight.checked_div(total)?;
            for e in &mut edges[first..] {
                for slot in &mut e.layout {
                    if let Some(term) = &mut slot.copy_trace_term {
                        *term = term
                            .checked_add(u32::try_from(trace_count).map_err(|_| {
                                Failure::new(FailCode::Overflow, "copy trace index overflow")
                            })?)
                            .ok_or_else(|| {
                                Failure::new(FailCode::Overflow, "copy trace index overflow")
                            })?;
                    }
                }
                if repeated {
                    e.trace.insert(
                        0,
                        T::Copies {
                            kind: ProducerKind::GeneratedBranch,
                            count: copies,
                        },
                    );
                }
                e.trace.insert(
                    0,
                    T::Exact(ProducerStep {
                        kind: ProducerKind::NestedStep,
                        ordinal: u32::try_from(ordinal).map_err(|_| {
                            Failure::new(FailCode::Overflow, "step ordinal overflow")
                        })?,
                    }),
                );
                e.layout.insert(
                    0,
                    FrozenUseSlotLayout {
                        prefix: normalized,
                        width,
                        copy_trace_term: repeated.then_some(1),
                    },
                );
            }
            prefix =
                prefix.checked_add(weight.checked_mul(Ratio64::from_int(i64::from(copies)))?)?;
        }
        Ok(M::Preserve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::step::Step;
    fn limits() -> SongAssetLimits {
        SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 10000,
            max_source_files: 10,
            max_source_bytes: 10000,
            max_banks: 10,
            max_walk_nodes: 10000,
            max_walk_depth: 256,
        }
    }
    #[test]
    fn intrinsic_repeat_clamps_without_expanding_and_shifts_nested_trace_indices() {
        for (count, expected) in [(-2, 0), (0, 0), (2, 2), (5000, 4096)] {
            let leaf = Rc::new(Pat::new(
                PatNode::Pure(Step::bare(Value::Int(7))),
                None,
                false,
            ));
            let repeated = Rc::new(Pat::new(
                PatNode::Repeat(leaf, PParam::Const(Value::Int(count))),
                None,
                true,
            ));
            let list = crate::value::value::ListVal {
                items: vec![Value::Pattern(repeated), Value::Nil].into(),
                prov: None,
            };
            let pattern = Pat::new(
                PatNode::Steps(
                    vec![
                        Step::bare(Value::Nil),
                        Step::bare(Value::List(Rc::new(list))),
                        Step::bare(Value::Nil),
                    ]
                    .into(),
                ),
                None,
                true,
            );
            let mut remaining = 10000;
            let graph = analyze(
                &pattern,
                &[],
                &mut remaining,
                limits(),
                &PreparedShapes::default(),
            )
            .unwrap();
            let root = &graph.nodes[graph.root as usize];
            let ns = crate::ns::namespace::Namespace::new(crate::ns::namespace::Prelude::core());
            let mut vm = crate::vm::vm::Vm::new();
            let mut query = crate::vm::query_vm::VmQuery::new(&mut vm, &ns);
            let cells = crate::pattern::eval::InputCells::new();
            let mut cx = crate::pattern::eval::QueryCtx::new(&mut query, &cells, 1);
            let realized = crate::pattern::query::query_traced(
                &pattern,
                crate::pattern::query::TimeSpan::cycle(0).unwrap(),
                &mut cx,
            );
            assert!(realized.faults.is_empty());
            assert_eq!(realized.events.len(), expected as usize);
            if expected == 0 {
                assert!(root.edges.is_empty());
                continue;
            }
            assert_eq!(root.edges.len(), 1);
            let edge = &root.edges[0];
            assert_eq!(edge.layout.len(), 2);
            assert_eq!(edge.layout[1].copy_trace_term, Some(2));
            assert_eq!(
                edge.layout[1].width,
                Ratio64::new(1, i64::from(expected) + 1).unwrap()
            );
            assert!(matches!(edge.trace[2],T::Copies{count,..} if count==expected));
            assert!(graph.nodes.len() < 5);
            crate::song::source_uses::layout::validate_layout(edge).unwrap();
            for event in &realized.events {
                let trace = &event.producer.as_ref().unwrap().steps;
                let slot = crate::song::source_uses::layout::slot_interval(edge, 0, trace).unwrap();
                assert_eq!(event.whole, Some(slot));
            }
        }
    }
    #[test]
    fn weighted_nested_lists_preserve_default_depth_and_work_limits() {
        for depth in [200, 300] {
            let leaf = Rc::new(Pat::new(PatNode::Pure(Step::bare(Value::Nil)), None, false));
            let mut value = Value::Pattern(leaf);
            for _ in 0..depth {
                value = Value::List(Rc::new(crate::value::value::ListVal {
                    items: vec![value].into(),
                    prov: None,
                }));
            }
            let pattern = Pat::new(PatNode::Steps(vec![Step::bare(value)].into()), None, true);
            let mut remaining = 100_000;
            let result = analyze(
                &pattern,
                &[],
                &mut remaining,
                limits(),
                &PreparedShapes::default(),
            );
            if depth == 200 {
                let graph = result.unwrap();
                assert_eq!(graph.nodes[graph.root as usize].edges[0].layout.len(), 201);
            } else {
                assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
            }
            let mut remaining = 10;
            assert_eq!(
                analyze(
                    &pattern,
                    &[],
                    &mut remaining,
                    limits(),
                    &PreparedShapes::default()
                )
                .unwrap_err()
                .code,
                FailCode::FuelExhausted
            );
        }
    }
    #[test]
    fn static_weight_overflow_rejects_before_successful_layout() {
        let leaf = Rc::new(Pat::new(PatNode::Pure(Step::bare(Value::Nil)), None, false));
        let held = Rc::new(Pat::new(
            PatNode::Hold(leaf, PParam::Const(Value::Int64(i64::MAX))),
            None,
            true,
        ));
        let pattern = Pat::new(
            PatNode::Steps(vec![Step::bare(Value::Pattern(held)), Step::bare(Value::Nil)].into()),
            None,
            true,
        );
        let mut remaining = 10000;
        assert_eq!(
            analyze(
                &pattern,
                &[],
                &mut remaining,
                limits(),
                &PreparedShapes::default()
            )
            .unwrap_err()
            .code,
            FailCode::Overflow
        );
    }
}
