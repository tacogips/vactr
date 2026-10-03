//! Owning fixtures: no sealed identity or raw execution constructors.
use super::*;
use crate::{
    ns::{
        evaluator::Evaluator,
        namespace::{Prelude, VarSlotRef},
        stage::RecordingSink,
    },
    pattern::{
        eval::{
            song_observation::CanonicalIndexCollector,
            song_replay::{OwnerInvocation, ReplayView},
            InputCells,
        },
        query::TimeSpan,
    },
    reader::span::FileId,
    session::song::{evaluate_song_candidate, freeze::Freeze, CandidateBuildCtx},
    song::{
        assets::{DecodedSongAssetFactory, SongAssetFactory, SongAssetLimits, SongSourceFile},
        query::issued::{query_part_issued, IssuedQueryBatch},
        SnapshotEpoch, SongLimits,
    },
    value::{intern::intern_sym, Ratio64, Value},
    vm::{
        fail::{FailCode, Failure},
        query_vm::MeteredSongQuery,
        vm::ReadObserver,
    },
};
use std::{cell::Cell, collections::BTreeMap};
struct Reads {
    count: Rc<Cell<u32>>,
    deny: bool,
}
impl ReadObserver for Reads {
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        if slot.name() == intern_sym("cut") {
            self.count.set(self.count.get() + 1);
            if self.deny {
                return Err(Failure::new(
                    FailCode::HostUnavailable,
                    "callback reread after issued retention",
                ));
            }
        }
        Ok(())
    }
}
struct Fixture {
    evaluator: Evaluator,
    original: Rc<Song>,
    reads: Rc<Cell<u32>>,
}
fn limits() -> SongLimits {
    SongLimits {
        max_nodes: 1_000_000,
        ..SongLimits::default()
    }
}
fn window(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
fn program(body: &str, ending: &str) -> String {
    format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\n{ending}")
}
impl Fixture {
    fn code(code: &str) -> Self {
        let factory =
            DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let assets = SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 32,
            max_source_bytes: 100_000,
            max_banks: 32,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        };
        evaluate_song_candidate(
            &format!("{code} > play-song"),
            "issued.vact",
            7,
            SnapshotEpoch(91),
            &CandidateBuildCtx {
                assets: &factory,
                asset_limits: assets,
                lock: None,
                cache: None,
            },
        )
        .unwrap();
        let preparation = factory
            .begin(
                SongSourceFile {
                    file: FileId::new(0),
                    path: crate::value::value::PathVal {
                        text: "issued.vact".into(),
                        file: None,
                    },
                },
                assets,
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
            "genuine forms: {:?}",
            forms.iter().map(|form| &form.value).collect::<Vec<_>>()
        );
        let closed = preparation.close().unwrap();
        let mut freeze = Freeze::new(&closed, assets);
        let Value::Song(original) = freeze
            .value(forms.last().unwrap().value.as_ref().unwrap())
            .unwrap()
        else {
            panic!("actual frozen Song")
        };
        crate::session::song::capture_original_test_routing(&evaluator, &original, assets).unwrap();
        let reads = Rc::new(Cell::new(0));
        evaluator
            .vm_and_ns()
            .0
            .set_read_observer(Some(Box::new(Reads {
                count: reads.clone(),
                deny: false,
            })));
        Self {
            evaluator,
            original,
            reads,
        }
    }
    fn work(&self, remaining: u32) -> SharedIndexWork {
        let work = CanonicalIndexCollector::new(remaining, limits()).unwrap();
        work.borrow_mut().original = Some(self.original.clone());
        work
    }
    fn deny(&mut self) {
        self.reads.set(0);
        self.evaluator
            .vm_and_ns()
            .0
            .set_read_observer(Some(Box::new(Reads {
                count: self.reads.clone(),
                deny: true,
            })));
    }
    fn retain(&mut self) -> (Rc<ReplayView>, Vec<Rc<OwnerInvocation>>) {
        self.retain_window(window(
            0,
            self.original.duration().num(),
            self.original.duration().den(),
        ))
    }
    fn retain_window(&mut self, span: TimeSpan) -> (Rc<ReplayView>, Vec<Rc<OwnerInvocation>>) {
        let work = self.work(limits().max_nodes);
        let cells = InputCells::default();
        let (vm, ns) = self.evaluator.vm_and_ns();
        let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
        let policy = limits();
        crate::song::query::observe_part(
            self.original.part(),
            span,
            &mut crate::song::query::SongQueryCtx {
                vm: &mut adapter,
                cells: &cells,
                seed: self.original.settings().seed,
                tempo: self.original.settings().tempo().unwrap(),
                limits: &policy,
            },
            work.clone(),
            0,
        )
        .unwrap();
        let retained = work.borrow().invocations.clone();
        (
            ReplayView::publish(self.original.clone(), &work).unwrap(),
            retained,
        )
    }
    fn issued(
        &mut self,
        span: TimeSpan,
        view: Option<Rc<ReplayView>>,
        remaining: u32,
        depth: u32,
    ) -> (SharedIndexWork, Result<IssuedQueryBatch, Failure>) {
        let work = self.work(remaining);
        let cells = InputCells::default();
        let policy = limits();
        let (vm, ns) = self.evaluator.vm_and_ns();
        let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
        let result = query_part_issued(
            &self.original,
            span,
            &mut crate::song::query::SongQueryCtx {
                vm: &mut adapter,
                cells: &cells,
                seed: self.original.settings().seed,
                tempo: self.original.settings().tempo().unwrap(),
                limits: &policy,
            },
            work.clone(),
            depth,
            view,
        );
        (work, result)
    }
}

