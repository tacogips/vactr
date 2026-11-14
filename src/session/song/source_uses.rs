//! Bounded structural source-use analysis, independent of resource deduplication.
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::song::assets::SongAssetLimits;
use crate::song::source_uses::{
    FrozenSourceUseEdge, FrozenSourceUseGraph, FrozenSourceUseNode, FrozenStaticCondition as C,
    FrozenUseMapping as M, FrozenUseOperation as O, FrozenUseReason as R, FrozenUseTraceTerm as T,
};
use crate::song::SongSource;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
mod layout;
mod slices;
pub(super) mod timing;

pub(super) fn analyze(
    pattern: &Pat,
    sources: &[Rc<SongSource>],
    remaining: &mut u32,
    limits: SongAssetLimits,
    shapes: &PreparedShapes,
) -> Result<FrozenSourceUseGraph, Failure> {
    let mut builder = Builder {
        sources,
        shapes,
        remaining,
        limits,
        nodes: Vec::new(),
        seen: BTreeMap::new(),
        visiting: BTreeSet::new(),
    };
    let root = builder.pat(pattern, 0)?;
    Ok(FrozenSourceUseGraph {
        root,
        nodes: builder.nodes,
    })
}
struct Builder<'a> {
    shapes: &'a PreparedShapes,
    sources: &'a [Rc<SongSource>],
    remaining: &'a mut u32,
    limits: SongAssetLimits,
    nodes: Vec<FrozenSourceUseNode>,
    seen: BTreeMap<usize, u32>,
    visiting: BTreeSet<usize>,
}
impl Builder<'_> {
    fn admit(&mut self, count: usize) -> Result<(), Failure> {
        let count = u32::try_from(count)
            .map_err(|_| Failure::new(FailCode::Overflow, "source-use collection overflow"))?;
        *self.remaining = self.remaining.checked_sub(count).ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "aggregate source-use analysis exhausted",
            )
        })?;
        Ok(())
    }
    fn pat(&mut self, pat: &Pat, depth: u32) -> Result<u32, Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source-use analysis depth exhausted",
            ));
        }
        self.admit(1)?;
        let identity = pat as *const Pat as usize;
        if let Some(index) = self.seen.get(&identity) {
            return Ok(*index);
        }
        if !self.visiting.insert(identity) {
            return Err(Failure::new(FailCode::Type, "cyclic source-use graph"));
        }
        // Keep unary recursive calls outside the large dispatch frame.
        let unary = match &pat.node {
            PatNode::Fast(p, k) => Some((p.as_ref(), O::Fast, rate(k, false))),
            PatNode::Slow(p, k) => Some((p.as_ref(), O::Slow, rate(k, true))),
            PatNode::Hurry(p, k) => Some((p.as_ref(), O::Hurry, rate(k, false))),
            PatNode::Rev(p) => Some((p.as_ref(), O::Rev, M::ReflectCycles)),
            PatNode::DegradeBy(p, _) => Some((p.as_ref(), O::DegradeBy, M::Preserve)),
            PatNode::Maybe(p, _) => Some((p.as_ref(), O::Maybe, M::Preserve)),
            PatNode::ScaleNotes(_, _, p) => Some((p.as_ref(), O::ScaleNotes, M::Preserve)),
            PatNode::Voicing(p) => Some((p.as_ref(), O::Voicing, M::Preserve)),
            PatNode::Fit(p) => Some((p.as_ref(), O::Fit, M::Preserve)),
            _ => None,
        };
        let node = if let Some((child, operation, mapping)) = unary {
            self.admit(3)?;
            let child = self.pat(child, depth + 1)?;
            FrozenSourceUseNode {
                operation,
                mapping,
                edges: vec![edge(child, 0)],
            }
        } else {
            self.inner(pat, depth)?
        };
        self.admit(1)?;
        let index = u32::try_from(self.nodes.len())
            .map_err(|_| Failure::new(FailCode::Overflow, "source-use node index overflow"))?;
        self.nodes.push(node);
        self.visiting.remove(&identity);
        self.seen.insert(identity, index);
        Ok(index)
    }
    #[inline(never)]
    fn inner(&mut self, pat: &Pat, depth: u32) -> Result<FrozenSourceUseNode, Failure> {
        let mut edges = Vec::new();
        let (operation, mapping) = match &pat.node {
            PatNode::SongSource(source) => {
                let index = self
                    .sources
                    .iter()
                    .position(|s| Rc::ptr_eq(s, source))
                    .ok_or_else(|| {
                        Failure::new(FailCode::Type, "source-use policy is not closed")
                    })?;
                (
                    O::Source,
                    M::Source {
                        policy: u32::try_from(index).map_err(|_| {
                            Failure::new(FailCode::Overflow, "source policy index overflow")
                        })?,
                    },
                )
            }
            PatNode::Stack(items)
            | PatNode::Cat(items)
            | PatNode::FastCat(items)
            | PatNode::Choose(items) => {
                self.admit(items.len().checked_mul(3).ok_or_else(|| {
                    Failure::new(FailCode::Overflow, "source-use edge overflow")
                })?)?;
                for (i, p) in items.iter().enumerate() {
                    edges.push(edge(
                        self.pat(p, depth + 1)?,
                        u32::try_from(i).map_err(|_| {
                            Failure::new(FailCode::Overflow, "source-use edge ordinal overflow")
                        })?,
                    ));
                }
                match &pat.node {
                    PatNode::Stack(_) => (O::Stack, M::Parallel),
                    PatNode::FastCat(_) => (O::FastCat, M::CycleConcat),
                    PatNode::Choose(_) => (O::Choose, M::Parallel),
                    _ => (O::Cat, M::CycleSelect),
                }
            }
            PatNode::Sound { src, .. } => {
                let p = match src {
                    PParam::Pat(p) => Some(p),
                    PParam::Const(Value::Pattern(p)) => Some(p),
                    _ => None,
                };
                if let Some(p) = p {
                    self.admit(3)?;
                    edges.push(edge(self.pat(p, depth + 1)?, 0));
                    (
                        O::Sound,
                        if pat.structured {
                            M::Preserve
                        } else {
                            M::SampleCycles { content: 0 }
                        },
                    )
                } else {
                    let mapping = match src {
                        PParam::Const(
                            Value::Keyword(_) | Value::Sound(_) | Value::Nil | Value::List(_),
                        ) => M::Empty,
                        _ => M::Uncertifiable(R::DynamicSource),
                    };
                    (O::Sound, mapping)
                }
            }
            PatNode::Every(_, f, p)
            | PatNode::WhenMod(_, _, f, p)
            | PatNode::SometimesBy(_, f, p)
            | PatNode::Superimpose(p, f)
            | PatNode::Jux(p, f)
            | PatNode::Off(p, _, f)
            | PatNode::Chunk(p, _, f) => {
                let key = shape_key(f, p);
                let prepared = key.and_then(|key| self.shapes.patterns.get(&key)).cloned();
                if let Some(prepared) = prepared {
                    self.admit(8)?;
                    edges.push(edge(self.pat(p, depth + 1)?, 0));
                    let mut transformed = self.pat(&prepared, depth + 1)?;
                    if let PatNode::Off(_, shift, _) = &pat.node {
                        let Some(amount) = numeric(shift) else {
                            return Ok(FrozenSourceUseNode {
                                operation: O::Off,
                                mapping: M::Uncertifiable(R::DynamicTiming),
                                edges: Vec::new(),
                            });
                        };
                        self.admit(2)?;
                        let virtual_index = u32::try_from(self.nodes.len()).map_err(|_| {
                            Failure::new(FailCode::Overflow, "shift node index overflow")
                        })?;
                        self.nodes.push(FrozenSourceUseNode {
                            operation: O::Off,
                            mapping: M::Shift { amount },
                            edges: vec![FrozenSourceUseEdge {
                                layout: Vec::new(),
                                trace: Vec::new(),
                                child: transformed,
                            }],
                        });
                        transformed = virtual_index;
                    }
                    edges.push(FrozenSourceUseEdge {
                        layout: Vec::new(),
                        trace: vec![
                            T::Exact(ProducerStep {
                                kind: ProducerKind::DynamicExpansion,
                                ordinal: 1,
                            }),
                            T::Exact(ProducerStep {
                                kind: ProducerKind::Child,
                                ordinal: 1,
                            }),
                        ],
                        child: transformed,
                    });
                    (
                        operation(&pat.node),
                        if matches!(
                            pat.node,
                            PatNode::Superimpose(..) | PatNode::Jux(..) | PatNode::Off(..)
                        ) {
                            M::Parallel
                        } else {
                            static_condition(&pat.node)
                        },
                    )
                } else {
                    (
                        operation(&pat.node),
                        M::Uncertifiable(
                            key.and_then(|key| self.shapes.rejected.get(&key))
                                .copied()
                                .unwrap_or(R::UncertifiedCallback),
                        ),
                    )
                }
            }
            PatNode::Pure(step) => (O::Pure, self.atomic_step(&step.value, depth, &mut edges)?),
            PatNode::Steps(steps) => {
                self.admit(steps.len())?;
                let values: Vec<_> = steps.iter().map(|s| &s.value).collect();
                let mapping = self.weighted_steps(&values, depth, &mut edges)?;
                (O::Steps, mapping)
            }
            _ => self.other(pat, depth, &mut edges)?,
        };
        Ok(FrozenSourceUseNode {
            operation,
            mapping,
            edges,
        })
    }
    fn other(
        &mut self,
        pat: &Pat,
        depth: u32,
        edges: &mut Vec<FrozenSourceUseEdge>,
    ) -> Result<(O, M), Failure> {
        let operation = operation(&pat.node);
        self.admit(
            super::child_count(pat)
                .checked_mul(3)
                .ok_or_else(|| Failure::new(FailCode::Overflow, "use edge overflow"))?,
        )?;
        let children = super::freeze::children(pat);
        for (i, p) in children.into_iter().enumerate() {
            edges.push(edge(
                self.pat(p, depth + 1)?,
                u32::try_from(i)
                    .map_err(|_| Failure::new(FailCode::Overflow, "use ordinal overflow"))?,
            ));
        }
        let mapping = match &pat.node {
            PatNode::Signal(_) => M::Empty,
            PatNode::Hold(_, _) => M::Preserve,
            PatNode::Repeat(_, n) => {
                if let Some(copies) = count(n) {
                    if copies == 1 {
                        M::Preserve
                    } else {
                        for e in edges.iter_mut() {
                            e.trace.insert(
                                0,
                                T::Copies {
                                    kind: ProducerKind::NestedStep,
                                    count: copies.min(4096),
                                },
                            );
                        }
                        M::CycleConcat
                    }
                } else {
                    M::Uncertifiable(R::DynamicCount)
                }
            }
            PatNode::Ply(_, n)
            | PatNode::Chop(_, n)
            | PatNode::Striate(_, n)
            | PatNode::Segment(_, n) => count(n)
                .map_or(M::Uncertifiable(R::DynamicCount), |count| M::Subdivide {
                    count,
                }),
            PatNode::Iter(_, n) => count(n).map_or(M::Uncertifiable(R::DynamicCount), |count| {
                M::Iterate { count }
            }),
            PatNode::Range(..) | PatNode::LoopAt(..) | PatNode::Arp(..) => M::Preserve,
            PatNode::Euclid(_, k, n, r) => static_sampling(k, n, r)?,
            PatNode::Control(_, value, content) | PatNode::Chord(value, content) => {
                if crate::pattern::combinators::control::gives_structure(content, value) {
                    M::Restructure {
                        timing: 0,
                        content: 1,
                    }
                } else {
                    M::SelectContent { content: 1 }
                }
            }
            PatNode::Slice {
                pat: subject,
                cuts,
                index,
            }
            | PatNode::Splice {
                pat: subject,
                cuts,
                index,
            } => self.slice_mapping(pat, subject, index, cuts)?,
            PatNode::Grid(..) => M::Restructure {
                timing: 1,
                content: 0,
            },
            PatNode::MidiNotes { .. } => M::Uncertifiable(R::UnsupportedLiveInput),
            _ => M::Uncertifiable(R::UncertifiedCallback),
        };
        Ok((operation, mapping))
    }
}
fn edge(child: u32, ordinal: u32) -> FrozenSourceUseEdge {
    FrozenSourceUseEdge {
        layout: Vec::new(),
        trace: vec![T::Exact(ProducerStep {
            kind: ProducerKind::Child,
            ordinal,
        })],
        child,
    }
}
fn numeric(param: &PParam) -> Option<Ratio64> {
    if let PParam::Const(v) = param {
        crate::pattern::eval::num_ratio(v)
    } else {
        None
    }
}
fn static_sampling(k: &PParam, n: &PParam, r: &PParam) -> Result<M, Failure> {
    let integer = |p: &PParam| -> Result<Option<i64>, Failure> {
        if let PParam::Const(v) = p {
            crate::pattern::eval::int_of(v)
                .map(Some)
                .ok_or_else(|| Failure::new(FailCode::Type, "expected an integer"))
        } else {
            Ok(None)
        }
    };
    let (k, n, r) = (integer(k)?, integer(n)?, integer(r)?);
    if n.is_some_and(|n| !(1..=4096).contains(&n)) {
        return Err(Failure::new(
            FailCode::Type,
            "euclid steps must be between 1 and 4096",
        ));
    }
    Ok(match (k, n, r) {
        (Some(pulses), Some(divisions), Some(rotation)) => M::SampleGrid {
            content: 0,
            sampling: crate::song::source_uses::FrozenStaticSampling::Euclid {
                pulses,
                divisions,
                rotation,
            },
        },
        (None, _, _) | (_, None, _) => M::Uncertifiable(R::DynamicCount),
        _ => M::Uncertifiable(R::DynamicTiming),
    })
}
fn static_condition(node: &PatNode) -> M {
    let integer = |p: &PParam| match p {
        PParam::Const(value) => crate::pattern::eval::int_of(value),
        _ => None,
    };
    let condition = match node {
        PatNode::Every(n, ..) => integer(n).map(|period| C::Every { period }),
        PatNode::WhenMod(a, b, ..) => integer(a)
            .zip(integer(b))
            .map(|(modulus, threshold)| C::WhenMod { modulus, threshold }),
        PatNode::Chunk(_, n, _) => integer(n).map(|divisions| C::Chunk { divisions }),
        PatNode::SometimesBy(..) => return M::Conditional,
        _ => return M::Uncertifiable(R::UncertifiedCallback),
    };
    condition.map_or(M::Uncertifiable(R::DynamicTiming), M::ConditionalStatic)
}
fn rate(param: &PParam, slow: bool) -> M {
    numeric(param)
        .and_then(|r| {
            if slow {
                Ratio64::ONE.checked_div(r).ok()
            } else {
                Some(r)
            }
        })
        .map_or(M::Uncertifiable(R::DynamicRate), |factor| M::Rate {
            factor,
        })
}
fn count(param: &PParam) -> Option<u32> {
    let ratio = numeric(param)?;
    if ratio.den() != 1 {
        return None;
    }
    u32::try_from(ratio.num()).ok().filter(|n| *n <= 4096)
}
fn operation(node: &PatNode) -> O {
    match node {
        PatNode::Steps(_) => O::Steps,
        PatNode::Pure(_) => O::Pure,
        PatNode::Sound { .. } => O::Sound,
        PatNode::Signal(_) => O::Signal,
        PatNode::SongSource(_) => O::Source,
        PatNode::Fast(..) => O::Fast,
        PatNode::Slow(..) => O::Slow,
        PatNode::Hurry(..) => O::Hurry,
        PatNode::Rev(..) => O::Rev,
        PatNode::Every(..) => O::Every,
        PatNode::WhenMod(..) => O::WhenMod,
        PatNode::SometimesBy(..) => O::SometimesBy,
        PatNode::DegradeBy(..) => O::DegradeBy,
        PatNode::Maybe(..) => O::Maybe,
        PatNode::Choose(..) => O::Choose,
        PatNode::Hold(..) => O::Hold,
        PatNode::Repeat(..) => O::Repeat,
        PatNode::Stack(..) => O::Stack,
        PatNode::Cat(..) => O::Cat,
        PatNode::FastCat(..) => O::FastCat,
        PatNode::Superimpose(..) => O::Superimpose,
        PatNode::Off(..) => O::Off,
        PatNode::Jux(..) => O::Jux,
        PatNode::Iter(..) => O::Iter,
        PatNode::Chop(..) => O::Chop,
        PatNode::Ply(..) => O::Ply,
        PatNode::Striate(..) => O::Striate,
        PatNode::Slice { .. } => O::Slice,
        PatNode::Splice { .. } => O::Splice,
        PatNode::LoopAt(..) => O::LoopAt,
        PatNode::Fit(..) => O::Fit,
        PatNode::Chunk(..) => O::Chunk,
        PatNode::Grid(..) => O::Grid,
        PatNode::Euclid(..) => O::Euclid,
        PatNode::Control(..) => O::Control,
        PatNode::ScaleNotes(..) => O::ScaleNotes,
        PatNode::Chord(..) => O::Chord,
        PatNode::Voicing(..) => O::Voicing,
        PatNode::Arp(..) => O::Arp,
        PatNode::Segment(..) => O::Segment,
        PatNode::Range(..) => O::Range,
        PatNode::MidiNotes { .. } => O::MidiNotes,
    }
}

