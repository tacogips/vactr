//! Charged postorder capture avoids retaining exhaustive dispatcher stack frames.
use super::*;

enum Task<'a> {
    Pattern {
        pat: &'a Pat,
        depth: u32,
        wrapper: bool,
    },
    Finish {
        pat: &'a Pat,
        depth: u32,
        wrapper: bool,
        previous_peak: u32,
    },
    Parameter {
        parameter: &'a PParam,
        depth: u32,
    },
    Atomic {
        value: &'a Value,
        depth: u32,
    },
    Step {
        value: &'a Value,
        depth: u32,
    },
}
fn schedule<'a>(
    capture: &mut TimingCapture<'_>,
    tasks: &mut Vec<Task<'a>>,
    task: Task<'a>,
) -> Result<(), Failure> {
    admit(capture.remaining, 1)?;
    tasks.push(task);
    Ok(())
}
fn cached(
    capture: &mut TimingCapture<'_>,
    pat: &Pat,
    depth: u32,
    wrapper: bool,
) -> Result<Option<u32>, Failure> {
    capture.enter(depth)?;
    let key = (pat as *const Pat as usize, wrapper);
    if let Some(&(node, height)) = capture.seen.get(&key) {
        let end = depth.checked_add(height).ok_or_else(overflow)?;
        if end > capture.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "cached index timing depth exhausted",
            ));
        }
        capture.peak = capture.peak.max(end);
        Ok(Some(node))
    } else {
        Ok(None)
    }
}
pub(super) fn capture_pat<'p, 'c: 'p>(
    capture: &mut TimingCapture<'c>,
    pat: &'p Pat,
    depth: u32,
    step_wrapper: bool,
) -> Result<u32, Failure> {
    if let Some(node) = cached(capture, pat, depth, step_wrapper)? {
        return Ok(node);
    }
    if capture.assembling {
        return Err(Failure::new(
            FailCode::Type,
            "index timing dependency was not captured postorder",
        ));
    }
    let root = (pat as *const Pat as usize, step_wrapper);
    let mut tasks = Vec::new();
    schedule(
        capture,
        &mut tasks,
        Task::Pattern {
            pat,
            depth,
            wrapper: step_wrapper,
        },
    )?;
    while let Some(task) = tasks.pop() {
        admit(capture.remaining, 1)?;
        match task {
            Task::Pattern {
                pat,
                depth,
                wrapper,
            } => {
                if cached(capture, pat, depth, wrapper)?.is_some() {
                    continue;
                }
                let key = (pat as *const Pat as usize, wrapper);
                admit(capture.remaining, 1)?;
                if !capture.visiting.insert(key) {
                    return Err(Failure::new(FailCode::Type, "cyclic index pattern"));
                }
                let previous_peak = capture.peak;
                capture.peak = depth + 1;
                schedule(
                    capture,
                    &mut tasks,
                    Task::Finish {
                        pat,
                        depth,
                        wrapper,
                        previous_peak,
                    },
                )?;
                dependencies(capture, &mut tasks, pat, depth)?;
            }
            Task::Finish {
                pat,
                depth,
                wrapper,
                previous_peak,
            } => {
                let mut node = TimingCapture::empty(pat.id, operation(&pat.node), pat.structured);
                // All pattern references were assembled postorder. The original
                // descriptor builder now reads cached references without descent.
                capture.assembling = true;
                let assembled = capture.fill(pat, &mut node, depth, wrapper);
                capture.assembling = false;
                assembled?;
                let height = capture.peak.checked_sub(depth).ok_or_else(overflow)?;
                capture.peak = capture.peak.max(previous_peak);
                let index = capture.push(node)?;
                let key = (pat as *const Pat as usize, wrapper);
                capture.visiting.remove(&key);
                admit(capture.remaining, 1)?;
                capture.seen.insert(key, (index, height));
            }
            Task::Parameter { parameter, depth } => {
                capture.enter(depth)?;
                match parameter {
                    PParam::Pat(p) | PParam::Const(Value::Pattern(p)) => schedule(
                        capture,
                        &mut tasks,
                        Task::Pattern {
                            pat: p,
                            depth: depth + 1,
                            wrapper: false,
                        },
                    )?,
                    PParam::Const(v) if number(v).is_none() && dynamic(v).is_none() => schedule(
                        capture,
                        &mut tasks,
                        Task::Atomic {
                            value: v,
                            depth: depth + 1,
                        },
                    )?,
                    _ => {}
                }
            }
            Task::Atomic { value, depth } => {
                capture.enter(depth)?;
                match value {
                    Value::Fn(_) => {
                        let shapes = capture.shapes;
                        if let Some(result) =
                            shapes.fixed_pattern(value, depth, capture.remaining, capture.limits)?
                        {
                            schedule(
                                capture,
                                &mut tasks,
                                Task::Pattern {
                                    pat: result,
                                    depth: depth + 1,
                                    wrapper: false,
                                },
                            )?;
                        }
                    }
                    Value::Pattern(p) => schedule(
                        capture,
                        &mut tasks,
                        Task::Pattern {
                            pat: p,
                            depth: depth + 1,
                            wrapper: false,
                        },
                    )?,
                    Value::List(list) => {
                        for value in list.items.iter().rev() {
                            schedule(
                                capture,
                                &mut tasks,
                                Task::Atomic {
                                    value,
                                    depth: depth + 1,
                                },
                            )?;
                        }
                    }
                    _ => {}
                }
            }
            Task::Step { value, depth } => {
                capture.enter(depth)?;
                match value {
                    Value::Pattern(p) => {
                        // Weighted Steps inspect wrapper parameters before the
                        // actual value-node squeeze; preserve both caller depths.
                        if let PatNode::Hold(_, param) | PatNode::Repeat(_, param) = &p.node {
                            schedule(
                                capture,
                                &mut tasks,
                                Task::Parameter {
                                    parameter: param,
                                    depth,
                                },
                            )?;
                        }
                        schedule(
                            capture,
                            &mut tasks,
                            Task::Pattern {
                                pat: p,
                                depth: depth + 1,
                                wrapper: true,
                            },
                        )?;
                    }
                    Value::List(list) => {
                        capture.enter(depth + 1)?;
                        for value in list.items.iter().rev() {
                            schedule(
                                capture,
                                &mut tasks,
                                Task::Step {
                                    value,
                                    depth: depth + 2,
                                },
                            )?;
                        }
                    }
                    _ => schedule(
                        capture,
                        &mut tasks,
                        Task::Atomic {
                            value,
                            depth: depth + 1,
                        },
                    )?,
                }
            }
        }
    }
    capture
        .seen
        .get(&root)
        .map(|(node, _)| *node)
        .ok_or_else(overflow)
}
fn dependencies<'a, 'c: 'a>(
    capture: &mut TimingCapture<'c>,
    tasks: &mut Vec<Task<'a>>,
    pat: &'a Pat,
    depth: u32,
) -> Result<(), Failure> {
    use PatNode::*;
    let mut parameter = |p: &'a PParam| {
        schedule(
            capture,
            tasks,
            Task::Parameter {
                parameter: p,
                depth: depth + 1,
            },
        )
    };
    // Parameter requests are issued before child requests. No operand or trace
    // is copied during this dependency scan.
    match &pat.node {
        Sound { src, kit } => {
            parameter(src)?;
            if let Some(kit) = kit {
                parameter(kit)?;
            }
        }
        Fast(_, k)
        | Slow(_, k)
        | Hurry(_, k)
        | DegradeBy(_, k)
        | Maybe(_, k)
        | Iter(_, k)
        | Chop(_, k)
        | Ply(_, k)
        | Striate(_, k)
        | LoopAt(_, k)
        | Arp(_, k)
        | Segment(_, k)
        | Hold(_, k)
        | Repeat(_, k)
        | Every(k, _, _)
        | SometimesBy(k, _, _)
        | Chunk(_, k, _)
        | Off(_, k, _) => parameter(k)?,
        Range(_, a, b) | WhenMod(a, b, _, _) => {
            parameter(a)?;
            parameter(b)?;
        }
        Euclid(_, a, b, c) => {
            parameter(a)?;
            parameter(b)?;
            parameter(c)?;
        }
        Slice { cuts, .. } | Splice { cuts, .. } => match cuts {
            SliceCuts::Equal(n) => parameter(n)?,
            SliceCuts::Manual(ps) => {
                for p in ps {
                    parameter(p)?;
                }
            }
        },
        _ => {}
    }
    match &pat.node {
        Pure(step) => schedule(
            capture,
            tasks,
            Task::Atomic {
                value: &step.value,
                depth: depth + 1,
            },
        )?,
        Steps(steps) => {
            capture.enter(depth + 1)?;
            for step in steps.iter().rev() {
                schedule(
                    capture,
                    tasks,
                    Task::Step {
                        value: &step.value,
                        depth: depth + 2,
                    },
                )?;
            }
        }
        Fast(p, _)
        | Slow(p, _)
        | Hurry(p, _)
        | DegradeBy(p, _)
        | Maybe(p, _)
        | Iter(p, _)
        | Chop(p, _)
        | Ply(p, _)
        | Striate(p, _)
        | LoopAt(p, _)
        | Arp(p, _)
        | Segment(p, _)
        | Hold(p, _)
        | Repeat(p, _)
        | Rev(p)
        | ScaleNotes(_, _, p)
        | Voicing(p)
        | Fit(p)
        | Range(p, _, _)
        | Euclid(p, _, _, _)
        | MidiNotes { subject: p, .. } => schedule(
            capture,
            tasks,
            Task::Pattern {
                pat: p,
                depth: depth + 1,
                wrapper: false,
            },
        )?,
        Choose(ps) | Stack(ps) | Cat(ps) | FastCat(ps) => {
            for p in ps.iter().rev() {
                schedule(
                    capture,
                    tasks,
                    Task::Pattern {
                        pat: p,
                        depth: depth + 1,
                        wrapper: false,
                    },
                )?;
            }
        }
        Grid(p, q)
        | Control(_, p, q)
        | Chord(p, q)
        | Slice {
            pat: p, index: q, ..
        }
        | Splice {
            pat: p, index: q, ..
        } => {
            schedule(
                capture,
                tasks,
                Task::Pattern {
                    pat: q,
                    depth: depth + 1,
                    wrapper: false,
                },
            )?;
            schedule(
                capture,
                tasks,
                Task::Pattern {
                    pat: p,
                    depth: depth + 1,
                    wrapper: false,
                },
            )?;
        }
        Every(_, f, p)
        | SometimesBy(_, f, p)
        | WhenMod(_, _, f, p)
        | Chunk(p, _, f)
        | Off(p, _, f)
        | Superimpose(p, f)
        | Jux(p, f) => {
            if let Some(output) = shape_key(f, p).and_then(|key| capture.shapes.patterns.get(&key))
            {
                schedule(
                    capture,
                    tasks,
                    Task::Pattern {
                        pat: output,
                        depth: depth + 1,
                        wrapper: false,
                    },
                )?;
            }
            schedule(
                capture,
                tasks,
                Task::Pattern {
                    pat: p,
                    depth: depth + 1,
                    wrapper: false,
                },
            )?;
        }
        Sound { .. } | Signal(_) | SongSource(_) => {}
    }
    Ok(())
}