fn setup() -> (Fixture, Rc<ReplayView>, Vec<Rc<OwnerInvocation>>) {
    let code = program("slice {beat -> p} 1 [cut]", "song selected tail-seconds: 0");
    let mut fixture = Fixture::code(&code);
    let (view, retained) = fixture.retain();
    assert!(!retained.is_empty());
    (fixture, view, retained)
}
fn matched<'a>(
    batch: &IssuedQueryBatch,
    retained: &'a [Rc<OwnerInvocation>],
) -> (&'a OwnerInvocation, Rc<InvocationSeal>) {
    for record in &batch.transcript.invocations {
        if let Some(invocation) = retained
            .iter()
            .find(|row| row.authentic_execution(&record.execution))
        {
            return (invocation, record.seal.clone());
        }
    }
    panic!("actual replay must share a genuine retained execution")
}
#[test]
fn genuine_replay_fresh_seal_authenticates_without_callback_reads() {
    let (mut fixture, view, retained) = setup();
    fixture.deny();
    let (_, batch) = fixture.issued(window(0, 2, 1), Some(view), limits().max_nodes, 0);
    let batch = batch.unwrap();
    let (invocation, seal) = matched(&batch, &retained);
    assert!(batch
        .transcript
        .authentic_retained_invocation(
            &fixture.original,
            &seal,
            invocation,
            &fixture.work(limits().max_nodes),
            0
        )
        .unwrap());
    let actual = batch
        .transcript
        .invocations
        .iter()
        .find(|row| Rc::ptr_eq(&row.seal, &seal))
        .unwrap();
    assert!(!Rc::ptr_eq(
        &actual.actual,
        retained
            .iter()
            .find(|row| row.authentic_execution(&actual.execution))
            .unwrap()
    ));
    assert_eq!(fixture.reads.get(), 0);
}
#[test]
fn genuine_same_song_new_execution_refuses_even_when_output_handles_match() {
    let (mut fixture, view, retained) = setup();
    let (_, cached) = fixture.issued(window(0, 2, 1), Some(view), limits().max_nodes, 0);
    let (_, fresh) = fixture.issued(window(0, 2, 1), None, limits().max_nodes, 0);
    let cached = cached.unwrap();
    let fresh = fresh.unwrap();
    assert_eq!(
        cached
            .rows
            .iter()
            .map(|row| &row.handle)
            .collect::<Vec<_>>(),
        fresh.rows.iter().map(|row| &row.handle).collect::<Vec<_>>()
    );
    for (a, b) in cached.rows.iter().zip(&fresh.rows) {
        assert_eq!(a.track, b.track);
        assert_eq!(a.instrument, b.instrument);
        assert_eq!(a.placement, b.placement);
        assert_eq!(a.tone, b.tone);
        assert_eq!(a.route, b.route);
        assert_eq!(a.commit_mode, b.commit_mode);
        assert_eq!(a.event.whole, b.event.whole);
        assert_eq!(a.event.part, b.event.part);
        assert_eq!(a.event.producer, b.event.producer);
        assert_eq!(a.event.occ, b.event.occ);
        assert_eq!(a.event.src, b.event.src);
        assert!(crate::value::eq::deep_eq(&a.event.value, &b.event.value).unwrap());
        assert_eq!(a.event.controls.len(), b.event.controls.len());
        for ((ak, av), (bk, bv)) in a.event.controls.iter().zip(&b.event.controls) {
            assert_eq!(ak, bk);
            assert!(crate::value::eq::deep_eq(av, bv).unwrap());
        }
        let origin = |row: &crate::song::query::issued::IssuedQueryRow| {
            row.event.song_source.as_ref().map(|origin| {
                crate::song::source::copy_origin(
                    origin,
                    &mut limits().max_nodes,
                    limits().max_depth,
                )
                .unwrap()
            })
        };
        assert_eq!(origin(a), origin(b));
    }
    fixture.deny();
    for row in &fresh.transcript.invocations {
        for invocation in &retained {
            assert!(!fresh
                .transcript
                .authentic_retained_invocation(
                    &fixture.original,
                    &row.seal,
                    invocation,
                    &fixture.work(limits().max_nodes),
                    0
                )
                .unwrap());
        }
    }
    assert_eq!(fixture.reads.get(), 0);
}
#[test]
fn real_foreign_original_and_swapped_seals_refuse_without_querying() {
    let (mut fixture, view, retained) = setup();
    let (mut foreign, other_view, other_retained) = setup();
    let (_, batch) = fixture.issued(window(0, 2, 1), Some(view), limits().max_nodes, 0);
    let (_, other) = foreign.issued(window(0, 2, 1), Some(other_view), limits().max_nodes, 0);
    let batch = batch.unwrap();
    let other = other.unwrap();
    let (invocation, seal) = matched(&batch, &retained);
    let (_, other_seal) = matched(&other, &other_retained);
    fixture.deny();
    foreign.deny();
    for (original, leaf, retained) in [
        (&foreign.original, &seal, invocation),
        (&fixture.original, &other_seal, invocation),
        (&fixture.original, &seal, other_retained[0].as_ref()),
    ] {
        assert!(!batch
            .transcript
            .authentic_retained_invocation(
                original,
                leaf,
                retained,
                &fixture.work(limits().max_nodes),
                0
            )
            .unwrap());
    }
    assert_eq!(fixture.reads.get(), 0);
    assert_eq!(foreign.reads.get(), 0);
}
#[test]
fn entire_two_scan_cost_exact_one_less_and_leaf_depth_boundary_are_original() {
    let (mut fixture, view, retained) = setup();
    fixture.deny();
    let (_, batch) = fixture.issued(window(0, 2, 1), Some(view), limits().max_nodes, 0);
    let batch = batch.unwrap();
    let (invocation, seal) = matched(&batch, &retained);
    let measured = fixture.work(limits().max_nodes);
    assert!(batch
        .transcript
        .authentic_retained_invocation(&fixture.original, &seal, invocation, &measured, 0)
        .unwrap());
    let cost = limits().max_nodes - measured.borrow().remaining();
    assert_eq!(
        cost as usize,
        1 + 2 * (batch.transcript.invocations.len() + 1)
    );
    let exact = fixture.work(cost);
    assert!(batch
        .transcript
        .authentic_retained_invocation(
            &fixture.original,
            &seal,
            invocation,
            &exact,
            limits().max_depth
        )
        .unwrap());
    assert_eq!(exact.borrow().remaining(), 0);
    let short = fixture.work(cost - 1);
    assert_eq!(
        batch
            .transcript
            .authentic_retained_invocation(&fixture.original, &seal, invocation, &short, 0)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert!(short.borrow().remaining() < cost - 1);
    let deep = fixture.work(cost);
    assert_eq!(
        batch
            .transcript
            .authentic_retained_invocation(
                &fixture.original,
                &seal,
                invocation,
                &deep,
                limits().max_depth + 1
            )
            .unwrap_err()
            .code,
        FailCode::DepthExceeded
    );
    assert_eq!(deep.borrow().remaining(), cost - 1);
    assert_eq!(fixture.reads.get(), 0);
}