pub(super) use super::shape_preparation::{shape_key, PreparedShapes};

#[cfg(test)]
mod conditional_tests {
    use super::*;
    #[test]
    fn static_integer_copy_matches_runtime_conversion_without_clamping() {
        let inner = Rc::new(Pat::silence());
        for value in [
            Value::Int(-2),
            Value::Int64(i64::MAX),
            Value::Ratio(Ratio64::new(6, 3).unwrap()),
            Value::Float(2.0),
            Value::Float64(8192.0),
        ] {
            let expected = crate::pattern::eval::int_of(&value).unwrap();
            assert_eq!(
                static_condition(&PatNode::Every(
                    PParam::Const(value),
                    Value::Nil,
                    inner.clone()
                )),
                M::ConditionalStatic(C::Every { period: expected })
            );
        }
    }
    #[test]
    fn invalid_and_nonconstant_integer_copy_remains_addressed_dynamic_timing() {
        let inner = Rc::new(Pat::silence());
        for value in [
            Value::Ratio(Ratio64::new(3, 2).unwrap()),
            Value::Float(2.5),
            Value::Float64(f64::NAN),
            Value::Float64(f64::INFINITY),
            Value::Float64(9.0e15),
        ] {
            assert_eq!(
                static_condition(&PatNode::Every(
                    PParam::Const(value),
                    Value::Nil,
                    inner.clone()
                )),
                M::Uncertifiable(R::DynamicTiming)
            );
        }
        assert_eq!(
            static_condition(&PatNode::Chunk(inner, PParam::Fn(Value::Nil), Value::Nil)),
            M::Uncertifiable(R::DynamicTiming)
        );
    }
}

