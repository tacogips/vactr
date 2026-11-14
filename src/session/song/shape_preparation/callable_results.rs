//! Fixed getters retain the actual copied closure and its exact immutable result.
use super::*;
use crate::compile::proto::Closure;
use crate::types::masks::MaskEntry;
use crate::vm::ops::Op;

fn inspect<'a>(
    value: &'a Value,
    depth: u32,
    remaining: &mut u32,
    limits: SongAssetLimits,
) -> Result<Option<&'a Rc<Pat>>, Failure> {
    if depth >= limits.max_walk_depth {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "fixed callable depth exhausted",
        ));
    }
    admit(remaining, 1)?;
    let Value::Fn(closure) = value else {
        return Ok(None);
    };
    let proto = &closure.proto;
    let cost = proto
        .code
        .len()
        .checked_add(closure.captures.len())
        .and_then(|n| n.checked_add(closure.mask.0.len()))
        .and_then(|n| n.checked_add(proto.arity.names.len()))
        .and_then(|n| n.checked_add(proto.arity.scalar_types.len()))
        .ok_or_else(|| Failure::new(FailCode::Overflow, "fixed callable inspection overflow"))?;
    admit(remaining, cost)?;
    if proto.arity.fixed != 1
        || proto.arity.keys != 0
        || proto.locals < 1
        || proto.ctor.is_some()
        || closure.memo.is_some()
        || closure.mask.0.len() != 1
        || !matches!(
            closure.mask.0[0],
            MaskEntry::Value | MaskEntry::Fn | MaskEntry::Late | MaskEntry::Undetermined
        )
    {
        return Ok(None);
    }
    let load = match proto.code.as_slice() {
        [load, Op::Ret] | [load, Op::Force, Op::Ret] => load,
        _ => return Ok(None),
    };
    let loaded = match load {
        Op::LoadCapture(index) => {
            if usize::from(proto.captures) != closure.captures.len() || *index >= proto.captures {
                return Err(Failure::new(
                    FailCode::Type,
                    "fixed callable capture index mismatch",
                ));
            }
            closure.captures.get(usize::from(*index))
        }
        Op::LoadConst(index) => proto.consts.get(usize::from(*index)),
        _ => return Ok(None),
    }
    .ok_or_else(|| Failure::new(FailCode::Type, "fixed callable load index out of bounds"))?;
    Ok(match loaded {
        Value::Pattern(pattern) => Some(pattern),
        _ => None,
    })
}

struct PendingFixedResult {
    callable: Rc<Closure>,
    result: Rc<Pat>,
    depth: u32,
}
#[derive(Default)]
pub(super) struct PendingFixedResults {
    entries: Vec<PendingFixedResult>,
}
struct FixedPatternResult {
    callable: Rc<Closure>,
    result: Rc<Pat>,
}
#[derive(Default)]
pub(super) struct FixedCallableResults {
    entries: BTreeMap<usize, FixedPatternResult>,
}