impl TimingCapture<'_> {
    fn unary(&mut self, node: &mut Node, child: &Rc<Pat>, depth: u32) -> Result<(), Failure> {
        let child = self.pat(child, depth + 1, false)?;
        self.child(node, 0, child, &[exact(ProducerKind::Child, 0)])
    }
    fn fill(
        &mut self,
        pat: &Pat,
        node: &mut Node,
        depth: u32,
        step_wrapper: bool,
    ) -> Result<(), Failure> {
        use PatNode::*;
        match &pat.node {
            Pure(step) => {
                if self.fixed_value(node, &step.value, depth + 1)? {
                    return Ok(());
                }
                if dynamic(&step.value).is_some() {
                    admit(self.remaining, 1)?;
                    node.leaf_trace
                        .push(exact(ProducerKind::DynamicExpansion, 0));
                }
                node.leaf = Some(self.atomic_leaf(&step.value, depth + 1)?);
            }
            Steps(steps) => {
                admit(self.remaining, steps.len())?;
                let values: Vec<_> = steps.iter().map(|s| &s.value).collect();
                self.steps(node, &values, depth + 1)?;
            }
            Signal(_) => {
                node.leaf = Some(Leaf::Continuous);
                self.requires_realization = true;
            }
            SongSource(_) => {
                node.leaf = Some(Leaf::Dynamic(Dynamic::UnresolvedPattern));
                self.requires_realization = true;
            }
            Sound { src, kit } => {
                self.param(node, src, depth + 1)?;
                if let Some(kit) = kit {
                    self.param(node, kit, depth + 1)?;
                }
            }
            Fast(p, k)
            | Slow(p, k)
            | Hurry(p, k)
            | DegradeBy(p, k)
            | Maybe(p, k)
            | Iter(p, k)
            | Chop(p, k)
            | Ply(p, k)
            | Striate(p, k)
            | LoopAt(p, k)
            | Arp(p, k)
            | Segment(p, k) => {
                self.param(node, k, depth + 1)?;
                self.unary(node, p, depth)?;
            }
            Hold(p, k) => {
                self.param(node, k, depth + 1)?;
                self.unary(node, p, depth)?;
            }
            Repeat(p, n) => {
                self.param(node, n, depth + 1)?;
                let child = self.pat(p, depth + 1, false)?;
                let count = match n {
                    PParam::Const(v) => crate::pattern::eval::int_of(v)
                        .and_then(|n| u32::try_from(n.clamp(0, 4096)).ok()),
                    _ => None,
                };
                if !step_wrapper && count != Some(1) {
                    if let Some(count) = count {
                        self.child(
                            node,
                            0,
                            child,
                            &[
                                Term::Copies {
                                    kind: ProducerKind::NestedStep,
                                    count,
                                },
                                exact(ProducerKind::Child, 0),
                            ],
                        )?;
                    } else {
                        self.requires_realization = true;
                        self.child(node, 0, child, &[])?;
                    }
                } else {
                    self.child(node, 0, child, &[exact(ProducerKind::Child, 0)])?;
                }
            }
            Rev(p) | ScaleNotes(_, _, p) | Voicing(p) | Fit(p) => self.unary(node, p, depth)?,
            Choose(ps) | Stack(ps) | Cat(ps) | FastCat(ps) => {
                for (role, p) in ps.iter().enumerate() {
                    let child = self.pat(p, depth + 1, false)?;
                    self.child(
                        node,
                        u32::try_from(role).map_err(|_| overflow())?,
                        child,
                        &[exact(
                            ProducerKind::Child,
                            u32::try_from(role).map_err(|_| overflow())?,
                        )],
                    )?;
                }
            }
            Range(p, a, b) | Euclid(p, a, b, _) => {
                self.param(node, a, depth + 1)?;
                self.param(node, b, depth + 1)?;
                if let Euclid(_, _, _, c) = &pat.node {
                    self.param(node, c, depth + 1)?;
                }
                self.unary(node, p, depth)?;
            }
            Grid(p, q) | Control(_, p, q) | Chord(p, q) => {
                let a = self.pat(p, depth + 1, false)?;
                self.child(node, 0, a, &[exact(ProducerKind::Child, 0)])?;
                let b = self.pat(q, depth + 1, false)?;
                self.child(node, 1, b, &[exact(ProducerKind::Child, 1)])?;
            }
            Slice {
                pat: p,
                cuts,
                index,
            }
            | Splice {
                pat: p,
                cuts,
                index,
            } => {
                match cuts {
                    SliceCuts::Equal(n) => self.param(node, n, depth + 1)?,
                    SliceCuts::Manual(ps) => {
                        admit(self.remaining, ps.len())?;
                        for p in ps {
                            node.parameters.push(self.parameter(p, depth + 1)?);
                        }
                    }
                }
                let a = self.pat(p, depth + 1, false)?;
                self.child(node, 0, a, &[exact(ProducerKind::Child, 0)])?;
                let b = self.pat(index, depth + 1, false)?;
                self.child(node, 1, b, &[exact(ProducerKind::Child, 1)])?;
            }
            Every(n, f, p) | SometimesBy(n, f, p) | Chunk(p, n, f) | Off(p, n, f) => {
                self.param(node, n, depth + 1)?;
                self.transform(node, p, f, depth)?;
            }
            WhenMod(a, b, f, p) => {
                self.param(node, a, depth + 1)?;
                self.param(node, b, depth + 1)?;
                self.transform(node, p, f, depth)?;
            }
            Superimpose(p, f) | Jux(p, f) => self.transform(node, p, f, depth)?,
            MidiNotes { subject, channel } => {
                if let Some(channel) = channel {
                    admit(self.remaining, 1)?;
                    node.parameters
                        .push(Parameter::Number(Number::Int(i32::from(*channel))));
                }
                self.requires_realization = true;
                self.unary(node, subject, depth)?;
            }
        }
        Ok(())
    }
    fn transform(
        &mut self,
        node: &mut Node,
        p: &Rc<Pat>,
        f: &Value,
        depth: u32,
    ) -> Result<(), Failure> {
        self.unary(node, p, depth)?;
        if let Some(output) = shape_key(f, p).and_then(|key| self.shapes.patterns.get(&key)) {
            let child = self.pat(output, depth + 1, false)?;
            self.child(
                node,
                1,
                child,
                &[
                    exact(ProducerKind::DynamicExpansion, 1),
                    exact(ProducerKind::Child, 1),
                ],
            )?;
        } else {
            self.requires_realization = true;
            admit(self.remaining, 1)?;
            node.parameters
                .push(Parameter::Dynamic(dynamic(f).unwrap_or(Dynamic::Function)));
        }
        Ok(())
    }
}