#[cfg(test)]
mod sampling_tests {
    use super::*;
    #[test]
    fn numeric_failures_are_exact_even_with_dynamic_neighbors() {
        let dynamic = PParam::Fn(Value::Nil);
        for n in [0, -1, 4097] {
            let e = static_sampling(&dynamic, &PParam::int(n), &dynamic).unwrap_err();
            assert_eq!(e.code, FailCode::Type);
            assert_eq!(e.message, "euclid steps must be between 1 and 4096");
        }
        for v in [
            Value::Ratio(Ratio64::new(3, 2).unwrap()),
            Value::Float64(f64::NAN),
            Value::Float64(f64::INFINITY),
            Value::Str(Rc::from("not numeric")),
        ] {
            for position in 0..3 {
                let mut params = [PParam::int(1), PParam::int(4), PParam::int(0)];
                params[position] = PParam::Const(v.clone());
                let e = static_sampling(&params[0], &params[1], &params[2]).unwrap_err();
                assert_eq!(e.code, FailCode::Type);
                assert_eq!(e.message, "expected an integer");
            }
        }
    }
    #[test]
    fn raw_integer_extremes_and_dynamic_classification_match_runtime() {
        let mapping = static_sampling(
            &PParam::Const(Value::Int64(i64::MIN)),
            &PParam::Const(Value::Ratio(Ratio64::new(8, 2).unwrap())),
            &PParam::Const(Value::Int64(i64::MAX)),
        )
        .unwrap();
        assert_eq!(
            mapping,
            M::SampleGrid {
                content: 0,
                sampling: crate::song::source_uses::FrozenStaticSampling::Euclid {
                    pulses: i64::MIN,
                    divisions: 4,
                    rotation: i64::MAX
                }
            }
        );
        let dynamic = PParam::Fn(Value::Nil);
        assert_eq!(
            static_sampling(&dynamic, &PParam::int(4), &PParam::int(0)).unwrap(),
            M::Uncertifiable(R::DynamicCount)
        );
        assert_eq!(
            static_sampling(&PParam::int(1), &PParam::int(4), &dynamic).unwrap(),
            M::Uncertifiable(R::DynamicTiming)
        );
    }
}