impl PendingFixedResults {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn discover(
        &mut self,
        callable: &Value,
        depth: u32,
        remaining: &mut u32,
        limits: SongAssetLimits,
    ) -> Result<Option<Rc<Pat>>, Failure> {
        let Some(result) = inspect(callable, depth, remaining, limits)? else {
            return Ok(None);
        };
        let Value::Fn(closure) = callable else {
            return Err(super::super::failure("fixed callable value invariant"));
        };
        admit(
            remaining,
            self.entries
                .len()
                .checked_add(2)
                .ok_or_else(|| super::super::failure("fixed callable record overflow"))?,
        )?;
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| Rc::ptr_eq(&entry.callable, closure))
        {
            entry.depth = entry.depth.max(depth);
        } else {
            self.entries.push(PendingFixedResult {
                callable: closure.clone(),
                result: result.clone(),
                depth,
            });
        }
        Ok(Some(result.clone()))
    }
    pub(super) fn freeze_records(
        self,
        freeze: &mut super::super::freeze::Freeze<'_>,
        remaining: &mut u32,
    ) -> Result<Self, Failure> {
        admit(remaining, self.entries.len())?;
        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            let Value::Fn(callable) = freeze.value(&Value::Fn(entry.callable))? else {
                return Err(super::super::failure("copied fixed callable invariant"));
            };
            let Value::Pattern(result) = freeze.value(&Value::Pattern(entry.result))? else {
                return Err(super::super::failure("copied fixed result invariant"));
            };
            entries.push(PendingFixedResult {
                callable,
                result,
                depth: entry.depth,
            });
        }
        Ok(Self { entries })
    }
    pub(super) fn admit_closed(
        self,
        evaluator: &mut crate::ns::evaluator::Evaluator,
        assets: &mut crate::song::assets::PinnedSongAssets,
        limits: SongAssetLimits,
        remaining: &mut u32,
    ) -> Result<FixedCallableResults, Failure> {
        let (vm, ns) = evaluator.vm_and_ns();
        let mut out = FixedCallableResults::default();
        for entry in self.entries {
            admit(remaining, 2)?;
            let value = Value::Fn(entry.callable.clone());
            let Some(result) = inspect(&value, entry.depth, remaining, limits)? else {
                return Err(super::super::failure("copied fixed callable proof changed"));
            };
            if !Rc::ptr_eq(result, &entry.result) {
                return Err(super::super::failure(
                    "copied fixed callable result mismatch",
                ));
            }
            let mut reduced = limits;
            reduced.max_walk_depth =
                limits
                    .max_walk_depth
                    .checked_sub(entry.depth)
                    .ok_or_else(|| {
                        Failure::new(
                            FailCode::DepthExceeded,
                            "fixed result ancestor depth exhausted",
                        )
                    })?;
            let deps = dependencies(&[Value::Pattern(entry.result.clone())], reduced, remaining)?;
            if (super::super::ClosedShapeCtx {
                vm,
                ns,
                assets,
                remaining,
                limits,
                depth: entry.depth,
            })
            .accepts(&deps)?
            {
                admit(remaining, 1)?;
                out.entries.insert(
                    Rc::as_ptr(&entry.callable) as usize,
                    FixedPatternResult {
                        callable: entry.callable,
                        result: entry.result,
                    },
                );
            }
        }
        Ok(out)
    }
}
impl FixedCallableResults {
    pub(super) fn pattern<'a>(
        &'a self,
        value: &Value,
        depth: u32,
        remaining: &mut u32,
        limits: SongAssetLimits,
    ) -> Result<Option<&'a Rc<Pat>>, Failure> {
        if depth >= limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "fixed result lookup depth exhausted",
            ));
        }
        admit(
            remaining,
            self.entries
                .len()
                .checked_add(1)
                .ok_or_else(|| super::super::failure("fixed result lookup overflow"))?,
        )?;
        let Value::Fn(closure) = value else {
            return Ok(None);
        };
        let Some(entry) = self.entries.get(&(Rc::as_ptr(closure) as usize)) else {
            return Ok(None);
        };
        if !Rc::ptr_eq(closure, &entry.callable) {
            return Err(super::super::failure(
                "fixed callable strong identity mismatch",
            ));
        }
        Ok(Some(&entry.result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ns::{
        evaluator::Evaluator,
        namespace::{FormGen, Prelude},
        stage::RecordingSink,
    };
    use crate::reader::span::{FileId, Span};
    use crate::song::assets::{DecodedSongAssetFactory, SongAssetFactory, SongSourceFile};
    use crate::song::{Part, PartEdit, PartNode, Song};
    use crate::value::value::PathVal;
    fn limits() -> SongAssetLimits {
        SongAssetLimits {
            max_resources: 32,
            max_pcm_bytes: 1_000_000,
            max_source_files: 8,
            max_source_bytes: 100_000,
            max_banks: 8,
            max_walk_nodes: 1_000_000,
            max_walk_depth: 256,
        }
    }
    fn copied(code: &str) -> (PreparedShapes, Rc<Song>) {
        let factory =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let preparation = factory
            .begin(
                SongSourceFile {
                    file: FileId::new(0),
                    path: PathVal {
                        text: "getters.vact".into(),
                        file: None,
                    },
                },
                limits(),
            )
            .unwrap();
        let mut evaluator = Evaluator::new(
            Prelude::core(),
            preparation.source_loader(),
            Box::new(RecordingSink::default()),
        );
        let forms = evaluator.eval_str(code, FileId::new(0)).unwrap();
        assert!(
            forms.iter().all(|form| form.value.is_ok()),
            "actual evaluator failures: {:?}",
            forms
                .iter()
                .filter_map(|form| form.value.as_ref().err())
                .collect::<Vec<_>>()
        );
        let Value::Song(song) = forms.last().unwrap().value.as_ref().unwrap() else {
            panic!("actual evaluated Song")
        };
        let song = song.clone();
        let mut remaining = limits().max_walk_nodes;
        let pending =
            super::super::discover_shapes(&mut evaluator, &song, limits(), &mut remaining).unwrap();
        let mut closed = preparation.close().unwrap();
        let mut reduced = limits();
        reduced.max_walk_nodes = remaining
            .checked_sub(u32::try_from(pending.record_count()).unwrap())
            .unwrap();
        let mut freeze = super::super::super::freeze::Freeze::new(&closed, reduced);
        for slot in evaluator.candidate_slots() {
            freeze.slot(&slot).unwrap();
        }
        let Value::Song(song) = freeze.value(&Value::Song(song)).unwrap() else {
            panic!("actual frozen Song")
        };
        let pending = pending.freeze_records(&mut freeze, &mut remaining).unwrap();
        let work = freeze.consumed_work();
        drop(freeze);
        remaining = remaining.checked_sub(work).unwrap();
        let shapes = pending
            .admit_closed(&mut evaluator, &mut closed, limits(), &mut remaining)
            .unwrap();
        (shapes, song)
    }
    fn getter(part: &Part) -> (&Rc<Closure>, &Rc<Pat>) {
        let PartNode::Edit {
            source,
            edit:
                PartEdit::TransformInstrument {
                    pattern,
                    track,
                    selector,
                },
        } = part.node()
        else {
            panic!("actual transform")
        };
        let PatNode::Slice { pat, .. } = &pattern.node else {
            panic!("actual Slice")
        };
        assert!(!pat.structured);
        let PatNode::Pure(step) = &pat.node else {
            panic!("actual unstructured Pure")
        };
        let Value::Fn(closure) = &step.value else {
            panic!("actual compiled closure")
        };
        assert_eq!(closure.proto.arity.fixed, 1);
        assert_eq!(closure.proto.arity.keys, 0);
        assert_eq!(closure.mask.0.as_ref(), &[MaskEntry::Value]);
        let [Op::LoadCapture(index), Op::Ret] = closure.proto.code.as_slice() else {
            panic!("compiled getter code: {:?}", closure.proto.code)
        };
        let Value::Pattern(result) = &closure.captures[usize::from(*index)] else {
            panic!("actual direct captured Pattern")
        };
        let PatNode::SongSource(selected) = &result.node else {
            panic!("actual selected Source")
        };
        // transform_instrument deliberately clones the immutable Part twice.
        let selected_part = selected.part();
        assert_eq!(source.revision(), selected_part.revision());
        assert_eq!(source.duration(), selected_part.duration());
        assert_eq!(source.seed_identity(), selected_part.seed_identity());
        assert_eq!(source.tracks(), selected_part.tracks());
        assert_eq!(source.node_count(), selected_part.node_count());
        assert_eq!(source.depth(), selected_part.depth());
        assert_eq!(*track, selected.track());
        assert_eq!(selector, selected.selector());
        let (PartNode::Capture(original), PartNode::Capture(captured)) =
            (source.node(), selected_part.node())
        else {
            panic!("actual captured Part payloads")
        };
        assert_eq!(
            original.keys().collect::<Vec<_>>(),
            captured.keys().collect::<Vec<_>>()
        );
        for (track, pattern) in original {
            assert!(
                Rc::ptr_eq(pattern, &captured[track]),
                "same shared Freeze preserves captured payload identity"
            );
        }
        assert!(result.structured);
        (closure, result)
    }
    #[test]
    fn compiled_copied_getters_bind_distinct_captures_and_exact_work_depth() {
        let (shapes,song) = copied("fn indexed p:\n\tslice {beat -> p} 2 [0 nil]\nlet a {part [drums: {s :analog}] duration: 2}\nlet b {part [drums: {s :analog}] duration: 3}\nlet x {transform-instrument a :drums :analog indexed}\nlet y {transform-instrument b :drums :analog indexed}\nsong {sequence [x y]} tail-seconds: 0");
        assert!(shapes.rejected.is_empty());
        let PartNode::Sequence(parts) = song.part().node() else {
            panic!("actual sequence")
        };
        assert_eq!(parts.len(), 2);
        let (a, pa) = getter(&parts[0]);
        let (b, pb) = getter(&parts[1]);
        assert!(Rc::ptr_eq(&a.proto, &b.proto));
        assert!(!Rc::ptr_eq(a, b));
        assert!(!Rc::ptr_eq(pa, pb));
        for (closure, result) in [(a, pa), (b, pb)] {
            let value = Value::Fn(closure.clone());
            let proved = shapes
                .fixed_pattern(&value, 0, &mut 10000, limits())
                .unwrap()
                .unwrap();
            assert!(Rc::ptr_eq(proved, result));
            let foreign = Value::Fn(Rc::new(Closure {
                proto: closure.proto.clone(),
                captures: closure.captures.clone(),
                mask: closure.mask.clone(),
                memo: None,
            }));
            assert!(
                shapes
                    .fixed_pattern(&foreign, 0, &mut 10000, limits())
                    .unwrap()
                    .is_none(),
                "equal code/captures do not replace issued strong closure identity"
            );
            let mut left = 10000;
            assert!(Rc::ptr_eq(
                inspect(&value, 0, &mut left, limits()).unwrap().unwrap(),
                result
            ));
            let cost = 10000 - left;
            let mut exact = cost;
            assert!(inspect(&value, 0, &mut exact, limits()).unwrap().is_some());
            assert_eq!(
                inspect(&value, 0, &mut (cost - 1), limits())
                    .unwrap_err()
                    .code,
                FailCode::FuelExhausted
            );
            assert_eq!(
                inspect(&value, limits().max_walk_depth, &mut 10000, limits())
                    .unwrap_err()
                    .code,
                FailCode::DepthExceeded
            );
        }
    }
    #[test]
    fn fixed_result_discovers_nested_callbacks_once_before_closed_copy() {
        super::super::SHAPE_CALLS.with(|calls| calls.set(0));
        let (shapes,_) = copied("fn indexed p:\n\tslice {beat -> p} 2 [0 nil]\nfn wrapped p:\n\tindexed {every p 2 {q -> first [q]}}\nlet a {part [drums: {s :analog}] duration: 4}\nsong {transform-instrument a :drums :analog wrapped} tail-seconds: 0");
        assert_eq!(super::super::SHAPE_CALLS.with(std::cell::Cell::get), 1);
        assert_eq!(shapes.patterns.len(), 1);
        assert!(shapes.rejected.is_empty());
        assert_eq!(shapes.fixed.entries.len(), 1);
    }
    #[test]
    fn unsupported_time_dependent_body_is_not_guessed_or_executed() {
        for code in [
            "fn indexed p:\n\tslice {beat -> fast p beat} 2 [0 nil]\nlet a {part [drums: {s :analog}] duration: 2}\nsong {transform-instrument a :drums :analog indexed} tail-seconds: 0",
            "let shared {s :analog}\nfn indexed p:\n\tslice {beat -> shared} 2 [0 nil]\nlet a {part [drums: {s :analog}] duration: 2}\nsong {transform-instrument a :drums :analog indexed} tail-seconds: 0",
        ] {
            super::super::SHAPE_CALLS.with(|calls| calls.set(0));
            let (shapes,_) = copied(code);
            assert!(shapes.fixed.entries.is_empty());
            assert_eq!(super::super::SHAPE_CALLS.with(std::cell::Cell::get),0);
        }
    }
    #[test]
    fn recognized_malformed_load_index_is_an_error_before_any_result() {
        let mut builder = crate::compile::proto::Fb::new(
            None,
            Span::new(FileId::new(0), 0, 1),
            crate::compile::proto::FbKind::Func,
        );
        builder.arity = crate::compile::proto::Arity::fixed(1);
        builder.nlocals = 1;
        builder.code = vec![Op::LoadConst(7), Op::Ret];
        let value = Value::Fn(Rc::new(Closure {
            proto: Rc::new(builder.finish(FormGen::new(0))),
            captures: Box::new([]),
            mask: crate::types::masks::ForcingMask(Box::new([MaskEntry::Value])),
            memo: None,
        }));
        assert_eq!(
            inspect(&value, 0, &mut 10000, limits()).unwrap_err().code,
            FailCode::Type
        );
    }
}
