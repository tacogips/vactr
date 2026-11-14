//! Static cut copying with actual Subject/Index structure, without querying timing.
use super::{count, numeric, Builder, Pat, PatNode, Ratio64, M, R};
use crate::pattern::pat::SliceCuts;
use crate::song::source_uses::FrozenSliceStructure;
use crate::vm::fail::Failure;
impl Builder<'_> {
    pub(super) fn slice_mapping(
        &mut self,
        slice: &Pat,
        subject: &Pat,
        index: &Pat,
        cuts: &SliceCuts,
    ) -> Result<M, Failure> {
        let starts = match cuts {
            SliceCuts::Equal(n) => {
                let Some(n) = count(n).filter(|n| *n > 0 && *n <= 4096) else {
                    return Ok(M::Uncertifiable(R::DynamicCount));
                };
                self.admit(n as usize)?;
                (0..n)
                    .map(|i| Ratio64::new(i64::from(i), i64::from(n)))
                    .collect::<Result<Vec<_>, _>>()?
            }
            SliceCuts::Manual(points) => {
                self.admit(points.len())?;
                let Some(starts) = points.iter().map(numeric).collect::<Option<Vec<_>>>() else {
                    return Ok(M::Uncertifiable(R::DynamicTiming));
                };
                starts
            }
        };
        self.admit(2)?;
        let structure = if !subject.structured && index.structured {
            FrozenSliceStructure::Index { issuer: slice.id }
        } else {
            FrozenSliceStructure::Subject { issuer: slice.id }
        };
        Ok(M::Slices {
            starts,
            splice: matches!(slice.node, PatNode::Splice { .. }),
            structure,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::eval::{InputCells, QueryVm};
    use crate::reader::span::FileId;
    use crate::session::song::freeze::Freeze;
    use crate::song::assets::{
        DecodedSongAssetFactory, SongAssetFactory, SongAssetLimits, SongSourceFile,
    };
    use crate::song::{
        capture_part, query_part, InstrumentSelector, SongLimits, SongQueryCtx, SongSource,
    };
    use crate::value::intern::intern_kw;
    use crate::value::value::{PathVal, Sound, Value};
    use std::collections::BTreeMap;
    use std::rc::Rc;
    struct CountingVm<'a> {
        inner: crate::vm::query_vm::VmQuery<'a>,
        calls: usize,
    }
    impl QueryVm for CountingVm<'_> {
        fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure> {
            self.calls += 1;
            self.inner.call(f, args)
        }
        fn deref(&mut self, r: &crate::ns::namespace::VarSlotRef) -> Result<Value, Failure> {
            self.inner.deref(r)
        }
        fn take_output(&mut self) -> Vec<(crate::vm::fail::Origin, Rc<str>)> {
            self.inner.take_output()
        }
        fn put_output(&mut self, o: Vec<(crate::vm::fail::Origin, Rc<str>)>) {
            self.inner.put_output(o);
        }
        fn sound_kit(&mut self) -> Result<Value, Failure> {
            self.inner.sound_kit()
        }
    }
    #[test]
    fn dynamic_index_issues_actual_timing_without_rng_replay() {
        let limits = SongAssetLimits {
            max_resources: 8,
            max_pcm_bytes: 100000,
            max_source_files: 4,
            max_source_bytes: 100000,
            max_banks: 4,
            max_walk_nodes: 100000,
            max_walk_depth: 256,
        };
        let factory =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let prep = factory
            .begin(
                SongSourceFile {
                    file: FileId::new(0),
                    path: PathVal {
                        text: "dynamic.vact".into(),
                        file: None,
                    },
                },
                limits,
            )
            .unwrap();
        let mut evaluator = crate::ns::evaluator::Evaluator::new(
            crate::ns::namespace::Prelude::core(),
            prep.source_loader(),
            Box::new(crate::ns::stage::RecordingSink::default()),
        );
        let forms = evaluator
            .eval_str("[{t -> 0} nil {t -> 1}]", FileId::new(0))
            .unwrap();
        let value = forms.last().unwrap().value.as_ref().unwrap();
        let closed = prep.close().unwrap();
        let mut freeze = Freeze::new(&closed, limits);
        let value = freeze.value(value).unwrap();
        let work = freeze.consumed_work();
        assert!(work > 0);
        drop(freeze);
        let Value::List(list) = value else {
            panic!("actual copied function list")
        };
        assert!(matches!(list.items[0], Value::Fn(_)));
        let index = Rc::new(crate::pattern::step::steps(
            crate::pattern::step::steps_of_list(&list),
            None,
        ));
        let track = intern_kw("drums");
        let sound = Sound::Builtin(intern_kw("analog"));
        let base = Rc::new(
            capture_part(
                [(
                    track,
                    Rc::new(crate::pattern::build::pure(
                        Value::Sound(Rc::new(sound.clone())),
                        None,
                    )),
                )]
                .into(),
                Ratio64::from_int(2),
            )
            .unwrap(),
        );
        let source =
            SongSource::new(base, track, InstrumentSelector::new(vec![sound]).unwrap()).unwrap();
        let subject = Rc::new(Pat::new(PatNode::SongSource(Rc::new(source)), None, false));
        let pattern = Rc::new(crate::pattern::combinators::region::slice(
            subject,
            SliceCuts::Equal(crate::pattern::pat::PParam::Const(Value::Int(2))),
            index,
            None,
        ));
        let part = capture_part([(track, pattern)].into(), Ratio64::from_int(2)).unwrap();
        let (vm, ns) = evaluator.vm_and_ns();
        let mut query = CountingVm {
            inner: crate::vm::query_vm::VmQuery::new(vm, ns),
            calls: 0,
        };
        let cells = InputCells::new();
        let rows = query_part(
            &part,
            crate::pattern::TimeSpan::cycle(0).unwrap(),
            &mut SongQueryCtx {
                vm: &mut query,
                cells: &cells,
                seed: 7,
                tempo: Default::default(),
                limits: &SongLimits::default(),
            },
        )
        .unwrap();
        assert_eq!(query.calls, 2);
        assert_eq!(rows.len(), 2);
        for row in rows {
            let origin = row.event.song_source.unwrap();
            let frozen = crate::song::source::copy_origin(&origin, &mut 100000, 256).unwrap();
            assert_eq!(query.calls, 2, "copy never replays actual callbacks");
            assert_eq!(frozen.slice_timings().len(), 1);
            assert_eq!(
                frozen.slice_timings()[0].index_whole(),
                row.event.whole.unwrap()
            );
            assert_eq!(
                frozen.slice_timings()[0].sample_start(),
                row.event.whole.unwrap().begin
            );
        }
    }
}
